use std::collections::BTreeSet;
use std::thread::current;

use cosmwasm_std::{
    entry_point, from_json, to_json_binary, Addr, AllBalanceResponse, BankMsg, BankQuery, Binary,
    Coin, Deps, DepsMut, Env, MessageInfo, QueryRequest, Reply, Response as CwResponse, StdError,
    StdResult, SubMsg, WasmMsg,
};
use hydro_interface::msgs::ExecuteMsg::{LockTokens, RefreshLockDuration, UnlockTokens, Vote};
use hydro_interface::msgs::{CurrentRoundResponse, HydroQueryMsg, ProposalToLockups};
use hydro_interface::state::query_lock_entries;
use neutron_sdk::bindings::msg::NeutronMsg;
use serde::{Deserialize, Serialize};
use zephyrus_core::msgs::{
    BuildVesselParams, ConstantsResponse, ExecuteMsg, HydroProposalId, InstantiateMsg, MigrateMsg,
    QueryMsg, RoundId, VesselsResponse, VesselsToHarbor, VotingPowerResponse,
};
use zephyrus_core::state::{Constants, HydroConfig, HydroLockId, Vessel};

use crate::state::VesselHarbor;
use crate::{
    errors::ContractError,
    helpers::ibc::{DenomTrace, QuerierExt as IbcQuerierExt},
    helpers::vectors::{compare_coin_vectors, compare_u64_vectors},
    state,
};

type Response = CwResponse<NeutronMsg>;

const HYDRO_LOCK_TOKENS_REPLY_ID: u64 = 1;
const DECOMMISSION_REPLY_ID: u64 = 2;

const MAX_PAGINATION_LIMIT: usize = 1000;
const DEFAULT_PAGINATION_LIMIT: usize = 100;

#[derive(Serialize, Deserialize)]
struct LockTokensReplyPayload {
    params: BuildVesselParams,
    tokenized_share_record_id: u64,
    owner: Addr,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DecommissionVesselsParameters {
    previous_balances: Vec<Coin>,
    expected_unlocked_ids: Vec<u64>,
    vessel_owner: Addr,
}

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, StdError> {
    state::initialize_sequences(deps.storage)?;

    let mut whitelist_admins: Vec<Addr> = vec![];
    for admin in msg.whitelist_admins {
        let admin_addr = deps.api.addr_validate(&admin)?;
        if !whitelist_admins.contains(&admin_addr) {
            whitelist_admins.push(admin_addr.clone());
        }
    }
    state::update_whitelist_admins(deps.storage, whitelist_admins)?;
    let hydro_config = HydroConfig {
        hydro_contract_address: deps.api.addr_validate(&msg.hydro_contract_address)?,
        hydro_tribute_contract_address: deps.api.addr_validate(&msg.tribute_contract_address)?,
    };

    let hydromancer_address = deps.api.addr_validate(&msg.default_hydromancer_address)?;

    let default_hydromancer_id = state::insert_new_hydromancer(
        deps.storage,
        hydromancer_address,
        msg.default_hydromancer_name,
        msg.default_hydromancer_commission_rate,
    )?;

    let constant = Constants {
        default_hydromancer_id,
        paused_contract: false,
        hydro_config,
    };
    state::update_constants(deps.storage, constant)?;

    Ok(Response::default())
}

fn extract_tokenized_share_record_id(denom_trace: &DenomTrace) -> Option<u64> {
    denom_trace
        .base_denom
        .rsplit_once('/')
        .and_then(|(_, id_str)| id_str.parse().ok())
}

fn execute_build_vessel(
    deps: DepsMut,
    info: MessageInfo,
    vessels: Vec<BuildVesselParams>,
    receiver: Option<String>,
) -> Result<Response, ContractError> {
    let constants = state::get_constants(deps.storage)?;
    validate_contract_is_not_paused(&constants)?;

    if info.funds.is_empty() {
        return Err(ContractError::NoTokensReceived);
    }

    if vessels.len() != info.funds.len() {
        return Err(ContractError::CreateVesselParamsLengthMismatch {
            params_len: vessels.len(),
            funds_len: info.funds.len(),
        });
    }

    let owner = receiver
        .map(|addr| deps.api.addr_validate(&addr))
        .transpose()?
        .unwrap_or(info.sender);

    let mut hydro_lock_msgs = vec![];

    // Note: Check again when IBC v2 is out, because the order of tokens may not be deterministic
    // Today with IBC v1, IBC transfers can only send one token at once, so we don't have any issue
    // And for tokens directly sent to the contract, the order is deterministic
    for (params, token) in vessels.into_iter().zip(info.funds) {
        if !state::hydromancer_exists(deps.storage, params.hydromancer_id) {
            return Err(ContractError::HydromancerNotFound {
                hydromancer_id: params.hydromancer_id,
            });
        }

        let denom_trace = deps.querier.ibc_denom_trace(&token.denom)?;

        let tokenized_share_record_id = extract_tokenized_share_record_id(&denom_trace)
            .ok_or_else(|| ContractError::InvalidLsmTokenReceived(token.denom.clone()))?;

        if state::is_tokenized_share_record_used(deps.storage, tokenized_share_record_id) {
            return Err(ContractError::TokenizedShareRecordAlreadyInUse(
                tokenized_share_record_id,
            ));
        }

        let payload = to_json_binary(&LockTokensReplyPayload {
            params,
            tokenized_share_record_id,
            owner: owner.clone(),
        })?;

        let contract_addr = constants
            .hydro_config
            .hydro_contract_address
            .clone()
            .into_string();

        let msg = to_json_binary(&LockTokens {
            lock_duration: params.lock_duration,
        })?;

        let lock_submsg = SubMsg::reply_on_success(
            WasmMsg::Execute {
                contract_addr,
                msg,
                funds: vec![token],
            },
            HYDRO_LOCK_TOKENS_REPLY_ID,
        )
        .with_payload(payload);

        hydro_lock_msgs.push(lock_submsg);
    }

    Ok(Response::new().add_submessages(hydro_lock_msgs))
}

// This function loops through all the vessels, and filters those who have auto_maintenance true
// Then, it combines them by hydro_lock_duration, and calls execute_update_vessels_class
fn execute_auto_maintain(deps: DepsMut, _info: MessageInfo) -> Result<Response, ContractError> {
    let constants = state::get_constants(deps.storage)?;
    validate_contract_is_not_paused(&constants)?;

    let vessels_ids_by_hydro_lock_duration = state::get_vessels_id_by_class()?;

    let iterator = vessels_ids_by_hydro_lock_duration.range(
        deps.storage,
        None,
        None,
        cosmwasm_std::Order::Ascending,
    );

    let mut response = Response::new();
    let hydro_config = constants.hydro_config;

    // Collect all keys into a Vec<u64>
    for item in iterator {
        let (hydro_period, hydro_lock_ids) = item?;

        if hydro_lock_ids.is_empty() {
            continue;
        }

        let refresh_duration_msg = RefreshLockDuration {
            lock_ids: hydro_lock_ids.iter().cloned().collect(),
            lock_duration: hydro_period,
        };

        let execute_refresh_msg = WasmMsg::Execute {
            contract_addr: hydro_config.hydro_contract_address.to_string(),
            msg: to_json_binary(&refresh_duration_msg)?,
            funds: vec![],
        };

        response = response
            .add_attribute("Action", "Refresh lock duration")
            .add_attribute(
                ["ids ", &hydro_period.to_string()].concat(),
                hydro_lock_ids
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(","),
            );
        response = response.add_message(execute_refresh_msg);
    }

    if response.messages.is_empty() {
        return Err(ContractError::NoVesselsToAutoMaintain {});
    }

    Ok(response)
}

// This function takes a list of vessels (hydro_lock_ids) and a duration
// And calls the Hydro function:
// ExecuteMsg::RefreshLockDuration {
//     lock_ids,
//     lock_duration,
// }
// TODO: Need to be careful that all the vessels are currently less than hydro_lock_duration
// Otherwise, the RefreshLockDuration will fail
fn execute_update_vessels_class(
    deps: DepsMut,
    info: MessageInfo,
    hydro_lock_ids: Vec<u64>,
    hydro_lock_duration: u64,
) -> Result<Response, ContractError> {
    let constants = state::get_constants(deps.storage)?;
    validate_contract_is_not_paused(&constants)?;

    let hydro_config = constants.hydro_config;

    let refresh_duration_msg = RefreshLockDuration {
        lock_ids: hydro_lock_ids,
        lock_duration: hydro_lock_duration,
    };

    // There should not be any funds?
    let execute_refresh_duration_msg = WasmMsg::Execute {
        contract_addr: hydro_config.hydro_contract_address.to_string(),
        msg: to_json_binary(&refresh_duration_msg)?,
        funds: info.funds.clone(),
    };

    Ok(Response::new().add_message(execute_refresh_duration_msg))
}

fn execute_modify_auto_maintenance(
    deps: DepsMut,
    info: MessageInfo,
    hydro_lock_ids: Vec<u64>,
    auto_maintenance: bool,
) -> Result<Response, ContractError> {
    let constants = state::get_constants(deps.storage)?;
    validate_contract_is_not_paused(&constants)?;

    if !state::are_vessels_owned_by(deps.storage, &info.sender, &hydro_lock_ids)? {
        return Err(ContractError::Unauthorized {});
    }

    for hydro_lock_id in hydro_lock_ids.iter() {
        state::modify_auto_maintenance(deps.storage, *hydro_lock_id, auto_maintenance)?;
    }

    Ok(Response::new()
        .add_attribute("action", "modify_auto_maintenance")
        .add_attribute("new_auto_maintenance", auto_maintenance.to_string())
        .add_attribute(
            "hydro_lock_id",
            hydro_lock_ids
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(","),
        ))
}

fn execute_pause_contract(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    validate_admin_address(&deps, &info.sender)?;
    let mut constants = state::get_constants(deps.storage)?;
    validate_contract_is_not_paused(&constants)?;
    constants.paused_contract = true;
    state::update_constants(deps.storage, constants)?;
    Ok(Response::new()
        .add_attribute("action", "pause_contract")
        .add_attribute("sender", info.sender))
}

fn execute_unpause_contract(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    validate_admin_address(&deps, &info.sender)?;
    let mut constants = state::get_constants(deps.storage)?;
    if !constants.paused_contract {
        return Err(ContractError::Std(StdError::generic_err(
            "Cannot unpause: Contract not paused",
        )));
    }
    constants.paused_contract = false;

    state::update_constants(deps.storage, constants)?;
    Ok(Response::new()
        .add_attribute("action", "unpause_contract")
        .add_attribute("sender", info.sender))
}

fn execute_decommission_vessels(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    hydro_lock_ids: Vec<u64>,
) -> Result<Response, ContractError> {
    let constants = state::get_constants(deps.storage)?;
    validate_contract_is_not_paused(&constants)?;

    if !state::are_vessels_owned_by(deps.storage, &info.sender, &hydro_lock_ids)? {
        return Err(ContractError::Unauthorized {});
    }

    let hydro_config = constants.hydro_config;

    // Check the current balance before unlocking tokens
    let balance_query = BankQuery::AllBalances {
        address: env.contract.address.to_string(),
    };
    let previous_balances: AllBalanceResponse =
        deps.querier.query(&QueryRequest::Bank(balance_query))?;

    // Retrieve the lock_entries from Hydro, and check which ones are expired
    let lock_entries = query_lock_entries(
        &deps.querier,
        hydro_config.hydro_contract_address.clone(),
        env.contract.address,
        &hydro_lock_ids,
    )?;

    let mut expected_unlocked_ids = vec![];
    for lock_entry in lock_entries {
        if lock_entry.1.lock_end < env.block.time {
            expected_unlocked_ids.push(lock_entry.0);
        }
    }

    // Create the execute message for unlocking
    let hydro_unlock_msg = UnlockTokens {
        lock_ids: Some(hydro_lock_ids.clone()),
    };

    let execute_hydro_unlock_msg = WasmMsg::Execute {
        contract_addr: hydro_config.hydro_contract_address.to_string(),
        msg: to_json_binary(&hydro_unlock_msg)?,
        funds: vec![],
    };

    let decommission_vessels_params = DecommissionVesselsParameters {
        previous_balances: previous_balances.amount,
        expected_unlocked_ids,
        vessel_owner: info.sender.clone(),
    };

    let execute_hydro_unlock_msg: SubMsg<NeutronMsg> =
        SubMsg::reply_on_success(execute_hydro_unlock_msg, DECOMMISSION_REPLY_ID)
            .with_payload(to_json_binary(&decommission_vessels_params)?);

    Ok(Response::new().add_submessage(execute_hydro_unlock_msg))
}

pub fn has_duplicate_harbor_id_in_vote(
    vessels_harbors: Vec<VesselsToHarbor>,
) -> (bool, Option<HydroProposalId>) {
    let mut seen = BTreeSet::new();
    for item in vessels_harbors.iter() {
        if !seen.insert(item.harbor_id) {
            return (true, Some(item.harbor_id));
        }
    }
    (false, None)
}

pub fn has_duplicate_vessel_id_in_vote(
    vessels_harbors: Vec<VesselsToHarbor>,
) -> (bool, Option<HydroLockId>) {
    let mut seen = BTreeSet::new();
    for item in vessels_harbors.iter() {
        for vessel_id in item.vessel_ids.iter() {
            if !seen.insert(*vessel_id) {
                return (true, Some(*vessel_id));
            }
        }
    }
    (false, None)
}

fn execute_hydromancer_vote(
    deps: DepsMut,
    info: MessageInfo,
    tranche_id: u64,
    vessels_harbors: Vec<VesselsToHarbor>,
) -> Result<Response, ContractError> {
    let constants = state::get_constants(deps.storage)?;
    validate_contract_is_not_paused(&constants)?;

    let (has_duplicated_harbor, harbor_id) =
        has_duplicate_harbor_id_in_vote(vessels_harbors.clone());
    if has_duplicated_harbor {
        let harbor_id = harbor_id.expect("If there is duplicated harbor, id should be present");
        return Err(ContractError::VoteDuplicatedHarborId { harbor_id });
    }

    let (has_duplicated_vessel_id, vessel_id) =
        has_duplicate_vessel_id_in_vote(vessels_harbors.clone());
    if has_duplicated_vessel_id {
        let vessel_id = vessel_id.expect("If there is duplicated vessel, id should be present");
        return Err(ContractError::VoteDuplicatedVesselId { vessel_id });
    }

    let hydromancer_id = state::get_hydromancer_id_by_address(deps.storage, info.sender)?;
    let current_round_id = query_hydro_current_round(
        deps.as_ref(),
        constants.hydro_config.hydro_contract_address.to_string(),
    )?;
    let mut proposal_votes = vec![];
    for vessels_to_harbor in vessels_harbors {
        let mut lock_ids = vec![];

        for vessel_id in vessels_to_harbor.vessel_ids.iter() {
            let vessel = state::get_vessel(deps.storage, *vessel_id)?;
            if vessel.hydromancer_id != hydromancer_id {
                return Err(ContractError::InvalidHydromancerId {
                    vessel_id: vessel.hydro_lock_id,
                    hydromancer_id: hydromancer_id,
                    vessel_hydromancer_id: vessel.hydromancer_id,
                });
            }
            if state::is_vessel_under_user_control(
                deps.storage,
                tranche_id,
                current_round_id,
                vessel.hydro_lock_id,
            ) {
                return Err(ContractError::VesselUnderUserControl {
                    vessel_id: vessel.hydro_lock_id,
                });
            }
            let previous_harbor_id = state::get_harbor_of_vessel(
                deps.storage,
                tranche_id,
                current_round_id,
                vessel.hydro_lock_id,
            )?;
            match previous_harbor_id {
                Some(previous_harbor_id) => {
                    if previous_harbor_id != vessels_to_harbor.harbor_id {
                        //vote has changed
                        state::remove_vessel_harbor(
                            deps.storage,
                            tranche_id,
                            current_round_id,
                            previous_harbor_id,
                            vessel.hydro_lock_id,
                        )?;
                        //save could be done after the match statement, but it will be done also whan previous harbor id is the same as the new one
                        state::add_vessel_to_harbor(
                            deps.storage,
                            tranche_id,
                            current_round_id,
                            vessels_to_harbor.harbor_id,
                            &VesselHarbor {
                                user_control: false,
                                hydro_lock_id: vessel.hydro_lock_id,
                                steerer_id: hydromancer_id,
                            },
                        )?;
                    }
                }
                None => {
                    state::add_vessel_to_harbor(
                        deps.storage,
                        tranche_id,
                        current_round_id,
                        vessels_to_harbor.harbor_id,
                        &VesselHarbor {
                            user_control: false,
                            hydro_lock_id: vessel.hydro_lock_id,
                            steerer_id: hydromancer_id,
                        },
                    )?;
                }
            }

            lock_ids.push(vessel.hydro_lock_id);
        }

        let proposal_to_lockups = ProposalToLockups {
            proposal_id: vessels_to_harbor.harbor_id,
            lock_ids,
        };
        proposal_votes.push(proposal_to_lockups);
    }

    let vote_message = Vote {
        tranche_id,
        proposals_votes: proposal_votes,
    };
    let execute_vote_msg = WasmMsg::Execute {
        contract_addr: constants.hydro_config.hydro_contract_address.to_string(),
        msg: to_json_binary(&vote_message)?,
        funds: vec![],
    };
    let response = Response::new().add_message(execute_vote_msg);
    Ok(response)
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::BuildVessel { vessels, receiver } => {
            execute_build_vessel(deps, info, vessels, receiver)
        }
        ExecuteMsg::AutoMaintain {} => execute_auto_maintain(deps, info),
        ExecuteMsg::UpdateVesselsClass {
            hydro_lock_ids,
            hydro_lock_duration,
        } => execute_update_vessels_class(deps, info, hydro_lock_ids, hydro_lock_duration),
        ExecuteMsg::ModifyAutoMaintenance {
            hydro_lock_ids,
            auto_maintenance,
        } => execute_modify_auto_maintenance(deps, info, hydro_lock_ids, auto_maintenance),
        ExecuteMsg::PauseContract {} => execute_pause_contract(deps, info),
        ExecuteMsg::UnpauseContract {} => execute_unpause_contract(deps, info),
        ExecuteMsg::DecommissionVessels { hydro_lock_ids } => {
            execute_decommission_vessels(deps, env, info, hydro_lock_ids)
        }
        ExecuteMsg::HydromancerVote {
            tranche_id,
            vessels_harbors,
        } => execute_hydromancer_vote(deps, info, tranche_id, vessels_harbors),
    }
}

fn query_voting_power(_deps: Deps, _env: Env) -> Result<VotingPowerResponse, StdError> {
    todo!()
}

fn query_hydro_current_round(deps: Deps, hydro_contract_addr: String) -> Result<RoundId, StdError> {
    let current_round_resp: CurrentRoundResponse = deps
        .querier
        .query_wasm_smart(hydro_contract_addr, &HydroQueryMsg::CurrentRound {})
        .expect("Failed to query hydro contract, hydro should be able to return the current round");
    Ok(current_round_resp.round_id)
}

fn query_vessels_by_owner(
    deps: Deps,
    owner: String,
    start_index: Option<usize>,
    limit: Option<usize>,
) -> StdResult<VesselsResponse> {
    let owner = deps.api.addr_validate(owner.as_str())?;
    let limit = limit
        .unwrap_or(DEFAULT_PAGINATION_LIMIT)
        .min(MAX_PAGINATION_LIMIT);
    let start_index = start_index.unwrap_or(0);

    let vessels = state::get_vessels_by_owner(deps.storage, owner.clone(), start_index, limit)
        .map_err(|e| {
            StdError::generic_err(format!("Failed to get vessels for {}: {}", owner, e))
        })?;

    let total = vessels.len();

    Ok(VesselsResponse {
        vessels,
        start_index,
        limit,
        total,
    })
}

fn query_vessels_by_hydromancer(
    deps: Deps,
    hydromancer_addr: String,
    start_index: Option<usize>,
    limit: Option<usize>,
) -> StdResult<VesselsResponse> {
    let hydromancer_addr = deps.api.addr_validate(hydromancer_addr.as_str())?;
    let limit = limit
        .unwrap_or(DEFAULT_PAGINATION_LIMIT)
        .min(MAX_PAGINATION_LIMIT);
    let start_index = start_index.unwrap_or(0);

    let vessels =
        state::get_vessels_by_hydromancer(deps.storage, hydromancer_addr, start_index, limit)?;
    let total = vessels.len();

    Ok(VesselsResponse {
        vessels,
        start_index,
        limit,
        total,
    })
}

fn query_constants(deps: Deps) -> StdResult<ConstantsResponse> {
    let constants = state::get_constants(deps.storage)?;
    Ok(ConstantsResponse { constants })
}

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, StdError> {
    match msg {
        QueryMsg::VotingPower {} => to_json_binary(&query_voting_power(deps, env)?),
        QueryMsg::VesselsByOwner {
            owner,
            start_index,
            limit,
        } => to_json_binary(&query_vessels_by_owner(deps, owner, start_index, limit)?),
        QueryMsg::VesselsByHydromancer {
            hydromancer_addr,
            start_index,
            limit,
        } => to_json_binary(&query_vessels_by_hydromancer(
            deps,
            hydromancer_addr,
            start_index,
            limit,
        )?),
        QueryMsg::Constants {} => to_json_binary(&query_constants(deps)?),
    }
}

fn validate_contract_is_not_paused(constant: &Constants) -> Result<(), ContractError> {
    match constant.paused_contract {
        true => Err(ContractError::Paused),
        false => Ok(()),
    }
}

fn validate_admin_address(deps: &DepsMut, sender: &Addr) -> Result<(), ContractError> {
    let whitelisted = state::is_whitelisted_admin(deps.storage, sender)?;
    match whitelisted {
        true => Ok(()),
        false => Err(ContractError::Unauthorized {}),
    }
}
#[entry_point]
pub fn reply(deps: DepsMut, env: Env, reply: Reply) -> Result<Response, ContractError> {
    match reply.id {
        // Cas : retour du premier message
        HYDRO_LOCK_TOKENS_REPLY_ID => parse_lock_tokens_reply(reply)
            .and_then(|(id, payload)| handle_lock_tokens_reply(deps, id, payload)),

        DECOMMISSION_REPLY_ID => handle_unlock_tokens_reply(deps, env, reply),

        _ => Err(ContractError::CustomError {
            msg: "Unknown reply id".to_string(),
        }),
    }
}

#[entry_point]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, StdError> {
    Ok(Response::default())
}

fn handle_lock_tokens_reply(
    deps: DepsMut,
    hydro_lock_id: u64,
    LockTokensReplyPayload {
        params:
            BuildVesselParams {
                lock_duration,
                auto_maintenance,
                hydromancer_id,
            },
        tokenized_share_record_id,
        owner,
    }: LockTokensReplyPayload,
) -> Result<Response, ContractError> {
    let vessel = Vessel {
        hydro_lock_id,
        class_period: lock_duration,
        tokenized_share_record_id,
        hydromancer_id,
        auto_maintenance,
    };

    state::add_vessel(deps.storage, &vessel, &owner)?;

    Ok(Response::new()
        .add_attribute("action", "build_vessel")
        .add_attribute("hydro_lock_id", hydro_lock_id.to_string())
        .add_attribute("owner", owner))
}

fn parse_lock_tokens_reply(
    reply: Reply,
) -> Result<(HydroLockId, LockTokensReplyPayload), ContractError> {
    let response = reply
        .result
        .into_result()
        .expect("always issued on_success");

    let lock_id = response
        .events
        .into_iter()
        .flat_map(|e| e.attributes)
        .find_map(|attr| (attr.key == "lock_id").then(|| attr.value.parse().ok()))
        .flatten()
        .expect("lock tokens reply always contains valid lock_id attribute");

    let payload = from_json(reply.payload).expect("build vessel parameters always attached");

    Ok((lock_id, payload))
}

fn handle_unlock_tokens_reply(
    deps: DepsMut,
    env: Env,
    reply: Reply,
) -> Result<Response, ContractError> {
    let response = reply
        .result
        .into_result()
        .expect("always issued on_success");

    let decommission_vessels_params: DecommissionVesselsParameters =
        from_json(reply.payload).expect("decommission vessels parameters always attached");

    let previous_balances = decommission_vessels_params.previous_balances;

    // Retrieve unlocked tokens from reply
    let hydro_unlocked_tokens: Vec<Coin> = response
        .events
        .clone()
        .into_iter()
        .flat_map(|e| e.attributes)
        .find_map(|attr| {
            (attr.key == "unlocked_tokens").then(|| {
                if attr.value.is_empty() {
                    Vec::new()
                } else {
                    attr.value
                        .split(", ")
                        .map(|v| v.parse::<Coin>().unwrap())
                        .collect()
                }
            })
        })
        .expect("unlock tokens reply always contains valid unlocked_hydro_lock_ids attribute");

    // Check the new balance and compare with the previous one
    // Query current balance after unlocking
    let balance_query = BankQuery::AllBalances {
        address: env.contract.address.to_string(),
    };
    let current_balances: AllBalanceResponse =
        deps.querier.query(&QueryRequest::Bank(balance_query))?;

    // Calculate difference in balances
    let mut received_coins: Vec<Coin> = vec![];
    for current_coin in current_balances.amount {
        let previous_amount = previous_balances
            .iter()
            .find(|c| c.denom == current_coin.denom)
            .map(|c| c.amount)
            .unwrap_or_default();

        if current_coin.amount > previous_amount {
            received_coins.push(Coin {
                denom: current_coin.denom,
                amount: current_coin.amount - previous_amount,
            });
        }
    }

    // Compare hydro_unlocked_tokens with received_coins
    // It might not be in the same order
    if !compare_coin_vectors(hydro_unlocked_tokens.clone(), received_coins) {
        return Err(ContractError::CustomError {
            msg: "Unlocked tokens do not match the received ones".to_string(),
        });
    }

    // Forward all received tokens to the original sender
    let forward_msg = BankMsg::Send {
        to_address: decommission_vessels_params.vessel_owner.to_string(),
        amount: hydro_unlocked_tokens, // Forward all received tokens
    };

    // Retrieve unlocked lock IDs from the reply
    let unlocked_hydro_lock_ids: Vec<u64> = response
        .events
        .into_iter()
        .flat_map(|e| e.attributes)
        .find_map(|attr| {
            (attr.key == "unlocked_lock_ids").then(|| {
                if attr.value.is_empty() {
                    Vec::new()
                } else {
                    attr.value
                        .split(", ")
                        .map(|v| v.parse::<u64>().unwrap())
                        .collect()
                }
            })
        })
        .expect("Hydro's UnlockTokens reply always contains valid unlocked_lock_ids attribute");

    // Check if the unlocked lock IDs match the expected ones
    // It might not be in the same order
    if !compare_u64_vectors(
        unlocked_hydro_lock_ids.clone(),
        decommission_vessels_params.expected_unlocked_ids,
    ) {
        return Err(ContractError::CustomError {
            msg: "Unlocked lock IDs do not match the expected ones".to_string(),
        });
    }

    for hydro_lock_id in unlocked_hydro_lock_ids.iter() {
        state::remove_vessel(
            deps.storage,
            &decommission_vessels_params.vessel_owner,
            *hydro_lock_id,
        )?;
    }

    Ok(Response::new()
        .add_message(forward_msg)
        .add_attribute("action", "decommission_vessels")
        .add_attribute(
            "unlocked_hydro_lock_ids",
            unlocked_hydro_lock_ids
                .iter()
                .map(|x| x.to_string())
                .collect::<Vec<String>>()
                .join(", "),
        )
        .add_attribute(
            "owner",
            decommission_vessels_params.vessel_owner.to_string(),
        ))
}

#[cfg(test)]
mod test {
    use cosmwasm_std::{
        coin, coins, from_json,
        testing::{
            mock_dependencies as std_mock_dependencies, mock_env, MockApi,
            MockQuerier as StdMockQuerier, MockStorage,
        },
        Addr, Binary, ContractResult, CosmosMsg, DepsMut, Empty, GrpcQuery, MessageInfo, OwnedDeps,
        Querier, QuerierResult, QueryRequest, ReplyOn, WasmMsg,
    };
    use hydro_interface::msgs::ExecuteMsg as HydroExecuteMsg;
    use neutron_std::types::ibc::applications::transfer::v1::{
        DenomTrace, QueryDenomTraceRequest, QueryDenomTraceResponse,
    };
    use prost::Message;
    use zephyrus_core::msgs::{BuildVesselParams, InstantiateMsg};
    use zephyrus_core::state::Vessel;

    use crate::{contract::LockTokensReplyPayload, errors::ContractError};

    struct MockQuerier(StdMockQuerier);

    fn mock_grpc_query_handler(path: &str, data: &[u8]) -> QuerierResult {
        let contract_result: ContractResult<Binary> = match path {
            "/ibc.applications.transfer.v1.Query/DenomTrace" => {
                dbg!(String::from_utf8_lossy(data));
                let QueryDenomTraceRequest { hash } = QueryDenomTraceRequest::decode(data).unwrap();

                let denom_trace = match hash.as_str() {
                    "69ED129755461CF93B7E64A277A3552582B47A64F826F05E4F43E22C2D476C02" => {
                        DenomTrace {
                            path: "transfer/channel-0".to_owned(),
                            base_denom: "cosmosvaloper18hl5c9xn5dze2g50uaw0l2mr02ew57zk0auktn/12"
                                .to_owned(),
                        }
                    }
                    "FB6F9C479D2E47419EAA9C9A48B325F68A032F76AFA04890F1278C47BC0A8BB4" => {
                        DenomTrace {
                            path: "transfer/channel-0".to_owned(),
                            base_denom: "cosmosvaloper18hl5c9xn5dze2g50uaw0l2mr02ew57zk0auktn/10"
                                .to_owned(),
                        }
                    }
                    "27394FB092D2ECCD56123C74F36E4C1F926001CEADA9CA97EA622B25F41E5EB2" => {
                        DenomTrace {
                            path: "transfer/channel-0".to_owned(),
                            base_denom: "uatom".to_owned(),
                        }
                    }
                    _ => return QuerierResult::Err(cosmwasm_std::SystemError::Unknown {}),
                };

                ContractResult::Ok(
                    QueryDenomTraceResponse {
                        denom_trace: Some(denom_trace),
                    }
                    .encode_to_vec()
                    .into(),
                )
            }

            other => panic!("unexpected grpc query: {other}"),
        };

        QuerierResult::Ok(contract_result)
    }

    impl Querier for MockQuerier {
        fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
            let Some(QueryRequest::<Empty>::Grpc(GrpcQuery { path, data })) =
                from_json(bin_request).ok()
            else {
                return self.0.raw_query(bin_request);
            };

            mock_grpc_query_handler(&path, &data)
        }
    }

    fn mock_dependencies() -> OwnedDeps<MockStorage, MockApi, MockQuerier, Empty> {
        let OwnedDeps {
            storage,
            api,
            querier,
            custom_query_type,
        } = std_mock_dependencies();

        OwnedDeps {
            querier: MockQuerier(querier),
            storage,
            api,
            custom_query_type,
        }
    }

    fn make_valid_addr(seed: &str) -> Addr {
        MockApi::default().addr_make(seed)
    }

    fn init_contract(deps: DepsMut) {
        super::instantiate(
            deps,
            mock_env(),
            MessageInfo {
                sender: make_valid_addr("deployer"),
                funds: vec![],
            },
            InstantiateMsg {
                hydro_contract_address: make_valid_addr("hydro").into_string(),
                tribute_contract_address: make_valid_addr("tribute").into_string(),
                whitelist_admins: vec![make_valid_addr("admin").into_string()],
                default_hydromancer_name: make_valid_addr("zephyrus").into_string(),
                default_hydromancer_commission_rate: "0.1".parse().unwrap(),
                default_hydromancer_address: make_valid_addr("zephyrus").into_string(),
            },
        )
        .unwrap();
    }

    #[test]
    fn build_vessel_fails_if_funds_sent() {
        let mut deps = mock_dependencies();

        init_contract(deps.as_mut());

        assert_eq!(
            super::execute_build_vessel(
                deps.as_mut(),
                MessageInfo {
                    sender: make_valid_addr("alice"),
                    funds: vec![]
                },
                vec![],
                None
            )
            .unwrap_err(),
            ContractError::NoTokensReceived
        );
    }

    #[test]
    fn build_vessel_fails_if_params_len_not_equal_funds_len() {
        let mut deps = mock_dependencies();

        init_contract(deps.as_mut());

        assert_eq!(
            super::execute_build_vessel(
                deps.as_mut(),
                MessageInfo {
                    sender: make_valid_addr("alice"),
                    funds: coins(
                        1_000_000,
                        "ibc/69ED129755461CF93B7E64A277A3552582B47A64F826F05E4F43E22C2D476C02"
                    )
                },
                vec![],
                None
            )
            .unwrap_err(),
            ContractError::CreateVesselParamsLengthMismatch {
                params_len: 0,
                funds_len: 1
            }
        );

        assert_eq!(
            super::execute_build_vessel(
                deps.as_mut(),
                MessageInfo {
                    sender: make_valid_addr("alice"),
                    funds: coins(
                        1_000_000,
                        "ibc/69ED129755461CF93B7E64A277A3552582B47A64F826F05E4F43E22C2D476C02"
                    )
                },
                vec![
                    BuildVesselParams {
                        lock_duration: 12,
                        auto_maintenance: true,
                        hydromancer_id: 0
                    },
                    BuildVesselParams {
                        lock_duration: 12,
                        auto_maintenance: true,
                        hydromancer_id: 0
                    }
                ],
                None
            )
            .unwrap_err(),
            ContractError::CreateVesselParamsLengthMismatch {
                params_len: 2,
                funds_len: 1
            }
        );
    }

    #[test]
    fn build_vessel_fails_if_hydromancer_does_not_exist() {
        let mut deps = mock_dependencies();

        init_contract(deps.as_mut());

        assert_eq!(
            super::execute_build_vessel(
                deps.as_mut(),
                MessageInfo {
                    sender: make_valid_addr("alice"),
                    funds: coins(
                        1_000_000,
                        "ibc/69ED129755461CF93B7E64A277A3552582B47A64F826F05E4F43E22C2D476C02"
                    ),
                },
                vec![BuildVesselParams {
                    lock_duration: 12,
                    auto_maintenance: true,
                    hydromancer_id: 1
                },],
                None
            )
            .unwrap_err(),
            ContractError::HydromancerNotFound { hydromancer_id: 1 }
        );
    }

    #[test]
    fn build_vessel_fails_if_ibc_coin_denom_trace_is_not_share_record() {
        let mut deps = mock_dependencies();

        init_contract(deps.as_mut());

        assert_eq!(
            super::execute_build_vessel(
                deps.as_mut(),
                MessageInfo {
                    sender: make_valid_addr("alice"),
                    funds: coins(
                        1_000_000,
                        "ibc/27394FB092D2ECCD56123C74F36E4C1F926001CEADA9CA97EA622B25F41E5EB2"
                    )
                },
                vec![BuildVesselParams {
                    lock_duration: 12,
                    auto_maintenance: true,
                    hydromancer_id: 0
                },],
                None
            )
            .unwrap_err(),
            ContractError::InvalidLsmTokenReceived(
                "ibc/27394FB092D2ECCD56123C74F36E4C1F926001CEADA9CA97EA622B25F41E5EB2".to_owned()
            )
        );
    }

    #[test]
    fn build_vessel_response_contains_lock_submsg_per_lsm_share() {
        let mut deps = mock_dependencies();

        init_contract(deps.as_mut());

        let res = super::execute_build_vessel(
            deps.as_mut(),
            MessageInfo {
                sender: make_valid_addr("alice"),
                funds: vec![
                    coin(
                        1_000_000,
                        "ibc/69ED129755461CF93B7E64A277A3552582B47A64F826F05E4F43E22C2D476C02",
                    ),
                    coin(
                        500_000,
                        "ibc/FB6F9C479D2E47419EAA9C9A48B325F68A032F76AFA04890F1278C47BC0A8BB4",
                    ),
                ],
            },
            vec![
                BuildVesselParams {
                    lock_duration: 12,
                    auto_maintenance: true,
                    hydromancer_id: 0,
                },
                BuildVesselParams {
                    lock_duration: 3,
                    auto_maintenance: false,
                    hydromancer_id: 0,
                },
            ],
            None,
        )
        .unwrap();

        assert_eq!(res.messages.len(), 2);

        let decoded_submessages: Vec<(HydroExecuteMsg, (&str, u128), LockTokensReplyPayload)> = res
            .messages
            .iter()
            .map(|submsg| {
                assert_eq!(
                    submsg.reply_on,
                    ReplyOn::Success,
                    "all lock messages should be reply_on_success"
                );

                let CosmosMsg::Wasm(WasmMsg::Execute { msg, funds, .. }) = &submsg.msg else {
                    panic!("unexpected msg: {submsg:?}");
                };

                assert_eq!(
                    funds.len(),
                    1,
                    "each lock message should have exactly one coin attached"
                );

                (
                    from_json(msg.clone()).unwrap(),
                    (funds[0].denom.as_str(), funds[0].amount.u128()),
                    from_json(submsg.payload.clone()).unwrap(),
                )
            })
            .collect();

        assert!(matches!(
            decoded_submessages.as_slice(),
            [
                (
                    HydroExecuteMsg::LockTokens { lock_duration: 12 },
                    (
                        "ibc/69ED129755461CF93B7E64A277A3552582B47A64F826F05E4F43E22C2D476C02",
                        1_000_000
                    ),
                    LockTokensReplyPayload {
                        params: BuildVesselParams {
                            lock_duration: 12,
                            auto_maintenance: true,
                            hydromancer_id: 0
                        },
                        tokenized_share_record_id: 12,
                        ..
                    }
                ),
                (
                    HydroExecuteMsg::LockTokens { lock_duration: 3 },
                    (
                        "ibc/FB6F9C479D2E47419EAA9C9A48B325F68A032F76AFA04890F1278C47BC0A8BB4",
                        500_000
                    ),
                    LockTokensReplyPayload {
                        params: BuildVesselParams {
                            lock_duration: 3,
                            auto_maintenance: false,
                            hydromancer_id: 0
                        },
                        tokenized_share_record_id: 10,
                        ..
                    }
                )
            ]
        ))
    }

    #[test]
    fn handle_lock_tokens_reply_updates_state_correctly() {
        let mut deps = mock_dependencies();

        init_contract(deps.as_mut());

        let expected_owner = make_valid_addr("alice");

        let expected_vessel = Vessel {
            hydro_lock_id: 0,
            tokenized_share_record_id: 10,
            class_period: 3,
            auto_maintenance: false,
            hydromancer_id: 0,
        };

        super::handle_lock_tokens_reply(
            deps.as_mut(),
            0,
            LockTokensReplyPayload {
                params: BuildVesselParams {
                    lock_duration: 3,
                    auto_maintenance: false,
                    hydromancer_id: 0,
                },
                tokenized_share_record_id: 10,
                owner: expected_owner.clone(),
            },
        )
        .unwrap();

        assert_eq!(
            super::state::get_vessel(&deps.storage, 0).unwrap(),
            expected_vessel
        );

        assert_eq!(
            super::state::get_vessels_by_owner(&deps.storage, expected_owner.clone(), 0, 100)
                .unwrap(),
            vec![expected_vessel]
        );

        assert_eq!(
            super::state::get_vessels_by_hydromancer(
                &deps.storage,
                make_valid_addr("zephyrus"),
                0,
                100
            )
            .unwrap(),
            vec![expected_vessel]
        );

        assert!(super::state::is_tokenized_share_record_used(
            &deps.storage,
            10
        ));
    }

    #[test]
    fn build_vessel_fails_if_tokenized_share_record_already_in_use() {
        let mut deps = mock_dependencies();

        init_contract(deps.as_mut());

        super::handle_lock_tokens_reply(
            deps.as_mut(),
            0,
            LockTokensReplyPayload {
                params: BuildVesselParams {
                    lock_duration: 3,
                    auto_maintenance: false,
                    hydromancer_id: 0,
                },
                tokenized_share_record_id: 10,
                owner: make_valid_addr("alice"),
            },
        )
        .unwrap();

        assert_eq!(
            super::execute_build_vessel(
                deps.as_mut(),
                MessageInfo {
                    sender: make_valid_addr("alice"),
                    funds: vec![
                        coin(
                            1_000_000,
                            "ibc/69ED129755461CF93B7E64A277A3552582B47A64F826F05E4F43E22C2D476C02",
                        ),
                        coin(
                            500_000,
                            "ibc/FB6F9C479D2E47419EAA9C9A48B325F68A032F76AFA04890F1278C47BC0A8BB4",
                        ),
                    ],
                },
                vec![
                    BuildVesselParams {
                        lock_duration: 12,
                        auto_maintenance: true,
                        hydromancer_id: 0,
                    },
                    BuildVesselParams {
                        lock_duration: 3,
                        auto_maintenance: false,
                        hydromancer_id: 0,
                    },
                ],
                None,
            )
            .unwrap_err(),
            ContractError::TokenizedShareRecordAlreadyInUse(10)
        );
    }
}
