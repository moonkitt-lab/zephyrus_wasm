use std::collections::{BTreeSet, HashMap};

use cosmwasm_std::{
    to_json_binary, Addr, Api, BankMsg, Coin, Decimal, Deps, DepsMut, Storage, SubMsg, Uint128,
    WasmMsg,
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
    helpers::{
        hydro_queries::{
            query_hydro_derivative_token_info_providers, query_hydro_proposal,
            query_hydro_round_all_proposals,
        },
        hydromancer_tribute_data_loader::{DataLoader, StateDataLoader},
        tribute_queries::query_tribute_proposal_tributes,
    },
    state,
};

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
    api: &dyn Api,
) -> Result<SubMsg<NeutronMsg>, ContractError> {
    api.debug(&format!("ZEPH012: Building claim tribute sub msg - tribute_id: {}, amount: {:?}, balance_before: {:?}", 
        outstanding_tribute.tribute_id, outstanding_tribute.amount,
        balances.iter().find(|b| b.denom == outstanding_tribute.amount.denom)));

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
        .unwrap_or(Coin {
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

pub fn calculate_total_voting_power_of_hydromancer_on_proposal(
    storage: &dyn Storage,
    api: &dyn Api,
    hydromancer_id: HydromancerId,
    proposal_id: HydroProposalId,
    round_id: RoundId,
    token_info_provider: &HashMap<String, DenomInfoResponse>,
) -> Result<Decimal, ContractError> {
    let list_tws =
        state::get_hydromancer_proposal_time_weighted_shares(storage, proposal_id, hydromancer_id)?;

    api.debug(&format!(
        "ZEPH122: HYDROMANCER_TWS_DEBUG: hydromancer_id={}, proposal_id={}, list_tws={:?}",
        hydromancer_id, proposal_id, list_tws
    ));

    let mut total_voting_power = Decimal::zero();
    for (token_group_id, tws) in list_tws {
        let token_info = token_info_provider.get(&token_group_id).ok_or(
            ContractError::TokenInfoProviderNotFound {
                token_group_id: token_group_id.clone(),
                round_id,
            },
        )?;

        api.debug(&format!(
            "ZEPH123: TOKEN_INFO_DEBUG: token_group_id={}, tws={}, ratio={}",
            token_group_id, tws, token_info.ratio
        ));

        total_voting_power = total_voting_power
            .saturating_add(Decimal::from_ratio(tws, 1u128).saturating_mul(token_info.ratio));
    }

    api.debug(&format!(
        "ZEPH124: TOTAL_VP_DEBUG: hydromancer_id={}, proposal_id={}, total_voting_power={}",
        hydromancer_id, proposal_id, total_voting_power
    ));

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

    println!("ZEPH102: HYDROMANCER_TWS: hydromancer_id={}, round_id={}, locked_rounds={}, total_entries={}", 
        hydromancer_id, round_id, locked_rounds, list_tws.len());

    for ((locked_round, token_group_id), tws) in &list_tws {
        let token_info = token_info_provider.get(token_group_id).ok_or(
            ContractError::TokenInfoProviderNotFound {
                token_group_id: token_group_id.clone(),
                round_id,
            },
        )?;

        let voting_power_contribution =
            Decimal::from_ratio(*tws, 1u128).saturating_mul(token_info.ratio);

        if *locked_round < locked_rounds {
            println!("ZEPH103: HYDROMANCER_TWS_SKIP: hydromancer_id={}, locked_round={}, required={}, token_group_id={}, tws={}, contribution={} (SKIPPED)", 
                hydromancer_id, locked_round, locked_rounds, token_group_id, tws, voting_power_contribution);
            continue;
        }

        total_voting_power = total_voting_power.saturating_add(voting_power_contribution);

        println!("ZEPH104: HYDROMANCER_TWS_ADD: hydromancer_id={}, locked_round={}, token_group_id={}, tws={}, ratio={}, contribution={}, total_so_far={}", 
            hydromancer_id, locked_round, token_group_id, tws, token_info.ratio, voting_power_contribution, total_voting_power);
    }

    println!(
        "ZEPH105: HYDROMANCER_TWS_FINAL: hydromancer_id={}, total_voting_power={}",
        hydromancer_id, total_voting_power
    );
    Ok(total_voting_power)
}

pub fn calcul_total_voting_power_on_proposal(
    storage: &dyn Storage,
    proposal_id: HydroProposalId,
    round_id: RoundId,
    token_info_provider: &HashMap<String, DenomInfoResponse>,
) -> Result<Decimal, ContractError> {
    let list_tws = state::get_proposal_time_weighted_shares(storage, proposal_id);
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

        // DEBUG: Log each contribution
        println!("ZEPH100: TWS_PROPOSAL: proposal_id={}, token_group_id={}, tws={}, ratio={}, contribution={}, total_so_far={}", 
            proposal_id, token_group_id, tws, token_info.ratio, voting_power_contribution, total_voting_power);
    }

    println!(
        "ZEPH101: TWS_PROPOSAL_FINAL: proposal_id={}, total_voting_power={}",
        proposal_id, total_voting_power
    );
    Ok(total_voting_power)
}

pub fn calcul_voting_power_of_vessel(
    storage: &dyn Storage,
    api: &dyn Api,
    vessel_id: HydroLockId,
    round_id: RoundId,
    token_info_provider: &HashMap<String, DenomInfoResponse>,
) -> Result<Decimal, ContractError> {
    // Vessel shares should exist, but if not, the voting power is 0 — though doing it this way might let some errors go unnoticed.
    let vessel_share_info = state::get_vessel_shares_info(storage, round_id, vessel_id);
    if vessel_share_info.is_err() {
        api.debug(&format!(
            "ZEPH106: VESSEL_TWS: vessel_id={}, round_id={}, ERROR: no shares found",
            vessel_id, round_id
        ));
        return Ok(Decimal::zero());
    }
    let vessel_share_info = vessel_share_info.unwrap();
    let token_info = token_info_provider
        .get(&vessel_share_info.token_group_id)
        .ok_or(ContractError::TokenInfoProviderNotFound {
            token_group_id: vessel_share_info.token_group_id.clone(),
            round_id,
        })?;
    let voting_power = Decimal::from_ratio(vessel_share_info.time_weighted_shares, 1u128)
        .saturating_mul(token_info.ratio);

    api.debug(&format!("ZEPH107: VESSEL_TWS: vessel_id={}, round_id={}, token_group_id={}, tws={}, ratio={}, voting_power={}", 
        vessel_id, round_id, vessel_share_info.token_group_id, vessel_share_info.time_weighted_shares, token_info.ratio, voting_power));

    Ok(voting_power)
}

#[allow(clippy::too_many_arguments)]
pub fn calcul_rewards_amount_for_vessel_on_proposal(
    deps: Deps<'_>,
    round_id: RoundId,
    tranche_id: TrancheId,
    proposal_id: HydroProposalId,
    tribute_id: TributeId,
    constants: &zephyrus_core::state::Constants,
    token_info_provider: &HashMap<String, hydro_interface::msgs::DenomInfoResponse>,
    total_proposal_voting_power: Decimal,
    proposal_rewards: Coin,
    vessel_id: u64,
    data_loader: &dyn DataLoader,
) -> Result<Decimal, ContractError> {
    deps.api.debug(&format!("ZEPH070: Calculating vessel reward - vessel_id: {}, proposal_id: {}, total_power: {}, rewards: {:?}", 
        vessel_id, proposal_id, total_proposal_voting_power, proposal_rewards));

    let vessel = state::get_vessel(deps.storage, vessel_id)?;
    let voting_power = calcul_voting_power_of_vessel(
        deps.storage,
        deps.api,
        vessel_id,
        round_id,
        token_info_provider,
    )?;

    deps.api.debug(&format!(
        "ZEPH071: Vessel {} voting power: {}, user_control: {}",
        vessel_id,
        voting_power,
        vessel.is_under_user_control()
    ));

    if vessel.is_under_user_control() {
        let vessel_harbor =
            state::get_harbor_of_vessel(deps.storage, tranche_id, round_id, vessel_id)?;
        deps.api.debug(&format!(
            "ZEPH072: Vessel {} harbor: {:?}",
            vessel_id, vessel_harbor
        ));

        if vessel_harbor.is_some() {
            let vessel_harbor = vessel_harbor.unwrap();

            if vessel_harbor == proposal_id {
                deps.api.debug(&format!(
                    "ZEPH073: Vessel {} voted for proposal {}, calculating portion",
                    vessel_id, proposal_id
                ));
                let vp_ratio = voting_power
                    .checked_div(total_proposal_voting_power)
                    .map_err(|_| ContractError::CustomError {
                        msg: "Division by zero in voting power calculation".to_string(),
                    })?;

                let portion =
                    vp_ratio.saturating_mul(Decimal::from_ratio(proposal_rewards.amount, 1u128));

                deps.api.debug(&format!("ZEPH108: VESSEL_USER_REWARD: vessel_id={}, voting_power={}, total_proposal_power={}, vp_ratio={}, proposal_rewards={}, portion={}", 
                    vessel_id, voting_power, total_proposal_voting_power, vp_ratio, proposal_rewards.amount, portion));

                deps.api.debug(&format!(
                    "ZEPH074: Vessel {} portion: {}",
                    vessel_id, portion
                ));
                return Ok(portion);
            } else {
                deps.api.debug(&format!(
                    "ZEPH075: Vessel {} voted for different proposal ({}), no rewards",
                    vessel_id, vessel_harbor
                ));
            }
        } else {
            deps.api.debug(&format!(
                "ZEPH076: Vessel {} has no harbor assignment",
                vessel_id
            ));
        }
        Ok(Decimal::zero())
    } else {
        deps.api.debug(&format!(
            "ZEPH077: Vessel {} under hydromancer control",
            vessel_id
        ));
        // Vessel is under hydromancer control, we don't care if it was used or not, it take a portion of hydromancer rewards

        // Vessel shares should exist, but if not, the voting power is 0 — though doing it this way might let some errors go unnoticed.
        let vessel_shares = state::get_vessel_shares_info(deps.storage, round_id, vessel_id);
        if vessel_shares.is_err() {
            deps.api.debug(&format!(
                "ZEPH078: Vessel {} shares not found for hydromancer {}",
                vessel_id,
                vessel.hydromancer_id.unwrap()
            ));
            return Ok(Decimal::zero());
        }
        let vessel_shares = vessel_shares.unwrap();
        let proposal = query_hydro_proposal(&deps, constants, round_id, tranche_id, proposal_id)?;

        deps.api.debug(&format!(
            "ZEPH078: Vessel {} locked_rounds: {}, proposal duration: {}",
            vessel_id, vessel_shares.locked_rounds, proposal.deployment_duration
        ));

        if proposal.deployment_duration <= vessel_shares.locked_rounds {
            let total_hydromancer_locked_rounds_voting_power =
                calcul_total_voting_power_of_hydromancer_for_locked_rounds(
                    deps.storage,
                    vessel.hydromancer_id.unwrap(),
                    round_id,
                    proposal.deployment_duration,
                    token_info_provider,
                )?;
            let rewards_allocated_to_hydromancer = data_loader.load_hydromancer_tribute(
                deps.storage,
                vessel.hydromancer_id.unwrap(),
                round_id,
                tribute_id,
            )?;

            deps.api.debug(&format!(
                "ZEPH079: Hydromancer total power: {}, allocated rewards: {:?}",
                total_hydromancer_locked_rounds_voting_power, rewards_allocated_to_hydromancer
            ));

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

                deps.api.debug(&format!("ZEPH109: VESSEL_HYDROMANCER_REWARD: vessel_id={}, voting_power={}, total_hydromancer_power={}, vp_ratio={}, hydromancer_rewards={}, portion={}",
                    vessel_id, voting_power, total_hydromancer_locked_rounds_voting_power, vp_ratio,
                    rewards_allocated_to_hydromancer.rewards_for_users.amount, portion));

                deps.api.debug(&format!(
                    "ZEPH080: Vessel {} hydromancer portion: {}",
                    vessel_id, portion
                ));
                return Ok(portion);
            } else {
                deps.api.debug(&format!(
                    "ZEPH081: No hydromancer rewards allocated for vessel {}",
                    vessel_id
                ));
            }
        } else {
            deps.api.debug(&format!(
                "ZEPH082: Vessel {} locked rounds insufficient for proposal duration",
                vessel_id
            ));
        }

        deps.api
            .debug(&format!("ZEPH083: Vessel {} gets zero rewards", vessel_id));
        Ok(Decimal::zero())
    }
}

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
        deps.api,
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

    deps.api.debug(&format!(
        "ZEPH120: COMMISSION_DEBUG: hydromancer_id={}, funds={}, total_hydromancer_reward={}, commission_rate={}",
        hydromancer_id, funds.amount, total_hydromancer_reward, hydromancer.commission_rate
    ));

    let hydromancer_commission =
        total_hydromancer_reward.saturating_mul(hydromancer.commission_rate);

    deps.api.debug(&format!(
        "ZEPH121: COMMISSION_DEBUG: hydromancer_commission_decimal={}, hydromancer_commission_uint={}",
        hydromancer_commission, hydromancer_commission.to_uint_floor()
    ));

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
    deps.api.debug(&format!("ZEPH060: Calculating vessel rewards - tribute_id: {}, proposal_id: {}, vessels: {:?}, rewards: {:?}", 
        tribute_id, proposal_id, vessel_ids, tribute_rewards));

    // Log which vessels are voting on which proposal
    for vessel_id in &vessel_ids {
        if let Ok(vessel) = state::get_vessel(deps.storage, *vessel_id) {
            deps.api.debug(&format!(
                "ZEPH061: VESSEL_VOTE_INFO: vessel_id={}, owner_id={}, hydromancer_id={:?}",
                vessel_id, vessel.owner_id, vessel.hydromancer_id
            ));
        }
    }

    let mut amount_to_distribute = Decimal::zero();
    deps.api.debug(&format!("ZEPH110: DISTRIBUTE_START: tribute_id={}, proposal_id={}, vessels={:?}, tribute_rewards={:?}", 
        tribute_id, proposal_id, vessel_ids, tribute_rewards));

    for vessel_id in vessel_ids.clone() {
        if !state::is_vessel_tribute_claimed(deps.storage, vessel_id, tribute_id, deps.api) {
            deps.api.debug(&format!(
                "ZEPH061: Processing unclaimed vessel {}",
                vessel_id
            ));

            let proposal_vessel_rewards = calcul_rewards_amount_for_vessel_on_proposal(
                deps.as_ref(),
                round_id,
                tranche_id,
                proposal_id,
                tribute_id,
                &constants,
                &token_info_provider,
                total_proposal_voting_power,
                tribute_rewards.clone(),
                vessel_id,
                &StateDataLoader {},
            )?;

            deps.api.debug(&format!(
                "ZEPH062: Vessel {} reward amount: {}",
                vessel_id, proposal_vessel_rewards
            ));

            amount_to_distribute = amount_to_distribute.saturating_add(proposal_vessel_rewards);

            let floored_vessel_reward = proposal_vessel_rewards.to_uint_floor();
            deps.api.debug(&format!("ZEPH111: DISTRIBUTE_VESSEL: vessel_id={}, reward_decimal={}, reward_floored={}, total_so_far={}", 
                vessel_id, proposal_vessel_rewards, floored_vessel_reward, amount_to_distribute));

            deps.api.debug(&format!(
                "ZEPH063: Saving vessel {} claim: {} {}",
                vessel_id, floored_vessel_reward, tribute_rewards.denom
            ));

            state::save_vessel_tribute_claim(
                deps.storage,
                vessel_id,
                tribute_id,
                Coin {
                    denom: tribute_rewards.denom.clone(),
                    amount: floored_vessel_reward,
                },
                deps.api,
            )?;
            if state::is_vessel_tribute_claimed(deps.storage, vessel_id, tribute_id, deps.api) {
                deps.api.debug(&format!(
                    "ZEPH063bis: Vessel {} mark as claimed for tribute {}",
                    vessel_id, tribute_id
                ));
            } else {
                deps.api.debug(&format!(
                    "ZEPH063ter: Vessel {} unexpectedly not mark as claimed for tribute {}",
                    vessel_id, tribute_id
                ));
            }
        } else {
            deps.api.debug(&format!(
                "ZEPH064: Vessel {} already claimed tribute {}",
                vessel_id, tribute_id
            ));
        }
    }

    deps.api.debug(&format!(
        "ZEPH0065: Total amount to distribute: {} for vessel_ids: {:?}",
        amount_to_distribute, vessel_ids
    ));
    Ok(amount_to_distribute)
}

// READONLY method This function is used to calculate the rewards for the vessels on a tribute
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
    deps.api.debug(&format!("ZEPH060:READONLY Calculating vessel rewards - tribute_id: {}, proposal_id: {}, vessels: {:?}, rewards: {:?}", 
        tribute_id, proposal_id, vessel_ids, tribute_rewards));

    let mut amount_to_distribute = Decimal::zero();
    for vessel_id in vessel_ids.clone() {
        if !state::is_vessel_tribute_claimed(deps.storage, vessel_id, tribute_id, deps.api) {
            deps.api.debug(&format!(
                "ZEPH061: Processing unclaimed vessel {}",
                vessel_id
            ));

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
                data_loader,
            )?;

            deps.api.debug(&format!(
                "ZEPH062:READONLY  Vessel {} reward amount: {}",
                vessel_id, proposal_vessel_rewards
            ));

            amount_to_distribute = amount_to_distribute.saturating_add(proposal_vessel_rewards);

            let floored_vessel_reward = proposal_vessel_rewards.to_uint_floor();
            deps.api.debug(&format!(
                "ZEPH063:READONLY Saving vessel {} claim: {} {}",
                vessel_id, floored_vessel_reward, tribute_rewards.denom
            ));
        } else {
            deps.api.debug(&format!(
                "ZEPH064:READONLY Vessel {} already claimed tribute {}",
                vessel_id, tribute_id
            ));
        }
    }

    deps.api.debug(&format!(
        "ZEPH065:READONLY Total amount to distribute: {}",
        amount_to_distribute
    ));
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
    deps.api.debug(&format!(
        "ZEPH040: Starting reward distribution - sender: {}, round: {}, tranche: {}, vessels: {:?}",
        sender, round_id, tranche_id, vessel_ids
    ));

    let token_info_provider =
        query_hydro_derivative_token_info_providers(&deps.as_ref(), &constants, round_id)?;
    let all_round_proposals =
        query_hydro_round_all_proposals(&deps.as_ref(), &constants, round_id, tranche_id)?;

    deps.api.debug(&format!(
        "ZEPH041: Found {} proposals for round {}",
        all_round_proposals.len(),
        round_id
    ));

    let mut messages: Vec<BankMsg> = vec![];
    for proposal in all_round_proposals {
        deps.api.debug(&format!(
            "ZEPH042: Processing proposal_id: {}",
            proposal.proposal_id
        ));

        let proposal_tributes = query_tribute_proposal_tributes(
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
        );
        // If the total proposal voting power is not found, we skip the proposal it means that zephyrus did not vote on the proposal
        if total_proposal_voting_power.is_err() {
            continue;
        }
        let total_proposal_voting_power = total_proposal_voting_power.unwrap();

        deps.api.debug(&format!(
            "ZEPH043: Proposal {} has {} tributes, total voting power: {}",
            proposal.proposal_id,
            proposal_tributes.len(),
            total_proposal_voting_power
        ));

        if total_proposal_voting_power.is_zero() {
            deps.api.debug(&format!(
                "ZEPH044.1: Skipping proposal {} (no voting power)",
                proposal.proposal_id
            ));

            continue;
        }

        for tribute in proposal_tributes {
            // tributes that have been just claimed will be processed in the reply handler, so we skip them here
            if tributes_process_in_reply.contains(&tribute.tribute_id) {
                deps.api.debug(&format!(
                    "ZEPH044.2: Skipping tribute {} (will be processed in reply)",
                    tribute.tribute_id
                ));
                continue;
            }

            deps.api.debug(&format!(
                "ZEPH045: Processing tribute_id: {}, amount: {:?}",
                tribute.tribute_id, tribute.funds
            ));
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

                deps.api.debug(&format!(
                    "ZEPH046: Calculated amount to distribute: {}, for vessel_ids: {:?}",
                    amount_to_distribute, vessel_ids
                ));

                reward_amount = amount_to_distribute.to_uint_floor();
            }

            if !reward_amount.is_zero() {
                deps.api.debug(&format!(
                    "ZEPH047: Creating send message for {} {} to {}",
                    reward_amount, tribute.funds.denom, sender
                ));
                let send_msg = BankMsg::Send {
                    to_address: sender.to_string(),
                    amount: vec![Coin {
                        denom: tribute.funds.denom.clone(),
                        amount: reward_amount,
                    }],
                };
                messages.push(send_msg);
            } else {
                deps.api
                    .debug("ZEPH048: No rewards to distribute (floored amount is zero)");
            }

            // Process the case that sender is an hydromancer and send its commission to the sender
            let hydromancer_rewards_send_msg = process_hydromancer_claiming_rewards(
                &mut deps,
                sender.clone(),
                round_id,
                tribute.tribute_id,
            )?;
            if let Some(send_msg) = hydromancer_rewards_send_msg {
                deps.api
                    .debug("ZEPH049: Adding hydromancer commission message");
                messages.push(send_msg);
            }
        }
    }

    deps.api.debug(&format!(
        "ZEPH050: Reward distribution completed, generated {} messages",
        messages.len()
    ));

    Ok(messages)
}

pub fn calcul_protocol_comm_and_rest(
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

pub fn process_hydromancer_claiming_rewards(
    deps: &mut DepsMut<'_>,
    sender: Addr,
    round_id: RoundId,
    tribute_id: TributeId,
) -> Result<Option<BankMsg>, ContractError> {
    deps.api.debug(&format!(
        "ZEPH090: Processing hydromancer rewards - sender: {}, tribute_id: {}",
        sender, tribute_id
    ));

    let hydromancer_id = state::get_hydromancer_id_by_address(deps.storage, sender.clone()).ok();
    if let Some(hydromancer_id) = hydromancer_id {
        deps.api.debug(&format!(
            "ZEPH091: Found hydromancer_id: {}",
            hydromancer_id
        ));

        if !state::is_hydromancer_tribute_claimed(deps.storage, hydromancer_id, tribute_id) {
            deps.api.debug(&format!(
                "ZEPH092: Hydromancer {} has not claimed tribute {}",
                hydromancer_id, tribute_id
            ));

            // Sender is an hydromancer, send its commission to the sender
            let hydromancer_tribute = state::get_hydromancer_rewards_by_tribute(
                deps.storage,
                hydromancer_id,
                round_id,
                tribute_id,
            )?;
            if let Some(hydromancer_tribute) = hydromancer_tribute {
                deps.api.debug(&format!(
                    "ZEPH093: Hydromancer commission: {:?}",
                    hydromancer_tribute.commission_for_hydromancer
                ));

                // Check if commission amount is greater than zero
                if !hydromancer_tribute
                    .commission_for_hydromancer
                    .amount
                    .is_zero()
                {
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
                    deps.api
                        .debug("ZEPH094: Returning hydromancer commission message");
                    return Ok(Some(send_to_hydromancer_msg));
                } else {
                    deps.api
                        .debug("ZEPH095: Hydromancer commission is zero, not sending");
                }
            } else {
                deps.api.debug(&format!(
                    "ZEPH096: No hydromancer tribute found for hydromancer {} tribute {}",
                    hydromancer_id, tribute_id
                ));
            }
        } else {
            deps.api.debug(&format!(
                "ZEPH097: Hydromancer {} already claimed tribute {}",
                hydromancer_id, tribute_id
            ));
        }
    } else {
        deps.api
            .debug(&format!("ZEPH098: Sender {} is not a hydromancer", sender));
    }
    deps.api.debug("ZEPH099: No hydromancer commission to send");
    Ok(None)
}

// READONLY method This function is used to calculate the rewards for the hydromancer on a tribute
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
