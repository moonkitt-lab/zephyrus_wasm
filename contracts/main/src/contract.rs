use std::collections::BTreeSet;

use cosmwasm_std::{
    entry_point, from_json, to_json_binary, Addr, AllBalanceResponse, BankMsg, BankQuery, Binary,
    Coin, Deps, DepsMut, Env, MessageInfo, QueryRequest, Reply, Response as CwResponse, StdError,
    StdResult, SubMsg, WasmMsg,
};

use hydro_interface::msgs::ExecuteMsg::{RefreshLockDuration, UnlockTokens, Unvote, Vote};
use hydro_interface::msgs::{
    CurrentRoundResponse, HydroConstantsResponse, HydroQueryMsg, LockupsSharesResponse,
    ProposalToLockups, RoundLockPowerSchedule, SpecificUserLockupsResponse, TranchesResponse,
};

use hydro_interface::state::query_lock_entries;
use neutron_sdk::bindings::msg::NeutronMsg;
use serde::{Deserialize, Serialize};
use zephyrus_core::msgs::{
    BuildVesselParams, ConstantsResponse, ExecuteMsg, HydroProposalId, InstantiateMsg, MigrateMsg,
    QueryMsg, RoundId, TrancheId, VesselHarborInfo, VesselHarborResponse, VesselInfo,
    VesselsResponse, VesselsToHarbor, VotingPowerResponse,
};
use zephyrus_core::state::{Constants, HydroConfig, HydroLockId, Vessel, VesselHarbor};

use crate::{
    errors::ContractError,
    helpers::vectors::{compare_coin_vectors, compare_u64_vectors},
    state,
};

type Response = CwResponse<NeutronMsg>;

const DECOMMISSION_REPLY_ID: u64 = 1;
const VOTE_REPLY_ID: u64 = 2;

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

fn validate_lock_duration(
    round_lock_power_schedule: &RoundLockPowerSchedule,
    lock_epoch_length: u64,
    lock_duration: u64,
) -> Result<(), ContractError> {
    let lock_times = round_lock_power_schedule
        .round_lock_power_schedule
        .iter()
        .map(|entry| entry.locked_rounds * lock_epoch_length)
        .collect::<Vec<u64>>();

    if !lock_times.contains(&lock_duration) {
        return Err(ContractError::Std(StdError::generic_err(format!(
            "Lock duration must be one of: {:?}; but was: {}",
            lock_times, lock_duration
        ))));
    }

    Ok(())
}
/// Receive Lockup as NFT and create a Vessel with some params from "msg"
fn execute_receive_nft(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    _sender: String,
    token_id: String,
    msg: Binary,
) -> Result<Response, ContractError> {
    let constants = state::get_constants(deps.storage)?;
    validate_contract_is_not_paused(&constants)?;

    // We don't use `sender` to determine who the owner should be, because
    // sender can be any operator or approved person on the NFT,
    // and we let that sender fill whatever they want as `owner` in `VesselInfo`
    // By checking that the NFT comes from Hydro, it is enough to ensure that the sender has permissions

    // 1. Check that NFT comes from Hydro
    if info.sender.to_string() != constants.hydro_config.hydro_contract_address.to_string() {
        return Err(ContractError::NftNotAccepted);
    }
    let current_round = query_hydro_current_round(
        deps.as_ref(),
        constants.hydro_config.hydro_contract_address.to_string(),
    )?;

    let vessel_info: VesselInfo = from_json(&msg)?;
    let hydro_lock_id: u64 = token_id.parse().unwrap();

    // 2. Check that owner is a valid address
    let owner_addr = deps.api.addr_validate(&vessel_info.owner)?;

    // 3. Check that Hydromancer exists
    if !state::hydromancer_exists(deps.storage, vessel_info.hydromancer_id) {
        return Err(ContractError::HydromancerNotFound {
            hydromancer_id: vessel_info.hydromancer_id,
        });
    }

    // 4. Check that class_period represents a valid lock duration
    let constant_response: HydroConstantsResponse = deps.querier.query_wasm_smart(
        constants.hydro_config.hydro_contract_address.to_string(),
        &HydroQueryMsg::Constants {},
    )?;
    validate_lock_duration(
        &constant_response.constants.round_lock_power_schedule,
        constant_response.constants.lock_epoch_length,
        vessel_info.class_period,
    )?;

    // 5. Check that we are owner of the lockup (as transfer happens before calling Zephyrus' Cw721ReceiveMsg)
    let user_specific_lockups: SpecificUserLockupsResponse = deps.querier.query_wasm_smart(
        constants.hydro_config.hydro_contract_address.to_string(),
        &HydroQueryMsg::SpecificUserLockups {
            address: env.contract.address.to_string(),
            lock_ids: vec![hydro_lock_id],
        },
    )?;
    if user_specific_lockups.lockups.is_empty() {
        return Err(ContractError::LockupNotOwned {
            id: token_id.to_string(),
        });
    }
    // 6. Owner could be a new user, so we need to insert it in state
    let mut owner_id = state::get_user_id_by_address(deps.storage, owner_addr.clone());
    if owner_id.is_err() {
        owner_id = state::insert_new_user(deps.storage, owner_addr.clone());
    }
    let owner_id = owner_id.expect("Owner id should be present");

    let lockup_shares_response = query_hydro_lockups_shares(
        deps.as_ref(),
        constants.hydro_config.hydro_contract_address.to_string(),
        vec![hydro_lock_id],
    )?;
    let current_time_weighted_shares =
        lockup_shares_response.lockups_shares_info[0].time_weighted_shares;
    let token_group_id = lockup_shares_response.lockups_shares_info[0]
        .token_group_id
        .clone();
    let locked_rounds = lockup_shares_response.lockups_shares_info[0].locked_rounds;

    // 7. Store the vessel in state
    let vessel = Vessel {
        hydro_lock_id,
        class_period: vessel_info.class_period,
        tokenized_share_record_id: None,
        hydromancer_id: vessel_info.hydromancer_id,
        auto_maintenance: vessel_info.auto_maintenance,
        owner_id,
    };
    state::add_vessel(deps.storage, &vessel, &owner_addr)?;

    let is_hydromancer_tw_shares_already_initialized = state::is_exist_tw_shares_for_hydromancer(
        deps.storage,
        vessel_info.hydromancer_id,
        current_round,
    )?;
    if is_hydromancer_tw_shares_already_initialized {
        state::save_vessel_shares_info(
            deps.storage,
            vessel.hydro_lock_id,
            current_round,
            current_time_weighted_shares.u128(),
            token_group_id.clone(),
            locked_rounds,
        )?;
        if let Some(locked_rounds) = locked_rounds {
            // hydromancer tw shares already initialized, so we need to add the new time weighted shares to the hydromancer
            state::add_time_weighted_shares_to_hydromancer(
                deps.storage,
                vessel_info.hydromancer_id,
                current_round,
                &token_group_id,
                locked_rounds,
                current_time_weighted_shares.u128(),
            )?;
        }
    }

    Ok(Response::default())
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
    mut deps: DepsMut,
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
    let sender = info.sender.clone();

    let hydromancer_id = state::get_hydromancer_id_by_address(deps.storage, sender.clone())
        .map_err(|err: StdError| ContractError::from(err))?;
    let current_round_id = query_hydro_current_round(
        deps.as_ref(),
        constants.hydro_config.hydro_contract_address.to_string(),
    )?;
    //initialize time weighted shares for hydromancer and current round if they are not initialized
    initialize_hydromancer_time_weighted_shares(
        &mut deps,
        tranche_id,
        sender.clone(),
        constants.hydro_config.hydro_contract_address.to_string(),
        current_round_id,
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
    let tranches: TranchesResponse = deps.querier.query_wasm_smart(
        constants.hydro_config.hydro_contract_address.to_string(),
        &HydroQueryMsg::Tranches {},
    )?;

    for hydro_lock_id in hydro_lock_ids.iter() {
        let vessel = state::get_vessel(deps.storage, *hydro_lock_id)?;
        let vessel_shares =
            state::get_vessel_shares_info(deps.storage, current_round_id, *hydro_lock_id)?;
        let previous_hydromancer_id = vessel.hydromancer_id;
        // If the vessel is locked, substract the time weighted shares from the previous hydromancer and add it to the new hydromancer
        // if Vessel was used by hydromancer or user, it will be unvoted, it means that the time weighted shares will have to be substracted from the hydromancer proposal and from total time wighted shares of the proposal
        if let Some(locked_rounds) = vessel_shares.locked_rounds {
            let is_previous_hydromancertws_already_initialize =
                state::is_exist_tw_shares_for_hydromancer(
                    deps.storage,
                    previous_hydromancer_id,
                    current_round_id,
                )?;

            if is_previous_hydromancertws_already_initialize {
                state::substract_time_weighted_shares_from_hydromancer(
                    deps.storage,
                    previous_hydromancer_id,
                    current_round_id,
                    &vessel_shares.token_group_id,
                    locked_rounds,
                    vessel_shares.time_weighted_shares,
                )?;
            }

            let is_new_hydromancertws_already_initialize =
                state::is_exist_tw_shares_for_hydromancer(
                    deps.storage,
                    hydromancer_id,
                    current_round_id,
                )?;

            if !is_new_hydromancertws_already_initialize {
                state::add_time_weighted_shares_to_hydromancer(
                    deps.storage,
                    hydromancer_id,
                    current_round_id,
                    &vessel_shares.token_group_id,
                    locked_rounds,
                    vessel_shares.time_weighted_shares,
                )?;
            }
            for tranche in tranches.tranches.iter() {
                let vessel_harbor = state::get_vessel_harbor(
                    deps.storage,
                    tranche.id,
                    current_round_id,
                    *hydro_lock_id,
                );
                if vessel_harbor.is_ok() {
                    let (vessel_harbor, proposal_id) = vessel_harbor.unwrap();
                    // Vessel is already used by hydromancer or user, substract the time weighted shares from the proposal
                    state::substract_time_weighted_shares_from_proposal(
                        deps.storage,
                        proposal_id,
                        &vessel_shares.token_group_id,
                        vessel_shares.time_weighted_shares,
                    )?;
                    if !vessel_harbor.user_control {
                        // Vessel was used by hydromancer, substract the time weighted shares from the hydromancer on the proposal
                        state::substract_time_weighted_shares_from_proposal_for_hydromancer(
                            deps.storage,
                            proposal_id,
                            previous_hydromancer_id,
                            &vessel_shares.token_group_id,
                            vessel_shares.time_weighted_shares,
                        )?;
                    }
                }
            }
        }

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
        let lockups_shares_response = query_hydro_lockups_shares(
            deps.as_ref(),
            constants.hydro_config.hydro_contract_address.to_string(),
            vessels_to_harbor.vessel_ids.clone(),
        )?;
        for lockup_shares_info in lockups_shares_response.lockups_shares_info.iter() {
            //if not under user control and already voted by hydromancer, lock_id should be unvote, otherwise if user vote the same proposal as hydromancer it will be skipped by hydro than zephyrus and still under hydromancer control
            if !state::is_vessel_under_user_control(
                deps.storage,
                tranche_id,
                current_round_id,
                lockup_shares_info.lock_id,
            ) && state::get_harbor_of_vessel(
                deps.storage,
                tranche_id,
                current_round_id,
                lockup_shares_info.lock_id,
            )?
            .is_some()
            {
                // vessel used by hydromancer should be unvoted
                unvote_ids.push(lockup_shares_info.lock_id);
            }
            let vessel_shares_info = state::get_vessel_shares_info(
                deps.storage,
                current_round_id,
                lockup_shares_info.lock_id,
            );
            if vessel_shares_info.is_err() {
                //if vessel shares info is not initialized, initialize it, it means that hydromancer has not voted yet
                state::save_vessel_shares_info(
                    deps.storage,
                    lockup_shares_info.lock_id,
                    current_round_id,
                    lockup_shares_info.time_weighted_shares.u128(),
                    lockup_shares_info.token_group_id.clone(),
                    lockup_shares_info.locked_rounds,
                )?;
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
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
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
        ExecuteMsg::UserVote {
            tranche_id,
            vessels_harbors,
        } => execute_user_vote(deps, info, tranche_id, vessels_harbors),

        ExecuteMsg::ReceiveNft(receive_msg) => execute_receive_nft(
            deps,
            env,
            info,
            receive_msg.sender,
            receive_msg.token_id,
            receive_msg.msg,
        ),
        ExecuteMsg::ChangeHydromancer {
            tranche_id,
            hydromancer_id,
            hydro_lock_ids,
        } => execute_change_hydromancer(deps, info, tranche_id, hydromancer_id, hydro_lock_ids),
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
        DECOMMISSION_REPLY_ID => handle_unlock_tokens_reply(deps, env, reply),
        VOTE_REPLY_ID => {
            let skipped_locks = parse_locks_skipped_reply(reply.clone())?;
            let payload: VoteReplyPayload =
                from_json(reply.payload).expect("Vote parameters always attached");
            handle_vote_reply(deps, payload, skipped_locks)
        }
        _ => Err(ContractError::CustomError {
            msg: "Unknown reply id".to_string(),
        }),
    }
}

#[entry_point]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, StdError> {
    Ok(Response::default())
}

//Handle vote reply, used after both user and hydromancer vote
fn handle_vote_reply(
    deps: DepsMut,
    payload: VoteReplyPayload,
    skipped_locks: Vec<u64>,
) -> Result<Response, ContractError> {
    for vessels_to_harbor in payload.vessels_harbors.clone() {
        let mut lock_ids = vec![];
        for vessel_id in vessels_to_harbor.vessel_ids.iter() {
            //if vessel is skipped, it means that hydro was not able to vote for it, zephyrus skips it too
            if skipped_locks.contains(vessel_id) {
                continue;
            }
            let vessel = state::get_vessel(deps.storage, *vessel_id)?;
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
            match previous_harbor_id {
                Some(previous_harbor_id) => {
                    if previous_harbor_id != vessels_to_harbor.harbor_id {
                        //vote has changed
                        state::remove_vessel_harbor(
                            deps.storage,
                            payload.tranche_id,
                            payload.round_id,
                            previous_harbor_id,
                            vessel.hydro_lock_id,
                        )?;
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

//initialize time weighted shares for hydromancer, tranche_id for the current round if they are not initialized yet
fn initialize_hydromancer_time_weighted_shares(
    deps: &mut DepsMut,
    tranche_id: TrancheId,
    hydromancer_addr: Addr,
    hydro_contract_addr: String,
    current_round: RoundId,
) -> Result<(), ContractError> {
    let hydromancer_id =
        state::get_hydromancer_id_by_address(deps.storage, hydromancer_addr.clone())?;
    // Test if time weighted shares for hydromancer, tranche_id for the current round are already initialized
    let is_hydromancer_tw_shares_already_initialized =
        state::is_exist_tw_shares_for_hydromancer(deps.storage, hydromancer_id, current_round)?;
    if !is_hydromancer_tw_shares_already_initialized {
        // Not initialized, we need to initialize them

        // Load all vessels for the hydromancer
        let vessels = state::get_vessels_by_hydromancer(
            deps.storage,
            hydromancer_addr.clone(),
            0,
            usize::MAX,
        )?;
        // Load lockup sahres for all hydromancer's vessels
        let lockups_shares_response = query_hydro_lockups_shares(
            deps.as_ref(),
            hydro_contract_addr,
            vessels.iter().map(|v| v.hydro_lock_id).collect(),
        )?;

        for lockup_shares in lockups_shares_response.lockups_shares_info {
            let vessel = state::get_vessels_by_ids(deps.storage, &[lockup_shares.lock_id])?
                .pop()
                .expect("Vessel should exist");

            state::save_vessel_shares_info(
                deps.storage,
                vessel.hydro_lock_id,
                current_round,
                lockup_shares.time_weighted_shares.u128(),
                lockup_shares.token_group_id.clone(),
                lockup_shares.locked_rounds,
            )?;
            let is_vessel_under_user_control = state::is_vessel_under_user_control(
                deps.storage,
                tranche_id,
                current_round,
                lockup_shares.lock_id,
            );
            if !is_vessel_under_user_control {
                if let Some(locked_rounds) = lockup_shares.locked_rounds {
                    // Vessel is still locked, it has voting power
                    state::add_time_weighted_shares_to_hydromancer(
                        deps.storage,
                        hydromancer_id,
                        current_round,
                        &lockup_shares.token_group_id,
                        locked_rounds,
                        lockup_shares.time_weighted_shares.u128(),
                    )
                    .expect("Failed to insert time weighted shares");
                }
            }
        }
    }

    Ok(())
}

fn query_hydro_lockups_shares(
    deps: Deps,
    hydro_contract_addr: String,
    vessel_ids: Vec<u64>,
) -> Result<LockupsSharesResponse, StdError> {
    let lockups_shares: LockupsSharesResponse = deps
        .querier
        .query_wasm_smart(
            hydro_contract_addr,
            &HydroQueryMsg::LockupsShares {
                lock_ids: vessel_ids.clone(),
            },
        )
        .map_err(|e| {
            StdError::generic_err(format!(
                "Failed to get time weighted shares for vessels {} from hydro : {}",
                vessel_ids
                    .iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>()
                    .join(","),
                e
            ))
        })?;
    Ok(lockups_shares)
}

#[cfg(test)]
mod test {
    use std::time::SystemTime;

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
        CurrentRoundResponse, ExecuteMsg as HydroExecuteMsg, HydroQueryMsg,
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
                HydroQueryMsg::Constants {} => {
                    QuerierResult::Err(cosmwasm_std::SystemError::Unknown {})
                }
                HydroQueryMsg::SpecificUserLockups { .. } => {
                    QuerierResult::Err(cosmwasm_std::SystemError::Unknown {})
                }
                HydroQueryMsg::LockupsShares { lock_ids } => {
                    QuerierResult::Err(cosmwasm_std::SystemError::Unknown {})
                }
                HydroQueryMsg::Tranches {} => {
                    QuerierResult::Err(cosmwasm_std::SystemError::Unknown {})
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
                deps.as_mut(),
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
                tokenized_share_record_id: Some(0),
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
            super::handle_vote_reply(deps.as_mut(), payload, vec![]).unwrap_err(),
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
                tokenized_share_record_id: Some(0),
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
            super::handle_vote_reply(deps.as_mut(), payload, vec![]).unwrap_err(),
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
                tokenized_share_record_id: Some(0),
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
            deps.as_mut(),
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
        let _ = super::handle_vote_reply(deps.as_mut(), payload, skipped_ids).unwrap();

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
                tokenized_share_record_id: Some(0),
                class_period: 12,
                auto_maintenance: true,
                hydromancer_id: default_hydromancer_id,
                owner_id: user_id,
            },
            &alice_address,
        )
        .expect("Should add vessel");

        let res = super::execute_hydromancer_vote(
            deps.as_mut(),
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

        let _ = super::handle_vote_reply(deps.as_mut(), payload, vec![]).unwrap();

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
                tokenized_share_record_id: Some(0),
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
            deps.as_mut(),
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

        let _ = super::handle_vote_reply(deps.as_mut(), payload, vec![]).unwrap();

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
                deps.as_mut(),
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
                deps.as_mut(),
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

    //TESTS USER VOTE
    #[test]
    fn user_vote_fails_not_zephyrus_user() {
        let mut deps = mock_dependencies();

        init_contract(deps.as_mut());
        let alice_address = make_valid_addr("alice");
        assert_eq!(
            super::execute_user_vote(
                deps.as_mut(),
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
                tokenized_share_record_id: Some(0),
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
            super::handle_vote_reply(deps.as_mut(), payload, vec![]).unwrap_err(),
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
                tokenized_share_record_id: Some(0),
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
        let _ = super::handle_vote_reply(deps.as_mut(), payload, vec![]).unwrap();

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
                tokenized_share_record_id: Some(0),
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
        let _ = super::handle_vote_reply(deps.as_mut(), payload, vec![]).unwrap();

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
                tokenized_share_record_id: Some(0),
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
                tokenized_share_record_id: Some(0),
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
                tokenized_share_record_id: Some(0),
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
                tokenized_share_record_id: Some(0),
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
                tokenized_share_record_id: Some(0),
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
                tokenized_share_record_id: Some(0),
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
}
