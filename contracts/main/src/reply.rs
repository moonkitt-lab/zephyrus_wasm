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
    calcul_total_voting_power_on_proposal, calculate_rewards_for_vessels_on_tribute,
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
    let constants = state::get_constants(deps.storage)?;
    let balance_query = deps
        .querier
        .query_balance(env.contract.address, payload.amount.denom.clone())?;
    let balance_expected = payload
        .balance_before_claim
        .amount
        .strict_add(payload.amount.amount.clone());
    // Check if the amount reveived is correct
    if balance_query.amount != balance_expected {
        return Err(ContractError::InsufficientTributeReceived {
            tribute_id: payload.tribute_id,
        });
    }

    let token_info_provider =
        query_hydro_derivative_token_info_providers(&deps.as_ref(), &constants, payload.round_id)?;
    let total_proposal_voting_power = calcul_total_voting_power_on_proposal(
        deps.storage,
        payload.proposal_id,
        payload.round_id,
        &token_info_provider,
    )?;
    let hydromancer_ids = state::get_all_hydromancers(deps.storage)?;
    for hydromancer_id in hydromancer_ids {
        allocate_rewards_to_hydromancer(
            &mut deps,
            &payload,
            &token_info_provider,
            total_proposal_voting_power,
            hydromancer_id,
        )?;
    }

    let (commission_amount, users_funds) = calcul_protocol_comm_and_rest(&payload, &constants);

    // Cumulate rewards for each vessel
    let amount_to_distribute = calculate_rewards_for_vessels_on_tribute(
        &mut deps,
        payload.vessel_ids.clone(),
        payload.tribute_id,
        payload.tranche_id,
        payload.round_id,
        payload.proposal_id,
        users_funds,
        constants.clone(),
        token_info_provider,
        total_proposal_voting_power,
    )?;
    let mut response = Response::new();

    // Send rewards to vessels owner
    if !amount_to_distribute.is_zero() {
        let send_msg = BankMsg::Send {
            to_address: payload.vessels_owner.to_string(),
            amount: vec![Coin {
                denom: payload.amount.denom.clone(),
                amount: amount_to_distribute.to_uint_floor(),
            }],
        };
        response = response.add_message(send_msg);
    }
    // Send commission to recipient
    if commission_amount.u128() > 0 {
        let send_msg = BankMsg::Send {
            to_address: constants.commission_recipient.to_string(),
            amount: vec![Coin {
                denom: payload.amount.denom.clone(),
                amount: commission_amount,
            }],
        };
        response = response.add_message(send_msg);
    }
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
    apply_hydromancer_tws_changes(deps.storage, hydromancer_tws_changes)?;
    apply_proposal_tws_changes(deps.storage, tws_changes.proposal_changes)?;
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
