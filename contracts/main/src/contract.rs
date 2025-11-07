use std::collections::{BTreeSet, HashMap, HashSet};

use cosmwasm_std::{
    entry_point, from_json, to_json_binary, Addr, Binary, Coin, Decimal, DepsMut, Env, MessageInfo,
    Response as CwResponse, StdError, StdResult, SubMsg, WasmMsg,
};
use hydro_interface::msgs::{ExecuteMsg as HydroExecuteMsg, ProposalToLockups, TributeClaim};
use neutron_sdk::bindings::msg::NeutronMsg;
use zephyrus_core::{
    msgs::{
        DecommissionVesselsReplyPayload, ExecuteMsg, InstantiateMsg, MigrateMsg,
        RefreshTimeWeightedSharesReplyPayload, TrancheId, VesselInfo, VesselsToHarbor,
        VoteReplyPayload, DECOMMISSION_REPLY_ID, REFRESH_TIME_WEIGHTED_SHARES_REPLY_ID,
        VOTE_REPLY_ID,
    },
    state::{Constants, HydroConfig, HydroLockId, Vessel},
};

use crate::{
    errors::ContractError,
    helpers::{
        auto_maintenance::{
            check_has_more_vessels_needing_maintenance, collect_vessels_needing_auto_maintenance,
        },
        hydro_queries::{
            query_hydro_constants, query_hydro_current_round, query_hydro_lockups_shares,
            query_hydro_lockups_with_tranche_infos, query_hydro_specific_tributes,
            query_hydro_specific_user_lockups, query_hydro_tranches,
        },
        rewards::{
            build_claim_tribute_sub_msg,
            distribute_rewards_for_all_tributes_already_claimed_on_hydro,
            get_current_balances_for_outstanding_tributes_denoms,
        },
        tws::{
            complete_hydromancer_time_weighted_shares, initialize_vessel_tws, reset_vessel_vote,
        },
        validation::{
            validate_admin_address, validate_contract_is_not_paused, validate_contract_is_paused,
            validate_hydromancer_controls_vessels, validate_hydromancer_exists,
            validate_lock_duration, validate_round_tranche_consistency,
            validate_user_controls_vessel, validate_user_owns_vessels,
            validate_vessels_not_tied_to_proposal, validate_vote_duplicates,
        },
        vectors::join_u64_ids,
        vessel_assignment::{
            assign_vessel_to_hydromancer, assign_vessel_to_user_control,
            categorize_vessels_by_control,
        },
    },
    state,
};

type Response = CwResponse<NeutronMsg>;

const WHITELIST_ADMINS_MAX_COUNT: usize = 50;

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    if msg.whitelist_admins.is_empty() {
        return Err(ContractError::WhitelistAdminsMustBeProvided);
    }
    state::initialize_sequences(deps.storage)?;

    let mut whitelist_admins: Vec<Addr> = vec![];
    for admin in msg.whitelist_admins {
        let admin_addr = deps.api.addr_validate(&admin)?;
        if !whitelist_admins.contains(&admin_addr) {
            whitelist_admins.push(admin_addr.clone());
        }
    }
    if whitelist_admins.len() > WHITELIST_ADMINS_MAX_COUNT {
        return Err(ContractError::WhitelistAdminsMaxCountExceeded {});
    }
    state::update_whitelist_admins(deps.storage, whitelist_admins)?;
    let hydro_config = HydroConfig {
        hydro_contract_address: deps.api.addr_validate(&msg.hydro_contract_address)?,
        hydro_tribute_contract_address: deps.api.addr_validate(&msg.tribute_contract_address)?,
    };

    let hydromancer_address = deps.api.addr_validate(&msg.default_hydromancer_address)?;
    let commission_recipient = deps.api.addr_validate(&msg.commission_recipient)?;
    // Validate commission rate is less than 0.5 (50%)
    let max_commission_rate: Decimal = Decimal::from_ratio(50_u128, 100_u128);
    if msg.commission_rate >= max_commission_rate
        || msg.default_hydromancer_commission_rate >= max_commission_rate
    {
        return Err(ContractError::CommissionRateMustBeLessThan100 {});
    }
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
        commission_rate: msg.commission_rate,
        commission_recipient,
        min_tokens_per_vessel: msg.min_tokens_per_vessel,
    };

    state::update_constants(deps.storage, constant)?;

    Ok(Response::default())
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::AutoMaintain {
            start_from_vessel_id,
            limit,
            class_period,
        } => execute_auto_maintain(deps, info, start_from_vessel_id, limit, class_period),
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
        ExecuteMsg::Unvote {
            tranche_id,
            vessel_ids,
        } => execute_unvote(deps, info, tranche_id, vessel_ids),
        ExecuteMsg::Claim {
            round_id,
            tranche_id,
            vessel_ids,
            tribute_ids,
        } => execute_claim(
            deps,
            env,
            info,
            round_id,
            tranche_id,
            vessel_ids,
            tribute_ids,
        ),
        ExecuteMsg::UpdateCommissionRate {
            new_commission_rate,
        } => execute_update_commission_rate(deps, info, new_commission_rate),
        ExecuteMsg::UpdateCommissionRecipient {
            new_commission_recipient,
        } => execute_update_commission_recipient(deps, info, new_commission_recipient),
        ExecuteMsg::SetAdminAddresses { admins } => execute_set_admin_addresses(deps, info, admins),
    }
}

fn execute_set_admin_addresses(
    deps: DepsMut,
    info: MessageInfo,
    admins: Vec<String>,
) -> Result<Response, ContractError> {
    let constants = state::get_constants(deps.storage)?;
    validate_contract_is_not_paused(&constants)?;
    validate_admin_address(deps.storage, &info.sender)?;
    let new_whitelist_admins: HashSet<Addr> = admins
        .into_iter()
        .map(|admin| deps.api.addr_validate(&admin))
        .collect::<Result<HashSet<Addr>, StdError>>()?;

    if new_whitelist_admins.len() > WHITELIST_ADMINS_MAX_COUNT {
        return Err(ContractError::WhitelistAdminsMaxCountExceeded {});
    }

    let old_whitelist_admins = state::get_whitelist_admins(deps.storage)?;
    let old_whitelist_admins_set: HashSet<Addr> = old_whitelist_admins.into_iter().collect();
    if new_whitelist_admins.is_disjoint(&old_whitelist_admins_set) {
        return Err(ContractError::CannotReplaceAllAdmins {});
    }

    state::update_whitelist_admins(deps.storage, new_whitelist_admins.into_iter().collect())?;
    Ok(Response::default().add_attribute("action", "set_admin_addresses"))
}

fn execute_update_commission_rate(
    deps: DepsMut,
    info: MessageInfo,
    new_commission_rate: Decimal,
) -> Result<Response, ContractError> {
    validate_admin_address(deps.storage, &info.sender)?;

    // Validate new commission rate is less than 1 (100%)
    if new_commission_rate > Decimal::one() {
        return Err(ContractError::CustomError {
            msg: "Commission rate must be less than 1 (100%)".to_string(),
        });
    }

    let mut constants = state::get_constants(deps.storage)?;
    constants.commission_rate = new_commission_rate;
    state::update_constants(deps.storage, constants)?;
    Ok(Response::default()
        .add_attribute("action", "change_commission_rate")
        .add_attribute("new_commission_rate", new_commission_rate.to_string()))
}

fn execute_update_commission_recipient(
    deps: DepsMut,
    info: MessageInfo,
    new_commission_recipient: String,
) -> Result<Response, ContractError> {
    validate_admin_address(deps.storage, &info.sender)?;

    let commission_recipient = deps.api.addr_validate(&new_commission_recipient)?;
    let mut constants = state::get_constants(deps.storage)?;
    constants.commission_recipient = commission_recipient;
    state::update_constants(deps.storage, constants)?;

    Ok(Response::default()
        .add_attribute("action", "change_commission_recipient")
        .add_attribute("new_commission_recipient", new_commission_recipient))
}

fn execute_claim(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    round_id: u64,
    tranche_id: u64,
    vessel_ids: Vec<u64>,
    tribute_ids: Vec<u64>,
) -> Result<Response, ContractError> {
    let constants = state::get_constants(deps.storage)?;
    validate_contract_is_not_paused(&constants)?;
    validate_user_owns_vessels(deps.storage, &info.sender, &vessel_ids)?;

    let contract_address = env.contract.address.clone();
    // remove duplicates ids
    let tribute_ids: HashSet<u64> = tribute_ids.into_iter().collect();

    let tributes = query_hydro_specific_tributes(
        &deps.as_ref(),
        &constants,
        tribute_ids.clone().into_iter().collect(),
    )?;
    // Validate round and tranche consistency, if round_id is not the same as the round_id in the tributes, return an error
    validate_round_tranche_consistency(&tributes.tributes, round_id, tranche_id)?;
    let mut outstanding_tributes = Vec::new();
    let mut tributes_processed = Vec::new();
    for tribute in tributes.tributes {
        if state::is_tribute_processed(deps.storage, tribute.tribute_id) {
            tributes_processed.push(tribute);
        } else {
            outstanding_tributes.push(tribute);
        }
    }

    let mut response = Response::new().add_attribute("action", "claim");

    // Note: We still need to process, even if we found 0 outstanding tributes to claim,
    // because they may have already been claimed previously
    response = process_outstanding_tribute_claims(
        deps.branch(),
        info,
        round_id,
        tranche_id,
        vessel_ids.clone(),
        &constants,
        &contract_address,
        tributes_processed.clone(),
        outstanding_tributes.clone(),
        response,
    )?;

    // Clear temporary distribution tracking data after successful batch completion
    state::clear_distribution_tracking(deps.storage)?;

    Ok(response
        .add_attribute("action", "claim")
        .add_attribute("round_id", round_id.to_string())
        .add_attribute("tranche_id", tranche_id.to_string())
        .add_attribute("vessel_ids", join_u64_ids(&vessel_ids))
        .add_attribute("tribute_ids", join_u64_ids(&tribute_ids))
        .add_attribute("tributes_processed", tributes_processed.len().to_string())
        .add_attribute(
            "hydro_outstanding_tributes",
            outstanding_tributes.len().to_string(),
        ))
}

#[allow(clippy::too_many_arguments)]
fn process_outstanding_tribute_claims(
    mut deps: DepsMut,
    info: MessageInfo,
    round_id: u64,
    tranche_id: u64,
    vessel_ids: Vec<u64>,
    constants: &Constants,
    contract_address: &Addr,
    tributes_already_claimed_on_hydro: Vec<TributeClaim>,
    outstanding_tributes: Vec<TributeClaim>,
    mut response: Response,
) -> Result<Response, ContractError> {
    let mut tributes_process_in_reply = BTreeSet::new();
    // To prevent denial of service on balance queries, we get only the current balances for the denoms of the outstanding tributes
    let mut balances = get_current_balances_for_outstanding_tributes_denoms(
        &deps,
        contract_address,
        &outstanding_tributes,
    )?;

    for outstanding_tribute in outstanding_tributes {
        let sub_msg = build_claim_tribute_sub_msg(
            round_id,
            tranche_id,
            &vessel_ids,
            &info.sender,
            constants,
            contract_address,
            &balances,
            &outstanding_tribute,
        )?;
        tributes_process_in_reply.insert(outstanding_tribute.tribute_id);

        response = response.add_submessage(sub_msg);

        // Update virtual balances for checking purposes
        if let Some(balance) = balances
            .iter_mut()
            .find(|balance| balance.denom == outstanding_tribute.amount.denom)
        {
            // balance found, add to the balance
            balance.amount = balance
                .amount
                .checked_add(outstanding_tribute.amount.amount)
                .map_err(|e| ContractError::Std(e.into()))?;
        } else {
            // balance not found, add it
            balances.push(outstanding_tribute.amount.clone());
        }
    }
    let messages = distribute_rewards_for_all_tributes_already_claimed_on_hydro(
        deps.branch(),
        info.sender.clone(),
        round_id,
        vessel_ids,
        constants.clone(),
        tributes_already_claimed_on_hydro,
    )?;

    Ok(response.add_messages(messages))
}

fn execute_unvote(
    deps: DepsMut,
    info: MessageInfo,
    tranche_id: u64,
    vessel_ids: Vec<u64>,
) -> Result<Response, ContractError> {
    let constants = state::get_constants(deps.storage)?;
    validate_contract_is_not_paused(&constants)?;

    let current_round_id = query_hydro_current_round(&deps.as_ref(), &constants)?;
    let user_addr = info.sender;
    for vessel_id in vessel_ids.iter() {
        let vessel = state::get_vessel(deps.storage, *vessel_id)?;
        validate_user_controls_vessel(deps.storage, user_addr.clone(), vessel.clone())?;

        if let Some(proposal_id) =
            state::get_harbor_of_vessel(deps.storage, tranche_id, current_round_id, *vessel_id)?
        {
            reset_vessel_vote(
                deps.storage,
                vessel,
                current_round_id,
                tranche_id,
                proposal_id,
            )?;
        }
    }
    let msg_unvote = HydroExecuteMsg::Unvote {
        tranche_id,
        lock_ids: vessel_ids.clone(),
    };
    let execute_unvote_msg = WasmMsg::Execute {
        contract_addr: constants.hydro_config.hydro_contract_address.to_string(),
        msg: to_json_binary(&msg_unvote)?,
        funds: vec![],
    };

    Ok(Response::default()
        .add_message(execute_unvote_msg)
        .add_attribute("action", "unvote"))
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
    if !state::hydromancer_exists(deps.storage, vessel_info.hydromancer_id)? {
        return Err(ContractError::HydromancerNotFound {
            identifier: vessel_info.hydromancer_id.to_string(),
        });
    }

    // 4. Check that class_period represents a valid lock duration
    let constant_response = query_hydro_constants(&deps.as_ref(), &constants)?;
    validate_lock_duration(
        &constant_response.constants.round_lock_power_schedule,
        constant_response.constants.lock_epoch_length,
        vessel_info.class_period,
    )?;

    // 5. Check that we are owner of the lockup (as transfer happens before calling Zephyrus' Cw721ReceiveMsg)
    let user_specific_lockups =
        query_hydro_specific_user_lockups(&deps.as_ref(), &env, &constants, vec![hydro_lock_id])?;
    if user_specific_lockups.lockups.is_empty() {
        return Err(ContractError::LockupNotOwned {
            id: token_id.to_string(),
        });
    }

    if user_specific_lockups.lockups[0]
        .lock_entry
        .funds
        .amount
        .u128()
        < constants.min_tokens_per_vessel
    {
        return Err(ContractError::CustomError {
            msg: format!(
                "Insufficient deposit. Minimum required: {}",
                constants.min_tokens_per_vessel
            ),
        });
    }

    // 6. Owner could be a new user, so we need to insert it in state
    let owner_id = state::get_user_id(deps.storage, &owner_addr)
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

    let lockup_info_response =
        query_hydro_lockups_shares(&deps.as_ref(), &constants, vec![hydro_lock_id])?;

    let lockup_info = &lockup_info_response.lockups_shares_info[0];
    let current_time_weighted_shares = lockup_info.time_weighted_shares.u128();
    let token_group_id = &lockup_info.token_group_id;
    let locked_rounds = lockup_info.locked_rounds;

    // Always save vessel shares info
    state::save_vessel_info_snapshot(
        deps.storage,
        vessel.hydro_lock_id,
        current_round,
        current_time_weighted_shares,
        token_group_id.clone(),
        locked_rounds,
        Some(vessel_info.hydromancer_id),
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
const DEFAULT_AUTO_MAINTAIN_LIMIT: usize = 50;
fn execute_auto_maintain(
    deps: DepsMut,
    _info: MessageInfo,
    start_from_vessel_id: Option<u64>,
    limit: Option<usize>,
    class_period: u64,
) -> Result<Response, ContractError> {
    let constants = state::get_constants(deps.storage)?;
    validate_contract_is_not_paused(&constants)?;

    let current_round_id = query_hydro_current_round(&deps.as_ref(), &constants)?;
    let hydro_constants_response = query_hydro_constants(&deps.as_ref(), &constants)?;
    let lock_epoch_length = hydro_constants_response.constants.lock_epoch_length;
    let max_vessels = limit.unwrap_or(DEFAULT_AUTO_MAINTAIN_LIMIT);

    // Collect all vessels that need auto-maintenance, sorted by vessel ID
    let vessels_needing_maintenance = collect_vessels_needing_auto_maintenance(
        deps.storage,
        current_round_id,
        start_from_vessel_id,
        max_vessels,
        lock_epoch_length,
        class_period,
    )?;

    if vessels_needing_maintenance.is_empty() {
        return Err(ContractError::NoVesselsToAutoMaintain {});
    }

    // Group vessels by their target class period for efficient batch processing
    let mut vessels_by_class: HashMap<u64, Vec<HydroLockId>> = HashMap::new();
    for (vessel_id, target_class_period) in &vessels_needing_maintenance {
        vessels_by_class
            .entry(*target_class_period)
            .or_default()
            .push(*vessel_id);
    }

    let mut response = Response::new().add_attribute("action", "auto_maintain");
    let mut total_vessels_processed = 0;
    let last_processed_vessel_id = vessels_needing_maintenance
        .last()
        .map(|(id, _)| *id)
        .ok_or(ContractError::NoVesselsToAutoMaintain {})?;

    // Process each class period batch
    for (target_class_period, vessel_ids) in &vessels_by_class {
        // Create refresh lock duration message for Hydro contract
        let refresh_duration_msg = HydroExecuteMsg::RefreshLockDuration {
            lock_ids: vessel_ids.clone(),
            lock_duration: *target_class_period,
        };

        let execute_refresh_msg = WasmMsg::Execute {
            contract_addr: constants.hydro_config.hydro_contract_address.to_string(),
            msg: to_json_binary(&refresh_duration_msg)?,
            funds: vec![],
        };

        // Create payload for reply handler
        let refresh_payload = RefreshTimeWeightedSharesReplyPayload {
            vessel_ids: vessel_ids.clone(),
            target_class_period: *target_class_period,
            current_round_id,
        };

        // Use SubMsg with reply to handle TWS updates after successful refresh
        let refresh_submsg =
            SubMsg::reply_on_success(execute_refresh_msg, REFRESH_TIME_WEIGHTED_SHARES_REPLY_ID)
                .with_payload(to_json_binary(&refresh_payload)?);

        response = response.add_submessage(refresh_submsg).add_attribute(
            format!("class_period_{}", target_class_period),
            join_u64_ids(vessel_ids),
        );

        total_vessels_processed += vessel_ids.len();
    }

    // Add pagination info
    response = response.add_attribute(
        "last_processed_vessel_id",
        last_processed_vessel_id.to_string(),
    );

    // Check if there are more vessels to process
    let has_more_vessels = check_has_more_vessels_needing_maintenance(
        deps.storage,
        current_round_id,
        last_processed_vessel_id,
        lock_epoch_length,
    )?;

    response = response.add_attribute("has_more", has_more_vessels.to_string());

    Ok(response
        .add_attribute(
            "total_vessels_processed",
            total_vessels_processed.to_string(),
        )
        .add_attribute(
            "class_periods_processed",
            vessels_by_class.len().to_string(),
        ))
}

// This function takes a list of vessels (hydro_lock_ids) and a duration
// And calls the Hydro function:
// ExecuteMsg::RefreshLockDuration {
//     lock_ids,
//     lock_duration,
// }
// NOTE: clients need to check that all the vessels are currently less than hydro_lock_duration or RefreshLockDuration will fail
fn execute_update_vessels_class(
    mut deps: DepsMut,
    info: MessageInfo,
    hydro_lock_ids: Vec<u64>,
    hydro_lock_duration: u64,
) -> Result<Response, ContractError> {
    let constants = state::get_constants(deps.storage)?;
    validate_contract_is_not_paused(&constants)?;

    validate_user_owns_vessels(deps.storage, &info.sender, &hydro_lock_ids)?;

    // Check that class_period represents a valid lock duration
    let constant_response = query_hydro_constants(&deps.as_ref(), &constants)?;
    validate_lock_duration(
        &constant_response.constants.round_lock_power_schedule,
        constant_response.constants.lock_epoch_length,
        hydro_lock_duration,
    )?;

    let current_round_id = query_hydro_current_round(&deps.as_ref(), &constants)?;

    initialize_vessel_tws(
        &mut deps,
        hydro_lock_ids.clone(),
        current_round_id,
        &constants,
    )?;

    let refresh_duration_msg = HydroExecuteMsg::RefreshLockDuration {
        lock_ids: hydro_lock_ids.clone(),
        lock_duration: hydro_lock_duration,
    };

    let execute_refresh_duration_msg = WasmMsg::Execute {
        contract_addr: constants.hydro_config.hydro_contract_address.to_string(),
        msg: to_json_binary(&refresh_duration_msg)?,
        funds: vec![],
    };

    // Create payload for reply handler
    let refresh_payload = RefreshTimeWeightedSharesReplyPayload {
        vessel_ids: hydro_lock_ids,
        target_class_period: hydro_lock_duration,
        current_round_id,
    };

    let sub_msg = SubMsg::reply_on_success(
        execute_refresh_duration_msg,
        REFRESH_TIME_WEIGHTED_SHARES_REPLY_ID,
    )
    .with_payload(to_json_binary(&refresh_payload)?);

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

    validate_user_owns_vessels(deps.storage, &info.sender, &hydro_lock_ids)?;

    for hydro_lock_id in hydro_lock_ids.iter() {
        state::modify_auto_maintenance(deps.storage, *hydro_lock_id, auto_maintenance)?;
    }

    Ok(Response::new()
        .add_attribute("action", "modify_auto_maintenance")
        .add_attribute("new_auto_maintenance", auto_maintenance.to_string())
        .add_attribute("hydro_lock_id", join_u64_ids(hydro_lock_ids)))
}

fn execute_pause_contract(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    let mut constants = state::get_constants(deps.storage)?;

    validate_admin_address(deps.storage, &info.sender)?;
    validate_contract_is_not_paused(&constants)?;

    constants.paused_contract = true;
    state::update_constants(deps.storage, constants)?;

    Ok(Response::new()
        .add_attribute("action", "pause_contract")
        .add_attribute("sender", info.sender))
}

fn execute_unpause_contract(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    let mut constants = state::get_constants(deps.storage)?;

    validate_admin_address(deps.storage, &info.sender)?;
    validate_contract_is_paused(&constants)?;

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

    validate_user_owns_vessels(deps.storage, &info.sender, &hydro_lock_ids)?;

    // Check the current balance before unlocking tokens

    // Retrieve the lock_entries from Hydro, and check which ones are expired
    let user_specific_lockups = query_hydro_specific_user_lockups(
        &deps.as_ref(),
        &env,
        &constants,
        hydro_lock_ids.clone(),
    )?;

    let lock_entries = user_specific_lockups.lockups;

    let mut expected_unlocked_ids = vec![];
    let mut lockup_denoms = HashSet::new();
    for lock_entry in lock_entries {
        if lock_entry.lock_entry.lock_end < env.block.time {
            expected_unlocked_ids.push(lock_entry.lock_entry.lock_id);
        }
        lockup_denoms.insert(lock_entry.lock_entry.funds.denom.clone());
    }
    let mut previous_balances: Vec<Coin> = Vec::new();
    // to prevent denial of service on balance queries, we get only the current balances for the denoms of the lockups
    for lockup_denom in lockup_denoms {
        let balance = deps
            .querier
            .query_balance(env.contract.address.clone(), lockup_denom.clone())?;
        previous_balances.push(balance);
    }

    // Create the execute message for unlocking
    let hydro_unlock_msg = HydroExecuteMsg::UnlockTokens {
        lock_ids: Some(hydro_lock_ids.clone()),
    };

    let execute_hydro_unlock_msg = WasmMsg::Execute {
        contract_addr: constants.hydro_config.hydro_contract_address.to_string(),
        msg: to_json_binary(&hydro_unlock_msg)?,
        funds: vec![],
    };

    let decommission_vessels_params = DecommissionVesselsReplyPayload {
        previous_balances,
        expected_unlocked_ids,
        vessel_owner: info.sender.clone(),
    };

    let execute_hydro_unlock_msg: SubMsg<NeutronMsg> =
        SubMsg::reply_on_success(execute_hydro_unlock_msg, DECOMMISSION_REPLY_ID)
            .with_payload(to_json_binary(&decommission_vessels_params)?);

    Ok(Response::new().add_submessage(execute_hydro_unlock_msg))
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
    let hydromancer_id = state::get_hydromancer_id_by_address(deps.storage, info.sender.clone())
        .map_err(|_| ContractError::HydromancerNotFound {
            identifier: info.sender.to_string(),
        })?;

    let mut proposals_votes = Vec::with_capacity(vessels_harbors.len());
    for vh in vessels_harbors.clone() {
        // Validate that all vessels are controlled by the hydromancer
        validate_hydromancer_controls_vessels(deps.storage, hydromancer_id, &vh.vessel_ids)?;
        proposals_votes.push(ProposalToLockups {
            proposal_id: vh.harbor_id,
            lock_ids: vh.vessel_ids,
        });
    }

    // We need to initialize the Hydromancer TWS when the hydromancer votes
    // It's only initialized once per round / hydromancer
    complete_hydromancer_time_weighted_shares(
        &mut deps,
        hydromancer_id,
        &constants,
        current_round_id,
    )?;

    // Prepare the Vote message with payload
    let vote_message = HydroExecuteMsg::Vote {
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
    // Convert to HashSet to avoid duplicates
    let vessel_ids: HashSet<u64> = vessel_ids.into_iter().collect();
    let vessel_ids: Vec<u64> = vessel_ids.into_iter().collect();
    validate_contract_is_not_paused(&constants)?;
    validate_user_owns_vessels(deps.storage, &info.sender, &vessel_ids)?;
    validate_hydromancer_exists(deps.storage, new_hydromancer_id)?;

    let lockups_with_per_tranche_infos =
        query_hydro_lockups_with_tranche_infos(&deps.as_ref(), &env, &constants, &vessel_ids)?;
    validate_vessels_not_tied_to_proposal(&lockups_with_per_tranche_infos)?;

    let current_round_id = query_hydro_current_round(&deps.as_ref(), &constants)?;
    let tranche_ids = query_hydro_tranches(&deps.as_ref(), &constants)?;

    // Categorize vessels by their current control state
    let (vessels_not_yet_controlled, vessels_already_controlled) =
        categorize_vessels_by_control(deps.storage, new_hydromancer_id, &vessel_ids)?;

    // Step 1: Handle vessels that need hydromancer change
    for vessel_id in &vessels_not_yet_controlled {
        // Use the comprehensive assignment function that handles all cleanup and reassignment
        assign_vessel_to_hydromancer(
            deps.storage,
            *vessel_id,
            new_hydromancer_id,
            current_round_id,
            &tranche_ids,
        )?;
    }

    // Step 2: Batch initialize TWS for all vessels that need it
    // (vessels now have correct hydromancer assignments)
    initialize_vessel_tws(&mut deps, vessel_ids.clone(), current_round_id, &constants)?;

    let response = Response::new()
        .add_attribute("action", "change_hydromancer")
        .add_attribute("new_hydromancer_id", new_hydromancer_id.to_string())
        .add_attribute(
            "processed_vessels",
            join_u64_ids(&vessels_not_yet_controlled),
        )
        .add_attribute(
            "already_controlled_vessels",
            join_u64_ids(&vessels_already_controlled),
        );

    if vessels_not_yet_controlled.is_empty() {
        // nothing left to do
        return Ok(response);
    }

    // Step 3: Send unvote message for vessels that changed hydromancer (or that were controlled by user)
    let unvote_msg = HydroExecuteMsg::Unvote {
        tranche_id,
        lock_ids: vessels_not_yet_controlled.clone(),
    };

    let execute_unvote_msg = WasmMsg::Execute {
        contract_addr: constants.hydro_config.hydro_contract_address.to_string(),
        msg: to_json_binary(&unvote_msg)?,
        funds: vec![],
    };

    Ok(Response::new().add_message(execute_unvote_msg))
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

        // If vessel is already under user control there is nothing to do
        if vessel.is_under_user_control() {
            continue;
        }

        // Check if vessel was voting on any tranche (need to unvote)
        for tranche_id in &tranche_ids {
            if let Ok(Some(_proposal_id)) =
                state::get_harbor_of_vessel(deps.storage, *tranche_id, current_round_id, vessel_id)
            {
                // Vessel was voting, need to unvote
                unvote_ids_by_tranche
                    .entry(*tranche_id)
                    .or_default()
                    .push(vessel_id);
            }
        }

        // Use the comprehensive assignment function that handles all cleanup
        assign_vessel_to_user_control(deps.storage, vessel_id, current_round_id, &tranche_ids)?;

        new_vessels_under_user_control.push(vessel_id);
    }

    let mut response = Response::new();
    for (tranche_id, lock_ids) in unvote_ids_by_tranche.into_iter() {
        response = response.add_message(WasmMsg::Execute {
            msg: to_json_binary(&HydroExecuteMsg::Unvote {
                tranche_id,
                lock_ids,
            })?,
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

fn execute_user_vote(
    deps: DepsMut,
    info: MessageInfo,
    tranche_id: u64,
    vessels_harbors: Vec<VesselsToHarbor>,
) -> Result<Response, ContractError> {
    let constants = state::get_constants(deps.storage)?;
    validate_contract_is_not_paused(&constants)?;

    validate_vote_duplicates(&vessels_harbors)?;

    let user_id = state::get_user_id(deps.storage, &info.sender).map_err(|_| {
        ContractError::UserNotFound {
            identifier: info.sender.to_string(),
        }
    })?;

    let current_round_id = query_hydro_current_round(&deps.as_ref(), &constants)?;
    let mut proposal_votes = vec![];

    for vessels_to_harbor in vessels_harbors.clone() {
        let lockups_info_response = query_hydro_lockups_shares(
            &deps.as_ref(),
            &constants,
            vessels_to_harbor.vessel_ids.clone(),
        )?;

        for lockup_info in lockups_info_response.lockups_shares_info {
            let vessel = state::get_vessel(deps.storage, lockup_info.lock_id)?;

            // Check that the vessel belongs to the user
            if vessel.owner_id != user_id {
                return Err(ContractError::Unauthorized {});
            }

            // Even if a vessel is owned by the user, if it's under hydromancer control, user can't vote with it
            if !vessel.is_under_user_control() {
                return Err(ContractError::VesselUnderHydromancerControl {
                    vessel_id: lockup_info.lock_id,
                });
            }

            let vessel_shares_info =
                state::get_vessel_shares_info(deps.storage, current_round_id, lockup_info.lock_id);
            if vessel_shares_info.is_err() {
                state::save_vessel_info_snapshot(
                    deps.storage,
                    lockup_info.lock_id,
                    current_round_id,
                    lockup_info.time_weighted_shares.u128(),
                    lockup_info.token_group_id,
                    lockup_info.locked_rounds,
                    None,
                )?;
            }
        }

        let proposal_to_lockups = ProposalToLockups {
            proposal_id: vessels_to_harbor.harbor_id,
            lock_ids: vessels_to_harbor.vessel_ids,
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

    let vote_message = HydroExecuteMsg::Vote {
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
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}
