use crate::helpers::vectors::join_u64_ids;
use cosmwasm_std::{Deps, Env, StdError, StdResult};
use hydro_interface::msgs::{
    CurrentRoundResponse, DenomInfoResponse, DerivativeTokenInfoProviderQueryMsg,
    HydroConstantsResponse, HydroQueryMsg, LockupWithPerTrancheInfo, LockupsSharesResponse,
    OutstandingTributeClaimsResponse, SpecificUserLockupsResponse,
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
) -> StdResult<LockupsSharesResponse> {
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
                "Failed to get time weighted shares for vessels {} from hydro: {}",
                join_u64_ids(vessel_ids),
                e
            ))
        })?;
    Ok(lockups_shares)
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
) -> StdResult<Vec<DenomInfoResponse>> {
    let token_info_providers: TokenInfoProvidersResponse = deps.querier.query_wasm_smart(
        constants.hydro_config.hydro_contract_address.to_string(),
        &HydroQueryMsg::TokenInfoProviders {},
    )?;
    let mut providers: Vec<DenomInfoResponse> = vec![];

    for provider in token_info_providers.providers {
        if let TokenInfoProvider::Derivative(derivative) = provider {
            for (round, denom_info) in derivative.cache {
                if round == round_id {
                    providers.push(denom_info);
                } else {
                    // DenomInfo is not cached for the round, query the provider contract to get the denom info for the round
                    let denom_info: DenomInfoResponse = deps.querier.query_wasm_smart(
                        derivative.contract.clone(),
                        &DerivativeTokenInfoProviderQueryMsg::DenomInfo { round_id },
                    )?;
                    providers.push(denom_info);
                }
            }
        }
    }
    Ok(providers)
}
