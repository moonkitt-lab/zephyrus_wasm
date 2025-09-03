use std::collections::HashMap;

use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Coin, Deps, Env, StdError, StdResult, Uint128,
};

use zephyrus_core::{
    msgs::{
        ConstantsResponse, HydromancerId, QueryMsg, RewardInfo, RoundId, TributeId,
        VesselHarborInfo, VesselHarborResponse, VesselsResponse, VesselsRewardsResponse,
        VotingPowerResponse,
    },
    state::HydromancerTribute,
};

use crate::{
    helpers::{
        hydro_queries::{
            query_hydro_derivative_token_info_providers, query_hydro_outstanding_tribute_claims,
            query_hydro_round_all_proposals,
        },
        hydromancer_tribute_data_loader::{DataLoader, InMemoryDataLoader, StateDataLoader},
        rewards::{
            allocate_rewards_to_hydromancer, calcul_protocol_comm_and_rest,
            calcul_total_voting_power_on_proposal, calculate_hydromancer_claiming_rewards,
            calculate_rewards_for_vessels_on_tribute,
        },
        tribute_queries::query_tribute_proposal_tributes,
        validation::validate_no_duplicate_ids,
    },
    state,
};

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
        QueryMsg::VesselsRewards {
            user_address,
            round_id,
            tranche_id,
            vessel_ids,
        } => to_json_binary(&query_vessels_rewards(
            deps,
            env,
            user_address,
            round_id,
            tranche_id,
            vessel_ids,
        )?),
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

// Query rewards for a user (if it's an hydromancer, it will be the commission) and vessels on a tranche and round, don't control if user own vessels to let an hydromancer query all rewards of its votes
pub fn query_vessels_rewards(
    deps: Deps,
    env: Env,
    user_address: String,
    round_id: u64,
    tranche_id: u64,
    vessel_ids: Vec<u64>,
) -> StdResult<VesselsRewardsResponse> {
    let user_address = deps.api.addr_validate(user_address.as_str())?;
    let constants = state::get_constants(deps.storage)?;
    let token_info_provider =
        query_hydro_derivative_token_info_providers(&deps, &constants, round_id)
            .map_err(|e| StdError::generic_err(e.to_string()))?;
    let all_round_proposals =
        query_hydro_round_all_proposals(&deps, &constants, round_id, tranche_id)
            .map_err(|e| StdError::generic_err(e.to_string()))?;

    let mut coins: Vec<RewardInfo> = vec![];
    let outstanding_tributes =
        query_hydro_outstanding_tribute_claims(&deps, env, &constants, round_id, tranche_id);
    for proposal in all_round_proposals {
        let proposal_tributes =
            query_tribute_proposal_tributes(&deps, &constants, round_id, proposal.proposal_id)
                .map_err(|e| StdError::generic_err(e.to_string()))?;
        let total_proposal_voting_power = calcul_total_voting_power_on_proposal(
            deps.storage,
            proposal.proposal_id,
            round_id,
            &token_info_provider,
        )
        .map_err(|e| StdError::generic_err(e.to_string()))?;

        for tribute in proposal_tributes {
            let tribute_processed = state::is_tribute_processed(deps.storage, tribute.tribute_id);
            let mut data_loader: Box<dyn DataLoader> = Box::new(StateDataLoader {});
            let zephyrus_rewards;
            if !tribute_processed {
                if let Ok(outstanding_tributes) = &outstanding_tributes {
                    let outstanding_tribute = outstanding_tributes
                        .claims
                        .iter()
                        .find(|t| t.tribute_id == tribute.tribute_id);
                    if let Some(outstanding_tribute) = outstanding_tribute {
                        zephyrus_rewards = outstanding_tribute.amount.clone();
                    } else {
                        // there is no outstanding tribute for this tribute, so there not yet rewards to distribute we can skip
                        continue;
                    }
                } else {
                    return Err(StdError::generic_err(
                        "Error querying outstanding claims on hydro",
                    ));
                }
            } else {
                zephyrus_rewards = state::get_tribute_processed(deps.storage, tribute.tribute_id)?
                    .expect("Tribute has been processed, Rewards should exist here");
            }

            let (_, users_funds) =
                calcul_protocol_comm_and_rest(zephyrus_rewards.clone(), &constants);

            if !tribute_processed {
                let hydromancer_ids = state::get_all_hydromancers(deps.storage)?;
                let mut hydromancer_rewards: HashMap<
                    (HydromancerId, RoundId, TributeId),
                    HydromancerTribute,
                > = HashMap::new();
                for hydromancer_id in hydromancer_ids {
                    let hydromancer_tribute = allocate_rewards_to_hydromancer(
                        deps,
                        proposal.proposal_id,
                        round_id,
                        users_funds.clone(),
                        &token_info_provider,
                        total_proposal_voting_power,
                        hydromancer_id,
                    )
                    .map_err(|e| StdError::generic_err(e.to_string()))?;
                    hydromancer_rewards.insert(
                        (hydromancer_id, round_id, tribute.tribute_id),
                        hydromancer_tribute,
                    );
                }
                data_loader = Box::new(InMemoryDataLoader {
                    hydromancer_tributes: hydromancer_rewards,
                });
            }

            // Cumulate rewards for each vessel
            let amount_to_distribute = calculate_rewards_for_vessels_on_tribute(
                deps,
                vessel_ids.clone(),
                tribute.tribute_id,
                tribute.tranche_id,
                tribute.round_id,
                tribute.proposal_id,
                users_funds.clone(),
                constants.clone(),
                token_info_provider.clone(),
                total_proposal_voting_power,
                &*data_loader,
            )
            .map_err(|e| StdError::generic_err(e.to_string()))?;

            let floored_amount = amount_to_distribute.to_uint_floor();
            let mut rewards_info = Option::None;
            if !floored_amount.is_zero() {
                let coin = Coin {
                    denom: tribute.funds.denom.clone(),
                    amount: floored_amount,
                };

                rewards_info = Some(RewardInfo {
                    coin,
                    tribute_id: tribute.tribute_id,
                    proposal_id: proposal.proposal_id,
                });
            }

            // Process the case that sender is an hydromancer and send its commission to the sender
            let hydromancer_rewards = calculate_hydromancer_claiming_rewards(
                deps,
                user_address.clone(),
                round_id,
                tribute.tribute_id,
                &*data_loader,
            )
            .map_err(|e| StdError::generic_err(e.to_string()))?;
            if let Some(hydromancer_rewards) = hydromancer_rewards {
                if let Some(mut rewards) = rewards_info {
                    rewards.coin.amount =
                        rewards.coin.amount.strict_add(hydromancer_rewards.amount);
                    rewards_info = Some(rewards);
                } else {
                    rewards_info = Some(RewardInfo {
                        coin: hydromancer_rewards,
                        tribute_id: tribute.tribute_id,
                        proposal_id: proposal.proposal_id,
                    });
                }
            }
            if let Some(rewards) = rewards_info {
                coins.push(rewards);
            }
        }
    }
    Ok(VesselsRewardsResponse {
        round_id,
        tranche_id,
        rewards: coins,
    })
}
