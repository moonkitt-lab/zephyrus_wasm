use std::collections::HashSet;

use crate::{errors::ContractError, state};
use cosmwasm_std::{Addr, Storage};
use hydro_interface::msgs::{LockupWithPerTrancheInfo, RoundLockPowerSchedule, TributeClaim};
use zephyrus_core::msgs::{HydroLockId, HydromancerId, VesselsToHarbor};
use zephyrus_core::state::{Constants, Vessel};

/// Validate that the contract is not paused
pub fn validate_contract_is_not_paused(constants: &Constants) -> Result<(), ContractError> {
    if constants.paused_contract {
        return Err(ContractError::Paused);
    }
    Ok(())
}

/// Validate that the contract is paused
pub fn validate_contract_is_paused(constants: &Constants) -> Result<(), ContractError> {
    if !constants.paused_contract {
        return Err(ContractError::NotPaused);
    }
    Ok(())
}

/// Validate that a hydromancer exists
pub fn validate_hydromancer_exists(
    storage: &dyn Storage,
    hydromancer_id: HydromancerId,
) -> Result<(), ContractError> {
    if !state::hydromancer_exists(storage, hydromancer_id)? {
        return Err(ContractError::HydromancerNotFound {
            identifier: hydromancer_id.to_string(),
        });
    }
    Ok(())
}

/// Validate that vessels are under user control (not hydromancer controlled)
pub fn validate_vessels_under_user_control(
    storage: &dyn Storage,
    vessel_ids: &[HydroLockId],
) -> Result<(), ContractError> {
    for &vessel_id in vessel_ids {
        let vessel = state::get_vessel(storage, vessel_id)?;
        if vessel.hydromancer_id.is_some() {
            return Err(ContractError::VesselUnderHydromancerControl { vessel_id });
        }
    }
    Ok(())
}

/// Validate vote for duplicate harbor and vessel IDs
pub fn validate_vote_duplicates(vessels_harbors: &[VesselsToHarbor]) -> Result<(), ContractError> {
    use std::collections::HashSet;

    let mut seen_harbors = HashSet::new();
    let mut seen_vessels = HashSet::new();

    for vessels_to_harbor in vessels_harbors {
        // Check for duplicate harbor IDs
        if !seen_harbors.insert(vessels_to_harbor.harbor_id) {
            return Err(ContractError::DuplicateHarborId {
                harbor_id: vessels_to_harbor.harbor_id,
            });
        }

        // Check for duplicate vessel IDs
        for &vessel_id in &vessels_to_harbor.vessel_ids {
            if !seen_vessels.insert(vessel_id) {
                return Err(ContractError::DuplicateVesselId { vessel_id });
            }
        }
    }

    Ok(())
}

/// Generic function to validate no duplicate IDs in a slice
pub fn validate_no_duplicate_ids(ids: &[u64], id_type: &str) -> Result<(), ContractError> {
    use std::collections::HashSet;

    let mut seen_ids = HashSet::new();
    for &id in ids {
        if !seen_ids.insert(id) {
            return match id_type {
                "Vessel" => Err(ContractError::DuplicateVesselId { vessel_id: id }),
                "Harbor" => Err(ContractError::DuplicateHarborId { harbor_id: id }),
                _ => Err(ContractError::CustomError {
                    msg: format!("Duplicate {} ID: {}", id_type, id),
                }),
            };
        }
    }
    Ok(())
}

pub fn validate_admin_address(storage: &dyn Storage, sender: &Addr) -> Result<(), ContractError> {
    if !state::is_whitelisted_admin(storage, sender)? {
        return Err(ContractError::Unauthorized {});
    }
    Ok(())
}

pub fn validate_user_owns_vessels(
    storage: &dyn Storage,
    owner: &Addr,
    vessel_ids: &[u64],
) -> Result<(), ContractError> {
    if !state::are_vessels_owned_by(storage, owner, vessel_ids)? {
        return Err(ContractError::Unauthorized {});
    }
    Ok(())
}

pub fn validate_hydromancer_controls_vessels(
    storage: &dyn Storage,
    hydromancer_id: u64,
    vessel_ids: &[u64],
) -> Result<(), ContractError> {
    if !state::are_vessels_controlled_by_hydromancer(storage, hydromancer_id, vessel_ids)? {
        return Err(ContractError::Unauthorized {});
    }
    Ok(())
}

pub fn validate_vessels_not_tied_to_proposal(
    lockups_with_per_tranche_infos: &[LockupWithPerTrancheInfo],
) -> Result<(), ContractError> {
    if let Some(lockup_with_tranche_info) = lockups_with_per_tranche_infos.iter().find(|lockup| {
        lockup
            .per_tranche_info
            .iter()
            .any(|tranche| tranche.tied_to_proposal.is_some())
    }) {
        return Err(ContractError::VesselTiedToProposalNotTransferable {
            vessel_id: lockup_with_tranche_info.lock_with_power.lock_entry.lock_id,
        });
    }

    Ok(())
}

pub fn validate_lock_duration(
    round_lock_power_schedule: &RoundLockPowerSchedule,
    lock_epoch_length: u64,
    lock_duration: u64,
) -> Result<(), ContractError> {
    let lock_times = round_lock_power_schedule
        .round_lock_power_schedule
        .iter()
        .map(|entry| entry.locked_rounds * lock_epoch_length)
        .collect::<Vec<u64>>();

    if !lock_times.contains(&lock_duration) {
        return Err(ContractError::InvalidLockDuration {
            valid_durations: lock_times,
            provided_duration: lock_duration,
        });
    }

    Ok(())
}

// Validate that the user controls the vessel
// If the vessel is under user control, check that the user is the owner
// If the vessel is under hydromancer control, check that the user is the hydromancer
pub fn validate_user_controls_vessel(
    storage: &dyn Storage,
    user_addr: Addr,
    vessel: Vessel,
) -> Result<(), ContractError> {
    // vessel is under user control, check that user is the owner, otherwise check that user is the hydromancer of the vessel
    if vessel.is_under_user_control() {
        let user_id = state::get_user_id_by_address(storage, user_addr)?;
        if vessel.owner_id != user_id {
            return Err(ContractError::Unauthorized {});
        }
    } else {
        let hydromancer_id = state::get_hydromancer_id_by_address(storage, user_addr)?;
        if vessel.hydromancer_id != Some(hydromancer_id) {
            return Err(ContractError::Unauthorized {});
        }
    }
    Ok(())
}

pub fn validate_round_tranche_consistency(
    outstanding_tributes: &[TributeClaim],
    round_id: u64,
    tranche_id: u64,
) -> Result<(), ContractError> {
    for tribute in outstanding_tributes {
        if tribute.round_id != round_id || tribute.tranche_id != tranche_id {
            return Err(ContractError::CustomError {
                msg: "Round and tranche ID mismatch in tributes".to_string(),
            });
        }
    }
    Ok(())
}
