use std::collections::{BTreeSet, HashMap};

use cosmwasm_std::{
    entry_point, from_json, to_json_binary, Addr, AllBalanceResponse, BankMsg, BankQuery, Binary,
    Coin, Deps, DepsMut, Env, MessageInfo, QueryRequest, Reply, Response as CwResponse, StdError,
    StdResult, Storage, SubMsg, WasmMsg,
};

use hydro_interface::msgs::ExecuteMsg::{RefreshLockDuration, UnlockTokens, Unvote, Vote};
use hydro_interface::msgs::{
    CurrentRoundResponse, HydroConstantsResponse, HydroQueryMsg, LockupsSharesResponse,
    ProposalToLockups, RoundLockPowerSchedule, SpecificUserLockupsResponse,
    SpecificUserLockupsWithTrancheInfosResponse, TranchesResponse,
};

use neutron_sdk::bindings::msg::NeutronMsg;
use serde::{Deserialize, Serialize};
use zephyrus_core::msgs::{
    ConstantsResponse, ExecuteMsg, HydroProposalId, InstantiateMsg, MigrateMsg, QueryMsg, RoundId,
    TrancheId, VesselHarborInfo, VesselHarborResponse, VesselInfo, VesselsResponse,
    VesselsToHarbor, VotingPowerResponse,
};
use zephyrus_core::state::{
    Constants, HydroConfig, HydroLockId, Vessel, VesselHarbor, VesselSharesInfo,
};

use crate::{
    errors::ContractError,
    helpers::vectors::{compare_coin_vectors, compare_u64_vectors, join_u64_ids},
    state,
};

type Response = CwResponse<NeutronMsg>;

const DECOMMISSION_REPLY_ID: u64 = 1;
const VOTE_REPLY_ID: u64 = 2;
const REFRESH_TIME_WEIGHTED_SHARES_REPLY_ID: u64 = 3;

const MAX_PAGINATION_LIMIT: usize = 1000;
const DEFAULT_PAGINATION_LIMIT: usize = 100;

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
    let current_round = query_hydro_current_round(&deps.as_ref(), &constants)?;

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
    let owner_id = state::get_user_id_by_address(deps.storage, owner_addr.clone())
        .or_else(|_| state::insert_new_user(deps.storage, owner_addr.clone()))?;

    // 7. Store the vessel in state
    let vessel = Vessel {
        hydro_lock_id,
        class_period: vessel_info.class_period,
        tokenized_share_record_id: None,
        hydromancer_id: Some(vessel_info.hydromancer_id),
        auto_maintenance: vessel_info.auto_maintenance,
        owner_id,
    };
    state::add_vessel(deps.storage, &vessel, &owner_addr)?;

    let lockup_shares_response =
        query_hydro_lockups_shares(&deps.as_ref(), &constants, vec![hydro_lock_id])?;

    let lockup_info = &lockup_shares_response.lockups_shares_info[0];
    let current_time_weighted_shares = lockup_info.time_weighted_shares.u128();
    let token_group_id = &lockup_info.token_group_id;
    let locked_rounds = lockup_info.locked_rounds;

    // Always save vessel shares info
    state::save_vessel_shares_info(
        deps.storage,
        vessel.hydro_lock_id,
        current_round,
        current_time_weighted_shares,
        token_group_id.clone(),
        locked_rounds,
    )?;

    if current_time_weighted_shares > 0 {
        state::add_time_weighted_shares_to_hydromancer(
            deps.storage,
            vessel_info.hydromancer_id,
            current_round,
            token_group_id,
            locked_rounds,
            current_time_weighted_shares,
        )?;
    }

    Ok(Response::default())
}

// This function loops through all the vessels, and filters those who have auto_maintenance true
// Then, it combines them by hydro_lock_duration, and calls execute_update_vessels_class
fn execute_auto_maintain(mut deps: DepsMut, _info: MessageInfo) -> Result<Response, ContractError> {
    let constants = state::get_constants(deps.storage)?;
    validate_contract_is_not_paused(&constants)?;
    let current_round_id = query_hydro_current_round(&deps.as_ref(), &constants)?;

    let vessels_ids_by_hydro_lock_duration = state::get_vessel_ids_auto_maintained_by_class()?;

    let iterator = vessels_ids_by_hydro_lock_duration.range(
        deps.as_ref().storage,
        None,
        None,
        cosmwasm_std::Order::Ascending,
    );

    let mut response = Response::new();

    // Collect all items first to avoid borrowing conflicts
    let items: Vec<_> = iterator.collect::<StdResult<_>>()?;

    // Process collected items
    for item in items {
        let (hydro_period, hydro_lock_ids) = item;

        if hydro_lock_ids.is_empty() {
            continue;
        }
        initialize_vessel_tws(
            &mut deps,
            hydro_lock_ids.clone().into_iter().collect(),
            current_round_id,
            &constants,
        )?;

        let refresh_duration_msg = RefreshLockDuration {
            lock_ids: hydro_lock_ids.clone().into_iter().collect(),
            lock_duration: hydro_period,
        };

        let execute_refresh_msg = WasmMsg::Execute {
            contract_addr: constants.hydro_config.hydro_contract_address.to_string(),
            msg: to_json_binary(&refresh_duration_msg)?,
            funds: vec![],
        };
        // Use SubMsg instead of Message, because we have to handle reply on success and update all time weighted shares see : handle_refresh_time_weighted_shares_reply
        let sub_msg =
            SubMsg::reply_on_success(execute_refresh_msg, REFRESH_TIME_WEIGHTED_SHARES_REPLY_ID)
                .with_payload(to_json_binary(&hydro_lock_ids)?);

        response = response
            .add_attribute("Action", "Refresh lock duration")
            .add_attribute(
                ["ids ", &hydro_period.to_string()].concat(),
                join_u64_ids(hydro_lock_ids),
            );
        response = response.add_submessage(sub_msg);
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
    mut deps: DepsMut,
    info: MessageInfo,
    hydro_lock_ids: Vec<u64>,
    hydro_lock_duration: u64,
) -> Result<Response, ContractError> {
    let constants = state::get_constants(deps.storage)?;
    validate_contract_is_not_paused(&constants)?;

    let current_round_id = query_hydro_current_round(&deps.as_ref(), &constants)?;

    initialize_vessel_tws(
        &mut deps,
        hydro_lock_ids.clone(),
        current_round_id,
        &constants,
    )?;

    let refresh_duration_msg = RefreshLockDuration {
        lock_ids: hydro_lock_ids,
        lock_duration: hydro_lock_duration,
    };

    // There should not be any funds?
    let execute_refresh_duration_msg = WasmMsg::Execute {
        contract_addr: constants.hydro_config.hydro_contract_address.to_string(),
        msg: to_json_binary(&refresh_duration_msg)?,
        funds: info.funds.clone(),
    };

    let sub_msg = SubMsg::reply_on_success(
        execute_refresh_duration_msg,
        REFRESH_TIME_WEIGHTED_SHARES_REPLY_ID,
    );

    Ok(Response::new().add_submessage(sub_msg))
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
        .add_attribute("hydro_lock_id", join_u64_ids(hydro_lock_ids)))
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
    let user_specific_lockups: SpecificUserLockupsResponse = deps.querier.query_wasm_smart(
        hydro_config.hydro_contract_address.to_string(),
        &HydroQueryMsg::SpecificUserLockups {
            address: env.contract.address.to_string(),
            lock_ids: hydro_lock_ids.clone(),
        },
    )?;

    let lock_entries = user_specific_lockups.lockups;

    let mut expected_unlocked_ids = vec![];
    for lock_entry in lock_entries {
        if lock_entry.lock_entry.lock_end < env.block.time {
            expected_unlocked_ids.push(lock_entry.lock_entry.lock_id);
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

pub fn find_duplicate_harbor_id_in_vote(
    vessels_harbors: &[VesselsToHarbor],
) -> Option<HydroProposalId> {
    let mut seen_harbor_ids = BTreeSet::new();
    for item in vessels_harbors {
        if !seen_harbor_ids.insert(item.harbor_id) {
            return Some(item.harbor_id);
        }
    }
    None
}

pub fn find_duplicate_vessel_id_in_vote(
    vessels_harbors: &[VesselsToHarbor],
) -> Option<HydroLockId> {
    let mut seen_vessel_ids = BTreeSet::new();
    for item in vessels_harbors {
        for vessel_id in item.vessel_ids.iter() {
            if !seen_vessel_ids.insert(*vessel_id) {
                return Some(*vessel_id);
            }
        }
    }
    None
}

pub fn find_duplicate_ids(ids: &[u64]) -> Option<u64> {
    let mut seen = BTreeSet::new();
    for &id in ids {
        if !seen.insert(id) {
            return Some(id);
        }
    }
    None
}

fn validate_vote_duplicates(vessels_harbors: &[VesselsToHarbor]) -> Result<(), ContractError> {
    if let Some(harbor_id) = find_duplicate_harbor_id_in_vote(vessels_harbors) {
        return Err(ContractError::VoteDuplicatedHarborId { harbor_id });
    }

    if let Some(vessel_id) = find_duplicate_vessel_id_in_vote(vessels_harbors) {
        return Err(ContractError::VoteDuplicatedVesselId { vessel_id });
    }

    Ok(())
}

fn execute_hydromancer_vote(
    mut deps: DepsMut,
    info: MessageInfo,
    tranche_id: u64,
    vessels_harbors: Vec<VesselsToHarbor>,
) -> Result<Response, ContractError> {
    let constants = state::get_constants(deps.storage)?;

    validate_contract_is_not_paused(&constants)?;
    validate_vote_duplicates(&vessels_harbors)?;

    let current_round_id = query_hydro_current_round(&deps.as_ref(), &constants)?;
    let hydromancer_id = state::get_hydromancer_id_by_address(deps.storage, info.sender)?;

    // We need to initialize the Hydromancer TWS when the hydromancer votes
    // It's only initialized once per round / hydromancer
    complete_hydromancer_time_weighted_shares(
        &mut deps,
        hydromancer_id,
        &constants,
        current_round_id,
    )?;

    // Prepare the proposals_votes
    let proposals_votes: Vec<ProposalToLockups> = vessels_harbors
        .iter()
        .map(|vessels_to_harbor| {
            // Validate that all vessels are controlled by the hydromancer
            validate_hydromancer_controls_vessels(
                deps.storage,
                hydromancer_id,
                &vessels_to_harbor.vessel_ids,
            )?;

            Ok(ProposalToLockups {
                proposal_id: vessels_to_harbor.harbor_id,
                lock_ids: vessels_to_harbor.vessel_ids.clone(),
            })
        })
        .collect::<Result<Vec<_>, ContractError>>()?;

    // Prepare the Vote message with payload
    let vote_message = Vote {
        tranche_id,
        proposals_votes,
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

    let execute_hydro_vote_msg =
        SubMsg::reply_on_success(execute_hydro_vote_msg, VOTE_REPLY_ID).with_payload(payload);

    Ok(Response::new().add_submessage(execute_hydro_vote_msg))
}

fn execute_change_hydromancer(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    tranche_id: u64,
    new_hydromancer_id: u64,
    vessel_ids: Vec<u64>,
) -> Result<Response, ContractError> {
    let constants = state::get_constants(deps.storage)?;

    validate_contract_is_not_paused(&constants)?;
    validate_user_owns_vessels(deps.storage, &info.sender, &vessel_ids)?;
    validate_vessels_not_tied_to_proposal(&deps.as_ref(), &env, &constants, &vessel_ids)?;
    validate_hydromancer_exists(deps.storage, new_hydromancer_id)?;

    let current_round_id = query_hydro_current_round(&deps.as_ref(), &constants)?;
    let tranche_ids = query_hydro_tranches(&deps.as_ref(), &constants)?;

    // Categorize vessels by their current control state
    let (vessels_not_yet_controlled, vessels_already_controlled) =
        categorize_vessels_by_control(deps.storage, new_hydromancer_id, &vessel_ids)?;

    // Step 1: Handle vessels that need hydromancer change
    for vessel_id in &vessels_not_yet_controlled {
        let vessel = state::get_vessel(deps.storage, *vessel_id)?;

        // Handle existing TWS if vessel was already initialized for this round
        if let Ok(vessel_shares) =
            state::get_vessel_shares_info(deps.storage, current_round_id, *vessel_id)
        {
            // Remove TWS from previous hydromancer if it had one
            if let Some(previous_hydromancer_id) = vessel.hydromancer_id {
                state::substract_time_weighted_shares_from_hydromancer(
                    deps.storage,
                    previous_hydromancer_id,
                    current_round_id,
                    &vessel_shares.token_group_id,
                    vessel_shares.locked_rounds,
                    vessel_shares.time_weighted_shares,
                )?;
            }

            // Handle existing votes across all tranches
            remove_vessel_tws_from_proposals(
                deps.storage,
                *vessel_id,
                &vessel,
                &vessel_shares,
                &tranche_ids,
                current_round_id,
            )?;
        }

        // Change the vessel's hydromancer assignment
        state::change_vessel_hydromancer(
            deps.storage,
            &tranche_ids,
            *vessel_id,
            current_round_id,
            new_hydromancer_id,
        )?;
    }

    // Step 2: Batch initialize TWS for all vessels that need it
    // (vessels now have correct hydromancer assignments)
    initialize_vessel_tws(&mut deps, vessel_ids.clone(), current_round_id, &constants)?;

    // Step 3: Send unvote message for vessels that changed hydromancer (or that were controlled by user)
    let response = if !vessels_not_yet_controlled.is_empty() {
        let unvote_msg = Unvote {
            tranche_id,
            lock_ids: vessels_not_yet_controlled.clone(),
        };

        let execute_unvote_msg = WasmMsg::Execute {
            contract_addr: constants.hydro_config.hydro_contract_address.to_string(),
            msg: to_json_binary(&unvote_msg)?,
            funds: vec![],
        };

        Response::new().add_message(execute_unvote_msg)
    } else {
        Response::new()
    };

    Ok(response
        .add_attribute("action", "change_hydromancer")
        .add_attribute("new_hydromancer_id", new_hydromancer_id.to_string())
        .add_attribute(
            "processed_vessels",
            join_u64_ids(&vessels_not_yet_controlled),
        )
        .add_attribute(
            "already_controlled_vessels",
            join_u64_ids(&vessels_already_controlled),
        ))
}

/// Categorize vessels into those not yet controlled by the hydromancer vs already controlled
fn categorize_vessels_by_control(
    storage: &dyn Storage,
    new_hydromancer_id: u64,
    vessel_ids: &[u64],
) -> Result<(Vec<u64>, Vec<u64>), ContractError> {
    let mut not_controlled = Vec::new();
    let mut already_controlled = Vec::new();

    for &vessel_id in vessel_ids {
        let vessel = state::get_vessel(storage, vessel_id)?;

        if vessel.hydromancer_id == Some(new_hydromancer_id) {
            already_controlled.push(vessel_id);
        } else {
            not_controlled.push(vessel_id);
        }
    }

    Ok((not_controlled, already_controlled))
}

/// Handle existing votes for a vessel across all tranches
fn remove_vessel_tws_from_proposals(
    storage: &mut dyn Storage,
    vessel_id: u64,
    vessel: &Vessel,
    vessel_shares: &VesselSharesInfo,
    tranche_ids: &[u64],
    current_round_id: RoundId,
) -> Result<(), ContractError> {
    // Only process if vessel has voting power
    if vessel_shares.time_weighted_shares == 0 {
        return Ok(());
    }

    for &tranche_id in tranche_ids {
        if let Ok((_, proposal_id)) =
            state::get_vessel_harbor(storage, tranche_id, current_round_id, vessel_id)
        {
            // Remove vessel's TWS from the proposal
            state::substract_time_weighted_shares_from_proposal(
                storage,
                proposal_id,
                &vessel_shares.token_group_id,
                vessel_shares.time_weighted_shares,
            )?;

            // If vessel was controlled by a hydromancer, also remove from hydromancer's proposal contribution
            if let Some(previous_hydromancer_id) = vessel.hydromancer_id {
                state::substract_time_weighted_shares_from_proposal_for_hydromancer(
                    storage,
                    proposal_id,
                    previous_hydromancer_id,
                    &vessel_shares.token_group_id,
                    vessel_shares.time_weighted_shares,
                )?;
            }
        }
    }
    Ok(())
}

fn execute_take_control(
    deps: DepsMut,
    info: MessageInfo,
    vessel_ids: Vec<u64>,
) -> Result<Response, ContractError> {
    let constants = state::get_constants(deps.storage)?;
    validate_contract_is_not_paused(&constants)?;
    validate_user_owns_vessels(deps.storage, &info.sender, &vessel_ids)?;

    let current_round_id = query_hydro_current_round(&deps.as_ref(), &constants)?;
    let tranche_ids = query_hydro_tranches(&deps.as_ref(), &constants)?;

    let mut unvote_ids_by_tranche: HashMap<TrancheId, Vec<HydroLockId>> = HashMap::new();
    let mut new_vessels_under_user_control: Vec<HydroLockId> = vec![];

    for vessel_id in vessel_ids {
        let vessel = state::get_vessel(deps.storage, vessel_id)?;

        // If vessel is already under user control there is nothing to do, if not we should reset hydromancer shares, and unvote if it was voted
        // We also don't care if TWS are initialized, as we only care for vessels that are under hydromancer control, or that vote
        if vessel.is_under_user_control() {
            continue;
        }

        // If under Hydromancer, then the shares are already initialized
        // TODO: Not sure anymore
        let vessel_shares =
            state::get_vessel_shares_info(deps.storage, current_round_id, vessel_id)?;

        // If Vessel has VP we should substract the time weighted shares from the hydromancer
        if vessel_shares.time_weighted_shares > 0 {
            state::substract_time_weighted_shares_from_hydromancer(
                deps.storage,
                vessel.hydromancer_id.unwrap(),
                current_round_id,
                &vessel_shares.token_group_id,
                vessel_shares.locked_rounds,
                vessel_shares.time_weighted_shares,
            )?;
        }

        // Vessel was controlled by hydromancer, if hydromancer already voted with it, it should be unvoted
        for tranche_id in &tranche_ids {
            let proposal_id = state::get_harbor_of_vessel(
                deps.storage,
                *tranche_id,
                current_round_id,
                vessel_id,
            )?;

            if proposal_id.is_none() {
                continue;
            }

            let proposal_id = proposal_id.unwrap();
            state::substract_time_weighted_shares_from_proposal(
                deps.storage,
                proposal_id,
                &vessel_shares.token_group_id,
                vessel_shares.time_weighted_shares,
            )?;
            state::substract_time_weighted_shares_from_proposal_for_hydromancer(
                deps.storage,
                proposal_id,
                vessel.hydromancer_id.unwrap(),
                &vessel_shares.token_group_id,
                vessel_shares.time_weighted_shares,
            )?;

            // vessel used by hydromancer should be unvoted
            unvote_ids_by_tranche
                .entry(*tranche_id)
                .or_default()
                .push(vessel_id);
        }
        new_vessels_under_user_control.push(vessel_id);
        state::take_control_of_vessels(deps.storage, vessel_id)?;
    }

    let mut response = Response::new();
    for (tranche_id, unvote_ids) in unvote_ids_by_tranche.iter() {
        let unvote_msg = Unvote {
            tranche_id: *tranche_id,
            lock_ids: unvote_ids.clone(),
        };
        response = response.add_message(WasmMsg::Execute {
            msg: to_json_binary(&unvote_msg)?,
            contract_addr: constants.hydro_config.hydro_contract_address.to_string(),
            funds: vec![],
        });
    }

    Ok(response
        .add_attribute("action", "take_control")
        .add_attribute(
            "new_vessels_under_user_control",
            join_u64_ids(new_vessels_under_user_control),
        ))
}

fn validate_user_owns_vessels(
    storage: &dyn Storage,
    owner: &Addr,
    vessel_ids: &[u64],
) -> Result<(), ContractError> {
    if !state::are_vessels_owned_by(storage, owner, vessel_ids)? {
        return Err(ContractError::Unauthorized {});
    }
    Ok(())
}

fn validate_hydromancer_controls_vessels(
    storage: &dyn Storage,
    hydromancer_id: u64,
    vessel_ids: &[u64],
) -> Result<(), ContractError> {
    if !state::are_vessels_controlled_by_hydromancer(storage, hydromancer_id, vessel_ids)? {
        return Err(ContractError::Unauthorized {});
    }
    Ok(())
}

fn validate_hydromancer_exists(
    storage: &dyn Storage,
    hydromancer_id: u64,
) -> Result<(), ContractError> {
    if !state::hydromancer_exists(storage, hydromancer_id) {
        return Err(ContractError::Unauthorized {});
    }
    Ok(())
}

fn validate_vessels_not_tied_to_proposal(
    deps: &Deps,
    env: &Env,
    constants: &Constants,
    hydro_lock_ids: &[u64],
) -> Result<(), ContractError> {
    let user_lockups_with_tranche_infos: SpecificUserLockupsWithTrancheInfosResponse =
        deps.querier.query_wasm_smart(
            constants.hydro_config.hydro_contract_address.to_string(),
            &HydroQueryMsg::SpecificUserLockupsWithTrancheInfos {
                address: env.contract.address.to_string(),
                lock_ids: hydro_lock_ids.to_vec(),
            },
        )?;

    if let Some(lockup_with_tranche_info) = user_lockups_with_tranche_infos
        .lockups_with_per_tranche_infos
        .iter()
        .find(|lockup| {
            lockup
                .per_tranche_info
                .iter()
                .any(|tranche| tranche.tied_to_proposal.is_some())
        })
    {
        return Err(ContractError::VesselTiedToProposalNotTransferable {
            vessel_id: lockup_with_tranche_info.lock_with_power.lock_entry.lock_id,
        });
    }

    Ok(())
}

fn execute_user_vote(
    deps: DepsMut,
    info: MessageInfo,
    tranche_id: u64,
    vessels_harbors: Vec<VesselsToHarbor>,
) -> Result<Response, ContractError> {
    let constants = state::get_constants(deps.storage)?;
    validate_contract_is_not_paused(&constants)?;

    validate_vote_duplicates(&vessels_harbors)?;

    let user_id = state::get_user_id_by_address(deps.storage, info.sender)
        .map_err(|err: StdError| ContractError::from(err))?;
    let current_round_id = query_hydro_current_round(&deps.as_ref(), &constants)?;
    let mut proposal_votes = vec![];

    for vessels_to_harbor in vessels_harbors.clone() {
        let lockups_shares_response = query_hydro_lockups_shares(
            &deps.as_ref(),
            &constants,
            vessels_to_harbor.vessel_ids.clone(),
        )?;

        for lockup_shares_info in lockups_shares_response.lockups_shares_info.iter() {
            let vessel = state::get_vessel(deps.storage, lockup_shares_info.lock_id)?;

            // Check that the vessel belongs to the user
            if vessel.owner_id != user_id {
                return Err(ContractError::Unauthorized {});
            }

            // Even if a vessel is owned by the user, if it's under hydromancer control, user can't vote with it
            if !vessel.is_under_user_control() {
                return Err(ContractError::VesselUnderHydromancerControl {
                    vessel_id: lockup_shares_info.lock_id,
                });
            }

            let vessel_shares_info = state::get_vessel_shares_info(
                deps.storage,
                current_round_id,
                lockup_shares_info.lock_id,
            );
            if vessel_shares_info.is_err() {
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
    let response = Response::new();

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
        } => {
            execute_change_hydromancer(deps, env, info, tranche_id, hydromancer_id, hydro_lock_ids)
        }
        ExecuteMsg::TakeControl { vessel_ids } => execute_take_control(deps, info, vessel_ids),
    }
}

fn query_voting_power(_deps: Deps, _env: Env) -> Result<VotingPowerResponse, StdError> {
    todo!()
}

fn query_hydro_current_round(deps: &Deps, constants: &Constants) -> Result<RoundId, StdError> {
    let current_round_resp: CurrentRoundResponse = deps.querier.query_wasm_smart(
        constants.hydro_config.hydro_contract_address.to_string(),
        &HydroQueryMsg::CurrentRound {},
    )?;
    Ok(current_round_resp.round_id)
}

fn query_hydro_tranches(deps: &Deps, constants: &Constants) -> Result<Vec<TrancheId>, StdError> {
    let tranches: TranchesResponse = deps.querier.query_wasm_smart(
        constants.hydro_config.hydro_contract_address.to_string(),
        &HydroQueryMsg::Tranches {},
    )?;
    Ok(tranches
        .tranches
        .into_iter()
        .map(|tranche| tranche.id)
        .collect())
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
    hydromancer_address: String,
    start_index: Option<usize>,
    limit: Option<usize>,
) -> StdResult<VesselsResponse> {
    let hydromancer_addr = deps.api.addr_validate(hydromancer_address.as_str())?;

    let limit = limit
        .unwrap_or(DEFAULT_PAGINATION_LIMIT)
        .min(MAX_PAGINATION_LIMIT);
    let start_index = start_index.unwrap_or(0);

    let hydromancer_id = state::get_hydromancer_id_by_address(deps.storage, hydromancer_addr)?;

    let vessels =
        state::get_vessels_by_hydromancer(deps.storage, hydromancer_id, start_index, limit)?;
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
    // Do not allow query with duplicate vessel IDs
    if let Some(vessel_id) = find_duplicate_ids(&vessel_ids) {
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
    if constant.paused_contract {
        return Err(ContractError::Paused);
    }
    Ok(())
}

fn validate_admin_address(deps: &DepsMut, sender: &Addr) -> Result<(), ContractError> {
    if !state::is_whitelisted_admin(deps.storage, sender)? {
        return Err(ContractError::Unauthorized {});
    }
    Ok(())
}

#[entry_point]
pub fn reply(deps: DepsMut, env: Env, reply: Reply) -> Result<Response, ContractError> {
    match reply.id {
        DECOMMISSION_REPLY_ID => {
            let hydro_unlocked_tokens: Vec<Coin> = parse_unlocked_token_from_reply(&reply)?;
            let unlocked_hydro_lock_ids: Vec<u64> = parse_unlocked_lock_ids_reply(&reply)?;
            let payload: DecommissionVesselsParameters = from_json(reply.payload)?;
            handle_unlock_tokens_reply(
                deps,
                env,
                payload,
                hydro_unlocked_tokens,
                unlocked_hydro_lock_ids,
            )
        }
        VOTE_REPLY_ID => {
            let skipped_locks = parse_locks_skipped_reply(&reply)?;
            let payload: VoteReplyPayload = from_json(&reply.payload)?;
            handle_vote_reply(deps, payload, skipped_locks)
        }
        REFRESH_TIME_WEIGHTED_SHARES_REPLY_ID => {
            let payload: Vec<u64> = from_json(&reply.payload)?;
            handle_refresh_time_weighted_shares_reply(deps, payload)
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

fn handle_refresh_time_weighted_shares_reply(
    deps: DepsMut,
    payload: Vec<u64>,
) -> Result<Response, ContractError> {
    let constants = state::get_constants(deps.storage)?;

    let current_round_id = query_hydro_current_round(&deps.as_ref(), &constants)?;
    let tranche_ids = query_hydro_tranches(&deps.as_ref(), &constants)?;

    let vessels_shares_info =
        query_hydro_lockups_shares(&deps.as_ref(), &constants, payload.clone())?;

    for lockup_shares in vessels_shares_info.lockups_shares_info.iter() {
        let lock_id = lockup_shares.lock_id;
        let new_time_weighted_share = lockup_shares.time_weighted_shares;
        let vessel = state::get_vessel(deps.storage, lock_id)?;
        let vessel_shares_info_before =
            state::get_vessel_shares_info(deps.storage, current_round_id, lock_id)?;
        // TODO: just need to add the difference. When we refresh TWS, it can only add more, not change the group ID, not change the hydromancer, etc.
        if !vessel.is_under_user_control() {
            if vessel_shares_info_before.locked_rounds > 0 {
                state::substract_time_weighted_shares_from_hydromancer(
                    deps.storage,
                    vessel.hydromancer_id.unwrap(),
                    current_round_id,
                    &vessel_shares_info_before.token_group_id,
                    vessel_shares_info_before.locked_rounds,
                    vessel_shares_info_before.time_weighted_shares,
                )?;
            }
            if lockup_shares.locked_rounds > 0 {
                state::add_time_weighted_shares_to_hydromancer(
                    deps.storage,
                    vessel.hydromancer_id.unwrap(),
                    current_round_id,
                    &lockup_shares.token_group_id,
                    lockup_shares.locked_rounds,
                    new_time_weighted_share.u128(),
                )?;
            }
        }

        state::save_vessel_shares_info(
            deps.storage,
            vessel.hydro_lock_id,
            current_round_id,
            new_time_weighted_share.u128(),
            lockup_shares.token_group_id.to_string(),
            lockup_shares.locked_rounds,
        )?;
        for tranche_id in tranche_ids.iter() {
            let vessel_harbor =
                state::get_vessel_harbor(deps.storage, *tranche_id, current_round_id, lock_id).ok();
            if let Some((vessel_harbor, hydro_proposal_id)) = vessel_harbor {
                state::substract_time_weighted_shares_from_proposal(
                    deps.storage,
                    hydro_proposal_id,
                    &vessel_shares_info_before.token_group_id,
                    vessel_shares_info_before.time_weighted_shares,
                )?;

                state::add_time_weighted_shares_to_proposal(
                    deps.storage,
                    hydro_proposal_id,
                    &lockup_shares.token_group_id,
                    new_time_weighted_share.u128(),
                )?;
                if vessel_harbor.user_control && !vessel.is_under_user_control() {
                    // TODO: just need to add the difference. When we refresh TWS, it can only add more, not change the group ID, not change the hydromancer, etc.
                    state::substract_time_weighted_shares_from_proposal_for_hydromancer(
                        deps.storage,
                        hydro_proposal_id,
                        vessel.hydromancer_id.unwrap(),
                        &vessel_shares_info_before.token_group_id,
                        vessel_shares_info_before.time_weighted_shares,
                    )?;
                    state::add_time_weighted_shares_to_proposal_for_hydromancer(
                        deps.storage,
                        hydro_proposal_id,
                        vessel.hydromancer_id.unwrap(),
                        &lockup_shares.token_group_id,
                        new_time_weighted_share.u128(),
                    )?;
                }
            }
        }
    }

    Ok(Response::new())
}

//Handle vote reply, used after both user and hydromancer vote
fn handle_vote_reply(
    deps: DepsMut,
    payload: VoteReplyPayload,
    skipped_locks: Vec<u64>,
) -> Result<Response, ContractError> {
    for vessels_to_harbor in payload.vessels_harbors.clone() {
        let mut lock_ids = vec![];
        let constants = state::get_constants(deps.storage)?;

        let vessels_shares = query_hydro_lockups_shares(
            &deps.as_ref(),
            &constants,
            vessels_to_harbor.vessel_ids.clone(),
        )?;

        for vessel_shares_info in vessels_shares.lockups_shares_info.iter() {
            // if vessel is skipped, it means that hydro was not able to vote for it, zephyrus skips it too
            if skipped_locks.contains(&vessel_shares_info.lock_id) {
                continue;
            }

            let vessel_id = vessel_shares_info.lock_id;
            let vessel = state::get_vessel(deps.storage, vessel_id)?;

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
                        state::substract_time_weighted_shares_from_proposal(
                            deps.storage,
                            previous_harbor_id,
                            &vessel_shares_info.token_group_id,
                            vessel_shares_info.time_weighted_shares.u128(),
                        )?;
                        state::add_time_weighted_shares_to_proposal(
                            deps.storage,
                            vessels_to_harbor.harbor_id,
                            &vessel_shares_info.token_group_id,
                            vessel_shares_info.time_weighted_shares.u128(),
                        )?;
                        // if it's a hydromancer vote, add time weighted shares to proposal for hydromancer
                        if !payload.user_vote && !vessel_shares_info.time_weighted_shares.is_zero()
                        {
                            state::add_time_weighted_shares_to_proposal_for_hydromancer(
                                deps.storage,
                                vessels_to_harbor.harbor_id,
                                payload.steerer_id,
                                &vessel_shares_info.token_group_id,
                                vessel_shares_info.time_weighted_shares.u128(),
                            )?;
                            state::substract_time_weighted_shares_from_proposal_for_hydromancer(
                                deps.storage,
                                previous_harbor_id,
                                payload.steerer_id,
                                &vessel_shares_info.token_group_id,
                                vessel_shares_info.time_weighted_shares.u128(),
                            )?;
                        }
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
                    // update time weighted shares for proposal
                    state::add_time_weighted_shares_to_proposal(
                        deps.storage,
                        vessels_to_harbor.harbor_id,
                        &vessel_shares_info.token_group_id,
                        vessel_shares_info.time_weighted_shares.u128(),
                    )?;
                    if !payload.user_vote && !vessel_shares_info.time_weighted_shares.is_zero() {
                        // should always be some, because hydro has accepted the vote
                        state::add_time_weighted_shares_to_proposal_for_hydromancer(
                            deps.storage,
                            vessels_to_harbor.harbor_id,
                            payload.steerer_id,
                            &vessel_shares_info.token_group_id,
                            vessel_shares_info.time_weighted_shares.u128(),
                        )?;
                    }
                }
            }

            lock_ids.push(vessel.hydro_lock_id);
        }
    }
    Ok(Response::new().add_attribute("skipped_locks", join_u64_ids(skipped_locks)))
}

fn parse_u64_list_from_reply(
    reply: &Reply,
    attribute_key: &str,
) -> Result<Vec<u64>, ContractError> {
    let response = reply
        .result
        .clone()
        .into_result()
        .map_err(|e| ContractError::Std(StdError::generic_err(e)))?;

    let attribute_value = response
        .events
        .iter()
        .flat_map(|e| &e.attributes)
        .find_map(|attr| (attr.key == attribute_key).then_some(&attr.value))
        .ok_or_else(|| {
            ContractError::Std(StdError::generic_err(format!(
                "{} attribute not found",
                attribute_key
            )))
        })?;

    if attribute_value.is_empty() {
        return Ok(vec![]);
    }

    attribute_value
        .split(',')
        .map(|s| s.trim().parse::<u64>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| {
            ContractError::Std(StdError::generic_err(format!(
                "Failed to parse {} ID: {}",
                attribute_key, e
            )))
        })
}

fn parse_coins_from_reply(reply: &Reply, attribute_key: &str) -> Result<Vec<Coin>, ContractError> {
    let response = reply
        .result
        .clone()
        .into_result()
        .map_err(|e| ContractError::Std(StdError::generic_err(e.clone())))?;

    let attribute_value = response
        .events
        .iter()
        .flat_map(|e| &e.attributes)
        .find_map(|attr| (attr.key == attribute_key).then_some(&attr.value))
        .ok_or_else(|| {
            ContractError::Std(StdError::generic_err(format!(
                "{} attribute not found",
                attribute_key
            )))
        })?;

    if attribute_value.is_empty() {
        return Ok(vec![]);
    }

    attribute_value
        .split(", ") // Note: Hydro uses ", " separator
        .map(|s| s.trim().parse::<Coin>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| {
            ContractError::Std(StdError::generic_err(format!(
                "Failed to parse {} coin: {}",
                attribute_key, e
            )))
        })
}

// Now your original functions become:
fn parse_locks_skipped_reply(reply: &Reply) -> Result<Vec<u64>, ContractError> {
    parse_u64_list_from_reply(reply, "locks_skipped")
}

fn parse_unlocked_lock_ids_reply(reply: &Reply) -> Result<Vec<u64>, ContractError> {
    parse_u64_list_from_reply(reply, "unlocked_lock_ids")
}

fn parse_unlocked_token_from_reply(reply: &Reply) -> Result<Vec<Coin>, ContractError> {
    parse_coins_from_reply(&reply, "unlocked_tokens")
}

fn handle_unlock_tokens_reply(
    deps: DepsMut,
    env: Env,
    decommission_vessels_params: DecommissionVesselsParameters,
    hydro_unlocked_tokens: Vec<Coin>,
    unlocked_hydro_lock_ids: Vec<u64>,
) -> Result<Response, ContractError> {
    let previous_balances = decommission_vessels_params.previous_balances;

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
            join_u64_ids(unlocked_hydro_lock_ids),
        )
        .add_attribute(
            "owner",
            decommission_vessels_params.vessel_owner.to_string(),
        ))
}

// Complete time weighted shares for the hydromancer, for the current round
// Only needs to be called when a Hydromancer votes
fn complete_hydromancer_time_weighted_shares(
    deps: &mut DepsMut,
    hydromancer_id: u64,
    constants: &Constants,
    current_round_id: RoundId,
) -> Result<(), ContractError> {
    if !state::is_hydromancer_tws_complete(deps.storage, current_round_id, hydromancer_id) {
        return Ok(());
    }

    // Load all vessels for the hydromancer
    let vessels = state::get_vessels_by_hydromancer(deps.storage, hydromancer_id, 0, usize::MAX)?;

    // Query lockup shares for all hydromancer's vessels
    let lockups_shares_response = query_hydro_lockups_shares(
        &deps.as_ref(),
        &constants,
        vessels.iter().map(|v| v.hydro_lock_id).collect(),
    )?;

    for lockup_shares in lockups_shares_response.lockups_shares_info {
        state::save_vessel_shares_info(
            deps.storage,
            lockup_shares.lock_id,
            current_round_id,
            lockup_shares.time_weighted_shares.u128(),
            lockup_shares.token_group_id.clone(),
            lockup_shares.locked_rounds,
        )?;

        // Vessel has voting power
        if !lockup_shares.time_weighted_shares.is_zero() {
            state::add_time_weighted_shares_to_hydromancer(
                deps.storage,
                hydromancer_id,
                current_round_id,
                &lockup_shares.token_group_id,
                lockup_shares.locked_rounds,
                lockup_shares.time_weighted_shares.u128(),
            )?;
        }
    }

    // Mark as completed
    state::mark_hydromancer_tws_complete(deps.storage, current_round_id, hydromancer_id)?;

    Ok(())
}

fn query_hydro_lockups_shares(
    deps: &Deps,
    constants: &Constants,
    vessel_ids: Vec<u64>,
) -> Result<LockupsSharesResponse, StdError> {
    let lockups_shares: LockupsSharesResponse = deps
        .querier
        .query_wasm_smart(
            constants.hydro_config.hydro_contract_address.to_string(),
            &HydroQueryMsg::LockupsShares {
                lock_ids: vessel_ids.clone(),
            },
        )
        .map_err(|e| {
            StdError::generic_err(format!(
                "Failed to get time weighted shares for vessels {} from hydro : {}",
                join_u64_ids(vessel_ids),
                e
            ))
        })?;
    Ok(lockups_shares)
}

/// Initialize time weighted shares for vessels that don't have them yet.
/// For vessels controlled by hydromancers, also updates the hydromancer's TWS.
fn initialize_vessel_tws(
    deps: &mut DepsMut,
    lock_ids: Vec<u64>,
    current_round_id: RoundId,
    constants: &Constants,
) -> Result<(), ContractError> {
    // Filter out vessels that already have TWS initialized for this round
    let missing_lock_ids: Vec<u64> = lock_ids
        .into_iter()
        .filter(|&lock_id| !state::has_vessel_shares_info(deps.storage, current_round_id, lock_id))
        .collect();

    if missing_lock_ids.is_empty() {
        return Ok(());
    }

    // Query TWS data from Hydro contract for missing vessels
    let lockups_shares_response =
        query_hydro_lockups_shares(&deps.as_ref(), constants, missing_lock_ids)?;

    // Process each vessel's TWS data
    for lockup_info in &lockups_shares_response.lockups_shares_info {
        // Save vessel TWS info
        state::save_vessel_shares_info(
            deps.storage,
            lockup_info.lock_id,
            current_round_id,
            lockup_info.time_weighted_shares.u128(),
            lockup_info.token_group_id.clone(),
            lockup_info.locked_rounds,
        )?;

        // Update hydromancer TWS if vessel is controlled by one
        let vessel = state::get_vessel(deps.storage, lockup_info.lock_id)?;
        if let Some(hydromancer_id) = vessel.hydromancer_id {
            state::add_time_weighted_shares_to_hydromancer(
                deps.storage,
                hydromancer_id,
                current_round_id,
                &lockup_info.token_group_id,
                lockup_info.locked_rounds,
                lockup_info.time_weighted_shares.u128(),
            )?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use std::time::SystemTime;

    use cosmwasm_std::{
        coin, from_json,
        testing::{
            mock_dependencies as std_mock_dependencies, mock_env, MockApi,
            MockQuerier as StdMockQuerier, MockStorage,
        },
        to_json_binary, Addr, Binary, ContractResult, CosmosMsg, Decimal, DepsMut, Empty,
        GrpcQuery, MessageInfo, OwnedDeps, Querier, QuerierResult, QueryRequest, ReplyOn, StdError,
        Timestamp, Uint128, WasmMsg, WasmQuery,
    };
    use hydro_interface::msgs::{
        CollectionInfo, CurrentRoundResponse, ExecuteMsg as HydroExecuteMsg, HydroConstants,
        HydroConstantsResponse, HydroQueryMsg, LockEntryV2, LockEntryWithPower, LockPowerEntry,
        LockupWithPerTrancheInfo, LockupsSharesInfo, LockupsSharesResponse, PerTrancheLockupInfo,
        RoundLockPowerSchedule, SpecificUserLockupsResponse,
        SpecificUserLockupsWithTrancheInfosResponse, Tranche, TranchesResponse,
    };
    use neutron_std::types::ibc::applications::transfer::v1::{
        DenomTrace, QueryDenomTraceRequest, QueryDenomTraceResponse,
    };
    use prost::Message;
    use zephyrus_core::{
        msgs::{ExecuteMsg, VesselInfo},
        state::Vessel,
    };
    use zephyrus_core::{
        msgs::{InstantiateMsg, VesselsToHarbor},
        state::VesselHarbor,
    };

    use crate::{
        contract::VoteReplyPayload,
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
                    let response = to_json_binary(&HydroConstantsResponse {
                        constants: HydroConstants {
                            round_length: 1000,
                            lock_epoch_length: 1,
                            first_round_start: Timestamp::from_seconds(1000),
                            max_locked_tokens: 50_000,
                            known_users_cap: 250,
                            paused: false,
                            max_deployment_duration: 3,
                            round_lock_power_schedule: RoundLockPowerSchedule {
                                round_lock_power_schedule: vec![
                                    LockPowerEntry {
                                        locked_rounds: 1,
                                        power_scaling_factor: Decimal::one(),
                                    },
                                    LockPowerEntry {
                                        locked_rounds: 2,
                                        power_scaling_factor: Decimal::from_ratio(5u128, 4u128),
                                    },
                                    LockPowerEntry {
                                        locked_rounds: 3,
                                        power_scaling_factor: Decimal::from_ratio(3u128, 2u128),
                                    },
                                ],
                            },
                            cw721_collection_info: CollectionInfo {
                                name: "Test Collection".to_string(),
                                symbol: "TEST".to_string(),
                            },
                        },
                    })
                    .unwrap();
                    QuerierResult::Ok(ContractResult::Ok(response))
                }
                HydroQueryMsg::SpecificUserLockups { address, lock_ids } => {
                    let mut lockups_with_power: Vec<LockEntryWithPower> = vec![];
                    for lock_id in lock_ids {
                        lockups_with_power.push(LockEntryWithPower {
                            lock_entry: LockEntryV2 {
                                lock_id,
                                owner: Addr::unchecked(address.clone()),
                                funds: coin(1000u128, "uatom"),
                                lock_start: Timestamp::from_seconds(1000),
                                lock_end: Timestamp::from_seconds(2000),
                            },
                            current_voting_power: Uint128::from(1000u128),
                        });
                    }
                    let response = to_json_binary(&SpecificUserLockupsResponse {
                        lockups: lockups_with_power,
                    })
                    .unwrap();
                    QuerierResult::Ok(ContractResult::Ok(response))
                }
                HydroQueryMsg::LockupsShares { lock_ids } => {
                    let mut shares_info: Vec<LockupsSharesInfo> = vec![];
                    for lock_id in lock_ids {
                        shares_info.push(LockupsSharesInfo {
                            lock_id,
                            time_weighted_shares: Uint128::from(1000u128),
                            token_group_id: "dAtom".to_string(),
                            locked_rounds: 1,
                        });
                    }
                    let response = to_json_binary(&LockupsSharesResponse {
                        lockups_shares_info: shares_info,
                    })
                    .unwrap();
                    QuerierResult::Ok(ContractResult::Ok(response))
                }
                HydroQueryMsg::Tranches {} => {
                    let response = to_json_binary(&TranchesResponse {
                        tranches: vec![Tranche {
                            id: 1,
                            name: "Atom".to_string(),
                            metadata: "".to_string(),
                        }],
                    })
                    .unwrap();
                    QuerierResult::Ok(ContractResult::Ok(response))
                }
                HydroQueryMsg::SpecificUserLockupsWithTrancheInfos {
                    address: _,
                    lock_ids,
                } => {
                    let mut lockup_tranche_infos: Vec<LockupWithPerTrancheInfo> = vec![];
                    for lock_id in lock_ids {
                        let mut per_tranche_infos: Vec<PerTrancheLockupInfo> = vec![];
                        per_tranche_infos.push(PerTrancheLockupInfo {
                            tranche_id: 1,
                            next_round_lockup_can_vote: 2,
                            current_voted_on_proposal: None,
                            tied_to_proposal: None,
                            historic_voted_on_proposals: vec![],
                        });
                        lockup_tranche_infos.push(LockupWithPerTrancheInfo {
                            lock_with_power: LockEntryWithPower {
                                lock_entry: LockEntryV2 {
                                    lock_id,
                                    owner: make_valid_addr("owner"),
                                    funds: coin(1000u128, "uatom"),
                                    lock_start: Timestamp::from_seconds(1000),
                                    lock_end: Timestamp::from_seconds(2000),
                                },
                                current_voting_power: Uint128::from(1000u128),
                            },
                            per_tranche_info: per_tranche_infos,
                        });
                    }
                    let response = to_json_binary(&SpecificUserLockupsWithTrancheInfosResponse {
                        lockups_with_per_tranche_infos: vec![],
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
    fn hydromancer_vote_with_vessel_controlled_other_hydromancer_fail() {
        let mut deps = mock_dependencies();

        init_contract(deps.as_mut());

        let alice_address = make_valid_addr("alice");
        let user_id = state::insert_new_user(deps.as_mut().storage, alice_address.clone())
            .expect("Should add user");

        let hydromancer_address = make_valid_addr("hydromancer");

        state::insert_new_hydromancer(
            deps.as_mut().storage,
            hydromancer_address.clone(),
            "hydromancer 1".to_string(),
            Decimal::percent(10),
        )
        .expect("Should add hydromancer");

        state::add_vessel(
            deps.as_mut().storage,
            &Vessel {
                hydro_lock_id: 0,
                tokenized_share_record_id: None,
                class_period: 12,
                auto_maintenance: true,
                hydromancer_id: Some(0), // Default hydromancer (not the one created above)
                owner_id: user_id,
            },
            &alice_address,
        )
        .expect("Should add vessel");

        // Hydromancer 1 tries to vote with a vessel that is controlled by Zephyrus (hydromancer 0)
        let result = super::execute_hydromancer_vote(
            deps.as_mut(),
            MessageInfo {
                sender: hydromancer_address.clone(),
                funds: vec![],
            },
            1,
            vec![{
                VesselsToHarbor {
                    harbor_id: 1,
                    vessel_ids: vec![0],
                }
            }],
        );
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ContractError::Unauthorized);
    }

    #[test]
    fn hydromancer_vote_with_vessel_under_user_control_fail() {
        let mut deps = mock_dependencies();

        init_contract(deps.as_mut());

        let alice_address = make_valid_addr("alice");
        let user_id = state::insert_new_user(deps.as_mut().storage, alice_address.clone())
            .expect("Should add user");

        let default_hydromancer_id = state::get_constants(deps.as_mut().storage)
            .unwrap()
            .default_hydromancer_id;
        let default_hydromancer_address =
            state::get_hydromancer(deps.as_mut().storage, default_hydromancer_id)
                .unwrap()
                .address;

        state::add_vessel(
            deps.as_mut().storage,
            &Vessel {
                hydro_lock_id: 0,
                tokenized_share_record_id: None,
                class_period: 12,
                auto_maintenance: true,
                hydromancer_id: None, // under user control
                owner_id: user_id,
            },
            &alice_address,
        )
        .expect("Should add vessel");

        // Hydromancer 1 tries to vote with a vessel that is controlled by Zephyrus (hydromancer 0)
        let result = super::execute_hydromancer_vote(
            deps.as_mut(),
            MessageInfo {
                sender: default_hydromancer_address,
                funds: vec![],
            },
            1,
            vec![{
                VesselsToHarbor {
                    harbor_id: 1,
                    vessel_ids: vec![0],
                }
            }],
        );
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ContractError::Unauthorized);
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
                hydromancer_id: Some(default_hydromancer_id),
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
        //vote should be skipped so harbor1 should not have vessels
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
                hydromancer_id: Some(default_hydromancer_id),
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
        let constants = state::get_constants(deps.as_mut().storage).unwrap();
        let alice_address = make_valid_addr("alice");
        state::insert_new_user(deps.as_mut().storage, alice_address.clone())
            .expect("Should add user");
        let default_hydromancer_id = state::get_constants(deps.as_mut().storage)
            .unwrap()
            .default_hydromancer_id;

        let receive_msg = ExecuteMsg::ReceiveNft(zephyrus_core::msgs::Cw721ReceiveMsg {
            sender: alice_address.to_string(),
            token_id: "0".to_string(),
            msg: to_json_binary(&VesselInfo {
                owner: alice_address.to_string(),
                auto_maintenance: true,
                hydromancer_id: default_hydromancer_id,
                class_period: 3,
            })
            .unwrap(),
        });
        // Create a vessel simulating the nft reveive
        let result = super::execute(
            deps.as_mut(),
            mock_env(),
            MessageInfo {
                sender: constants.hydro_config.hydro_contract_address.clone(),
                funds: vec![],
            },
            receive_msg,
        );
        assert!(result.is_ok());

        // Simulate hydromancer vote with vessel
        let msg_vote_hydromancer = ExecuteMsg::HydromancerVote {
            tranche_id: 1,
            vessels_harbors: vec![VesselsToHarbor {
                harbor_id: 2,
                vessel_ids: vec![0],
            }],
        };
        let hydromancer =
            state::get_hydromancer(deps.as_mut().storage, constants.default_hydromancer_id)
                .unwrap();

        let result = super::execute(
            deps.as_mut(),
            mock_env(),
            MessageInfo {
                sender: hydromancer.address.clone(),
                funds: vec![],
            },
            msg_vote_hydromancer,
        );
        assert!(result.is_ok());
        let result = result.unwrap();

        let payload = VoteReplyPayload {
            tranche_id: 1,
            round_id: 1,
            user_vote: false,
            steerer_id: default_hydromancer_id,
            vessels_harbors: vec![{
                VesselsToHarbor {
                    harbor_id: 2,
                    vessel_ids: vec![0],
                }
            }],
        };

        let _ = super::handle_vote_reply(deps.as_mut(), payload, vec![]).unwrap();

        assert_eq!(result.messages.len(), 1);
        let msg_vote_hydromancer = ExecuteMsg::HydromancerVote {
            tranche_id: 1,
            vessels_harbors: vec![VesselsToHarbor {
                harbor_id: 1,
                vessel_ids: vec![0],
            }],
        };

        let result = super::execute(
            deps.as_mut(),
            mock_env(),
            MessageInfo {
                sender: hydromancer.address.clone(),
                funds: vec![],
            },
            msg_vote_hydromancer,
        );
        assert!(result.is_ok());
        let decoded_submessages: Vec<HydroExecuteMsg> = result
            .unwrap()
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
        let alice_user_id = state::insert_new_user(deps.as_mut().storage, alice_address.clone())
            .expect("Should add user");

        let bob_address = make_valid_addr("bob");
        state::insert_new_user(deps.as_mut().storage, bob_address.clone())
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
                hydromancer_id: Some(default_hydromancer_id),
                owner_id: alice_user_id,
            },
            &alice_address,
        )
        .expect("Should add vessel");

        let result = super::execute_user_vote(
            deps.as_mut(),
            MessageInfo {
                sender: bob_address.clone(),
                funds: vec![],
            },
            1,
            vec![{
                VesselsToHarbor {
                    harbor_id: 1,
                    vessel_ids: vec![0],
                }
            }],
        );
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ContractError::Unauthorized);
    }

    #[test]
    fn user_new_vote_succeed() {
        let mut deps = mock_dependencies();

        init_contract(deps.as_mut());
        let constants = state::get_constants(deps.as_mut().storage).unwrap();
        let alice_address = make_valid_addr("alice");
        let user_id = state::insert_new_user(deps.as_mut().storage, alice_address.clone())
            .expect("Should add user");
        let default_hydromancer_id = state::get_constants(deps.as_mut().storage)
            .unwrap()
            .default_hydromancer_id;

        let receive_msg = ExecuteMsg::ReceiveNft(zephyrus_core::msgs::Cw721ReceiveMsg {
            sender: alice_address.to_string(),
            token_id: "0".to_string(),
            msg: to_json_binary(&VesselInfo {
                owner: alice_address.to_string(),
                auto_maintenance: true,
                hydromancer_id: default_hydromancer_id,
                class_period: 3,
            })
            .unwrap(),
        });
        // Create a vessel simulating the nft reveive
        let result = super::execute(
            deps.as_mut(),
            mock_env(),
            MessageInfo {
                sender: constants.hydro_config.hydro_contract_address.clone(),
                funds: vec![],
            },
            receive_msg,
        );
        assert!(result.is_ok());

        let take_control_msg = ExecuteMsg::TakeControl {
            vessel_ids: vec![0],
        };
        let result = super::execute(
            deps.as_mut(),
            mock_env(),
            MessageInfo {
                sender: alice_address.clone(),
                funds: vec![],
            },
            take_control_msg,
        );
        assert!(result.is_ok());

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

        let constants = state::get_constants(deps.as_mut().storage).unwrap();

        let alice_address = make_valid_addr("alice");
        let user_id = state::insert_new_user(deps.as_mut().storage, alice_address.clone())
            .expect("Should add user");

        let default_hydromancer_id = state::get_constants(deps.as_mut().storage)
            .unwrap()
            .default_hydromancer_id;
        let default_hydromancer =
            state::get_hydromancer(deps.as_mut().storage, constants.default_hydromancer_id)
                .unwrap();

        let receive_msg = ExecuteMsg::ReceiveNft(zephyrus_core::msgs::Cw721ReceiveMsg {
            sender: alice_address.to_string(),
            token_id: "0".to_string(),
            msg: to_json_binary(&VesselInfo {
                owner: alice_address.to_string(),
                auto_maintenance: true,
                hydromancer_id: default_hydromancer_id,
                class_period: 3,
            })
            .unwrap(),
        });
        // Create a vessel simulating the nft reveive
        let result = super::execute(
            deps.as_mut(),
            mock_env(),
            MessageInfo {
                sender: constants.hydro_config.hydro_contract_address.clone(),
                funds: vec![],
            },
            receive_msg,
        );
        assert!(result.is_ok());

        // Simulate hydromancer vote with vessel
        let msg_vote_hydromancer = ExecuteMsg::HydromancerVote {
            tranche_id: 1,
            vessels_harbors: vec![VesselsToHarbor {
                harbor_id: 1,
                vessel_ids: vec![0],
            }],
        };

        let result = super::execute(
            deps.as_mut(),
            mock_env(),
            MessageInfo {
                sender: default_hydromancer.address.clone(),
                funds: vec![],
            },
            msg_vote_hydromancer,
        );
        assert!(result.is_ok());

        let take_control_msg = ExecuteMsg::TakeControl {
            vessel_ids: vec![0],
        };
        let result = super::execute(
            deps.as_mut(),
            mock_env(),
            MessageInfo {
                sender: alice_address.clone(),
                funds: vec![],
            },
            take_control_msg,
        );
        assert!(result.is_ok());

        let user_vote_msg = ExecuteMsg::UserVote {
            tranche_id: 1,
            vessels_harbors: vec![VesselsToHarbor {
                harbor_id: 1,
                vessel_ids: vec![0],
            }],
        };

        let res = super::execute(
            deps.as_mut(),
            mock_env(),
            MessageInfo {
                sender: alice_address.clone(),
                funds: vec![],
            },
            user_vote_msg,
        );
        assert!(res.is_ok());
        let res = res.unwrap();
        assert_eq!(res.messages.len(), 1);

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
                mock_env(),
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
                hydromancer_id: Some(default_hydromancer_id),
                owner_id: user_id,
            },
            &alice_address,
        )
        .expect("Should add vessel");

        assert_eq!(
            super::execute_change_hydromancer(
                deps.as_mut(),
                mock_env(),
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
                hydromancer_id: Some(default_hydromancer_id),
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
                hydromancer_id: Some(default_hydromancer_id),
                owner_id: bob_id,
            },
            &bob_address,
        )
        .expect("Should add vessel");

        assert_eq!(
            super::execute_change_hydromancer(
                deps.as_mut(),
                mock_env(),
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
        let alice_user_id = state::insert_new_user(deps.as_mut().storage, alice_address.clone())
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
                hydromancer_id: Some(default_hydromancer_id),
                owner_id: alice_user_id,
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
        .expect("Hydromancer should be added!");

        let res = super::execute_change_hydromancer(
            deps.as_mut(),
            mock_env(),
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
        assert_eq!(vessel.hydromancer_id.unwrap(), new_hydromancer_id);
    }

    #[test]
    fn change_hydromancer_1_vessels_already_vote_success() {
        let mut deps = mock_dependencies();

        init_contract(deps.as_mut());
        let constants = state::get_constants(deps.as_mut().storage).unwrap();
        let alice_address = make_valid_addr("alice");

        state::insert_new_user(deps.as_mut().storage, alice_address.clone())
            .expect("Should add user");

        let default_hydromancer_id = state::get_constants(deps.as_mut().storage)
            .unwrap()
            .default_hydromancer_id;

        let receive_msg = ExecuteMsg::ReceiveNft(zephyrus_core::msgs::Cw721ReceiveMsg {
            sender: alice_address.to_string(),
            token_id: "0".to_string(),
            msg: to_json_binary(&VesselInfo {
                owner: alice_address.to_string(),
                auto_maintenance: true,
                hydromancer_id: default_hydromancer_id,
                class_period: 3,
            })
            .unwrap(),
        });
        // Create a vessel simulating the nft reveive
        let result = super::execute(
            deps.as_mut(),
            mock_env(),
            MessageInfo {
                sender: constants.hydro_config.hydro_contract_address.clone(),
                funds: vec![],
            },
            receive_msg,
        );
        assert!(result.is_ok());

        // Simulate hydromancer vote with vessel
        let msg_vote_hydromancer = ExecuteMsg::HydromancerVote {
            tranche_id: 1,
            vessels_harbors: vec![VesselsToHarbor {
                harbor_id: 1,
                vessel_ids: vec![0],
            }],
        };
        let hydromancer =
            state::get_hydromancer(deps.as_mut().storage, constants.default_hydromancer_id)
                .unwrap();

        let result = super::execute(
            deps.as_mut(),
            mock_env(),
            MessageInfo {
                sender: hydromancer.address.clone(),
                funds: vec![],
            },
            msg_vote_hydromancer,
        );
        assert!(result.is_ok());

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
            mock_env(),
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
        assert_eq!(vessel.hydromancer_id.unwrap(), new_hydromancer_id);

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
        let constants = state::get_constants(deps.as_mut().storage).unwrap();
        let alice_address = make_valid_addr("alice");

        state::insert_new_user(deps.as_mut().storage, alice_address.clone())
            .expect("Should add user");

        let default_hydromancer_id = state::get_constants(deps.as_mut().storage)
            .unwrap()
            .default_hydromancer_id;

        let receive_msg = ExecuteMsg::ReceiveNft(zephyrus_core::msgs::Cw721ReceiveMsg {
            sender: alice_address.to_string(),
            token_id: "0".to_string(),
            msg: to_json_binary(&VesselInfo {
                owner: alice_address.to_string(),
                auto_maintenance: true,
                hydromancer_id: default_hydromancer_id,
                class_period: 3,
            })
            .unwrap(),
        });
        // Create a vessel simulating the nft reveive
        let result = super::execute(
            deps.as_mut(),
            mock_env(),
            MessageInfo {
                sender: constants.hydro_config.hydro_contract_address.clone(),
                funds: vec![],
            },
            receive_msg,
        );
        assert!(result.is_ok());

        let take_control_msg = ExecuteMsg::TakeControl {
            vessel_ids: vec![0],
        };
        let result = super::execute(
            deps.as_mut(),
            mock_env(),
            MessageInfo {
                sender: alice_address.clone(),
                funds: vec![],
            },
            take_control_msg,
        );
        assert!(result.is_ok());

        let user_vote_msg = ExecuteMsg::UserVote {
            tranche_id: 1,
            vessels_harbors: vec![VesselsToHarbor {
                harbor_id: 1,
                vessel_ids: vec![0],
            }],
        };

        let res = super::execute(
            deps.as_mut(),
            mock_env(),
            MessageInfo {
                sender: alice_address.clone(),
                funds: vec![],
            },
            user_vote_msg,
        );
        assert!(res.is_ok());
        let res = super::execute_change_hydromancer(
            deps.as_mut(),
            mock_env(),
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
        assert_eq!(vessel.hydromancer_id.unwrap(), default_hydromancer_id);

        assert!(
            state::get_vessel_to_harbor_by_harbor_id(deps.as_ref().storage, 1, 1, 1)
                .unwrap()
                .is_empty()
        );
        assert!(!state::is_vessel_used_under_user_control(
            deps.as_ref().storage,
            1,
            1,
            0
        ))
    }
}
