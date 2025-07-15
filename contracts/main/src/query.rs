use cosmwasm_std::{entry_point, to_json_binary, Binary, Deps, Env, StdError, StdResult};

use zephyrus_core::msgs::{
    ConstantsResponse, QueryMsg, VesselHarborInfo, VesselHarborResponse, VesselsResponse,
    VotingPowerResponse,
};

use crate::{helpers::validation::validate_no_duplicate_ids, state};

const MAX_PAGINATION_LIMIT: usize = 1000;
const DEFAULT_PAGINATION_LIMIT: usize = 100;

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

fn query_voting_power(_deps: Deps, _env: Env) -> Result<VotingPowerResponse, StdError> {
    todo!()
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
    validate_no_duplicate_ids(&vessel_ids, "Vessel")
        .map_err(|e| StdError::generic_err(e.to_string()))?;

    let mut vessels_harbor_info = vec![];
    for vessel_id in vessel_ids {
        if !state::vessel_exists(deps.storage, vessel_id) {
            return Err(StdError::not_found(format!(
                "Vessel {} does not exist",
                vessel_id
            )));
        }
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
