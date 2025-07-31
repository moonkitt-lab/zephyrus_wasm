use std::collections::{BTreeSet, HashMap};

use cosmwasm_std::{
    to_json_binary, Addr, BankMsg, Coin, Decimal, DepsMut, Storage, SubMsg, Uint128, WasmMsg,
};
use hydro_interface::msgs::{DenomInfoResponse, ExecuteMsg as HydroExecuteMsg};
use neutron_sdk::bindings::msg::NeutronMsg;
use zephyrus_core::{
    msgs::{
        ClaimTributeReplyPayload, HydroLockId, HydroProposalId, HydromancerId, RoundId, TrancheId,
        TributeId, CLAIM_TRIBUTE_REPLY_ID,
    },
    state::{Constants, HydromancerTribute},
};

use crate::{
    errors::ContractError,
    helpers::hydro_queries::{
        query_hydro_derivative_token_info_providers, query_hydro_proposal,
        query_hydro_proposal_tributes, query_hydro_round_all_proposals,
    },
    state,
};

pub fn build_claim_tribute_sub_msg(
    round_id: u64,
    tranche_id: u64,
    vessel_ids: &Vec<u64>,
    owner: &Addr,
    constants: &Constants,
    contract_address: &Addr,
    balances: &Vec<Coin>,
    outstanding_tribute: &hydro_interface::msgs::TributeClaim,
) -> Result<SubMsg<NeutronMsg>, ContractError> {
    let claim_msg = HydroExecuteMsg::ClaimTribute {
        round_id,
        tranche_id,
        tribute_id: outstanding_tribute.tribute_id,
        voter_address: contract_address.to_string(),
    };
    let execute_claim_msg = WasmMsg::Execute {
        contract_addr: constants.hydro_config.hydro_contract_address.to_string(),
        msg: to_json_binary(&claim_msg)?,
        funds: vec![],
    };
    let balance_before_claim = balances
        .iter()
        .find(|balance| balance.denom == outstanding_tribute.amount.denom)
        .cloned()
        .unwrap_or(Coin {
            denom: outstanding_tribute.amount.denom.clone(),
            amount: Uint128::zero(),
        });
    let payload = ClaimTributeReplyPayload {
        proposal_id: outstanding_tribute.proposal_id,
        tribute_id: outstanding_tribute.tribute_id,
        round_id: round_id,
        tranche_id: tranche_id,
        amount: outstanding_tribute.amount.clone(),
        balance_before_claim,
        vessels_owner: owner.clone(),
        vessel_ids: vessel_ids.clone(),
    };
    let sub_msg: SubMsg<NeutronMsg> =
        SubMsg::reply_on_success(execute_claim_msg, CLAIM_TRIBUTE_REPLY_ID)
            .with_payload(to_json_binary(&payload)?);
    Ok(sub_msg)
}

pub fn calcul_total_voting_power_of_hydromancer_on_proposal(
    storage: &dyn Storage,
    hydromancer_id: HydromancerId,
    proposal_id: HydroProposalId,
    round_id: RoundId,
    token_info_provider: &HashMap<String, DenomInfoResponse>,
) -> Result<Decimal, ContractError> {
    let list_tws =
        state::get_hydromancer_proposal_time_weighted_shares(storage, hydromancer_id, proposal_id)?;
    let mut total_voting_power = Decimal::zero();
    for (token_group_id, tws) in list_tws {
        let token_info = token_info_provider.get(&token_group_id).ok_or(
            ContractError::TokenInfoProviderNotFound {
                token_group_id: token_group_id.clone(),
                round_id: round_id,
            },
        )?;
        total_voting_power = total_voting_power
            .saturating_add(Decimal::from_ratio(tws, 1u128).saturating_mul(token_info.ratio));
    }
    Ok(total_voting_power)
}

pub fn calcul_total_voting_power_of_hydromancer_for_locked_rounds(
    storage: &dyn Storage,
    hydromancer_id: HydromancerId,
    round_id: RoundId,
    locked_rounds: u64,
    token_info_provider: &HashMap<String, DenomInfoResponse>,
) -> Result<Decimal, ContractError> {
    let list_tws =
        state::get_hydromancer_time_weighted_shares_by_round(storage, round_id, hydromancer_id)?;
    let mut total_voting_power = Decimal::zero();
    for ((locked_round, token_group_id), tws) in list_tws {
        if locked_round < locked_rounds {
            continue;
        }
        let token_info = token_info_provider.get(&token_group_id).ok_or(
            ContractError::TokenInfoProviderNotFound {
                token_group_id: token_group_id.clone(),
                round_id: round_id,
            },
        )?;
        total_voting_power = total_voting_power
            .saturating_add(Decimal::from_ratio(tws, 1u128).saturating_mul(token_info.ratio));
    }
    Ok(total_voting_power)
}

pub fn calcul_total_voting_power_on_proposal(
    storage: &dyn Storage,
    proposal_id: HydroProposalId,
    round_id: RoundId,
    token_info_provider: &HashMap<String, DenomInfoResponse>,
) -> Result<Decimal, ContractError> {
    let list_tws = state::get_proposal_time_weighted_shares(storage, proposal_id)?;
    let mut total_voting_power = Decimal::zero();
    for (token_group_id, tws) in list_tws {
        let token_info = token_info_provider.get(&token_group_id).ok_or(
            ContractError::TokenInfoProviderNotFound {
                token_group_id: token_group_id.clone(),
                round_id: round_id,
            },
        )?;
        total_voting_power = total_voting_power
            .saturating_add(Decimal::from_ratio(tws, 1u128).saturating_mul(token_info.ratio));
    }
    Ok(total_voting_power)
}

pub fn calcul_voting_power_of_vessel(
    storage: &dyn Storage,
    vessel_id: HydroLockId,
    round_id: RoundId,
    token_info_provider: &HashMap<String, DenomInfoResponse>,
) -> Result<Decimal, ContractError> {
    // Vessel shares should exist, but if not, the voting power is 0 — though doing it this way might let some errors go unnoticed.
    let vessel_share_info = state::get_vessel_shares_info(storage, round_id, vessel_id);
    if vessel_share_info.is_err() {
        return Ok(Decimal::zero());
    }
    let vessel_share_info = vessel_share_info.unwrap();
    let token_info = token_info_provider
        .get(&vessel_share_info.token_group_id)
        .ok_or(ContractError::TokenInfoProviderNotFound {
            token_group_id: vessel_share_info.token_group_id.clone(),
            round_id: round_id,
        })?;
    let voting_power = Decimal::from_ratio(vessel_share_info.time_weighted_shares, 1u128)
        .saturating_mul(token_info.ratio);
    Ok(voting_power)
}

pub fn calcul_rewards_amount_for_vessel_on_proposal(
    deps: &DepsMut<'_>,
    round_id: RoundId,
    tranche_id: TrancheId,
    proposal_id: HydroProposalId,
    tribute_id: TributeId,
    constants: &zephyrus_core::state::Constants,
    token_info_provider: &HashMap<String, hydro_interface::msgs::DenomInfoResponse>,
    total_proposal_voting_power: Decimal,
    proposal_rewards: Coin,
    vessel_id: u64,
) -> Result<Decimal, ContractError> {
    let vessel = state::get_vessel(deps.storage, vessel_id)?;
    let voting_power =
        calcul_voting_power_of_vessel(deps.storage, vessel_id, round_id, token_info_provider)?;

    if vessel.is_under_user_control() {
        let vessel_harbor =
            state::get_harbor_of_vessel(deps.storage, tranche_id, round_id, vessel_id)?;
        if vessel_harbor.is_some() {
            let vessel_harbor = vessel_harbor.unwrap();

            if vessel_harbor == proposal_id {
                let portion = voting_power
                    .checked_div(total_proposal_voting_power)
                    .map_err(|_| ContractError::CustomError {
                        msg: "Division by zero in voting power calculation".to_string(),
                    })?
                    .saturating_mul(Decimal::from_ratio(proposal_rewards.amount, 1u128));
                return Ok(portion);
            }
        }
        Ok(Decimal::zero())
    } else {
        // Vessel is under hydromancer control, we don't care if it was used or not, it take a portion of hydromancer rewards
        let vessel_shares = state::get_vessel_shares_info(deps.storage, round_id, vessel_id)?;
        let proposal =
            query_hydro_proposal(&deps.as_ref(), constants, round_id, tranche_id, proposal_id)?;
        if proposal.deployment_duration <= vessel_shares.locked_rounds {
            let total_hydromancer_locked_rounds_voting_power =
                calcul_total_voting_power_of_hydromancer_for_locked_rounds(
                    deps.storage,
                    vessel.hydromancer_id.unwrap(),
                    round_id,
                    proposal.deployment_duration,
                    token_info_provider,
                )?;
            let rewards_allocated_to_hydromancer = state::get_hydromancer_rewards_by_tribute(
                deps.storage,
                vessel.hydromancer_id.unwrap(),
                round_id,
                tribute_id,
            )?;

            if let Some(rewards_allocated_to_hydromancer) = rewards_allocated_to_hydromancer {
                let portion = voting_power
                    .checked_div(total_hydromancer_locked_rounds_voting_power)
                    .map_err(|_| ContractError::CustomError {
                        msg: "Division by zero in voting power calculation".to_string(),
                    })?
                    .saturating_mul(Decimal::from_ratio(
                        rewards_allocated_to_hydromancer.rewards_for_users.amount,
                        1u128,
                    ));
                return Ok(portion);
            }
        }

        Ok(Decimal::zero())
    }
}

pub fn allocate_rewards_to_hydromancer(
    deps: &mut DepsMut<'_>,
    payload: &ClaimTributeReplyPayload,
    token_info_provider: &HashMap<String, hydro_interface::msgs::DenomInfoResponse>,
    total_proposal_voting_power: Decimal,
    hydromancer_id: u64,
) -> Result<(), ContractError> {
    let hydromancer_voting_power = calcul_total_voting_power_of_hydromancer_on_proposal(
        deps.storage,
        hydromancer_id,
        payload.proposal_id,
        payload.round_id,
        token_info_provider,
    )?;
    let hydromancer_portion = hydromancer_voting_power
        .checked_div(total_proposal_voting_power)
        .map_err(|_| ContractError::CustomError {
            msg: "Division by zero in voting power calculation".to_string(),
        })?;
    let total_hydromancer_reward = Decimal::from_ratio(payload.amount.amount.clone(), 1u128)
        .saturating_mul(hydromancer_portion);

    let hydromancer = state::get_hydromancer(deps.storage, hydromancer_id)?;
    let hydromancer_commission =
        total_hydromancer_reward.saturating_mul(hydromancer.commission_rate);
    let mut rewards_for_users = total_hydromancer_reward
        .saturating_sub(hydromancer_commission)
        .to_uint_floor();
    let hydromancer_commission = hydromancer_commission.to_uint_floor();

    let rest = payload
        .amount
        .amount
        .saturating_sub(hydromancer_commission)
        .saturating_sub(rewards_for_users);
    // we add the rest to users rewards
    rewards_for_users = rewards_for_users.checked_add(rest).unwrap();

    state::add_new_rewards_to_hydromancer(
        deps.storage,
        hydromancer_id,
        payload.round_id,
        payload.tribute_id,
        HydromancerTribute {
            rewards_for_users: Coin {
                denom: payload.amount.denom.clone(),
                amount: rewards_for_users,
            },
            commission_for_hydromancer: Coin {
                denom: payload.amount.denom.clone(),
                amount: hydromancer_commission,
            },
        },
    )?;
    Ok(())
}

pub fn calculate_rewards_for_vessels_on_tribute(
    deps: &mut DepsMut<'_>,
    vessel_ids: Vec<u64>,
    tribute_id: TributeId,
    tranche_id: TrancheId,
    round_id: RoundId,
    proposal_id: HydroProposalId,
    tribute_rewards: Coin,
    constants: zephyrus_core::state::Constants,
    token_info_provider: HashMap<String, hydro_interface::msgs::DenomInfoResponse>,
    total_proposal_voting_power: Decimal,
) -> Result<Decimal, ContractError> {
    let mut amount_to_distribute = Decimal::zero();
    for vessel_id in vessel_ids.clone() {
        if !state::is_vessel_tribute_claimed(deps.storage, vessel_id, tribute_id) {
            let proposal_vessel_rewards = calcul_rewards_amount_for_vessel_on_proposal(
                deps,
                round_id,
                tranche_id,
                proposal_id,
                tribute_id,
                &constants,
                &token_info_provider,
                total_proposal_voting_power,
                tribute_rewards.clone(),
                vessel_id,
            )?;
            amount_to_distribute =
                amount_to_distribute.saturating_add(proposal_vessel_rewards.clone());
            state::save_vessel_tribute_claim(
                deps.storage,
                vessel_id,
                tribute_id,
                Coin {
                    denom: tribute_rewards.denom.clone(),
                    amount: proposal_vessel_rewards.to_uint_floor(),
                },
            )?;
        }
    }
    Ok(amount_to_distribute)
}

pub fn distribute_rewards_for_all_round_proposals(
    mut deps: DepsMut<'_>,
    sender: Addr,
    round_id: u64,
    tranche_id: u64,
    vessel_ids: Vec<u64>,
    constants: Constants,
    tributes_process_in_reply: BTreeSet<u64>,
) -> Result<Vec<BankMsg>, ContractError> {
    let token_info_provider =
        query_hydro_derivative_token_info_providers(&deps.as_ref(), &constants, round_id)?;
    let all_round_proposals =
        query_hydro_round_all_proposals(&deps.as_ref(), &constants, round_id, tranche_id)?;
    let mut messages: Vec<BankMsg> = vec![];
    for proposal in all_round_proposals {
        let proposal_tributes = query_hydro_proposal_tributes(
            &deps.as_ref(),
            &constants,
            round_id,
            proposal.proposal_id,
        )?;
        let total_proposal_voting_power = calcul_total_voting_power_on_proposal(
            deps.storage,
            proposal.proposal_id,
            round_id,
            &token_info_provider,
        )?;
        for tribute in proposal_tributes {
            // tributes that have been just claimed will be processed in the reply handler, so we skip them here
            if tributes_process_in_reply.contains(&tribute.tribute_id) {
                continue;
            }
            // Cumulate rewards for each vessel
            let amount_to_distribute = calculate_rewards_for_vessels_on_tribute(
                &mut deps,
                vessel_ids.clone(),
                tribute.tribute_id,
                tribute.tranche_id,
                tribute.round_id,
                tribute.proposal_id,
                tribute.funds.clone(),
                constants.clone(),
                token_info_provider.clone(),
                total_proposal_voting_power,
            )?;
            if !amount_to_distribute.is_zero() {
                let send_msg = BankMsg::Send {
                    to_address: sender.to_string(),
                    amount: vec![Coin {
                        denom: tribute.funds.denom.clone(),
                        amount: amount_to_distribute.to_uint_floor(),
                    }],
                };
                messages.push(send_msg);
            }
        }
    }
    Ok(messages)
}

pub fn calcul_protocol_comm_and_rest(
    payload: &ClaimTributeReplyPayload,
    constants: &zephyrus_core::state::Constants,
) -> (Uint128, Coin) {
    // deduct commission from the amount
    let commission_amount = Decimal::from_ratio(payload.amount.amount, 1u128)
        .saturating_mul(constants.commission_rate)
        .to_uint_ceil();
    let total_for_users = payload.amount.amount.saturating_sub(commission_amount);
    let user_funds = Coin {
        denom: payload.amount.denom.clone(),
        amount: total_for_users,
    };
    (commission_amount, user_funds)
}
