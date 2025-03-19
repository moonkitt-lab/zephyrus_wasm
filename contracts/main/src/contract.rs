use std::collections::{BTreeSet, HashMap};

use cosmwasm_std::{
    entry_point, from_json, to_json_binary, Addr, AllBalanceResponse, BankMsg, BankQuery, Binary,
    Coin, Decimal, Deps, DepsMut, Env, MessageInfo, QueryRequest, Reply, Response as CwResponse,
    StdError, StdResult, SubMsg, Uint128, WasmMsg,
};
use hydro_interface::msgs::HydroExecuteMsg::{
    LockTokens, RefreshLockDuration, UnlockTokens, Unvote, Vote,
};
use hydro_interface::msgs::{
    CurrentRoundResponse, CurrentRoundTimeWeightedSharesResponse, HydroExecuteMsg, HydroQueryMsg,
    OutstandingTributeClaimsResponse, ProposalToLockups, TributeClaim, TributeQueryMsg,
    ValidatorPowerRatioResponse,
};
use hydro_interface::state::query_lock_entries;
use neutron_sdk::bindings::msg::NeutronMsg;
use serde::{Deserialize, Serialize};
use zephyrus_core::msgs::{
    BuildVesselParams, ConstantsResponse, ExecuteMsg, HydroProposalId, HydromancerId,
    InstantiateMsg, MigrateMsg, QueryMsg, RoundId, TrancheId, UserId, VesselHarborInfo,
    VesselHarborResponse, VesselsResponse, VesselsToHarbor, VotingPowerResponse,
};
use zephyrus_core::state::{Constants, HydroConfig, HydroLockId, Vessel, VesselHarbor};

use crate::state::get_vessels_by_hydromancer;
use crate::{
    errors::ContractError,
    helpers::ibc::{DenomTrace, QuerierExt as IbcQuerierExt},
    helpers::vectors::{compare_coin_vectors, compare_u64_vectors},
    state,
};

type Response = CwResponse<NeutronMsg>;

const HYDRO_LOCK_TOKENS_REPLY_ID: u64 = 1;
const DECOMMISSION_REPLY_ID: u64 = 2;
const VOTE_REPLY_ID: u64 = 3;
const CLAIM_REPLY_ID: u64 = 4;

const MAX_PAGINATION_LIMIT: usize = 1000;
const DEFAULT_PAGINATION_LIMIT: usize = 100;

#[derive(Serialize, Deserialize)]
struct LockTokensReplyPayload {
    params: BuildVesselParams,
    tokenized_share_record_id: u64,
    owner: Addr,
    owner_id: u64,
}

#[derive(Serialize, Deserialize)]
struct VoteReplyPayload {
    tranche_id: u64,
    vessels_harbors: Vec<VesselsToHarbor>,
    steerer_id: u64,
    round_id: u64,
    user_vote: bool,
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

    let mut owner_id = state::get_user_id_by_address(deps.storage, owner.clone());
    if owner_id.is_err() {
        owner_id = state::insert_new_user(deps.storage, owner.clone());
    }
    let owner_id = owner_id.expect("Owner id should be present");
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
            owner_id,
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

pub fn has_duplicate_vessel_id(vessel_ids: Vec<HydroLockId>) -> (bool, Option<HydroLockId>) {
    let mut seen = BTreeSet::new();
    for item in vessel_ids.iter() {
        if !seen.insert(item) {
            return (true, Some(*item));
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
    deps: &mut DepsMut,
    env: Env,
    info: MessageInfo,
    tranche_id: u64,
    vessels_harbors: Vec<VesselsToHarbor>,
) -> Result<Response, ContractError> {
    let constants = state::get_constants(deps.storage)?;
    validate_contract_is_not_paused(&constants)?;

    //initialize time weighted shares for hydromancer and current round if they are not initialized
    initialize_time_weighted_shares_for_hydromancer_and_current_round(
        deps,
        env.clone(),
        tranche_id,
        info.sender.clone(),
        constants.hydro_config.hydro_contract_address.to_string(),
    )?;
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

    let hydromancer_id = state::get_hydromancer_id_by_address(deps.storage, info.sender.clone())
        .map_err(|err: StdError| ContractError::from(err))?;
    let current_round_id = query_hydro_current_round(
        deps.as_ref(),
        constants.hydro_config.hydro_contract_address.to_string(),
    )?;
    let mut proposal_votes = vec![];
    for vessels_to_harbor in vessels_harbors.clone() {
        let proposal_to_lockups = ProposalToLockups {
            proposal_id: vessels_to_harbor.harbor_id,
            lock_ids: vessels_to_harbor.vessel_ids.clone(),
        };
        proposal_votes.push(proposal_to_lockups);
    }

    let vote_message = Vote {
        tranche_id,
        proposals_votes: proposal_votes,
    };
    let execute_hydro_vote_msg = WasmMsg::Execute {
        contract_addr: constants.hydro_config.hydro_contract_address.to_string(),
        msg: to_json_binary(&vote_message)?,
        funds: vec![],
    };
    let payload = to_json_binary(&VoteReplyPayload {
        tranche_id,
        vessels_harbors,
        steerer_id: hydromancer_id,
        round_id: current_round_id,
        user_vote: false,
    })?;
    let execute_hydro_vote_msg: SubMsg<NeutronMsg> =
        SubMsg::reply_on_success(execute_hydro_vote_msg, VOTE_REPLY_ID).with_payload(payload);
    let response = Response::new().add_submessage(execute_hydro_vote_msg);
    Ok(response)
}

fn execute_claim(
    deps: &mut DepsMut,
    env: Env,
    info: MessageInfo,
    tranche_id: u64,
    round_ids: Vec<u64>,
) -> Result<Response, ContractError> {
    let constants = state::get_constants(deps.storage)?;
    validate_contract_is_not_paused(&constants)?;

    let hydromancer_id =
        state::get_hydromancer_id_by_address(deps.storage, info.sender.clone()).ok();
    let user_id = state::get_user_id_by_address(deps.storage, info.sender.clone()).ok();

    let mut response = Response::new();
    let current_round_id = query_hydro_current_round(
        deps.as_ref(),
        constants
            .hydro_config
            .hydro_contract_address
            .to_string()
            .clone(),
    )?;
    for round_id in round_ids.iter() {
        if *round_id >= current_round_id {
            continue;
        }
        // intialize voting power if it's not yet initialized, vp are initialized on first claim for a round
        if !state::has_at_least_one_tribute_for_tranche_round(deps.storage, tranche_id, *round_id)?
        {
            initialize_user_voting_power_and_deleguated_to_hydromancers_by_trancheid_roundid(
                deps,
                constants.clone(),
                tranche_id,
                *round_id,
            )?;
        }
        let outstanding_tribute_claims = query_hydro_outstanding_tribute_claims(
            deps.as_ref(),
            env.clone(),
            constants
                .hydro_config
                .hydro_tribute_contract_address
                .clone(),
            tranche_id,
            *round_id,
        )?;

        for claim in outstanding_tribute_claims.claims.iter() {
            let claim_msg = WasmMsg::Execute {
                contract_addr: constants
                    .hydro_config
                    .hydro_tribute_contract_address
                    .to_string(),
                msg: to_json_binary(&HydroExecuteMsg::ClaimTribute {
                    round_id: claim.round_id,
                    tranche_id: claim.tranche_id,
                    tribute_id: claim.tribute_id,
                    voter_address: env.contract.address.to_string(),
                })?,
                funds: vec![],
            };

            let execute_hydro_claim_msg: SubMsg<NeutronMsg> =
                SubMsg::reply_on_success(claim_msg, CLAIM_REPLY_ID)
                    .with_payload(to_json_binary(&claim)?);

            response = response.add_submessage(execute_hydro_claim_msg);
        }

        //now we need to distribute to the sender tributes already claimed
        match user_id {
            Some(user_id) => {}
            None => {}
        }
        match hydromancer_id {
            Some(hydromancer_id) => {}
            None => {}
        }
    }

    Ok(response)
}

fn execute_change_hydromancer(
    deps: DepsMut,
    info: MessageInfo,
    tranche_id: u64,
    hydromancer_id: u64,
    hydro_lock_ids: Vec<u64>,
) -> Result<Response, ContractError> {
    let constants = state::get_constants(deps.storage)?;
    validate_contract_is_not_paused(&constants)?;

    if !state::are_vessels_owned_by(deps.storage, &info.sender, &hydro_lock_ids)? {
        return Err(ContractError::Unauthorized {});
    }

    state::get_hydromancer(deps.storage, hydromancer_id)?;
    let current_round_id = query_hydro_current_round(
        deps.as_ref(),
        constants.hydro_config.hydro_contract_address.to_string(),
    )?;

    for hydro_lock_id in hydro_lock_ids.iter() {
        state::change_vessel_hydromancer(
            deps.storage,
            tranche_id,
            *hydro_lock_id,
            current_round_id,
            hydromancer_id,
        )?;
    }
    let unvote_msg = Unvote {
        tranche_id,
        lock_ids: hydro_lock_ids.clone(),
    };

    let execute_unvote_msg = WasmMsg::Execute {
        contract_addr: constants.hydro_config.hydro_contract_address.to_string(),
        msg: to_json_binary(&unvote_msg)?,
        funds: vec![],
    };

    Ok(Response::new()
        .add_message(execute_unvote_msg)
        .add_attribute("action", "change_hydromancer")
        .add_attribute("new_hydromancer_id", hydromancer_id.to_string())
        .add_attribute(
            "hydro_lock_id",
            hydro_lock_ids
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(","),
        ))
}

fn execute_user_vote(
    deps: DepsMut,
    env: Env,
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

    let user_id = state::get_user_id_by_address(deps.storage, info.sender)
        .map_err(|err: StdError| ContractError::from(err))?;
    let current_round_id = query_hydro_current_round(
        deps.as_ref(),
        constants.hydro_config.hydro_contract_address.to_string(),
    )?;
    let mut proposal_votes = vec![];
    let mut unvote_ids = vec![];

    for vessels_to_harbor in vessels_harbors.clone() {
        for vessel_id in vessels_to_harbor.vessel_ids.iter() {
            //if not under user control and already voted by hydromancer, lock_id should be unvote, otherwise if user vote the same proposal as hydromancer it will be skipped by hydro than zephyrus and still under hydromancer control
            if !state::is_vessel_under_user_control(
                deps.storage,
                tranche_id,
                current_round_id,
                *vessel_id,
            ) {
                if let Some(_) = state::get_harbor_of_vessel(
                    deps.storage,
                    tranche_id,
                    current_round_id,
                    *vessel_id,
                )? {
                    //vessel used by hydromancer should be unvoted
                    unvote_ids.push(vessel_id.clone());
                }
            }
        }

        let proposal_to_lockups = ProposalToLockups {
            proposal_id: vessels_to_harbor.harbor_id,
            lock_ids: vessels_to_harbor.vessel_ids.clone(),
        };
        proposal_votes.push(proposal_to_lockups);
    }
    let mut response = Response::new();
    if !unvote_ids.is_empty() {
        let unvote_msg = Unvote {
            tranche_id,
            lock_ids: unvote_ids.clone(),
        };

        let execute_unvote_msg = WasmMsg::Execute {
            contract_addr: constants.hydro_config.hydro_contract_address.to_string(),
            msg: to_json_binary(&unvote_msg)?,
            funds: vec![],
        };
        response = response.add_message(execute_unvote_msg);
    }

    let payload = to_json_binary(&VoteReplyPayload {
        tranche_id,
        vessels_harbors,
        steerer_id: user_id,
        round_id: current_round_id,
        user_vote: true,
    })?;

    let vote_message = Vote {
        tranche_id,
        proposals_votes: proposal_votes,
    };
    let execute_hydro_vote_msg = WasmMsg::Execute {
        contract_addr: constants.hydro_config.hydro_contract_address.to_string(),
        msg: to_json_binary(&vote_message)?,
        funds: vec![],
    };
    let execute_hydro_vote_msg: SubMsg<NeutronMsg> =
        SubMsg::reply_on_success(execute_hydro_vote_msg, VOTE_REPLY_ID).with_payload(payload);
    Ok(response.add_submessage(execute_hydro_vote_msg))
}

#[entry_point]
pub fn execute(
    mut deps: DepsMut,
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
        } => execute_hydromancer_vote(&mut deps, env, info, tranche_id, vessels_harbors),
        ExecuteMsg::UserVote {
            tranche_id,
            vessels_harbors,
        } => execute_user_vote(deps, env, info, tranche_id, vessels_harbors),
        ExecuteMsg::ChangeHydromancer {
            tranche_id,
            hydromancer_id,
            hydro_lock_ids,
        } => execute_change_hydromancer(deps, info, tranche_id, hydromancer_id, hydro_lock_ids),
        ExecuteMsg::Claim {
            tranche_id,
            round_ids,
        } => execute_claim(&mut deps, env, info, tranche_id, round_ids),
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

fn query_hydro_outstanding_tribute_claims(
    deps: Deps,
    env: Env,
    tribute_address_contract: Addr,
    tranche_id: TrancheId,
    round_id: RoundId,
) -> Result<OutstandingTributeClaimsResponse, StdError> {
    let outstanding_tribute_claims: OutstandingTributeClaimsResponse =
        deps.querier.query_wasm_smart(
            tribute_address_contract,
            &TributeQueryMsg::OutstandingTributeClaims {
                user_address: env.contract.address.to_string(),
                round_id: round_id,
                tranche_id: tranche_id,
                start_from: 0,
                limit: 1000, //probably never reaches this limit, if it does, next user claim will process the rest
            },
        )?;
    Ok(outstanding_tribute_claims)
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

//initialize time weighted shares for hydromancer and current round if they are not initialized
fn initialize_time_weighted_shares_for_hydromancer_and_current_round(
    deps: &mut DepsMut,
    env: Env,
    tranche_id: TrancheId,
    hydromancer_addr: Addr,
    hydro_contract_addr: String,
) -> Result<(), StdError> {
    let hydromancer_id =
        state::get_hydromancer_id_by_address(deps.storage, hydromancer_addr.clone())?;
    let current_round = query_hydro_current_round(deps.as_ref(), hydro_contract_addr.clone())?;
    let hydromancer_round_shares_already_initialized = state::has_shares_for_hydromancer_and_round(
        deps.storage,
        hydromancer_id,
        tranche_id,
        current_round,
    )?;
    println!(
        "hydromancer_round_shares_already_initialized: {}",
        hydromancer_round_shares_already_initialized
    );
    if !hydromancer_round_shares_already_initialized {
        let count_vessels = state::get_vessels_count_by_hydromancer(deps.storage, hydromancer_id)?;
        let vessels = state::get_vessels_by_hydromancer(
            deps.storage,
            hydromancer_addr.clone(),
            0,
            count_vessels,
        )?;
        let weighted_shares = query_hydro_current_time_weighted_shares_by_hydromancer(
            deps.as_ref(),
            env,
            hydro_contract_addr,
            hydromancer_id,
            vessels,
        )?;

        for lock_time_weighted_share in weighted_shares.lock_time_weighted_shares {
            let is_vessel_under_user_control = state::is_vessel_under_user_control(
                deps.storage,
                tranche_id,
                current_round,
                lock_time_weighted_share.lock_id,
            );
            if !is_vessel_under_user_control {
                state::add_weighted_shares_to_hydromancer(
                    deps.storage,
                    hydromancer_id,
                    tranche_id,
                    weighted_shares.round_id,
                    &lock_time_weighted_share.validator,
                    lock_time_weighted_share.time_weighted_share,
                )
                .expect("Failed to insert time weighted shares");
                let vessel = state::get_vessels_by_ids(
                    deps.storage,
                    &vec![lock_time_weighted_share.lock_id],
                )?
                .pop()
                .expect("Vessel should exist");
                state::add_weighted_shares_to_user_hydromancer(
                    deps.storage,
                    tranche_id,
                    vessel.owner_id,
                    hydromancer_id,
                    weighted_shares.round_id,
                    &lock_time_weighted_share.validator,
                    lock_time_weighted_share.time_weighted_share,
                )?;
            }
        }
    }

    Ok(())
}

fn query_hydro_current_time_weighted_shares_by_hydromancer(
    deps: Deps,
    env: Env,
    hydro_contract_addr: String,
    hydromancer_id: u64,
    vessels: Vec<Vessel>,
) -> Result<CurrentRoundTimeWeightedSharesResponse, StdError> {
    let lock_ids = vessels.iter().map(|v| v.hydro_lock_id).collect::<Vec<_>>();
    let time_weighted_shares: CurrentRoundTimeWeightedSharesResponse = deps
        .querier
        .query_wasm_smart(
            hydro_contract_addr,
            &HydroQueryMsg::CurrentRoundTimeWeightedShares {
                owner: env.contract.address.to_string(),
                lock_ids,
            },
        )
        .map_err(|e| {
            StdError::generic_err(format!(
                "Failed to get time weighted shares for hydromancer {} from hydro : {}",
                hydromancer_id, e
            ))
        })?;
    Ok(time_weighted_shares)
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

fn query_vessels_harbor(
    deps: Deps,
    tranche_id: u64,
    round_id: u64,
    vessel_ids: Vec<u64>,
) -> StdResult<VesselHarborResponse> {
    let (has_duplicated_vessel_id, vessel_id) = has_duplicate_vessel_id(vessel_ids.clone());
    if has_duplicated_vessel_id {
        let vessel_id = vessel_id.expect("If there is duplicated vessel, id should be present");
        return Err(StdError::generic_err(format!(
            "Duplicated vessel id: {}",
            vessel_id
        )));
    }
    let mut vessels_harbor_info = vec![];
    for vessel_id in vessel_ids {
        let _ = state::get_vessel(deps.storage, vessel_id)?; //return error if there is one vessel that does not exist
        let vessel_harbor = state::get_vessel_harbor(deps.storage, tranche_id, round_id, vessel_id);
        match vessel_harbor {
            Err(_) => vessels_harbor_info.push(VesselHarborInfo {
                vessel_to_harbor: None,
                vessel_id,
                harbor_id: None,
            }),
            Ok(vessel_harbor) => vessels_harbor_info.push(VesselHarborInfo {
                vessel_to_harbor: Some(vessel_harbor.0),
                vessel_id,
                harbor_id: Some(vessel_harbor.1),
            }),
        }
    }

    Ok(VesselHarborResponse {
        vessels_harbor_info,
    })
}

fn initialize_user_voting_power_and_deleguated_to_hydromancers_by_trancheid_roundid(
    deps: &mut DepsMut,
    constants: Constants,
    tranche_id: TrancheId,
    round_id: RoundId,
) -> Result<(), ContractError> {
    let user_validator_shares = state::get_users_hydromancer_shares_by_user_tranche_round(
        deps.storage,
        tranche_id,
        round_id,
    )?;
    for user_val_shares in user_validator_shares {
        let (user_id, hydromancer_id, validator, shares) = user_val_shares;
        let val_info_response: ValidatorPowerRatioResponse = deps.querier.query_wasm_smart(
            constants.hydro_config.hydro_contract_address.to_string(),
            &HydroQueryMsg::ValidatorPowerRatio {
                validator: validator.clone(),
                round_id: round_id,
            },
        )?;
        let validator_power_ratio = val_info_response.ratio;
        let voting_power =
            validator_power_ratio.checked_mul(Decimal::from_ratio(shares, Uint128::one()));
        let voting_power = match voting_power {
            Err(_) => {
                // if there was an overflow error, log this but return 0
                deps.api.debug(&format!(
                            "An error occured while computing voting power for time weighted shares: {:?} and validator : {:?}",
                            shares,validator
                        ));

                Uint128::zero()
            }
            Ok(current_voting_power) => current_voting_power.to_uint_ceil(),
        };

        state::add_user_hydromancer_tranche_round_voting_power(
            deps.storage,
            user_id,
            hydromancer_id,
            tranche_id,
            round_id,
            voting_power.u128(),
        )?;
        state::add_hydromancer_tranche_round_voting_power(
            deps.storage,
            hydromancer_id,
            tranche_id,
            round_id,
            voting_power.u128(),
        )?;
    }
    Ok(())
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
        QueryMsg::VesselsHarbor {
            tranche_id,
            round_id,
            lock_ids,
        } => to_json_binary(&query_vessels_harbor(deps, tranche_id, round_id, lock_ids)?),
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
        VOTE_REPLY_ID => {
            let skipped_locks = parse_locks_skipped_reply(reply.clone())?;
            let payload: VoteReplyPayload =
                from_json(reply.payload).expect("Vote parameters always attached");
            handle_vote_reply(deps, env, payload, skipped_locks)
        }
        CLAIM_REPLY_ID => handle_claim_reply(deps, env, reply),
        _ => Err(ContractError::CustomError {
            msg: "Unknown reply id".to_string(),
        }),
    }
}

#[entry_point]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, StdError> {
    Ok(Response::default())
}

fn handle_claim_reply(deps: DepsMut, env: Env, reply: Reply) -> Result<Response, ContractError> {
    let claim: TributeClaim = from_json(reply.payload)?;
    let constants = state::get_constants(deps.storage)?;
    let hydromancer_validator_shares =
        state::get_proposal_time_weigthed_shares_by_hydromancer_validators(
            deps.storage,
            claim.proposal_id,
        )?;
    let mut zephyrus_voting_power: u128 = 0u128;
    let mut hydromancer_voting_power: HashMap<HydromancerId, u128> = HashMap::new();
    let mut user_voting_power: HashMap<UserId, u128> = HashMap::new();
    for hydro_val_sahres in hydromancer_validator_shares {
        let (hydromancer_id, validator, shares) = hydro_val_sahres;

        let val_info_response: ValidatorPowerRatioResponse = deps.querier.query_wasm_smart(
            constants.hydro_config.hydro_contract_address.to_string(),
            &HydroQueryMsg::ValidatorPowerRatio {
                validator: validator.clone(),
                round_id: claim.round_id,
            },
        )?;
        let validator_power_ratio = val_info_response.ratio;
        let voting_power =
            validator_power_ratio.checked_mul(Decimal::from_ratio(shares, Uint128::one()));
        let voting_power = match voting_power {
            Err(_) => {
                // if there was an overflow error, log this but return 0
                deps.api.debug(&format!(
                            "An error occured while computing voting power for time weighted shares: {:?} and validator : {:?}",
                            shares,validator
                        ));

                Uint128::zero()
            }
            Ok(current_voting_power) => current_voting_power.to_uint_ceil(),
        };
        let vp = hydromancer_voting_power
            .entry(hydromancer_id)
            .or_insert(0u128);
        *vp += voting_power.u128();
        zephyrus_voting_power += voting_power.u128();
    }
    let user_validator_shares = state::get_proposal_time_weigthed_shares_by_user_validators(
        deps.storage,
        claim.proposal_id,
    )?;
    for user_val_shares in user_validator_shares {
        let (user_id, validator, shares) = user_val_shares;

        let val_info_response: ValidatorPowerRatioResponse = deps.querier.query_wasm_smart(
            constants.hydro_config.hydro_contract_address.to_string(),
            &HydroQueryMsg::ValidatorPowerRatio {
                validator: validator.clone(),
                round_id: claim.round_id,
            },
        )?;

        let validator_power_ratio = val_info_response.ratio;
        let voting_power =
            validator_power_ratio.checked_mul(Decimal::from_ratio(shares, Uint128::one()));
        let voting_power = match voting_power {
            Err(_) => {
                // if there was an overflow error, log this but return 0
                deps.api.debug(&format!(
                            "An error occured while computing voting power for time weighted shares: {:?} and validator : {:?}",
                            shares,validator
                        ));

                Uint128::zero()
            }
            Ok(current_voting_power) => current_voting_power.to_uint_ceil(),
        };
        let vp = hydromancer_voting_power.entry(user_id).or_insert(0u128);
        *vp += voting_power.u128();
        zephyrus_voting_power += voting_power.u128();
    }
    let mut total_coin_distributed = Uint128::zero();
    for (hydromancer_id, value) in hydromancer_voting_power.iter() {
        let amount = Decimal::from_ratio(claim.amount.amount, Uint128::one())
            * Decimal::from_ratio(*value, zephyrus_voting_power);
        let amount = amount.to_uint_floor();
        total_coin_distributed += amount;
        state::add_tribute_to_hydromancer(
            deps.storage,
            claim.tranche_id,
            claim.round_id,
            *hydromancer_id,
            claim.tribute_id,
            Coin::new(amount, claim.amount.denom.clone()),
        )?;
    }
    for (user_id, value) in user_voting_power.iter() {
        let amount = Decimal::from_ratio(claim.amount.amount, Uint128::one())
            * Decimal::from_ratio(*value, zephyrus_voting_power);
        let amount = amount.to_uint_floor();
        total_coin_distributed += amount;
        state::add_tribute_to_user(
            deps.storage,
            claim.tranche_id,
            claim.round_id,
            *user_id,
            claim.tribute_id,
            Coin::new(amount, claim.amount.denom.clone()),
        )?;
    }
    Ok(Response::new())
}

//Handle vote reply, used after both user and hydromancer vote
fn handle_vote_reply(
    deps: DepsMut,
    env: Env,
    payload: VoteReplyPayload,
    skipped_locks: Vec<u64>,
) -> Result<Response, ContractError> {
    let constants = state::get_constants(deps.storage)?;
    for vessels_to_harbor in payload.vessels_harbors.clone() {
        let mut lock_ids = vec![];

        for vessel_id in vessels_to_harbor.vessel_ids.iter() {
            //if vessel is skipped, it means that hydro was not able to vote for it, zephyrus skips it too
            if skipped_locks.contains(vessel_id) {
                continue;
            }
            let vessel = state::get_vessel(deps.storage, *vessel_id)?;
            let current_time_weighted_shares: CurrentRoundTimeWeightedSharesResponse =
                deps.querier.query_wasm_smart(
                    constants.hydro_config.hydro_contract_address.to_string(),
                    &HydroQueryMsg::CurrentRoundTimeWeightedShares {
                        owner: env.contract.address.to_string(),
                        lock_ids: vec![vessel.hydro_lock_id],
                    },
                )?;
            if current_time_weighted_shares
                .lock_time_weighted_shares
                .is_empty()
            {
                return Err(ContractError::NoTimeWeightedShares {
                    lock_id: vessel.hydro_lock_id,
                });
            }
            let lock_current_weighted_shares =
                &current_time_weighted_shares.lock_time_weighted_shares[0];
            if payload.user_vote {
                //control that vessel is owned by user who wants to vote
                if vessel.owner_id != payload.steerer_id {
                    return Err(ContractError::InvalidUserId {
                        vessel_id: vessel.hydro_lock_id,
                        user_id: payload.steerer_id,
                        vessel_user_id: vessel.owner_id,
                    });
                }
            } else {
                //control that vessel is delegated to hydromancer who wants to vote
                if vessel.hydromancer_id != payload.steerer_id {
                    return Err(ContractError::InvalidHydromancerId {
                        vessel_id: vessel.hydro_lock_id,
                        hydromancer_id: payload.steerer_id,
                        vessel_hydromancer_id: vessel.hydromancer_id,
                    });
                }
                //hydromancer can't vote with a vessel under user control
                if state::is_vessel_under_user_control(
                    deps.storage,
                    payload.tranche_id,
                    payload.round_id,
                    vessel.hydro_lock_id,
                ) {
                    return Err(ContractError::VesselUnderUserControl {
                        vessel_id: vessel.hydro_lock_id,
                    });
                }
            }

            let previous_harbor_id = state::get_harbor_of_vessel(
                deps.storage,
                payload.tranche_id,
                payload.round_id,
                vessel.hydro_lock_id,
            )?;
            let is_hydromancer_shares_initialized = state::has_shares_for_hydromancer_and_round(
                deps.storage,
                vessel.hydromancer_id,
                payload.tranche_id,
                payload.round_id,
            )?;
            match previous_harbor_id {
                Some(previous_harbor_id) => {
                    let previous_vessel_to_harbor = state::get_vessel_to_harbor(
                        deps.storage,
                        payload.tranche_id,
                        payload.round_id,
                        previous_harbor_id,
                        vessel.hydro_lock_id,
                    )?;

                    if previous_harbor_id != vessels_to_harbor.harbor_id {
                        //vote has changed
                        state::remove_vessel_harbor(
                            deps.storage,
                            payload.tranche_id,
                            payload.round_id,
                            previous_harbor_id,
                            vessel.hydro_lock_id,
                        )?;

                        if payload.user_vote {
                            //user vote
                            if !previous_vessel_to_harbor.user_control {
                                if is_hydromancer_shares_initialized {
                                    //vessel was controled by hydromancer and now it is controlled by user
                                    //hydromancer shares are intialized we can substract them
                                    state::sub_weighted_shares_to_hydromancer(
                                        deps.storage,
                                        previous_vessel_to_harbor.steerer_id,
                                        payload.tranche_id,
                                        payload.round_id,
                                        &lock_current_weighted_shares.validator,
                                        lock_current_weighted_shares.time_weighted_share,
                                    )?;
                                    state::sub_weighted_shares_to_proposal_hydromancer(
                                        deps.storage,
                                        previous_vessel_to_harbor.steerer_id,
                                        previous_harbor_id,
                                        &lock_current_weighted_shares.validator,
                                        lock_current_weighted_shares.time_weighted_share,
                                    )?;
                                    state::sub_weighted_shares_to_user_hydromancer(
                                        deps.storage,
                                        payload.steerer_id,
                                        payload.tranche_id,
                                        vessel.hydromancer_id,
                                        payload.round_id,
                                        &lock_current_weighted_shares.validator,
                                        lock_current_weighted_shares.time_weighted_share,
                                    )?;
                                }
                            } else {
                                //vesssel was already under user control
                                state::sub_weighted_shares_under_user_control_for_proposal(
                                    deps.storage,
                                    payload.steerer_id,
                                    previous_harbor_id,
                                    &lock_current_weighted_shares.validator,
                                    lock_current_weighted_shares.time_weighted_share,
                                )?;
                            }

                            state::add_weighted_shares_under_user_control_for_proposal(
                                deps.storage,
                                payload.steerer_id,
                                vessels_to_harbor.harbor_id,
                                &lock_current_weighted_shares.validator,
                                lock_current_weighted_shares.time_weighted_share,
                            )?;
                        } else {
                            //Hydromancer vote
                            //remove weighted shares from old proposal id
                            state::sub_weighted_shares_to_proposal_hydromancer(
                                deps.storage,
                                payload.steerer_id,
                                previous_harbor_id,
                                &lock_current_weighted_shares.validator,
                                lock_current_weighted_shares.time_weighted_share,
                            )?;
                            //add weighted shares to new proposal id
                            state::add_weighted_shares_to_proposal_hydromancer(
                                deps.storage,
                                payload.steerer_id,
                                vessels_to_harbor.harbor_id,
                                &lock_current_weighted_shares.validator,
                                lock_current_weighted_shares.time_weighted_share,
                            )?;
                        }
                        //save could be done after the match statement, but it will be done also when previous harbor id is the same as the new one
                        state::add_vessel_to_harbor(
                            deps.storage,
                            payload.tranche_id,
                            payload.round_id,
                            vessels_to_harbor.harbor_id,
                            &VesselHarbor {
                                user_control: payload.user_vote,
                                hydro_lock_id: vessel.hydro_lock_id,
                                steerer_id: payload.steerer_id,
                            },
                        )?;
                    }
                }
                None => {
                    if payload.user_vote {
                        state::add_weighted_shares_under_user_control_for_proposal(
                            deps.storage,
                            payload.steerer_id,
                            vessels_to_harbor.harbor_id,
                            &lock_current_weighted_shares.validator,
                            lock_current_weighted_shares.time_weighted_share,
                        )?;
                    } else {
                        if is_hydromancer_shares_initialized {
                            //vessel was controled by hydromancer and now it is controlled by user
                            //hydromancer shares are intialized we can substract them
                            //add weighted shares to new proposal id
                            state::add_weighted_shares_to_proposal_hydromancer(
                                deps.storage,
                                payload.steerer_id,
                                vessels_to_harbor.harbor_id,
                                &lock_current_weighted_shares.validator,
                                lock_current_weighted_shares.time_weighted_share,
                            )?;
                        }
                    }
                    state::add_vessel_to_harbor(
                        deps.storage,
                        payload.tranche_id,
                        payload.round_id,
                        vessels_to_harbor.harbor_id,
                        &VesselHarbor {
                            user_control: payload.user_vote,
                            hydro_lock_id: vessel.hydro_lock_id,
                            steerer_id: payload.steerer_id,
                        },
                    )?;
                }
            }

            lock_ids.push(vessel.hydro_lock_id);
        }
    }
    Ok(Response::new().add_attribute(
        "skipped_locks",
        skipped_locks
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(","),
    ))
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
        owner_id,
    }: LockTokensReplyPayload,
) -> Result<Response, ContractError> {
    let vessel = Vessel {
        hydro_lock_id,
        class_period: lock_duration,
        tokenized_share_record_id,
        hydromancer_id,
        auto_maintenance,
        owner_id,
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

fn parse_locks_skipped_reply(reply: Reply) -> Result<Vec<u64>, ContractError> {
    let response = reply
        .result
        .into_result()
        .expect("always issued on_success");

    let skipped_locks = response
        .events
        .into_iter()
        .flat_map(|e| e.attributes)
        .find_map(|attr| (attr.key == "locks_skipped").then_some(attr.value))
        .expect("Vote reply always contains locks_skipped attribute");

    Ok(if skipped_locks.is_empty() {
        vec![]
    } else {
        skipped_locks
            .split(',')
            .map(|s| s.parse().unwrap()) // Attention: `unwrap` peut toujours paniquer ici !
            .collect()
    })
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
    use std::{collections::HashMap, time::SystemTime};

    use cosmwasm_std::{
        coin, coins, from_json,
        testing::{
            mock_dependencies as std_mock_dependencies, mock_env, MockApi,
            MockQuerier as StdMockQuerier, MockStorage,
        },
        to_json_binary, Addr, Binary, ContractResult, CosmosMsg, Decimal, DepsMut, Empty,
        GrpcQuery, MessageInfo, OwnedDeps, Querier, QuerierResult, QueryRequest, ReplyOn, StdError,
        WasmMsg, WasmQuery,
    };
    use hydro_interface::msgs::{
        CurrentRoundResponse, CurrentRoundTimeWeightedSharesResponse, HydroExecuteMsg,
        HydroQueryMsg, LockTimeWeightedShare, ValidatorPowerRatioResponse,
    };
    use neutron_std::types::ibc::applications::transfer::v1::{
        DenomTrace, QueryDenomTraceRequest, QueryDenomTraceResponse,
    };
    use prost::Message;
    use zephyrus_core::state::Vessel;
    use zephyrus_core::{
        msgs::{BuildVesselParams, InstantiateMsg, VesselsToHarbor},
        state::VesselHarbor,
    };

    use crate::{
        contract::{LockTokensReplyPayload, VoteReplyPayload},
        errors::ContractError,
        state::{self},
    };

    struct MockQuerier(StdMockQuerier);

    fn mock_wasm_query_handler(contract_addr: &str, msg: &Binary) -> QuerierResult {
        let hydro_addr: String = make_valid_addr("hydro").into_string();
        if contract_addr == hydro_addr {
            let query: HydroQueryMsg = match from_json(msg) {
                Ok(q) => q,
                Err(_) => return QuerierResult::Err(cosmwasm_std::SystemError::Unknown {}),
            };
            match query {
                HydroQueryMsg::ValidatorPowerRatio {
                    round_id: _,
                    validator: _,
                } => {
                    let response = to_json_binary(&ValidatorPowerRatioResponse {
                        ratio: Decimal::one(),
                    })
                    .unwrap();
                    QuerierResult::Ok(ContractResult::Ok(response))
                }
                HydroQueryMsg::CurrentRoundTimeWeightedShares { lock_ids, owner } => {
                    let mut map = HashMap::new();

                    map.insert(0, 1_000_000);
                    map.insert(1, 2_000_000);
                    map.insert(2, 3_000_000);
                    map.insert(3, 6_000_000);
                    map.insert(4, 12_000_000);

                    let shares = lock_ids
                        .iter()
                        .map(|lock_id| {
                            let val = "crosnest";
                            LockTimeWeightedShare {
                                lock_id: *lock_id,
                                time_weighted_share: if let Some(v) = map.get(lock_id) {
                                    *v
                                } else {
                                    0
                                },
                                validator: val.to_string(),
                            }
                        })
                        .collect();
                    let response = to_json_binary(&CurrentRoundTimeWeightedSharesResponse {
                        round_id: 1,
                        lock_time_weighted_shares: shares,
                    })
                    .unwrap();
                    QuerierResult::Ok(ContractResult::Ok(response))
                }
                HydroQueryMsg::CurrentRound {} => {
                    let response = to_json_binary(&CurrentRoundResponse {
                        round_id: 1,
                        round_end: cosmwasm_std::Timestamp::from_seconds(
                            SystemTime::now()
                                .duration_since(SystemTime::UNIX_EPOCH)
                                .unwrap()
                                .as_secs(),
                        ),
                    })
                    .unwrap();
                    QuerierResult::Ok(ContractResult::Ok(response))
                }
            }
        } else {
            QuerierResult::Err(cosmwasm_std::SystemError::Unknown {})
        }
    }

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
            println!("MockQuerier!");
            let request: QueryRequest<Empty> = from_json(bin_request).ok().unwrap();

            match request {
                QueryRequest::<Empty>::Grpc(GrpcQuery { path, data }) => {
                    mock_grpc_query_handler(&path, &data)
                }
                QueryRequest::Wasm(WasmQuery::Smart { contract_addr, msg }) => {
                    println!("WasmQuery::Smart {}", contract_addr);

                    mock_wasm_query_handler(&contract_addr, &msg)
                }
                _ => self.0.raw_query(bin_request),
            }
            // let Some(QueryRequest::<Empty>::Grpc(GrpcQuery { path, data })) =
            //     from_json(bin_request).ok()
            // else {
            //     return self.0.raw_query(bin_request);
            // };

            // mock_grpc_query_handler(&path, &data)
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
    fn hydromancer_vote_fails_not_hydromancer() {
        let mut deps = mock_dependencies();

        init_contract(deps.as_mut());
        let alice_address = make_valid_addr("alice");
        assert_eq!(
            super::execute_hydromancer_vote(
                &mut deps.as_mut(),
                mock_env(),
                MessageInfo {
                    sender: alice_address.clone(),
                    funds: vec![]
                },
                1,
                vec![
                    {
                        VesselsToHarbor {
                            harbor_id: 1,
                            vessel_ids: vec![1, 2],
                        }
                    },
                    {
                        VesselsToHarbor {
                            harbor_id: 2,
                            vessel_ids: vec![3, 4],
                        }
                    }
                ]
            )
            .unwrap_err(),
            ContractError::from(StdError::generic_err(format!(
                "Hydromancer {} not found",
                alice_address.to_string()
            )))
        );
    }

    #[test]
    fn hydromancer_vote_with_other_vessels_fail() {
        let mut deps = mock_dependencies();

        init_contract(deps.as_mut());
        let constant = state::get_constants(deps.as_ref().storage).unwrap();

        let alice_address = make_valid_addr("alice");
        let user_id = state::insert_new_user(deps.as_mut().storage, alice_address.clone())
            .expect("Should add user");
        let alice_hydromancer_id = state::insert_new_hydromancer(
            deps.as_mut().storage,
            alice_address.clone(),
            "alice".to_string(),
            Decimal::percent(10),
        )
        .expect("Should add hydromancer");
        state::add_vessel(
            deps.as_mut().storage,
            &Vessel {
                hydro_lock_id: 0,
                tokenized_share_record_id: 0,
                class_period: 12,
                auto_maintenance: true,
                hydromancer_id: alice_hydromancer_id,
                owner_id: user_id,
            },
            &alice_address,
        )
        .expect("Should add vessel");
        println!("Execute vote hydromancer");
        let payload = VoteReplyPayload {
            tranche_id: 1,
            round_id: 1,
            user_vote: false,
            steerer_id: constant.default_hydromancer_id,
            vessels_harbors: vec![{
                VesselsToHarbor {
                    harbor_id: 1,
                    vessel_ids: vec![0],
                }
            }],
        };
        assert_eq!(
            super::handle_vote_reply(deps.as_mut(), mock_env(), payload, vec![]).unwrap_err(),
            ContractError::InvalidHydromancerId {
                vessel_id: 0,
                hydromancer_id: 0,
                vessel_hydromancer_id: 1
            }
        );
    }

    #[test]
    fn hydromancer_vote_with_vessels_under_user_control_fail() {
        let mut deps = mock_dependencies();

        init_contract(deps.as_mut());

        let alice_address = make_valid_addr("alice");
        let user_id = state::insert_new_user(deps.as_mut().storage, alice_address.clone())
            .expect("Should add user");
        let default_hydromancer_id = state::get_constants(deps.as_mut().storage)
            .unwrap()
            .default_hydromancer_id;
        state::add_vessel(
            deps.as_mut().storage,
            &Vessel {
                hydro_lock_id: 0,
                tokenized_share_record_id: 0,
                class_period: 12,
                auto_maintenance: true,
                hydromancer_id: default_hydromancer_id,
                owner_id: user_id,
            },
            &alice_address,
        )
        .expect("Should add vessel");

        state::add_vessel_to_harbor(
            deps.as_mut().storage,
            1,
            1,
            1,
            &VesselHarbor {
                user_control: true,
                hydro_lock_id: 0,
                steerer_id: 0,
            },
        )
        .expect("Should add vessel to harbor");
        let payload = VoteReplyPayload {
            tranche_id: 1,
            round_id: 1,
            user_vote: false,
            steerer_id: default_hydromancer_id,
            vessels_harbors: vec![{
                VesselsToHarbor {
                    harbor_id: 1,
                    vessel_ids: vec![0],
                }
            }],
        };
        assert_eq!(
            super::handle_vote_reply(deps.as_mut(), mock_env(), payload, vec![]).unwrap_err(),
            ContractError::VesselUnderUserControl { vessel_id: 0 }
        );
    }

    #[test]
    fn hydromancer_vote_succeed_without_change_because_vote_skipped_by_hydro() {
        let mut deps = mock_dependencies();

        init_contract(deps.as_mut());
        let alice_address = make_valid_addr("alice");
        let user_id = state::insert_new_user(deps.as_mut().storage, alice_address.clone())
            .expect("Should add user");
        let default_hydromancer_id = state::get_constants(deps.as_mut().storage)
            .unwrap()
            .default_hydromancer_id;
        state::add_vessel(
            deps.as_mut().storage,
            &Vessel {
                hydro_lock_id: 0,
                tokenized_share_record_id: 0,
                class_period: 12,
                auto_maintenance: true,
                hydromancer_id: default_hydromancer_id,
                owner_id: user_id,
            },
            &alice_address,
        )
        .expect("Should add vessel");

        state::add_vessel_to_harbor(
            deps.as_mut().storage,
            1,
            1,
            2,
            &VesselHarbor {
                user_control: false,
                hydro_lock_id: 0,
                steerer_id: default_hydromancer_id,
            },
        )
        .expect("Should add vessel to harbor");

        let res = super::execute_hydromancer_vote(
            &mut deps.as_mut(),
            mock_env(),
            MessageInfo {
                sender: make_valid_addr("zephyrus"),
                funds: vec![],
            },
            1,
            vec![{
                VesselsToHarbor {
                    harbor_id: 1,
                    vessel_ids: vec![0],
                }
            }],
        )
        .unwrap();

        assert_eq!(res.messages.len(), 1);

        let decoded_submessages: Vec<HydroExecuteMsg> = res
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

                assert_eq!(funds.len(), 0, "vote on hydro does not required funds");

                from_json(msg.clone()).unwrap()
            })
            .collect();

        if let [HydroExecuteMsg::Vote {
            tranche_id,
            proposals_votes,
        }] = decoded_submessages.as_slice()
        {
            assert_eq!(*tranche_id, 1);
            assert_eq!(proposals_votes.len(), 1);
            assert_eq!(proposals_votes[0].proposal_id, 1);
            assert_eq!(proposals_votes[0].lock_ids, vec![0]);
        } else {
            panic!("Le message ne correspond pas au pattern attendu !");
        }

        let payload = VoteReplyPayload {
            tranche_id: 1,
            round_id: 1,
            user_vote: false,
            steerer_id: default_hydromancer_id,
            vessels_harbors: vec![{
                VesselsToHarbor {
                    harbor_id: 1,
                    vessel_ids: vec![0],
                }
            }],
        };
        let skipped_ids = vec![0];
        let _ = super::handle_vote_reply(deps.as_mut(), mock_env(), payload, skipped_ids).unwrap();

        let vessels_to_harbor2 =
            state::get_vessel_to_harbor_by_harbor_id(deps.as_mut().storage, 1, 1, 2)
                .expect("Vessel to harbor should exist");
        assert_eq!(vessels_to_harbor2.len(), 1);
        assert_eq!(vessels_to_harbor2[0].1.hydro_lock_id, 0);
        assert_eq!(vessels_to_harbor2[0].1.steerer_id, default_hydromancer_id);
        //vote shoulb be skipped so harbor1 should not have vessels
        let vessels_to_harbor1 =
            state::get_vessel_to_harbor_by_harbor_id(deps.as_mut().storage, 1, 1, 1)
                .expect("Vessel to harbor should exist");
        assert_eq!(vessels_to_harbor1.len(), 0);
    }

    #[test]
    fn hydromancer_new_vote_succeed() {
        let mut deps = mock_dependencies();

        init_contract(deps.as_mut());

        let alice_address = make_valid_addr("alice");
        let user_id = state::insert_new_user(deps.as_mut().storage, alice_address.clone())
            .expect("Should add user");
        let default_hydromancer_id = state::get_constants(deps.as_mut().storage)
            .unwrap()
            .default_hydromancer_id;
        state::add_vessel(
            deps.as_mut().storage,
            &Vessel {
                hydro_lock_id: 0,
                tokenized_share_record_id: 0,
                class_period: 12,
                auto_maintenance: true,
                hydromancer_id: default_hydromancer_id,
                owner_id: user_id,
            },
            &alice_address,
        )
        .expect("Should add vessel");

        let res = super::execute_hydromancer_vote(
            &mut deps.as_mut(),
            mock_env(),
            MessageInfo {
                sender: make_valid_addr("zephyrus"),
                funds: vec![],
            },
            1,
            vec![{
                VesselsToHarbor {
                    harbor_id: 1,
                    vessel_ids: vec![0],
                }
            }],
        )
        .unwrap();

        assert_eq!(res.messages.len(), 1);

        let decoded_submessages: Vec<HydroExecuteMsg> = res
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

                assert_eq!(funds.len(), 0, "vote on hydro does not required funds");

                from_json(msg.clone()).unwrap()
            })
            .collect();

        if let [HydroExecuteMsg::Vote {
            tranche_id,
            proposals_votes,
        }] = decoded_submessages.as_slice()
        {
            assert_eq!(*tranche_id, 1);
            assert_eq!(proposals_votes.len(), 1);
            assert_eq!(proposals_votes[0].proposal_id, 1);
            assert_eq!(proposals_votes[0].lock_ids, vec![0]);
        } else {
            panic!("Le message ne correspond pas au pattern attendu !");
        }

        let payload = VoteReplyPayload {
            tranche_id: 1,
            round_id: 1,
            user_vote: false,
            steerer_id: default_hydromancer_id,
            vessels_harbors: vec![{
                VesselsToHarbor {
                    harbor_id: 1,
                    vessel_ids: vec![0],
                }
            }],
        };

        let _ = super::handle_vote_reply(deps.as_mut(), mock_env(), payload, vec![]).unwrap();

        let vessels_to_harbor =
            state::get_vessel_to_harbor_by_harbor_id(deps.as_mut().storage, 1, 1, 1)
                .expect("Vessel to harbor should exist");
        assert_eq!(vessels_to_harbor.len(), 1);
        assert_eq!(vessels_to_harbor[0].1.hydro_lock_id, 0);
        assert_eq!(vessels_to_harbor[0].1.steerer_id, default_hydromancer_id);
    }

    #[test]
    fn hydromancer_change_existing_vote_succeed() {
        let mut deps = mock_dependencies();

        init_contract(deps.as_mut());

        let alice_address = make_valid_addr("alice");
        let user_id = state::insert_new_user(deps.as_mut().storage, alice_address.clone())
            .expect("Should add user");
        let default_hydromancer_id = state::get_constants(deps.as_mut().storage)
            .unwrap()
            .default_hydromancer_id;
        state::add_vessel(
            deps.as_mut().storage,
            &Vessel {
                hydro_lock_id: 0,
                tokenized_share_record_id: 0,
                class_period: 12,
                auto_maintenance: true,
                hydromancer_id: default_hydromancer_id,
                owner_id: user_id,
            },
            &alice_address,
        )
        .expect("Should add vessel");

        state::add_vessel_to_harbor(
            deps.as_mut().storage,
            1,
            1,
            2,
            &VesselHarbor {
                user_control: false,
                hydro_lock_id: 0,
                steerer_id: default_hydromancer_id,
            },
        )
        .expect("Should add vessel to harbor");

        let res = super::execute_hydromancer_vote(
            &mut deps.as_mut(),
            mock_env(),
            MessageInfo {
                sender: make_valid_addr("zephyrus"),
                funds: vec![],
            },
            1,
            vec![{
                VesselsToHarbor {
                    harbor_id: 1,
                    vessel_ids: vec![0],
                }
            }],
        )
        .unwrap();

        assert_eq!(res.messages.len(), 1);

        let decoded_submessages: Vec<HydroExecuteMsg> = res
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

                assert_eq!(funds.len(), 0, "vote on hydro does not required funds");

                from_json(msg.clone()).unwrap()
            })
            .collect();

        if let [HydroExecuteMsg::Vote {
            tranche_id,
            proposals_votes,
        }] = decoded_submessages.as_slice()
        {
            assert_eq!(*tranche_id, 1);
            assert_eq!(proposals_votes.len(), 1);
            assert_eq!(proposals_votes[0].proposal_id, 1);
            assert_eq!(proposals_votes[0].lock_ids, vec![0]);
        } else {
            panic!("Le message ne correspond pas au pattern attendu !");
        }

        let payload = VoteReplyPayload {
            tranche_id: 1,
            round_id: 1,
            user_vote: false,
            steerer_id: default_hydromancer_id,
            vessels_harbors: vec![{
                VesselsToHarbor {
                    harbor_id: 1,
                    vessel_ids: vec![0],
                }
            }],
        };

        let _ = super::handle_vote_reply(deps.as_mut(), mock_env(), payload, vec![]).unwrap();

        let vessels_to_harbor1 =
            state::get_vessel_to_harbor_by_harbor_id(deps.as_mut().storage, 1, 1, 1)
                .expect("Vessel to harbor should exist");
        assert_eq!(vessels_to_harbor1.len(), 1);
        assert_eq!(vessels_to_harbor1[0].1.hydro_lock_id, 0);
        assert_eq!(vessels_to_harbor1[0].1.steerer_id, default_hydromancer_id);

        let vessels_to_harbor2 =
            state::get_vessel_to_harbor_by_harbor_id(deps.as_mut().storage, 1, 1, 2)
                .expect("Vessel to harbor should exist");
        assert_eq!(vessels_to_harbor2.len(), 0);
    }

    #[test]
    fn hydromancer_vote_fails_if_duplicate_vessel_id() {
        let mut deps = mock_dependencies();

        init_contract(deps.as_mut());

        assert_eq!(
            super::execute_hydromancer_vote(
                &mut deps.as_mut(),
                mock_env(),
                MessageInfo {
                    sender: make_valid_addr("zephyrus"),
                    funds: vec![]
                },
                1,
                vec![
                    {
                        VesselsToHarbor {
                            harbor_id: 1,
                            vessel_ids: vec![1, 2],
                        }
                    },
                    {
                        VesselsToHarbor {
                            harbor_id: 2,
                            vessel_ids: vec![2, 4],
                        }
                    }
                ]
            )
            .unwrap_err(),
            ContractError::VoteDuplicatedVesselId { vessel_id: 2 }
        );
    }

    #[test]
    fn hydromancer_vote_fails_if_duplicate_harbor() {
        let mut deps = mock_dependencies();

        init_contract(deps.as_mut());

        assert_eq!(
            super::execute_hydromancer_vote(
                &mut deps.as_mut(),
                mock_env(),
                MessageInfo {
                    sender: make_valid_addr("zephyrus"),
                    funds: vec![]
                },
                1,
                vec![
                    {
                        VesselsToHarbor {
                            harbor_id: 1,
                            vessel_ids: vec![1, 2],
                        }
                    },
                    {
                        VesselsToHarbor {
                            harbor_id: 1,
                            vessel_ids: vec![3, 4],
                        }
                    }
                ]
            )
            .unwrap_err(),
            ContractError::VoteDuplicatedHarborId { harbor_id: 1 }
        );
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
        let owner_id = state::insert_new_user(deps.as_mut().storage, expected_owner.clone())
            .expect("Should add user");
        let expected_vessel = Vessel {
            hydro_lock_id: 0,
            tokenized_share_record_id: 10,
            class_period: 3,
            auto_maintenance: false,
            hydromancer_id: 0,
            owner_id,
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
                owner_id,
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
        let alice_address = make_valid_addr("alice");
        let user_id = state::insert_new_user(deps.as_mut().storage, alice_address.clone())
            .expect("Should add user");
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
                owner: alice_address.clone(),
                owner_id: user_id,
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

    //TESTS USER VOTE
    #[test]
    fn user_vote_fails_not_zephyrus_user() {
        let mut deps = mock_dependencies();

        init_contract(deps.as_mut());
        let alice_address = make_valid_addr("alice");
        assert_eq!(
            super::execute_user_vote(
                deps.as_mut(),
                mock_env(),
                MessageInfo {
                    sender: alice_address.clone(),
                    funds: vec![]
                },
                1,
                vec![
                    {
                        VesselsToHarbor {
                            harbor_id: 1,
                            vessel_ids: vec![1, 2],
                        }
                    },
                    {
                        VesselsToHarbor {
                            harbor_id: 2,
                            vessel_ids: vec![3, 4],
                        }
                    }
                ]
            )
            .unwrap_err(),
            ContractError::from(StdError::generic_err(format!(
                "User {} not found",
                alice_address.to_string()
            )))
        );
    }

    #[test]
    fn user_vote_with_other_vessels_fail() {
        let mut deps = mock_dependencies();

        init_contract(deps.as_mut());

        let alice_address = make_valid_addr("alice");
        let bob_address = make_valid_addr("bob");
        let alice_user_id = state::insert_new_user(deps.as_mut().storage, alice_address.clone())
            .expect("Should add user");
        let bob_user_id = state::insert_new_user(deps.as_mut().storage, bob_address.clone())
            .expect("Should add user");
        let default_hydromancer_id = state::get_constants(deps.as_mut().storage)
            .unwrap()
            .default_hydromancer_id;
        state::add_vessel(
            deps.as_mut().storage,
            &Vessel {
                hydro_lock_id: 0,
                tokenized_share_record_id: 0,
                class_period: 12,
                auto_maintenance: true,
                hydromancer_id: default_hydromancer_id,
                owner_id: alice_user_id,
            },
            &alice_address,
        )
        .expect("Should add vessel");
        println!("Execute vote hydromancer");
        let payload = VoteReplyPayload {
            tranche_id: 1,
            round_id: 1,
            user_vote: true,
            steerer_id: bob_user_id,
            vessels_harbors: vec![{
                VesselsToHarbor {
                    harbor_id: 1,
                    vessel_ids: vec![0],
                }
            }],
        };
        assert_eq!(
            super::handle_vote_reply(deps.as_mut(), mock_env(), payload, vec![]).unwrap_err(),
            ContractError::InvalidUserId {
                vessel_id: 0,
                user_id: bob_user_id,
                vessel_user_id: alice_user_id
            }
        );
    }

    #[test]
    fn user_new_vote_succeed() {
        let mut deps = mock_dependencies();

        init_contract(deps.as_mut());

        let alice_address = make_valid_addr("alice");
        let user_id = state::insert_new_user(deps.as_mut().storage, alice_address.clone())
            .expect("Should add user");
        let default_hydromancer_id = state::get_constants(deps.as_mut().storage)
            .unwrap()
            .default_hydromancer_id;
        state::add_vessel(
            deps.as_mut().storage,
            &Vessel {
                hydro_lock_id: 0,
                tokenized_share_record_id: 0,
                class_period: 12,
                auto_maintenance: true,
                hydromancer_id: default_hydromancer_id,
                owner_id: user_id,
            },
            &alice_address,
        )
        .expect("Should add vessel");

        let res = super::execute_user_vote(
            deps.as_mut(),
            mock_env(),
            MessageInfo {
                sender: alice_address.clone(),
                funds: vec![],
            },
            1,
            vec![{
                VesselsToHarbor {
                    harbor_id: 1,
                    vessel_ids: vec![0],
                }
            }],
        )
        .unwrap();

        assert_eq!(res.messages.len(), 1);

        let decoded_submessages: Vec<HydroExecuteMsg> = res
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

                assert_eq!(funds.len(), 0, "vote on hydro does not required funds");

                from_json(msg.clone()).unwrap()
            })
            .collect();

        if let [HydroExecuteMsg::Vote {
            tranche_id,
            proposals_votes,
        }] = decoded_submessages.as_slice()
        {
            assert_eq!(*tranche_id, 1);
            assert_eq!(proposals_votes.len(), 1);
            assert_eq!(proposals_votes[0].proposal_id, 1);
            assert_eq!(proposals_votes[0].lock_ids, vec![0]);
        } else {
            panic!("Le message ne correspond pas au pattern attendu !");
        }

        let payload = VoteReplyPayload {
            tranche_id: 1,
            round_id: 1,
            user_vote: true,
            steerer_id: user_id,
            vessels_harbors: vec![{
                VesselsToHarbor {
                    harbor_id: 1,
                    vessel_ids: vec![0],
                }
            }],
        };
        let _ = super::handle_vote_reply(deps.as_mut(), mock_env(), payload, vec![]).unwrap();

        let vessels_to_harbor =
            state::get_vessel_to_harbor_by_harbor_id(deps.as_mut().storage, 1, 1, 1)
                .expect("Vessel to harbor should exist");
        assert_eq!(vessels_to_harbor.len(), 1);
        assert!(vessels_to_harbor[0].1.user_control);
        assert_eq!(vessels_to_harbor[0].1.hydro_lock_id, 0);
        assert_eq!(vessels_to_harbor[0].1.steerer_id, user_id);
    }

    #[test]
    fn user_change_existing_hydromancer_vote_succeed() {
        let mut deps = mock_dependencies();

        init_contract(deps.as_mut());

        let alice_address = make_valid_addr("alice");
        let user_id = state::insert_new_user(deps.as_mut().storage, alice_address.clone())
            .expect("Should add user");
        let default_hydromancer_id = state::get_constants(deps.as_mut().storage)
            .unwrap()
            .default_hydromancer_id;
        state::add_vessel(
            deps.as_mut().storage,
            &Vessel {
                hydro_lock_id: 0,
                tokenized_share_record_id: 0,
                class_period: 12,
                auto_maintenance: true,
                hydromancer_id: default_hydromancer_id,
                owner_id: user_id,
            },
            &alice_address,
        )
        .expect("Should add vessel");

        state::add_vessel_to_harbor(
            deps.as_mut().storage,
            1,
            1,
            2,
            &VesselHarbor {
                user_control: false,
                hydro_lock_id: 0,
                steerer_id: default_hydromancer_id,
            },
        )
        .expect("Should add vessel to harbor");

        let res = super::execute_user_vote(
            deps.as_mut(),
            mock_env(),
            MessageInfo {
                sender: alice_address.clone(),
                funds: vec![],
            },
            1,
            vec![{
                VesselsToHarbor {
                    harbor_id: 1,
                    vessel_ids: vec![0],
                }
            }],
        )
        .unwrap();
        //because vessel was used by hydromancer, there should be 2 messages (unvote, vote)
        assert_eq!(res.messages.len(), 2);

        let decoded_submessages: Vec<HydroExecuteMsg> = res
            .messages
            .iter()
            .filter(|submsg| submsg.reply_on == ReplyOn::Success)
            .map(|submsg| {
                assert_eq!(
                    submsg.reply_on,
                    ReplyOn::Success,
                    "all lock messages should be reply_on_success"
                );

                let CosmosMsg::Wasm(WasmMsg::Execute { msg, funds, .. }) = &submsg.msg else {
                    panic!("unexpected msg: {submsg:?}");
                };

                assert_eq!(funds.len(), 0, "vote on hydro does not required funds");

                from_json(msg.clone()).unwrap()
            })
            .collect();

        if let [HydroExecuteMsg::Vote {
            tranche_id,
            proposals_votes,
        }] = decoded_submessages.as_slice()
        {
            assert_eq!(*tranche_id, 1);
            assert_eq!(proposals_votes.len(), 1);
            assert_eq!(proposals_votes[0].proposal_id, 1);
            assert_eq!(proposals_votes[0].lock_ids, vec![0]);
        } else {
            panic!("Le message ne correspond pas au pattern attendu !");
        }
        let payload = VoteReplyPayload {
            tranche_id: 1,
            round_id: 1,
            user_vote: true,
            steerer_id: user_id,
            vessels_harbors: vec![{
                VesselsToHarbor {
                    harbor_id: 1,
                    vessel_ids: vec![0],
                }
            }],
        };
        let _ = super::handle_vote_reply(deps.as_mut(), mock_env(), payload, vec![]).unwrap();

        let vessels_to_harbor1 =
            state::get_vessel_to_harbor_by_harbor_id(deps.as_mut().storage, 1, 1, 1)
                .expect("Vessel to harbor should exist");
        assert_eq!(vessels_to_harbor1.len(), 1);
        assert!(vessels_to_harbor1[0].1.user_control);
        assert_eq!(vessels_to_harbor1[0].1.hydro_lock_id, 0);
        assert_eq!(vessels_to_harbor1[0].1.steerer_id, user_id);

        let vessels_to_harbor2 =
            state::get_vessel_to_harbor_by_harbor_id(deps.as_mut().storage, 1, 1, 2)
                .expect("Should return empty list");
        assert_eq!(vessels_to_harbor2.len(), 0);
    }

    #[test]
    fn user_vote_fails_if_duplicate_vessel_id() {
        let mut deps = mock_dependencies();

        init_contract(deps.as_mut());

        assert_eq!(
            super::execute_user_vote(
                deps.as_mut(),
                mock_env(),
                MessageInfo {
                    sender: make_valid_addr("zephyrus"),
                    funds: vec![]
                },
                1,
                vec![
                    {
                        VesselsToHarbor {
                            harbor_id: 1,
                            vessel_ids: vec![1, 2],
                        }
                    },
                    {
                        VesselsToHarbor {
                            harbor_id: 2,
                            vessel_ids: vec![2, 4],
                        }
                    }
                ]
            )
            .unwrap_err(),
            ContractError::VoteDuplicatedVesselId { vessel_id: 2 }
        );
    }

    #[test]
    fn user_vote_fails_if_duplicate_harbor() {
        let mut deps = mock_dependencies();

        init_contract(deps.as_mut());

        assert_eq!(
            super::execute_user_vote(
                deps.as_mut(),
                mock_env(),
                MessageInfo {
                    sender: make_valid_addr("zephyrus"),
                    funds: vec![]
                },
                1,
                vec![
                    {
                        VesselsToHarbor {
                            harbor_id: 1,
                            vessel_ids: vec![1, 2],
                        }
                    },
                    {
                        VesselsToHarbor {
                            harbor_id: 1,
                            vessel_ids: vec![3, 4],
                        }
                    }
                ]
            )
            .unwrap_err(),
            ContractError::VoteDuplicatedHarborId { harbor_id: 1 }
        );
    }

    #[test]
    fn change_hydromancer_for_unexisting_vessel_fail() {
        let mut deps = mock_dependencies();

        init_contract(deps.as_mut());

        assert_eq!(
            super::execute_change_hydromancer(
                deps.as_mut(),
                MessageInfo {
                    sender: make_valid_addr("alice"),
                    funds: vec![]
                },
                1,
                1,
                vec![0]
            )
            .unwrap_err(),
            ContractError::Unauthorized {}
        );
    }

    #[test]
    fn change_hydromancer_fail_bad_user() {
        let mut deps = mock_dependencies();

        init_contract(deps.as_mut());

        let alice_address = make_valid_addr("alice");
        let user_id = state::insert_new_user(deps.as_mut().storage, alice_address.clone())
            .expect("Should add user");
        let default_hydromancer_id = state::get_constants(deps.as_mut().storage)
            .unwrap()
            .default_hydromancer_id;
        state::add_vessel(
            deps.as_mut().storage,
            &Vessel {
                hydro_lock_id: 0,
                tokenized_share_record_id: 0,
                class_period: 12,
                auto_maintenance: true,
                hydromancer_id: default_hydromancer_id,
                owner_id: user_id,
            },
            &alice_address,
        )
        .expect("Should add vessel");

        assert_eq!(
            super::execute_change_hydromancer(
                deps.as_mut(),
                MessageInfo {
                    sender: make_valid_addr("bob"),
                    funds: vec![]
                },
                1,
                1,
                vec![0]
            )
            .unwrap_err(),
            ContractError::Unauthorized {}
        );
    }

    #[test]
    fn change_hydromancer_2_vessels_with_1_fail_bad_user() {
        let mut deps = mock_dependencies();

        init_contract(deps.as_mut());

        let alice_address = make_valid_addr("alice");
        let bob_address = make_valid_addr("bob");
        let user_id = state::insert_new_user(deps.as_mut().storage, alice_address.clone())
            .expect("Should add user");
        let bob_id = state::insert_new_user(deps.as_mut().storage, bob_address.clone())
            .expect("Should add user");
        let default_hydromancer_id = state::get_constants(deps.as_mut().storage)
            .unwrap()
            .default_hydromancer_id;
        state::add_vessel(
            deps.as_mut().storage,
            &Vessel {
                hydro_lock_id: 0,
                tokenized_share_record_id: 0,
                class_period: 12,
                auto_maintenance: true,
                hydromancer_id: default_hydromancer_id,
                owner_id: user_id,
            },
            &alice_address,
        )
        .expect("Should add vessel");

        state::add_vessel(
            deps.as_mut().storage,
            &Vessel {
                hydro_lock_id: 1,
                tokenized_share_record_id: 0,
                class_period: 12,
                auto_maintenance: true,
                hydromancer_id: default_hydromancer_id,
                owner_id: bob_id,
            },
            &bob_address,
        )
        .expect("Should add vessel");

        assert_eq!(
            super::execute_change_hydromancer(
                deps.as_mut(),
                MessageInfo {
                    sender: bob_address.clone(),
                    funds: vec![]
                },
                1,
                1,
                vec![0, 1]
            )
            .unwrap_err(),
            ContractError::Unauthorized {}
        );
    }

    #[test]
    fn change_hydromancer_1_vessels_hydromancer_success() {
        let mut deps = mock_dependencies();

        init_contract(deps.as_mut());

        let alice_address = make_valid_addr("alice");

        let user_id = state::insert_new_user(deps.as_mut().storage, alice_address.clone())
            .expect("Should add user");

        let default_hydromancer_id = state::get_constants(deps.as_mut().storage)
            .unwrap()
            .default_hydromancer_id;
        state::add_vessel(
            deps.as_mut().storage,
            &Vessel {
                hydro_lock_id: 0,
                tokenized_share_record_id: 0,
                class_period: 12,
                auto_maintenance: true,
                hydromancer_id: default_hydromancer_id,
                owner_id: user_id,
            },
            &alice_address,
        )
        .expect("Should add vessel");

        let bob_address = make_valid_addr("bob");
        let new_hydromancer_id = state::insert_new_hydromancer(
            deps.as_mut().storage,
            bob_address.clone(),
            "BOB".to_string(),
            Decimal::zero(),
        )
        .expect("Hydromance should be added !");

        let res = super::execute_change_hydromancer(
            deps.as_mut(),
            MessageInfo {
                sender: alice_address.clone(),
                funds: vec![],
            },
            1,
            new_hydromancer_id,
            vec![0],
        )
        .unwrap();

        //test if messages is correct and type Unvote

        let decoded_submessages: Vec<HydroExecuteMsg> = res
            .messages
            .iter()
            .map(|submsg| {
                let CosmosMsg::Wasm(WasmMsg::Execute { msg, funds, .. }) = &submsg.msg else {
                    panic!("unexpected msg: {submsg:?}");
                };

                assert_eq!(funds.len(), 0, "vote on hydro does not required funds");

                from_json(msg.clone()).unwrap()
            })
            .collect();

        if let [HydroExecuteMsg::Unvote {
            tranche_id,
            lock_ids,
        }] = decoded_submessages.as_slice()
        {
            assert_eq!(*tranche_id, 1);
            assert_eq!(lock_ids.len(), 1);
            assert_eq!(lock_ids[0], 0);
        } else {
            panic!("Message is not message that it should be !");
        }

        let vessel = state::get_vessel(deps.as_ref().storage, 0).expect("Vessel should exist !");
        assert_eq!(vessel.hydromancer_id, new_hydromancer_id);
    }

    #[test]
    fn change_hydromancer_1_vessels_already_vote_success() {
        let mut deps = mock_dependencies();

        init_contract(deps.as_mut());

        let alice_address = make_valid_addr("alice");

        let user_id = state::insert_new_user(deps.as_mut().storage, alice_address.clone())
            .expect("Should add user");

        let default_hydromancer_id = state::get_constants(deps.as_mut().storage)
            .unwrap()
            .default_hydromancer_id;
        state::add_vessel(
            deps.as_mut().storage,
            &Vessel {
                hydro_lock_id: 0,
                tokenized_share_record_id: 0,
                class_period: 12,
                auto_maintenance: true,
                hydromancer_id: default_hydromancer_id,
                owner_id: user_id,
            },
            &alice_address,
        )
        .expect("Should add vessel");

        state::add_vessel_to_harbor(
            deps.as_mut().storage,
            1,
            1,
            1,
            &VesselHarbor {
                user_control: false,
                steerer_id: default_hydromancer_id,
                hydro_lock_id: 0,
            },
        )
        .expect("Should add vessel to harbor");

        let bob_address = make_valid_addr("bob");
        let new_hydromancer_id = state::insert_new_hydromancer(
            deps.as_mut().storage,
            bob_address.clone(),
            "BOB".to_string(),
            Decimal::zero(),
        )
        .expect("Hydromance should be added !");

        let res = super::execute_change_hydromancer(
            deps.as_mut(),
            MessageInfo {
                sender: alice_address.clone(),
                funds: vec![],
            },
            1,
            new_hydromancer_id,
            vec![0],
        )
        .unwrap();

        //test if messages is correct and type Unvote

        let decoded_submessages: Vec<HydroExecuteMsg> = res
            .messages
            .iter()
            .map(|submsg| {
                let CosmosMsg::Wasm(WasmMsg::Execute { msg, funds, .. }) = &submsg.msg else {
                    panic!("unexpected msg: {submsg:?}");
                };

                assert_eq!(funds.len(), 0, "vote on hydro does not required funds");

                from_json(msg.clone()).unwrap()
            })
            .collect();

        if let [HydroExecuteMsg::Unvote {
            tranche_id,
            lock_ids,
        }] = decoded_submessages.as_slice()
        {
            assert_eq!(*tranche_id, 1);
            assert_eq!(lock_ids.len(), 1);
            assert_eq!(lock_ids[0], 0);
        } else {
            panic!("Message is not message that it should be !");
        }

        let vessel = state::get_vessel(deps.as_ref().storage, 0).expect("Vessel should exist !");
        assert_eq!(vessel.hydromancer_id, new_hydromancer_id);

        assert!(
            state::get_vessel_to_harbor_by_harbor_id(deps.as_ref().storage, 1, 1, 1)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn change_hydromancer_vessel_already_vote_under_user_control_success() {
        let mut deps = mock_dependencies();

        init_contract(deps.as_mut());

        let alice_address = make_valid_addr("alice");

        let user_id = state::insert_new_user(deps.as_mut().storage, alice_address.clone())
            .expect("Should add user");

        let default_hydromancer_id = state::get_constants(deps.as_mut().storage)
            .unwrap()
            .default_hydromancer_id;
        state::add_vessel(
            deps.as_mut().storage,
            &Vessel {
                hydro_lock_id: 0,
                tokenized_share_record_id: 0,
                class_period: 12,
                auto_maintenance: true,
                hydromancer_id: default_hydromancer_id,
                owner_id: user_id,
            },
            &alice_address,
        )
        .expect("Should add vessel");

        state::add_vessel_to_harbor(
            deps.as_mut().storage,
            1,
            1,
            1,
            &VesselHarbor {
                user_control: true,
                steerer_id: user_id,
                hydro_lock_id: 0,
            },
        )
        .expect("Should add vessel to harbor");

        let res = super::execute_change_hydromancer(
            deps.as_mut(),
            MessageInfo {
                sender: alice_address.clone(),
                funds: vec![],
            },
            1,
            default_hydromancer_id,
            vec![0],
        )
        .unwrap();

        //test if messages is correct and type Unvote

        let decoded_submessages: Vec<HydroExecuteMsg> = res
            .messages
            .iter()
            .map(|submsg| {
                let CosmosMsg::Wasm(WasmMsg::Execute { msg, funds, .. }) = &submsg.msg else {
                    panic!("unexpected msg: {submsg:?}");
                };

                assert_eq!(funds.len(), 0, "vote on hydro does not required funds");

                from_json(msg.clone()).unwrap()
            })
            .collect();

        if let [HydroExecuteMsg::Unvote {
            tranche_id,
            lock_ids,
        }] = decoded_submessages.as_slice()
        {
            assert_eq!(*tranche_id, 1);
            assert_eq!(lock_ids.len(), 1);
            assert_eq!(lock_ids[0], 0);
        } else {
            panic!("Message is not message that it should be !");
        }

        let vessel = state::get_vessel(deps.as_ref().storage, 0).expect("Vessel should exist !");
        assert_eq!(vessel.hydromancer_id, default_hydromancer_id);

        assert!(
            state::get_vessel_to_harbor_by_harbor_id(deps.as_ref().storage, 1, 1, 1)
                .unwrap()
                .is_empty()
        );
        assert!(!state::is_vessel_under_user_control(
            deps.as_ref().storage,
            1,
            1,
            0
        ))
    }

    #[test]
    fn hydromancer_vote_shares_initialized() {
        let mut deps = mock_dependencies();

        init_contract(deps.as_mut());

        let alice_address = make_valid_addr("alice");

        let user_id = state::insert_new_user(deps.as_mut().storage, alice_address.clone())
            .expect("Should add user");

        let zephyrus_addr = make_valid_addr("zephyrus");

        let default_hydromancer_id = state::get_constants(deps.as_mut().storage)
            .unwrap()
            .default_hydromancer_id;
        state::add_vessel(
            deps.as_mut().storage,
            &Vessel {
                hydro_lock_id: 0,
                tokenized_share_record_id: 0,
                class_period: 12,
                auto_maintenance: true,
                hydromancer_id: default_hydromancer_id,
                owner_id: user_id,
            },
            &alice_address,
        )
        .expect("Should add vessel");

        state::add_vessel(
            deps.as_mut().storage,
            &Vessel {
                hydro_lock_id: 1,
                tokenized_share_record_id: 1,
                class_period: 6,
                auto_maintenance: true,
                hydromancer_id: default_hydromancer_id,
                owner_id: user_id,
            },
            &alice_address,
        )
        .expect("Should add vessel");

        let _ = super::execute_hydromancer_vote(
            &mut deps.as_mut(),
            mock_env(),
            MessageInfo {
                sender: zephyrus_addr.clone(),
                funds: vec![],
            },
            1,
            vec![
                {
                    VesselsToHarbor {
                        harbor_id: 1,
                        vessel_ids: vec![1, 2],
                    }
                },
                {
                    VesselsToHarbor {
                        harbor_id: 2,
                        vessel_ids: vec![3, 4],
                    }
                },
            ],
        )
        .unwrap();
        let hydromancer_shares = state::get_hydromancer_shares_by_round(
            deps.as_ref().storage,
            default_hydromancer_id,
            1,
            1,
        )
        .expect("Hydromancer shares should exist");

        assert_eq!(hydromancer_shares.len(), 1);
        assert_eq!(hydromancer_shares[0].1, 1_000_000 + 2_000_000); //see mock querier , values are hardcoded by lock ids
        assert_eq!(hydromancer_shares[0].0, "crosnest");
    }

    #[test]
    fn hydromancer_vote_shares_initialized_with_user_controlled_vessel() {
        let mut deps = mock_dependencies();

        init_contract(deps.as_mut());

        let alice_address = make_valid_addr("alice");

        let user_id = state::insert_new_user(deps.as_mut().storage, alice_address.clone())
            .expect("Should add user");

        let zephyrus_addr = make_valid_addr("zephyrus");

        let default_hydromancer_id = state::get_constants(deps.as_mut().storage)
            .unwrap()
            .default_hydromancer_id;
        state::add_vessel(
            deps.as_mut().storage,
            &Vessel {
                hydro_lock_id: 0,
                tokenized_share_record_id: 0,
                class_period: 12,
                auto_maintenance: true,
                hydromancer_id: default_hydromancer_id,
                owner_id: user_id,
            },
            &alice_address,
        )
        .expect("Should add vessel");

        state::add_vessel_to_harbor(
            deps.as_mut().storage,
            1,
            1,
            1,
            &VesselHarbor {
                user_control: true,
                hydro_lock_id: 0,
                steerer_id: user_id,
            },
        )
        .expect("Should add vessel to harbor");

        state::add_vessel(
            deps.as_mut().storage,
            &Vessel {
                hydro_lock_id: 1,
                tokenized_share_record_id: 1,
                class_period: 6,
                auto_maintenance: true,
                hydromancer_id: default_hydromancer_id,
                owner_id: user_id,
            },
            &alice_address,
        )
        .expect("Should add vessel");

        let _ = super::execute_hydromancer_vote(
            &mut deps.as_mut(),
            mock_env(),
            MessageInfo {
                sender: zephyrus_addr.clone(),
                funds: vec![],
            },
            1,
            vec![
                {
                    VesselsToHarbor {
                        harbor_id: 1,
                        vessel_ids: vec![1, 2],
                    }
                },
                {
                    VesselsToHarbor {
                        harbor_id: 2,
                        vessel_ids: vec![3, 4],
                    }
                },
            ],
        )
        .unwrap();
        let hydromancer_shares = state::get_hydromancer_shares_by_round(
            deps.as_ref().storage,
            default_hydromancer_id,
            1,
            1,
        )
        .expect("Hydromancer shares should exist");

        assert_eq!(hydromancer_shares.len(), 1);
        assert_eq!(hydromancer_shares[0].1, 2_000_000); //see mock querier , value is hardcoded by lock ids
        assert_eq!(hydromancer_shares[0].0, "crosnest");
    }
}
