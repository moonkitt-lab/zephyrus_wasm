use std::collections::{HashMap, HashSet};

use cosmwasm_std::{
    to_json_binary, Addr, BankMsg, Coin, Decimal, Deps, DepsMut, Storage, SubMsg, Uint128, WasmMsg,
};
use hydro_interface::msgs::{DenomInfoResponse, ExecuteMsg as HydroExecuteMsg, TributeClaim};
use neutron_sdk::bindings::msg::NeutronMsg;
use zephyrus_core::{
    msgs::{
        ClaimTributeReplyPayload, HydroProposalId, HydromancerId, RoundId, TrancheId, TributeId,
        CLAIM_TRIBUTE_REPLY_ID,
    },
    state::{Constants, HydromancerTribute, VesselInfoSnapshot},
};

use crate::{
    errors::ContractError,
    helpers::{
        hydro_queries::{query_hydro_derivative_token_info_providers, query_hydro_proposal},
        hydromancer_tribute_data_loader::{DataLoader, StateDataLoader},
    },
    state,
};

/// Context object grouping the parameters required to compute a vessel's reward on a tribute
pub struct VesselRewardContext<'a> {
    pub round_id: RoundId,
    pub tranche_id: TrancheId,
    pub proposal_id: HydroProposalId,
    pub tribute_id: TributeId,
    pub constants: &'a zephyrus_core::state::Constants,
    pub token_info_provider: &'a HashMap<String, hydro_interface::msgs::DenomInfoResponse>,
    pub total_proposal_voting_power: Decimal,
    pub proposal_rewards: Coin,
}
/// Build claim tribute sub message for hydro tribute contract
#[allow(clippy::too_many_arguments)]
pub fn build_claim_tribute_sub_msg(
    round_id: u64,
    tranche_id: u64,
    vessel_ids: &[u64],
    owner: &Addr,
    constants: &Constants,
    contract_address: &Addr,
    balances: &[Coin],
    outstanding_tribute: &hydro_interface::msgs::TributeClaim,
) -> Result<SubMsg<NeutronMsg>, ContractError> {
    let claim_msg = HydroExecuteMsg::ClaimTribute {
        round_id,
        tranche_id,
        tribute_id: outstanding_tribute.tribute_id,
        voter_address: contract_address.to_string(),
    };
    let execute_claim_msg = WasmMsg::Execute {
        contract_addr: constants
            .hydro_config
            .hydro_tribute_contract_address
            .to_string(),
        msg: to_json_binary(&claim_msg)?,
        funds: vec![],
    };
    let balance_before_claim = balances
        .iter()
        .find(|balance| balance.denom == outstanding_tribute.amount.denom)
        .cloned()
        .unwrap_or_else(|| Coin {
            denom: outstanding_tribute.amount.denom.clone(),
            amount: Uint128::zero(),
        });

    let payload = ClaimTributeReplyPayload {
        proposal_id: outstanding_tribute.proposal_id,
        tribute_id: outstanding_tribute.tribute_id,
        round_id,
        tranche_id,
        amount: outstanding_tribute.amount.clone(),
        balance_before_claim: balance_before_claim.clone(),
        vessels_owner: owner.clone(),
        vessel_ids: vessel_ids.to_owned(),
    };
    let sub_msg: SubMsg<NeutronMsg> =
        SubMsg::reply_on_success(execute_claim_msg, CLAIM_TRIBUTE_REPLY_ID)
            .with_payload(to_json_binary(&payload)?);
    Ok(sub_msg)
}

/// Calculate the total voting power of a hydromancer for a specific proposal.
/// Use token info providers to get the ratio of the token group of each tws of vessels
pub fn calculate_total_voting_power_of_hydromancer_on_proposal(
    storage: &dyn Storage,
    hydromancer_id: HydromancerId,
    proposal_id: HydroProposalId,
    round_id: RoundId,
    token_info_provider: &HashMap<String, DenomInfoResponse>,
) -> Result<Decimal, ContractError> {
    let list_tws =
        state::get_hydromancer_proposal_time_weighted_shares(storage, proposal_id, hydromancer_id)?;

    let mut total_voting_power = Decimal::zero();
    for (token_group_id, tws) in list_tws {
        let token_info = token_info_provider.get(&token_group_id).ok_or(
            ContractError::TokenInfoProviderNotFound {
                token_group_id: token_group_id.clone(),
                round_id,
            },
        )?;

        total_voting_power = total_voting_power
            .saturating_add(Decimal::from_ratio(tws, 1u128).saturating_mul(token_info.ratio));
    }
    Ok(total_voting_power)
}
/// Calculate the total voting power of a hydromancer for a specific number of locked rounds.
pub fn calculate_total_voting_power_of_hydromancer_for_locked_rounds(
    storage: &dyn Storage,
    hydromancer_id: HydromancerId,
    round_id: RoundId,
    locked_rounds: u64,
    token_info_provider: &HashMap<String, DenomInfoResponse>,
) -> Result<Decimal, ContractError> {
    let list_tws =
        state::get_hydromancer_time_weighted_shares_by_round(storage, round_id, hydromancer_id)?;
    let mut total_voting_power = Decimal::zero();

    for ((locked_round, token_group_id), tws) in &list_tws {
        if *locked_round < locked_rounds {
            continue;
        }
        let token_info = token_info_provider.get(token_group_id).ok_or(
            ContractError::TokenInfoProviderNotFound {
                token_group_id: token_group_id.clone(),
                round_id,
            },
        )?;
        let voting_power_contribution =
            Decimal::from_ratio(*tws, 1u128).saturating_mul(token_info.ratio);

        total_voting_power = total_voting_power.saturating_add(voting_power_contribution);
    }

    Ok(total_voting_power)
}

/// Calculate the total voting power of a proposal.
pub fn calculate_total_voting_power_on_proposal(
    storage: &dyn Storage,
    proposal_id: HydroProposalId,
    round_id: RoundId,
    token_info_provider: &HashMap<String, DenomInfoResponse>,
) -> Result<Decimal, ContractError> {
    let list_tws = state::get_proposal_time_weighted_shares(storage, round_id, proposal_id);
    let list_tws = list_tws.unwrap();
    let mut total_voting_power = Decimal::zero();

    // DEBUG: Log all TWS for this proposal
    for (token_group_id, tws) in &list_tws {
        let token_info = token_info_provider.get(token_group_id).ok_or(
            ContractError::TokenInfoProviderNotFound {
                token_group_id: token_group_id.clone(),
                round_id,
            },
        )?;
        let voting_power_contribution =
            Decimal::from_ratio(*tws, 1u128).saturating_mul(token_info.ratio);
        total_voting_power = total_voting_power.saturating_add(voting_power_contribution);
    }

    Ok(total_voting_power)
}

/// Calculate the voting power of a vessel for a specific round.
pub fn calculate_voting_power_of_vessel(
    round_id: RoundId,
    token_info_provider: &HashMap<String, DenomInfoResponse>,
    vessel_snapshot: &VesselInfoSnapshot,
) -> Result<Decimal, ContractError> {
    let token_info = token_info_provider
        .get(&vessel_snapshot.token_group_id)
        .ok_or(ContractError::TokenInfoProviderNotFound {
            token_group_id: vessel_snapshot.token_group_id.clone(),
            round_id,
        })?;
    let voting_power = Decimal::from_ratio(vessel_snapshot.time_weighted_shares, 1u128)
        .saturating_mul(token_info.ratio);

    Ok(voting_power)
}

/// Calculate the rewards amount for a vessel on a specific tribute.
pub fn calculate_rewards_amount_for_vessel_on_tribute(
    deps: Deps<'_>,
    ctx: &VesselRewardContext,
    vessel_id: u64,
    data_loader: &dyn DataLoader,
) -> Result<Decimal, ContractError> {
    let vessel_snapshot = state::get_vessel_shares_info(deps.storage, ctx.round_id, vessel_id);
    if vessel_snapshot.is_err() {
        // Vessel snapshot should exist, but if not, the voting power is 0 — though doing it this way might let some errors go unnoticed.
        return Ok(Decimal::zero());
    }
    let vessel_snapshot = vessel_snapshot.unwrap();
    let voting_power =
        calculate_voting_power_of_vessel(ctx.round_id, ctx.token_info_provider, &vessel_snapshot)?;

    if vessel_snapshot.was_under_user_control() {
        let vessel_harbor =
            state::get_harbor_of_vessel(deps.storage, ctx.tranche_id, ctx.round_id, vessel_id)?;

        if let Some(vessel_harbor) = vessel_harbor {
            if vessel_harbor == ctx.proposal_id {
                let vp_ratio = voting_power
                    .checked_div(ctx.total_proposal_voting_power)
                    .map_err(|_| ContractError::CustomError {
                        msg: "Division by zero in voting power calculation".to_string(),
                    })?;

                let portion = vp_ratio
                    .saturating_mul(Decimal::from_ratio(ctx.proposal_rewards.amount, 1u128));

                return Ok(portion);
            }
        }
        Ok(Decimal::zero())
    } else {
        // Vessel is under hydromancer control, we don't care if it was used or not, it take a portion of hydromancer rewards
        let proposal = query_hydro_proposal(
            &deps,
            ctx.constants,
            ctx.round_id,
            ctx.tranche_id,
            ctx.proposal_id,
        )?;

        if proposal.deployment_duration <= vessel_snapshot.locked_rounds {
            let total_hydromancer_locked_rounds_voting_power =
                calculate_total_voting_power_of_hydromancer_for_locked_rounds(
                    deps.storage,
                    vessel_snapshot.hydromancer_id.unwrap(),
                    ctx.round_id,
                    proposal.deployment_duration,
                    ctx.token_info_provider,
                )?;
            let rewards_allocated_to_hydromancer = data_loader.load_hydromancer_tribute(
                deps.storage,
                vessel_snapshot.hydromancer_id.unwrap(),
                ctx.round_id,
                ctx.tribute_id,
            )?;

            if let Some(rewards_allocated_to_hydromancer) = rewards_allocated_to_hydromancer {
                let vp_ratio = voting_power
                    .checked_div(total_hydromancer_locked_rounds_voting_power)
                    .map_err(|_| ContractError::CustomError {
                        msg: "Division by zero in voting power calculation".to_string(),
                    })?;

                let portion = vp_ratio.saturating_mul(Decimal::from_ratio(
                    rewards_allocated_to_hydromancer.rewards_for_users.amount,
                    1u128,
                ));

                return Ok(portion);
            }
        }

        Ok(Decimal::zero())
    }
}
/// This methode calculate the portion of rewards (from a tribute) for a hydromancer and its commission
#[allow(clippy::too_many_arguments)]
pub fn allocate_rewards_to_hydromancer(
    deps: Deps<'_>,
    proposal_id: HydroProposalId,
    round_id: RoundId,
    funds: Coin,
    token_info_provider: &HashMap<String, hydro_interface::msgs::DenomInfoResponse>,
    total_proposal_voting_power: Decimal,
    hydromancer_id: u64,
) -> Result<HydromancerTribute, ContractError> {
    let hydromancer_voting_power = calculate_total_voting_power_of_hydromancer_on_proposal(
        deps.storage,
        hydromancer_id,
        proposal_id,
        round_id,
        token_info_provider,
    )?;
    let hydromancer_portion = hydromancer_voting_power
        .checked_div(total_proposal_voting_power)
        .map_err(|_| ContractError::CustomError {
            msg: "Division by zero in voting power calculation".to_string(),
        })?;
    let total_hydromancer_reward =
        Decimal::from_ratio(funds.amount, 1u128).saturating_mul(hydromancer_portion);

    let hydromancer = state::get_hydromancer(deps.storage, hydromancer_id)?;

    let hydromancer_commission =
        total_hydromancer_reward.saturating_mul(hydromancer.commission_rate);

    let rewards_for_users = total_hydromancer_reward
        .saturating_sub(hydromancer_commission)
        .to_uint_floor();

    let hydromancer_commission = hydromancer_commission.to_uint_floor();

    Ok(HydromancerTribute {
        rewards_for_users: Coin {
            denom: funds.denom.clone(),
            amount: rewards_for_users,
        },
        commission_for_hydromancer: Coin {
            denom: funds.denom.clone(),
            amount: hydromancer_commission,
        },
    })
}
/// Distribute the rewards for the vessels on a tribute
#[allow(clippy::too_many_arguments)]
pub fn distribute_rewards_for_vessels_on_tribute(
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
    let ctx = VesselRewardContext {
        round_id,
        tranche_id,
        proposal_id,
        tribute_id,
        constants: &constants,
        token_info_provider: &token_info_provider,
        total_proposal_voting_power,
        proposal_rewards: tribute_rewards.clone(),
    };

    for vessel_id in vessel_ids.clone() {
        if !state::is_vessel_tribute_claimed(deps.storage, vessel_id, tribute_id) {
            let proposal_vessel_rewards = calculate_rewards_amount_for_vessel_on_tribute(
                deps.as_ref(),
                &ctx,
                vessel_id,
                &StateDataLoader {},
            )?;

            amount_to_distribute = amount_to_distribute.saturating_add(proposal_vessel_rewards);

            let floored_vessel_reward = proposal_vessel_rewards.to_uint_floor();

            state::save_vessel_tribute_claim(
                deps.storage,
                vessel_id,
                tribute_id,
                Coin {
                    denom: tribute_rewards.denom.clone(),
                    amount: floored_vessel_reward,
                },
            )?;
        }
    }

    Ok(amount_to_distribute)
}

/// READONLY method This function is used to calculate the rewards for the vessels on a tribute (readonly version of distribute_rewards_for_vessels_on_tribute)
#[allow(clippy::too_many_arguments)]
pub fn calculate_rewards_for_vessels_on_tribute(
    deps: Deps<'_>,
    vessel_ids: Vec<u64>,
    tribute_id: TributeId,
    tranche_id: TrancheId,
    round_id: RoundId,
    proposal_id: HydroProposalId,
    tribute_rewards: Coin,
    constants: zephyrus_core::state::Constants,
    token_info_provider: HashMap<String, hydro_interface::msgs::DenomInfoResponse>,
    total_proposal_voting_power: Decimal,
    data_loader: &dyn DataLoader,
) -> Result<Decimal, ContractError> {
    let mut amount_to_distribute = Decimal::zero();
    let ctx = VesselRewardContext {
        round_id,
        tranche_id,
        proposal_id,
        tribute_id,
        constants: &constants,
        token_info_provider: &token_info_provider,
        total_proposal_voting_power,
        proposal_rewards: tribute_rewards.clone(),
    };
    for vessel_id in vessel_ids.clone() {
        if !state::is_vessel_tribute_claimed(deps.storage, vessel_id, tribute_id) {
            let proposal_vessel_rewards =
                calculate_rewards_amount_for_vessel_on_tribute(deps, &ctx, vessel_id, data_loader)?;

            amount_to_distribute = amount_to_distribute.saturating_add(proposal_vessel_rewards);
        }
    }

    Ok(amount_to_distribute)
}
/// Distribute the rewards for all vessels for all tributes in params that should alreadyhave been claimed on hydro
pub fn distribute_rewards_for_all_tributes_already_claimed_on_hydro(
    mut deps: DepsMut<'_>,
    sender: Addr,
    round_id: u64,
    vessel_ids: Vec<u64>,
    constants: Constants,
    tributes_already_claimed_on_hydro: Vec<TributeClaim>,
) -> Result<Vec<BankMsg>, ContractError> {
    let token_info_provider =
        query_hydro_derivative_token_info_providers(&deps.as_ref(), &constants, round_id)?;

    let mut messages: Vec<BankMsg> = vec![];
    for tribute in tributes_already_claimed_on_hydro {
        // If the total proposal voting power is not found, we skip the proposal it means that zephyrus did not vote on the proposal
        let Ok(total_proposal_voting_power) = calculate_total_voting_power_on_proposal(
            deps.storage,
            tribute.proposal_id,
            round_id,
            &token_info_provider,
        ) else {
            continue;
        };

        if total_proposal_voting_power.is_zero() {
            continue;
        }

        let tribute_funds_after_commission =
            state::get_tribute_processed(deps.storage, tribute.tribute_id)?;

        let mut reward_amount = Uint128::zero();

        // It is possible that there is no tributes yet for this proposal (liquidity not yet deployed)
        if let Some(tribute_rewards) = tribute_funds_after_commission {
            // Cumulate rewards for each vessel
            let amount_to_distribute = distribute_rewards_for_vessels_on_tribute(
                &mut deps,
                vessel_ids.clone(),
                tribute.tribute_id,
                tribute.tranche_id,
                tribute.round_id,
                tribute.proposal_id,
                tribute_rewards,
                constants.clone(),
                token_info_provider.clone(),
                total_proposal_voting_power,
            )?;

            reward_amount = amount_to_distribute.to_uint_floor();
        }

        if !reward_amount.is_zero() {
            let send_msg = BankMsg::Send {
                to_address: sender.to_string(),
                amount: vec![Coin {
                    denom: tribute.amount.denom.clone(),
                    amount: reward_amount,
                }],
            };
            messages.push(send_msg);
        }

        // Process the case that the vessel owner is also the hydromancer and send its commission to the message sender
        let hydromancer_rewards_send_msg = process_hydromancer_claiming_rewards(
            &mut deps,
            sender.clone(),
            round_id,
            tribute.tribute_id,
        )?;

        if let Some(send_msg) = hydromancer_rewards_send_msg {
            messages.push(send_msg);
        }
    }

    Ok(messages)
}

/// Calculate the protocol commission and the rest of the amount
pub fn calculate_protocol_comm_and_rest(
    amount: Coin,
    constants: &zephyrus_core::state::Constants,
) -> (Uint128, Coin) {
    // deduct commission from the amount
    let commission_amount = Decimal::from_ratio(amount.amount, 1u128)
        .saturating_mul(constants.commission_rate)
        .to_uint_ceil();
    let total_for_users = amount.amount.saturating_sub(commission_amount);
    let user_funds = Coin {
        denom: amount.denom.clone(),
        amount: total_for_users,
    };
    (commission_amount, user_funds)
}
/// Process the hydromancer claiming its commission
pub fn process_hydromancer_claiming_rewards(
    deps: &mut DepsMut<'_>,
    sender: Addr,
    round_id: RoundId,
    tribute_id: TributeId,
) -> Result<Option<BankMsg>, ContractError> {
    let Ok(hydromancer_id) = state::get_hydromancer_id_by_address(deps.storage, sender.clone())
    else {
        return Ok(None);
    };

    if state::is_hydromancer_tribute_claimed(deps.storage, hydromancer_id, tribute_id) {
        return Ok(None);
    }

    let Some(hydromancer_tribute) = state::get_hydromancer_rewards_by_tribute(
        deps.storage,
        hydromancer_id,
        round_id,
        tribute_id,
    )?
    else {
        return Ok(None);
    };

    if hydromancer_tribute
        .commission_for_hydromancer
        .amount
        .is_zero()
    {
        return Ok(None);
    }

    // Sender is an hydromancer with an unclaimed, non-zero commission
    let send_to_hydromancer_msg = BankMsg::Send {
        to_address: sender.to_string(),
        amount: vec![hydromancer_tribute.commission_for_hydromancer.clone()],
    };

    state::save_hydromancer_tribute_claim(
        deps.storage,
        hydromancer_id,
        tribute_id,
        hydromancer_tribute.commission_for_hydromancer,
    )?;

    Ok(Some(send_to_hydromancer_msg))
}

/// READONLY method This function is used to calculate the rewards for the hydromancer on a tribute
pub fn calculate_hydromancer_claiming_rewards(
    deps: Deps<'_>,
    sender: Addr,
    round_id: RoundId,
    tribute_id: TributeId,
    data_loader: &dyn DataLoader,
) -> Result<Option<Coin>, ContractError> {
    let hydromancer_id = state::get_hydromancer_id_by_address(deps.storage, sender.clone()).ok();
    if let Some(hydromancer_id) = hydromancer_id {
        if !state::is_hydromancer_tribute_claimed(deps.storage, hydromancer_id, tribute_id) {
            // Sender is an hydromancer, send its commission to the sender
            let hydromancer_tribute = data_loader.load_hydromancer_tribute(
                deps.storage,
                hydromancer_id,
                round_id,
                tribute_id,
            )?;
            if let Some(hydromancer_tribute) = hydromancer_tribute {
                // Check if commission amount is greater than zero
                if !hydromancer_tribute
                    .commission_for_hydromancer
                    .amount
                    .is_zero()
                {
                    let coin = hydromancer_tribute.commission_for_hydromancer.clone();
                    return Ok(Some(coin));
                }
            }
        }
    }
    Ok(None)
}

pub fn get_current_balances_for_outstanding_tributes_denoms(
    deps: &DepsMut<'_>,
    contract_address: &Addr,
    outstanding_tributes: &[TributeClaim],
) -> Result<Vec<cosmwasm_std::Coin>, ContractError> {
    let mut tribute_denoms: HashSet<String> = HashSet::new();
    for tribute in outstanding_tributes.iter() {
        tribute_denoms.insert(tribute.amount.denom.clone());
    }
    let mut balances = Vec::new();
    for tribute_denom in tribute_denoms {
        let balance = deps
            .querier
            .query_balance(contract_address.clone(), tribute_denom.clone())?;
        balances.push(balance);
    }
    Ok(balances)
}
