use cosmwasm_std::{Order, Storage};
use std::collections::{BTreeSet, HashMap};
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
    class_period: u64,
) -> Result<Vec<(HydroLockId, u64)>, ContractError> {
    let auto_maintained_vessels_by_class = state::get_vessel_ids_auto_maintained_by_class()?;

    // Collect all auto-maintained vessels with their target class periods
    let all_auto_maintained_vessels_by_class: BTreeSet<HydroLockId> =
        auto_maintained_vessels_by_class
            .load(storage, class_period)
            .unwrap_or_default();

    // Apply pagination
    let start_index = if let Some(start_vessel_id) = start_from_vessel_id {
        all_auto_maintained_vessels_by_class
            .iter()
            .position(|&vessel_id| vessel_id > start_vessel_id)
            .unwrap_or(all_auto_maintained_vessels_by_class.len())
    } else {
        0
    };

    let paginated_vessels_requiring_maintenance = all_auto_maintained_vessels_by_class
        .into_iter()
        .skip(start_index)
        .take(limit)
        .filter(|&vessel_id| {
            vessel_needs_auto_maintenance(
                storage,
                vessel_id,
                class_period,
                current_round_id,
                lock_epoch_length,
            )
        })
        .map(|vessel_id| (vessel_id, class_period))
        .collect();

    Ok(paginated_vessels_requiring_maintenance)
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
) -> bool {
    let Ok(vessel_shares) = state::get_vessel_shares_info(storage, current_round_id, vessel_id)
    else {
        // No vessel shares exist - needs maintenance
        return true;
    };

    // Vessel shares exist - check if locked_rounds * lock_epoch_length != target class period
    let vessel_effective_class_period = vessel_shares.locked_rounds * lock_epoch_length;
    vessel_effective_class_period != target_class_period
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
                )
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
