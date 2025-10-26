use crate::{errors::ContractError, helpers::hydro_queries::query_hydro_lockups_shares, state};
use cosmwasm_std::{DepsMut, Storage};
use hydro_interface::msgs::LockupsSharesInfo;
use std::cmp::Ordering;
use std::collections::HashMap;
use zephyrus_core::msgs::{HydroProposalId, HydromancerId, RoundId, TrancheId};
use zephyrus_core::state::{Constants, Vessel, VesselSharesInfo};

/// Batch hydromancer TWS changes in memory
pub fn batch_hydromancer_tws_changes(
    hydromancer_tws_changes: &mut HashMap<(HydromancerId, RoundId, String, u64), i128>,
    hydromancer_id: HydromancerId,
    current_round_id: RoundId,
    old_vessel_shares: &Option<VesselSharesInfo>,
    new_lockup_shares: &LockupsSharesInfo,
) {
    // Subtract old TWS
    if let Some(old_shares) = old_vessel_shares {
        if old_shares.time_weighted_shares > 0 {
            let key = (
                hydromancer_id,
                current_round_id,
                old_shares.token_group_id.clone(),
                old_shares.locked_rounds,
            );
            *hydromancer_tws_changes.entry(key).or_insert(0) -=
                old_shares.time_weighted_shares as i128;
        }
    }

    // Add new TWS
    if !new_lockup_shares.time_weighted_shares.is_zero() {
        let key = (
            hydromancer_id,
            current_round_id,
            new_lockup_shares.token_group_id.clone(),
            new_lockup_shares.locked_rounds,
        );
        *hydromancer_tws_changes.entry(key).or_insert(0) +=
            new_lockup_shares.time_weighted_shares.u128() as i128;
    }
}

#[derive(Default)]
pub struct TwsChanges {
    pub proposal_changes: HashMap<(HydroProposalId, String), i128>,
    pub proposal_hydromancer_changes: HashMap<(HydroProposalId, HydromancerId, String), i128>,
}

impl TwsChanges {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Batch proposal TWS changes in memory
pub fn batch_proposal_tws_changes(
    storage: &dyn Storage,
    tws_changes: &mut TwsChanges,
    vessel: &Vessel,
    old_vessel_shares: &Option<VesselSharesInfo>,
    new_lockup_shares: &LockupsSharesInfo,
    tranche_ids: &[TrancheId],
    current_round_id: RoundId,
) -> Result<(), ContractError> {
    for &tranche_id in tranche_ids {
        if let Ok(Some(proposal_id)) =
            state::get_harbor_of_vessel(storage, tranche_id, current_round_id, vessel.hydro_lock_id)
        {
            // Batch proposal total TWS changes
            if let Some(old_shares) = old_vessel_shares {
                // Subtract old TWS
                let key = (proposal_id, old_shares.token_group_id.clone());
                *tws_changes.proposal_changes.entry(key).or_insert(0) -=
                    old_shares.time_weighted_shares as i128;
            }

            // Add new TWS
            let key = (proposal_id, new_lockup_shares.token_group_id.clone());
            *tws_changes.proposal_changes.entry(key).or_insert(0) +=
                new_lockup_shares.time_weighted_shares.u128() as i128;

            // Batch hydromancer proposal TWS changes if applicable
            if let Some(hydromancer_id) = vessel.hydromancer_id {
                if let Some(old_shares) = old_vessel_shares {
                    // Subtract old TWS
                    let key = (
                        proposal_id,
                        hydromancer_id,
                        old_shares.token_group_id.clone(),
                    );
                    *tws_changes
                        .proposal_hydromancer_changes
                        .entry(key)
                        .or_insert(0) -= old_shares.time_weighted_shares as i128;
                }

                // Add new TWS
                let key = (
                    proposal_id,
                    hydromancer_id,
                    new_lockup_shares.token_group_id.clone(),
                );
                *tws_changes
                    .proposal_hydromancer_changes
                    .entry(key)
                    .or_insert(0) += new_lockup_shares.time_weighted_shares.u128() as i128;
            }
        }
    }
    Ok(())
}

pub fn apply_hydromancer_tws_changes(
    storage: &mut dyn Storage,
    hydromancer_tws_changes: HashMap<(HydromancerId, RoundId, String, u64), i128>,
) -> Result<(), ContractError> {
    for ((hydromancer_id, round_id, token_group_id, locked_rounds), tws_delta) in
        hydromancer_tws_changes
    {
        match tws_delta.cmp(&0) {
            Ordering::Greater => {
                state::add_time_weighted_shares_to_hydromancer(
                    storage,
                    hydromancer_id,
                    round_id,
                    &token_group_id,
                    locked_rounds,
                    tws_delta as u128,
                )?;
            }
            Ordering::Less => {
                state::substract_time_weighted_shares_from_hydromancer(
                    storage,
                    hydromancer_id,
                    round_id,
                    &token_group_id,
                    locked_rounds,
                    (-tws_delta) as u128,
                )?;
            }
            Ordering::Equal => {
                // No change needed when tws_delta is 0
            }
        }
    }
    Ok(())
}

/// Apply batched proposal TWS changes in single write operations
pub fn apply_proposal_tws_changes(
    storage: &mut dyn Storage,
    round_id: RoundId,
    proposal_tws_changes: HashMap<(HydroProposalId, String), i128>,
) -> Result<(), ContractError> {
    for ((proposal_id, token_group_id), tws_delta) in proposal_tws_changes {
        match tws_delta.cmp(&0) {
            Ordering::Greater => {
                state::add_time_weighted_shares_to_proposal(
                    storage,
                    round_id,
                    proposal_id,
                    &token_group_id,
                    tws_delta as u128,
                )?;
            }
            Ordering::Less => {
                state::substract_time_weighted_shares_from_proposal(
                    storage,
                    round_id,
                    proposal_id,
                    &token_group_id,
                    (-tws_delta) as u128,
                )?;
            }
            Ordering::Equal => {
                // No change needed when tws_delta is 0
            }
        }
    }
    Ok(())
}

/// Apply batched proposal hydromancer TWS changes in single write operations
pub fn apply_proposal_hydromancer_tws_changes(
    storage: &mut dyn Storage,
    proposal_hydromancer_tws_changes: HashMap<(HydroProposalId, HydromancerId, String), i128>,
) -> Result<(), ContractError> {
    for ((proposal_id, hydromancer_id, token_group_id), tws_delta) in
        proposal_hydromancer_tws_changes
    {
        match tws_delta.cmp(&0) {
            Ordering::Greater => {
                state::add_time_weighted_shares_to_proposal_for_hydromancer(
                    storage,
                    proposal_id,
                    hydromancer_id,
                    &token_group_id,
                    tws_delta as u128,
                )?;
            }
            Ordering::Less => {
                state::substract_time_weighted_shares_from_proposal_for_hydromancer(
                    storage,
                    proposal_id,
                    hydromancer_id,
                    &token_group_id,
                    (-tws_delta) as u128,
                )?;
            }
            Ordering::Equal => {
                // No change needed when tws_delta is 0
            }
        }
    }
    Ok(())
}

// Complete time weighted shares for the hydromancer, for the current round
// Only needs to be called when a Hydromancer votes
pub fn complete_hydromancer_time_weighted_shares(
    deps: &mut DepsMut,
    hydromancer_id: u64,
    constants: &Constants,
    current_round_id: RoundId,
) -> Result<(), ContractError> {
    if state::is_hydromancer_tws_complete(deps.storage, current_round_id, hydromancer_id) {
        return Ok(());
    }

    // Load all vessels for the hydromancer
    let vessels = state::get_vessels_by_hydromancer(deps.storage, hydromancer_id, 0, usize::MAX)?;

    // Query lockup shares for all hydromancer's vessels
    let lockups_shares_response = query_hydro_lockups_shares(
        &deps.as_ref(),
        constants,
        vessels.iter().map(|v| v.hydro_lock_id).collect(),
    )?;

    for lockup_shares in lockups_shares_response.lockups_shares_info {
        // if vessel shares info already exists it means that vessel was created and delegated to hydromancer before its vote it's weighted shares are already added, so we skip
        if state::has_vessel_shares_info(deps.storage, current_round_id, lockup_shares.lock_id) {
            continue;
        }
        state::save_vessel_shares_info(
            deps.storage,
            lockup_shares.lock_id,
            current_round_id,
            lockup_shares.time_weighted_shares.u128(),
            lockup_shares.token_group_id.clone(),
            lockup_shares.locked_rounds,
        )?;

        // Vessel has voting power
        if !lockup_shares.time_weighted_shares.is_zero() {
            state::add_time_weighted_shares_to_hydromancer(
                deps.storage,
                hydromancer_id,
                current_round_id,
                &lockup_shares.token_group_id,
                lockup_shares.locked_rounds,
                lockup_shares.time_weighted_shares.u128(),
            )?;
        }
    }

    // Mark as completed
    state::mark_hydromancer_tws_complete(deps.storage, current_round_id, hydromancer_id)?;

    Ok(())
}

/// Initialize time weighted shares for vessels that don't have them yet.
/// For vessels controlled by hydromancers, also updates the hydromancer's TWS.
pub fn initialize_vessel_tws(
    deps: &mut DepsMut,
    lock_ids: Vec<u64>,
    current_round_id: RoundId,
    constants: &Constants,
) -> Result<(), ContractError> {
    // Filter out vessels that already have TWS initialized for this round
    let missing_lock_ids: Vec<u64> = lock_ids
        .into_iter()
        .filter(|&lock_id| !state::has_vessel_shares_info(deps.storage, current_round_id, lock_id))
        .collect();

    if missing_lock_ids.is_empty() {
        return Ok(());
    }

    // Query TWS data from Hydro contract for missing vessels
    let lockups_shares_response =
        query_hydro_lockups_shares(&deps.as_ref(), constants, missing_lock_ids)?;

    // Process each vessel's TWS data
    for lockup_info in &lockups_shares_response.lockups_shares_info {
        // Save vessel TWS info
        state::save_vessel_shares_info(
            deps.storage,
            lockup_info.lock_id,
            current_round_id,
            lockup_info.time_weighted_shares.u128(),
            lockup_info.token_group_id.clone(),
            lockup_info.locked_rounds,
        )?;

        // Update hydromancer TWS if vessel is controlled by one
        let vessel = state::get_vessel(deps.storage, lockup_info.lock_id)?;
        if let Some(hydromancer_id) = vessel.hydromancer_id {
            state::add_time_weighted_shares_to_hydromancer(
                deps.storage,
                hydromancer_id,
                current_round_id,
                &lockup_info.token_group_id,
                lockup_info.locked_rounds,
                lockup_info.time_weighted_shares.u128(),
            )?;
        }
    }

    Ok(())
}

// Reset vessel vote by removing harbor mapping and substract TWS
// Typically called when a user unvotes a vessel
pub fn reset_vessel_vote(
    storage: &mut dyn Storage,
    vessel: Vessel,
    current_round_id: RoundId,
    tranche_id: TrancheId,
    proposal_id: HydroProposalId,
) -> Result<(), ContractError> {
    let vessel_shares =
        state::get_vessel_shares_info(storage, current_round_id, vessel.hydro_lock_id)
            .expect("Vessel shares for voted vessels should be initialized ");
    state::substract_time_weighted_shares_from_proposal(
        storage,
        current_round_id,
        proposal_id,
        &vessel_shares.token_group_id,
        vessel_shares.time_weighted_shares,
    )?;
    if !vessel.is_under_user_control() {
        let hydromancer_id = vessel.hydromancer_id.unwrap();
        state::substract_time_weighted_shares_from_proposal_for_hydromancer(
            storage,
            proposal_id,
            hydromancer_id,
            &vessel_shares.token_group_id,
            vessel_shares.time_weighted_shares,
        )?;
    }
    // Remove vessel harbor mapping
    state::remove_vessel_harbor(
        storage,
        tranche_id,
        current_round_id,
        proposal_id,
        vessel.hydro_lock_id,
    )?;
    Ok(())
}
