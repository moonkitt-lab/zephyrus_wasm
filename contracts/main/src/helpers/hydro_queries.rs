use std::collections::HashMap;

use crate::errors::ContractError;
use crate::helpers::vectors::join_u64_ids;
use cosmwasm_std::{Deps, Env, StdError, StdResult};
use hydro_interface::msgs::{
    CurrentRoundResponse, DenomInfoResponse, DerivativeTokenInfoProviderQueryMsg,
    HydroConstantsResponse, HydroQueryMsg, LockupVotingMetricsResponse, LockupWithPerTrancheInfo,
    OutstandingTributeClaimsResponse, Proposal, ProposalResponse, RoundProposalsResponse,
    SpecificTributesResponse, SpecificUserLockupsResponse,
    SpecificUserLockupsWithTrancheInfosResponse, TokenInfoProvider, TokenInfoProvidersResponse,
    TranchesResponse,
};
use zephyrus_core::msgs::{RoundId, TrancheId};
use zephyrus_core::state::Constants;

/// Query current round from Hydro contract
pub fn query_hydro_current_round(deps: &Deps, constants: &Constants) -> StdResult<RoundId> {
    let current_round_resp: CurrentRoundResponse = deps.querier.query_wasm_smart(
        constants.hydro_config.hydro_contract_address.to_string(),
        &HydroQueryMsg::CurrentRound {},
    )?;
    Ok(current_round_resp.round_id)
}

/// Query available tranches from Hydro contract
pub fn query_hydro_tranches(deps: &Deps, constants: &Constants) -> StdResult<Vec<TrancheId>> {
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

pub fn query_hydro_lockups_with_tranche_infos(
    deps: &Deps,
    env: &Env,
    constants: &Constants,
    vessel_ids: &[u64],
) -> StdResult<Vec<LockupWithPerTrancheInfo>> {
    let user_lockups_with_tranche_infos: SpecificUserLockupsWithTrancheInfosResponse =
        deps.querier.query_wasm_smart(
            constants.hydro_config.hydro_contract_address.to_string(),
            &HydroQueryMsg::SpecificUserLockupsWithTrancheInfos {
                address: env.contract.address.to_string(),
                lock_ids: vessel_ids.to_vec(),
            },
        )?;

    Ok(user_lockups_with_tranche_infos.lockups_with_per_tranche_infos)
}

pub fn query_hydro_lockups_shares(
    deps: &Deps,
    constants: &Constants,
    vessel_ids: Vec<u64>,
) -> StdResult<LockupVotingMetricsResponse> {
    let lockups_info: LockupVotingMetricsResponse = deps
        .querier
        .query_wasm_smart(
            constants.hydro_config.hydro_contract_address.to_string(),
            &HydroQueryMsg::LockupVotingMetrics {
                lock_ids: vessel_ids.clone(),
            },
        )
        .map_err(|e| {
            StdError::generic_err(format!(
                "Failed to get time weighted shares for vessels {} from hydro: {}",
                join_u64_ids(vessel_ids),
                e
            ))
        })?;
    Ok(lockups_info)
}

/// Query Hydro constants
pub fn query_hydro_constants(
    deps: &Deps,
    constants: &Constants,
) -> StdResult<HydroConstantsResponse> {
    let constant_response: HydroConstantsResponse = deps.querier.query_wasm_smart(
        constants.hydro_config.hydro_contract_address.to_string(),
        &HydroQueryMsg::Constants {},
    )?;
    Ok(constant_response)
}

/// Query specific user lockups from Hydro contract
pub fn query_hydro_specific_user_lockups(
    deps: &Deps,
    env: &Env,
    constants: &Constants,
    lock_ids: Vec<u64>,
) -> StdResult<SpecificUserLockupsResponse> {
    let user_specific_lockups: SpecificUserLockupsResponse = deps.querier.query_wasm_smart(
        constants.hydro_config.hydro_contract_address.to_string(),
        &HydroQueryMsg::SpecificUserLockups {
            address: env.contract.address.to_string(),
            lock_ids,
        },
    )?;
    Ok(user_specific_lockups)
}

pub fn query_hydro_outstanding_tribute_claims(
    deps: &Deps,
    env: Env,
    constants: &Constants,
    round_id: u64,
    tranche_id: u64,
) -> StdResult<OutstandingTributeClaimsResponse> {
    let outstanding_tribute_claims: OutstandingTributeClaimsResponse =
        deps.querier.query_wasm_smart(
            constants
                .hydro_config
                .hydro_tribute_contract_address
                .to_string(),
            &HydroQueryMsg::OutstandingTributeClaims {
                user_address: env.contract.address.to_string(),
                round_id,
                tranche_id,
            },
        )?;
    Ok(outstanding_tribute_claims)
}

pub fn query_hydro_derivative_token_info_providers(
    deps: &Deps,
    constants: &Constants,
    round_id: RoundId,
) -> StdResult<HashMap<String, DenomInfoResponse>> {
    let token_info_providers: TokenInfoProvidersResponse = deps.querier.query_wasm_smart(
        constants.hydro_config.hydro_contract_address.to_string(),
        &HydroQueryMsg::TokenInfoProviders {},
    )?;
    let mut providers: HashMap<String, DenomInfoResponse> = HashMap::new();

    for provider in token_info_providers.providers {
        if let TokenInfoProvider::Derivative(derivative) = provider {
            // Try to find cached denom info for the round
            let cached_denom_info = derivative.cache.get(&round_id);

            let denom_info = match cached_denom_info {
                Some(denom_info) => denom_info.clone(),
                None => {
                    // Cache is empty or doesn't contain the round, query the provider contract directly
                    deps.querier.query_wasm_smart(
                        derivative.contract.clone(),
                        &DerivativeTokenInfoProviderQueryMsg::DenomInfo { round_id },
                    )?
                }
            };

            providers.insert(denom_info.token_group_id.clone(), denom_info);
        }
    }
    Ok(providers)
}

pub fn query_hydro_proposal(
    deps: &Deps,
    constants: &Constants,
    round_id: u64,
    tranche_id: u64,
    proposal_id: u64,
) -> StdResult<Proposal> {
    let proposal: ProposalResponse = deps.querier.query_wasm_smart(
        constants.hydro_config.hydro_contract_address.to_string(),
        &HydroQueryMsg::Proposal {
            round_id,
            tranche_id,
            proposal_id,
        },
    )?;
    Ok(proposal.proposal)
}

pub fn query_hydro_round_all_proposals(
    deps: &Deps,
    constants: &Constants,
    round_id: RoundId,
    tranche_id: TrancheId,
) -> Result<Vec<Proposal>, ContractError> {
    let mut all_proposals = Vec::new();
    let mut start_from = 0u32;
    let limit = 100u32;
    let mut finished = false;

    while !finished {
        let response: RoundProposalsResponse = deps.querier.query_wasm_smart(
            constants.hydro_config.hydro_contract_address.clone(),
            &HydroQueryMsg::RoundProposals {
                round_id,
                tranche_id,
                start_from,
                limit,
            },
        )?;

        all_proposals.extend(response.proposals.clone());

        if response.proposals.len() < limit as usize {
            finished = true;
        }

        start_from += limit;
    }

    Ok(all_proposals)
}

pub fn query_hydro_specific_tributes(
    deps: &Deps,
    constants: &Constants,
    tribute_ids: Vec<u64>,
) -> StdResult<SpecificTributesResponse> {
    let specific_tributes: SpecificTributesResponse = deps.querier.query_wasm_smart(
        constants
            .hydro_config
            .hydro_tribute_contract_address
            .to_string(),
        &HydroQueryMsg::SpecificTributes { tribute_ids },
    )?;
    Ok(specific_tributes)
}
