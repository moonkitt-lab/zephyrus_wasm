use cosmwasm_std::{
    entry_point, from_json, AllBalanceResponse, BankMsg, BankQuery, Coin, DepsMut, Env,
    QueryRequest, Reply, Response as CwResponse, StdError,
};
use std::collections::HashMap;

use neutron_sdk::bindings::msg::NeutronMsg;

use zephyrus_core::msgs::{
    ClaimTributeReplyPayload, DecommissionVesselsReplyPayload, HydromancerId,
    RefreshTimeWeightedSharesReplyPayload, RoundId, VoteReplyPayload, CLAIM_TRIBUTE_REPLY_ID,
    DECOMMISSION_REPLY_ID, REFRESH_TIME_WEIGHTED_SHARES_REPLY_ID, VOTE_REPLY_ID,
};
use zephyrus_core::state::VesselHarbor;

use crate::helpers::hydro_queries::query_hydro_derivative_token_info_providers;
use crate::helpers::rewards::{
    allocate_rewards_to_hydromancer, calcul_protocol_comm_and_rest,
    calcul_total_voting_power_on_proposal, distribute_rewards_for_vessels_on_tribute,
    process_hydromancer_claiming_rewards,
};
use crate::{
    errors::ContractError,
    helpers::{
        hydro_queries::{query_hydro_lockups_shares, query_hydro_tranches},
        tws::{
            apply_hydromancer_tws_changes, apply_proposal_hydromancer_tws_changes,
            apply_proposal_tws_changes, batch_hydromancer_tws_changes, batch_proposal_tws_changes,
            TwsChanges,
        },
        vectors::{compare_coin_vectors, compare_u64_vectors, join_u64_ids},
    },
    state,
};

type Response = CwResponse<NeutronMsg>;

#[entry_point]
pub fn reply(deps: DepsMut, env: Env, reply: Reply) -> Result<Response, ContractError> {
    match reply.id {
        DECOMMISSION_REPLY_ID => {
            let hydro_unlocked_tokens: Vec<Coin> = parse_unlocked_token_from_reply(&reply)?;
            let unlocked_hydro_lock_ids: Vec<u64> = parse_unlocked_lock_ids_reply(&reply)?;
            let payload: DecommissionVesselsReplyPayload = from_json(reply.payload)?;
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
            let payload: RefreshTimeWeightedSharesReplyPayload = from_json(&reply.payload)?;
            handle_refresh_time_weighted_shares_reply(deps, payload)
        }
        CLAIM_TRIBUTE_REPLY_ID => {
            let payload: ClaimTributeReplyPayload = from_json(&reply.payload)?;
            handle_claim_tribute_reply(deps, env, payload)
        }
        _ => Err(ContractError::CustomError {
            msg: "Unknown reply id".to_string(),
        }),
    }
}

pub fn handle_claim_tribute_reply(
    mut deps: DepsMut<'_>,
    env: Env,
    payload: ClaimTributeReplyPayload,
) -> Result<Response, ContractError> {
    deps.api
        .debug("ZEPH997: CLAIM_TRIBUTE_REPLY HANDLER CALLED - TEST LOG");
    deps.api.debug(&format!("ZEPH020: Starting claim tribute reply handler - tribute_id: {}, proposal_id: {}, amount: {:?}", 
        payload.tribute_id, payload.proposal_id, payload.amount));

    let constants = state::get_constants(deps.storage)?;
    let balance_query = deps
        .querier
        .query_balance(env.contract.address, payload.amount.denom.clone())?;
    let balance_expected = payload
        .balance_before_claim
        .amount
        .strict_add(payload.amount.amount);

    // Get total amount distributed by previous tributes in this batch
    let total_distributed =
        state::get_total_distributed_amount(deps.storage, &payload.amount.denom)?;
    let balance_expected_adjusted = balance_expected.saturating_sub(total_distributed);

    deps.api.debug(&format!(
        "ZEPH021: Balance check - actual: {}, expected: {}, before_claim: {}, total_distributed: {}, adjusted_expected: {}",
        balance_query.amount, balance_expected, payload.balance_before_claim.amount, total_distributed, balance_expected_adjusted
    ));

    // Check if the amount received is correct, accounting for previous distributions
    if balance_query.amount != balance_expected_adjusted {
        deps.api.debug(&format!(
            "ZEPH022: ERROR - Balance mismatch! tribute_id: {}, actual: {}, expected_adjusted: {}",
            payload.tribute_id, balance_query.amount, balance_expected_adjusted
        ));
        return Err(ContractError::InsufficientTributeReceived {
            tribute_id: payload.tribute_id,
        });
    }

    let (commission_amount, users_and_hydromancers_funds) =
        calcul_protocol_comm_and_rest(payload.amount.clone(), &constants);
    deps.api.debug(&format!(
        "ZEPH023: Commission calculation - commission: {}, users_and_hydromancers_funds: {:?}",
        commission_amount, users_and_hydromancers_funds
    ));

    deps.api.debug(&format!(
        "ZEPH112: REPLY_COMMISSION: tribute_id={}, total_amount={:?}, commission={}, users_and_hydromancers_funds={:?}",
        payload.tribute_id, payload.amount, commission_amount, users_and_hydromancers_funds
    ));

    let token_info_provider =
        query_hydro_derivative_token_info_providers(&deps.as_ref(), &constants, payload.round_id)?;
    let total_proposal_voting_power = calcul_total_voting_power_on_proposal(
        deps.storage,
        payload.proposal_id,
        payload.round_id,
        &token_info_provider,
    )?;

    deps.api.debug(&format!(
        "ZEPH024: Total proposal voting power: {}",
        total_proposal_voting_power
    ));

    let hydromancer_ids = state::get_all_hydromancers(deps.storage)?;
    deps.api.debug(&format!(
        "ZEPH025: Allocating rewards to {} hydromancers",
        hydromancer_ids.len()
    ));

    for hydromancer_id in hydromancer_ids {
        let hydromancer_tribute = allocate_rewards_to_hydromancer(
            deps.as_ref(),
            payload.proposal_id,
            payload.round_id,
            users_and_hydromancers_funds.clone(),
            &token_info_provider,
            total_proposal_voting_power,
            hydromancer_id,
        )?;
        state::add_new_rewards_to_hydromancer(
            deps.storage,
            hydromancer_id,
            payload.round_id,
            payload.tribute_id,
            hydromancer_tribute,
        )?;
    }

    deps.api.debug(&format!(
        "ZEPH026: Calculating rewards for {} vessels",
        payload.vessel_ids.len()
    ));

    // Log vessel ownership for debugging
    deps.api.debug(&format!(
        "ZEPH027: VESSEL_OWNERSHIP: tribute_id={}, vessels={:?}, owner={}",
        payload.tribute_id, payload.vessel_ids, payload.vessels_owner
    ));

    // Cumulate rewards for each vessel
    deps.api.debug(&format!("ZEPH113: REPLY_BEFORE_DISTRIBUTE: tribute_id={}, vessels={:?}, users_and_hydromancers_funds={:?}, total_proposal_voting_power={}", 
        payload.tribute_id, payload.vessel_ids, users_and_hydromancers_funds, total_proposal_voting_power));

    let amount_to_distribute = distribute_rewards_for_vessels_on_tribute(
        &mut deps,
        payload.vessel_ids.clone(),
        payload.tribute_id,
        payload.tranche_id,
        payload.round_id,
        payload.proposal_id,
        users_and_hydromancers_funds.clone(),
        constants.clone(),
        token_info_provider,
        total_proposal_voting_power,
    )?;

    deps.api.debug(&format!(
        "ZEPH114: REPLY_AFTER_DISTRIBUTE: tribute_id={}, amount_to_distribute={}",
        payload.tribute_id, amount_to_distribute
    ));
    let mut response = Response::new();

    deps.api.debug(&format!(
        "ZEPH027: Amount to distribute: {}",
        amount_to_distribute
    ));
    // Send rewards to vessels owner
    let floored_amount = amount_to_distribute.to_uint_floor();
    deps.api
        .debug(&format!("ZEPH028: Floored amount: {}", floored_amount));

    deps.api.debug(&format!(
        "ZEPH115: REPLY_SEND: tribute_id={}, amount_decimal={}, amount_floored={}, owner={}",
        payload.tribute_id, amount_to_distribute, floored_amount, payload.vessels_owner
    ));

    if !floored_amount.is_zero() {
        deps.api.debug(&format!(
            "ZEPH029: Sending {} {} to vessel owner {}",
            floored_amount, payload.amount.denom, payload.vessels_owner
        ));
        let send_msg = BankMsg::Send {
            to_address: payload.vessels_owner.to_string(),
            amount: vec![Coin {
                denom: payload.amount.denom.clone(),
                amount: floored_amount,
            }],
        };
        response = response.add_message(send_msg);
    } else {
        deps.api
            .debug("ZEPH030: No rewards to send to vessel owner (floored amount is zero)");
    }

    // Send commission to recipient
    if commission_amount.u128() > 0 {
        deps.api.debug(&format!(
            "ZEPH031: Sending commission {} {} to {}",
            commission_amount, payload.amount.denom, constants.commission_recipient
        ));
        let send_msg = BankMsg::Send {
            to_address: constants.commission_recipient.to_string(),
            amount: vec![Coin {
                denom: payload.amount.denom.clone(),
                amount: commission_amount,
            }],
        };
        response = response.add_message(send_msg);
    } else {
        deps.api.debug("ZEPH032: No commission to send");
    }

    // Process the case that sender is an hydromancer and send its commission to the sender
    let hydromancer_rewards_send_msg = process_hydromancer_claiming_rewards(
        &mut deps,
        payload.vessels_owner.clone(),
        payload.round_id,
        payload.tribute_id,
    )?;

    // Record total distributed amount for this tribute to track for future tributes in same batch
    let mut total_distributed_amount = floored_amount
        .checked_add(commission_amount)
        .map_err(|e| ContractError::Std(e.into()))?;

    // Add hydromancer rewards if any and add to response
    if let Some(ref send_msg) = hydromancer_rewards_send_msg {
        deps.api.debug("ZEPH033: Sending hydromancer commission");
        response = response.add_message(send_msg.clone());

        // Extract amount from hydromancer message for tracking
        if let BankMsg::Send { amount, .. } = send_msg {
            if let Some(hydro_coin) = amount.iter().find(|c| c.denom == payload.amount.denom) {
                total_distributed_amount = total_distributed_amount
                    .checked_add(hydro_coin.amount)
                    .map_err(|e| ContractError::Std(e.into()))?;
            }
        }
    } else {
        deps.api.debug("ZEPH034: No hydromancer commission to send");
    }

    if !total_distributed_amount.is_zero() {
        state::record_tribute_distribution(
            deps.storage,
            payload.tribute_id,
            Coin {
                denom: payload.amount.denom.clone(),
                amount: total_distributed_amount,
            },
        )?;
        deps.api.debug(&format!(
            "ZEPH034.5: Recorded distribution of {} {} for tribute_id: {}",
            total_distributed_amount, payload.amount.denom, payload.tribute_id
        ));
    }
    //we mark the processed amount as the users funds, because the users funds are the amount that will be distributed to the vessels, not the tribute amount
    state::mark_tribute_processed(
        deps.storage,
        payload.tribute_id,
        users_and_hydromancers_funds.clone(),
    )?;
    deps.api
        .debug("ZEPH035: Claim tribute reply handler completed successfully");
    Ok(response.add_attribute("action", "handle_claim_tribute_reply"))
}

pub fn handle_refresh_time_weighted_shares_reply(
    deps: DepsMut,
    payload: RefreshTimeWeightedSharesReplyPayload,
) -> Result<Response, ContractError> {
    let constants = state::get_constants(deps.storage)?;
    let tranche_ids = query_hydro_tranches(&deps.as_ref(), &constants)?;

    // Query updated TWS from Hydro contract after the refresh
    let updated_lockups_shares =
        query_hydro_lockups_shares(&deps.as_ref(), &constants, payload.vessel_ids.clone())?;

    // Batch TWS changes in memory before applying
    let mut hydromancer_tws_changes: HashMap<(HydromancerId, RoundId, String, u64), i128> =
        HashMap::new();
    let mut tws_changes = TwsChanges::new();

    let mut vessels_tws_updated = Vec::new();

    for updated_lockup_shares in updated_lockups_shares.lockups_shares_info {
        let vessel_id = updated_lockup_shares.lock_id;
        let vessel = state::get_vessel(deps.storage, vessel_id)?;

        // Get old TWS if it exists
        let old_vessel_shares =
            state::get_vessel_shares_info(deps.storage, payload.current_round_id, vessel_id).ok();

        // Save new vessel shares info
        state::save_vessel_shares_info(
            deps.storage,
            vessel_id,
            payload.current_round_id,
            updated_lockup_shares.time_weighted_shares.u128(),
            updated_lockup_shares.token_group_id.clone(),
            updated_lockup_shares.locked_rounds,
        )?;

        // Batch hydromancer TWS changes if vessel is controlled by hydromancer
        if let Some(hydromancer_id) = vessel.hydromancer_id {
            batch_hydromancer_tws_changes(
                &mut hydromancer_tws_changes,
                hydromancer_id,
                payload.current_round_id,
                &old_vessel_shares,
                &updated_lockup_shares,
            );
        }

        // Batch proposal TWS changes if vessel is currently voting
        batch_proposal_tws_changes(
            deps.storage,
            &mut tws_changes,
            &vessel,
            &old_vessel_shares,
            &updated_lockup_shares,
            &tranche_ids,
            payload.current_round_id,
        )?;

        vessels_tws_updated.push(vessel_id);
    }

    // Apply all batched changes in single write operations
    deps.api.debug(&format!(
        "ZEPH302: APPLYING_HYDROMANCER_TWS_CHANGES: {} changes",
        hydromancer_tws_changes.len()
    ));
    apply_hydromancer_tws_changes(deps.storage, hydromancer_tws_changes)?;

    deps.api.debug(&format!(
        "ZEPH303: APPLYING_PROPOSAL_TWS_CHANGES: {} changes",
        tws_changes.proposal_changes.len()
    ));
    apply_proposal_tws_changes(deps.storage, tws_changes.proposal_changes)?;

    deps.api.debug(&format!(
        "ZEPH304: APPLYING_PROPOSAL_HYDROMANCER_TWS_CHANGES: {} changes",
        tws_changes.proposal_hydromancer_changes.len()
    ));
    apply_proposal_hydromancer_tws_changes(deps.storage, tws_changes.proposal_hydromancer_changes)?;

    Ok(Response::new()
        .add_attribute("action", "refresh_tws_reply")
        .add_attribute(
            "target_class_period",
            payload.target_class_period.to_string(),
        )
        .add_attribute("vessels_updated", join_u64_ids(&vessels_tws_updated))
        .add_attribute("round_id", payload.current_round_id.to_string()))
}

//Handle vote reply, used after both user and hydromancer vote
pub fn handle_vote_reply(
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
                        deps.api.debug(&format!("ZEPH305: VOTE_CHANGE_SUB_TWS: vessel_id={}, from_proposal={}, token_group_id={}, tws={}", 
                            vessel.hydro_lock_id, previous_harbor_id, vessel_shares_info.token_group_id, vessel_shares_info.time_weighted_shares.u128()));
                        state::substract_time_weighted_shares_from_proposal(
                            deps.storage,
                            previous_harbor_id,
                            &vessel_shares_info.token_group_id,
                            vessel_shares_info.time_weighted_shares.u128(),
                        )?;

                        deps.api.debug(&format!("ZEPH306: VOTE_CHANGE_ADD_TWS: vessel_id={}, to_proposal={}, token_group_id={}, tws={}", 
                            vessel.hydro_lock_id, vessels_to_harbor.harbor_id, vessel_shares_info.token_group_id, vessel_shares_info.time_weighted_shares.u128()));
                        state::add_time_weighted_shares_to_proposal(
                            deps.storage,
                            vessels_to_harbor.harbor_id,
                            &vessel_shares_info.token_group_id,
                            vessel_shares_info.time_weighted_shares.u128(),
                        )?;
                        // if it's a hydromancer vote, add time weighted shares to proposal for hydromancer
                        if !payload.user_vote && !vessel_shares_info.time_weighted_shares.is_zero()
                        {
                            deps.api.debug(&format!("ZEPH307: HYDROMANCER_VOTE_ADD_TWS: vessel_id={}, proposal={}, hydromancer={}, token_group_id={}, tws={}", 
                                vessel.hydro_lock_id, vessels_to_harbor.harbor_id, payload.steerer_id, vessel_shares_info.token_group_id, vessel_shares_info.time_weighted_shares.u128()));
                            state::add_time_weighted_shares_to_proposal_for_hydromancer(
                                deps.storage,
                                vessels_to_harbor.harbor_id,
                                payload.steerer_id,
                                &vessel_shares_info.token_group_id,
                                vessel_shares_info.time_weighted_shares.u128(),
                            )?;

                            deps.api.debug(&format!("ZEPH308: HYDROMANCER_VOTE_SUB_TWS: vessel_id={}, proposal={}, hydromancer={}, token_group_id={}, tws={}", 
                                vessel.hydro_lock_id, previous_harbor_id, payload.steerer_id, vessel_shares_info.token_group_id, vessel_shares_info.time_weighted_shares.u128()));
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
                    deps.api.debug(&format!(
                        "ZEPH124: FIRST_VOTE_DEBUG: vessel_id={}, proposal_id={}, user_vote={}, steerer_id={}, tws={}, is_zero={}",
                        vessel.hydro_lock_id, vessels_to_harbor.harbor_id, payload.user_vote, payload.steerer_id,
                        vessel_shares_info.time_weighted_shares, vessel_shares_info.time_weighted_shares.is_zero()
                    ));

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

                    deps.api.debug(&format!(
                        "ZEPH125: HYDROMANCER_CONDITION_DEBUG: user_vote={}, tws={}, is_zero={}, condition_result={}",
                        payload.user_vote, vessel_shares_info.time_weighted_shares,
                        vessel_shares_info.time_weighted_shares.is_zero(),
                        !payload.user_vote && !vessel_shares_info.time_weighted_shares.is_zero()
                    ));

                    if !payload.user_vote && !vessel_shares_info.time_weighted_shares.is_zero() {
                        // should always be some, because hydro has accepted the vote
                        deps.api.debug(&format!(
                            "ZEPH126: ADDING_HYDROMANCER_TWS: proposal_id={}, hydromancer_id={}, token_group_id={}, tws={}",
                            vessels_to_harbor.harbor_id, payload.steerer_id, vessel_shares_info.token_group_id, vessel_shares_info.time_weighted_shares.u128()
                        ));
                        state::add_time_weighted_shares_to_proposal_for_hydromancer(
                            deps.storage,
                            vessels_to_harbor.harbor_id,
                            payload.steerer_id,
                            &vessel_shares_info.token_group_id,
                            vessel_shares_info.time_weighted_shares.u128(),
                        )?;
                    } else {
                        deps.api.debug(&format!(
                            "ZEPH127: SKIPPING_HYDROMANCER_TWS: user_vote={}, tws={}, is_zero={}",
                            payload.user_vote,
                            vessel_shares_info.time_weighted_shares,
                            vessel_shares_info.time_weighted_shares.is_zero()
                        ));
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
    parse_coins_from_reply(reply, "unlocked_tokens")
}

pub fn handle_unlock_tokens_reply(
    deps: DepsMut,
    env: Env,
    decommission_vessels_params: DecommissionVesselsReplyPayload,
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
