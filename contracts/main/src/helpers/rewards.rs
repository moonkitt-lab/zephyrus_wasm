use std::collections::HashMap;

use cosmwasm_std::{to_json_binary, Addr, Coin, Decimal, Storage, SubMsg, Uint128, WasmMsg};
use hydro_interface::msgs::{DenomInfoResponse, ExecuteMsg as HydroExecuteMsg};
use neutron_sdk::bindings::msg::NeutronMsg;
use zephyrus_core::{
    msgs::{
        ClaimTributeReplyPayload, HydroLockId, HydroProposalId, HydromancerId, RoundId,
        CLAIM_TRIBUTE_REPLY_ID,
    },
    state::Constants,
};

use crate::{errors::ContractError, state};

pub fn build_claim_tribute_sub_msg(
    round_id: u64,
    tranche_id: u64,
    vessel_ids: &Vec<u64>,
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
    let vessel_share_info = state::get_vessel_shares_info(storage, round_id, vessel_id)?;
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
