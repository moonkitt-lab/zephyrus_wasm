use cosmwasm_std::{Order, Storage};
use std::collections::HashMap;
use zephyrus_core::msgs::{HydroLockId, RoundId};

use crate::{errors::ContractError, state};

/// Collect vessels that need auto maintenance with pagination
/// Uses the efficient AUTO_MAINTAINED_VESSELS_BY_CLASS index for optimal performance
pub fn collect_vessels_needing_auto_maintenance(
    storage: &dyn Storage,
    current_round_id: RoundId,
    start_from_vessel_id: Option<HydroLockId>,
    limit: usize,
    lock_epoch_length: u64,
) -> Result<Vec<(HydroLockId, u64)>, ContractError> {
    let auto_maintained_vessels_by_class = state::get_vessel_ids_auto_maintained_by_class()?;

    // Collect all auto-maintained vessels with their target class periods
    let mut all_auto_maintained_vessels: Vec<(HydroLockId, u64)> = Vec::new();

    for class_result in
        auto_maintained_vessels_by_class.range(storage, None, None, Order::Ascending)
    {
        let (target_class_period, vessel_ids_set) = class_result?;
        println!("target_class_period: {:?}", target_class_period);
        println!("vessel_ids_set: {:?}", vessel_ids_set);
        for vessel_id in vessel_ids_set {
            all_auto_maintained_vessels.push((vessel_id, target_class_period));
        }
    }

    // Sort by vessel ID for consistent pagination
    all_auto_maintained_vessels.sort_by_key(|(vessel_id, _)| *vessel_id);

    // Apply pagination
    let start_index = if let Some(start_vessel_id) = start_from_vessel_id {
        all_auto_maintained_vessels
            .binary_search_by_key(&start_vessel_id, |(vessel_id, _)| *vessel_id)
            .map(|i| i + 1) // Start from next vessel
            .unwrap_or_else(|i| i) // Or insertion point
    } else {
        0
    };

    let paginated_vessels: Vec<(HydroLockId, u64)> = all_auto_maintained_vessels
        .into_iter()
        .skip(start_index)
        .take(limit)
        .collect();

    // Filter to only vessels that actually need maintenance
    let mut vessels_needing_maintenance = Vec::new();

    for (vessel_id, target_class_period) in paginated_vessels {
        if vessel_needs_auto_maintenance(
            storage,
            vessel_id,
            target_class_period,
            current_round_id,
            lock_epoch_length,
        )? {
            vessels_needing_maintenance.push((vessel_id, target_class_period));
        }
    }

    Ok(vessels_needing_maintenance)
}

/// Check if a vessel needs auto maintenance for the current round
/// Returns true if the vessel's current locked_rounds (multiplied by lock_epoch_length) doesn't match target class period
/// or if the vessel has no shares initialized for this round
pub fn vessel_needs_auto_maintenance(
    storage: &dyn Storage,
    vessel_id: HydroLockId,
    target_class_period: u64,
    current_round_id: RoundId,
    lock_epoch_length: u64,
) -> Result<bool, ContractError> {
    // Check if vessel shares exist for this round
    match state::get_vessel_shares_info(storage, current_round_id, vessel_id) {
        Ok(vessel_shares) => {
            // Vessel shares exist - check if locked_rounds * lock_epoch_length != target class period
            let vessel_effective_class_period = vessel_shares.locked_rounds * lock_epoch_length;
            Ok(vessel_effective_class_period != target_class_period)
        }
        Err(_) => {
            // No vessel shares exist - needs maintenance
            Ok(true)
        }
    }
}

/// Check if there are more vessels needing maintenance after the last processed one
/// Uses the efficient AUTO_MAINTAINED_VESSELS_BY_CLASS index for optimal performance
pub fn check_has_more_vessels_needing_maintenance(
    storage: &dyn Storage,
    current_round_id: RoundId,
    last_processed_vessel_id: HydroLockId,
    lock_epoch_length: u64,
) -> Result<bool, ContractError> {
    let auto_maintained_vessels_by_class = state::get_vessel_ids_auto_maintained_by_class()?;

    // Look for any vessel with ID > last_processed_vessel_id that needs maintenance
    for class_result in
        auto_maintained_vessels_by_class.range(storage, None, None, Order::Ascending)
    {
        let (target_class_period, vessel_ids_set) = class_result?;

        for vessel_id in vessel_ids_set {
            if vessel_id > last_processed_vessel_id
                && vessel_needs_auto_maintenance(
                    storage,
                    vessel_id,
                    target_class_period,
                    current_round_id,
                    lock_epoch_length,
                )?
            {
                return Ok(true);
            }
        }
    }

    Ok(false)
}

/// Group vessels by their class period for batch processing
pub fn group_vessels_by_class_period(
    vessels: Vec<(HydroLockId, u64)>,
) -> HashMap<u64, Vec<HydroLockId>> {
    let mut vessels_by_class: HashMap<u64, Vec<HydroLockId>> = HashMap::new();

    for (vessel_id, class_period) in vessels {
        vessels_by_class
            .entry(class_period)
            .or_default()
            .push(vessel_id);
    }

    vessels_by_class
}
