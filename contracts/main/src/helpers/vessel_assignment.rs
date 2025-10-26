use cosmwasm_std::Storage;
use zephyrus_core::msgs::{HydroLockId, HydromancerId, RoundId, TrancheId};

use crate::{errors::ContractError, state};

/// Comprehensive vessel assignment function that handles all TWS cleanup and vessel reassignment
/// it is assumed that the Unvote message is issued for re-assigned vessels, so TWS should be subtracted from previous proposals
pub fn assign_vessel_to_hydromancer(
    storage: &mut dyn Storage,
    vessel_id: HydroLockId,
    new_hydromancer_id: HydromancerId,
    current_round_id: RoundId,
    tranche_ids: &[TrancheId],
) -> Result<(), ContractError> {
    let mut vessel = state::get_vessel(storage, vessel_id)?;
    let old_hydromancer_id = vessel.hydromancer_id;

    if let Some(old_hydromancer_id) = old_hydromancer_id {
        // Early return if vessel is already assigned to this hydromancer
        if old_hydromancer_id == new_hydromancer_id {
            return Ok(());
        }

        state::remove_vessel_from_hydromancer(storage, old_hydromancer_id, vessel_id)?;
    }

    // Update vessel assignment
    vessel.hydromancer_id = Some(new_hydromancer_id);
    state::save_vessel(storage, vessel_id, &vessel)?;
    state::add_vessel_to_hydromancer(storage, new_hydromancer_id, vessel_id)?;

    // CRITICAL: Remove vessel from ALL active proposals if it has TWS, otherwise nothing left to do
    let Ok(vessel_shares) = state::get_vessel_shares_info(storage, current_round_id, vessel_id)
    else {
        return Ok(());
    };

    // Remove from all proposals across all tranches
    for &tranche_id in tranche_ids {
        if let Ok(Some(proposal_id)) =
            state::get_harbor_of_vessel(storage, tranche_id, current_round_id, vessel_id)
        {
            // Remove vessel TWS from proposal totals
            state::substract_time_weighted_shares_from_proposal(
                storage,
                current_round_id,
                proposal_id,
                &vessel_shares.token_group_id,
                vessel_shares.time_weighted_shares,
            )?;

            // Remove vessel TWS from hydromancer-specific proposal totals (if applicable)
            if let Some(old_hydro_id) = old_hydromancer_id {
                state::substract_time_weighted_shares_from_proposal_for_hydromancer(
                    storage,
                    proposal_id,
                    old_hydro_id,
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
                vessel_id,
            )?;
        }
    }

    // Remove from old hydromancer totals (if applicable)
    if let Some(old_hydro_id) = old_hydromancer_id {
        state::substract_time_weighted_shares_from_hydromancer(
            storage,
            old_hydro_id,
            current_round_id,
            &vessel_shares.token_group_id,
            vessel_shares.locked_rounds,
            vessel_shares.time_weighted_shares,
        )?;
    }

    // Add to new hydromancer totals
    state::add_time_weighted_shares_to_hydromancer(
        storage,
        new_hydromancer_id,
        current_round_id,
        &vessel_shares.token_group_id,
        vessel_shares.locked_rounds,
        vessel_shares.time_weighted_shares,
    )?;

    Ok(())
}

/// Assign vessel to user control (remove from hydromancer control)
pub fn assign_vessel_to_user_control(
    storage: &mut dyn Storage,
    vessel_id: HydroLockId,
    current_round_id: RoundId,
    tranche_ids: &[TrancheId],
) -> Result<(), ContractError> {
    let mut vessel = state::get_vessel(storage, vessel_id)?;

    // Early return if vessel is already under user control
    if vessel.is_under_user_control() {
        return Ok(());
    }

    let hydromancer_id = vessel.hydromancer_id.unwrap();

    // Update vessel to user control
    vessel.hydromancer_id = None;
    state::save_vessel(storage, vessel_id, &vessel)?;

    // Remove from hydromancer vessels mapping
    state::remove_vessel_from_hydromancer(storage, hydromancer_id, vessel_id)?;

    // CRITICAL: Remove vessel from ALL active proposals first if it has TWS, or nothing else to do
    let Ok(vessel_shares) = state::get_vessel_shares_info(storage, current_round_id, vessel_id)
    else {
        return Ok(());
    };

    // Remove from all proposals across all tranches
    for &tranche_id in tranche_ids {
        if let Ok(Some(proposal_id)) =
            state::get_harbor_of_vessel(storage, tranche_id, current_round_id, vessel_id)
        {
            // Remove vessel TWS from proposal totals
            state::substract_time_weighted_shares_from_proposal(
                storage,
                current_round_id,
                proposal_id,
                &vessel_shares.token_group_id,
                vessel_shares.time_weighted_shares,
            )?;

            // Remove vessel TWS from hydromancer-specific proposal totals
            state::substract_time_weighted_shares_from_proposal_for_hydromancer(
                storage,
                proposal_id,
                hydromancer_id,
                &vessel_shares.token_group_id,
                vessel_shares.time_weighted_shares,
            )?;

            // Remove vessel harbor mapping
            state::remove_vessel_harbor(
                storage,
                tranche_id,
                current_round_id,
                proposal_id,
                vessel_id,
            )?;
        }
    }

    // Remove from hydromancer totals
    state::substract_time_weighted_shares_from_hydromancer(
        storage,
        hydromancer_id,
        current_round_id,
        &vessel_shares.token_group_id,
        vessel_shares.locked_rounds,
        vessel_shares.time_weighted_shares,
    )?;

    Ok(())
}

/// Categorize vessels into those not yet controlled by the hydromancer vs already controlled
pub fn categorize_vessels_by_control(
    storage: &dyn Storage,
    new_hydromancer_id: u64,
    vessel_ids: &[u64],
) -> Result<(Vec<u64>, Vec<u64>), ContractError> {
    let mut not_controlled = Vec::new();
    let mut already_controlled = Vec::new();

    for &vessel_id in vessel_ids {
        let vessel = state::get_vessel(storage, vessel_id)?;

        if vessel.hydromancer_id == Some(new_hydromancer_id) {
            already_controlled.push(vessel_id);
        } else {
            not_controlled.push(vessel_id);
        }
    }

    Ok((not_controlled, already_controlled))
}
