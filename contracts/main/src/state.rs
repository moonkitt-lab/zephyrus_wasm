use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Decimal, StdError, Storage};
use cw_storage_plus::{Item, Map};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use zephyrus_core::{
    msgs::{HydroProposalId, RoundId, TrancheId, UserControl},
    state::{Constants, HydroLockId, HydromancerId, UserId, Vessel},
};

use crate::errors::ContractError;

#[cw_serde]
pub struct Hydromancer {
    pub hydromancer_id: u64,
    pub address: Addr,
    pub name: String,
    pub commission_rate: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VesselHarbor {
    pub user_control: UserControl,
    pub steerer_id: u64,
    pub hydro_lock_id: HydroLockId,
}

impl PartialEq for VesselHarbor {
    fn eq(&self, other: &Self) -> bool {
        self.hydro_lock_id == other.hydro_lock_id
    }
}

impl Eq for VesselHarbor {}

impl PartialOrd for VesselHarbor {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.hydro_lock_id.cmp(&other.hydro_lock_id))
    }
}

impl Ord for VesselHarbor {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.hydro_lock_id.cmp(&other.hydro_lock_id)
    }
}

pub type TokenizedShareRecordId = u64;

// Sequences
const USER_NEXT_ID: Item<UserId> = Item::new("user_next_id");
const HYDROMANCER_NEXT_ID: Item<HydromancerId> = Item::new("hydromancer_next_id");

const CONSTANTS: Item<Constants> = Item::new("constants");

// Every address in this list is an admin
const WHITELIST_ADMINS: Item<Vec<Addr>> = Item::new("whitelist_admins");

const HYDROMANCERS: Map<HydromancerId, Hydromancer> = Map::new("hydromancers");
const HYDROMANCERID_BY_ADDR: Map<&str, HydromancerId> = Map::new("hydromancerid_address");

const VESSELS: Map<HydroLockId, Vessel> = Map::new("vessels");
// Addr as &str when used as a key allows for less cloning
const OWNER_VESSELS: Map<&str, BTreeSet<HydroLockId>> = Map::new("owner_vessels");

const TOKENIZED_SHARE_RECORDS: Map<TokenizedShareRecordId, HydroLockId> =
    Map::new("tokenized_share_records");

const HYDROMANCER_VESSELS: Map<HydromancerId, BTreeSet<HydroLockId>> =
    Map::new("hydromancer_vessels_ids");

const AUTO_MAINTAINED_VESSELS_BY_CLASS: Map<u64, BTreeSet<HydroLockId>> =
    Map::new("auto_maintained_vessels_by_class");

const VESSEL_TO_HARBOR: Map<(TrancheId, RoundId, HydroProposalId), BTreeSet<VesselHarbor>> =
    Map::new("vessel_to_harbor");
const VESSELS_UNDER_USER_CONTROL: Map<(TrancheId, RoundId), BTreeSet<HydroLockId>> =
    Map::new("vessels_under_user_control");

pub fn initialize_sequences(storage: &mut dyn Storage) -> Result<(), StdError> {
    USER_NEXT_ID.save(storage, &0)?;
    HYDROMANCER_NEXT_ID.save(storage, &0)?;
    Ok(())
}

pub fn update_constants(storage: &mut dyn Storage, constants: Constants) -> Result<(), StdError> {
    CONSTANTS.save(storage, &constants)?;
    Ok(())
}

pub fn get_constants(storage: &dyn Storage) -> Result<Constants, StdError> {
    CONSTANTS.load(storage)
}

pub fn update_whitelist_admins(
    storage: &mut dyn Storage,
    whitelist_admins: Vec<Addr>,
) -> Result<(), StdError> {
    WHITELIST_ADMINS.save(storage, &whitelist_admins)?;
    Ok(())
}

pub fn insert_new_hydromancer(
    storage: &mut dyn Storage,
    hydromancer_address: Addr,
    hydromancer_name: String,
    hydromancer_commission_rate: Decimal,
) -> Result<HydromancerId, StdError> {
    let hydromancer_id = HYDROMANCER_NEXT_ID.may_load(storage)?.unwrap_or_default();

    let hydromancer = Hydromancer {
        hydromancer_id,
        address: hydromancer_address.clone(),
        name: hydromancer_name,
        commission_rate: hydromancer_commission_rate,
    };
    HYDROMANCERS.save(storage, hydromancer_id, &hydromancer)?;

    HYDROMANCERID_BY_ADDR.save(storage, hydromancer_address.as_str(), &hydromancer_id)?;

    HYDROMANCER_NEXT_ID.save(storage, &(hydromancer_id + 1))?;

    Ok(hydromancer_id)
}

pub fn get_hydromancer(
    storage: &dyn Storage,
    hydromancer_id: HydromancerId,
) -> Result<Hydromancer, ContractError> {
    match HYDROMANCERS.load(storage, hydromancer_id) {
        Ok(hydromancer) => Ok(hydromancer),
        Err(_) => Err(ContractError::HydromancerNotFound { hydromancer_id }),
    }
}

pub fn get_hydromancer_id_by_address(
    storage: &dyn Storage,
    hydromancer_addr: Addr,
) -> Result<HydromancerId, StdError> {
    match HYDROMANCERID_BY_ADDR.load(storage, hydromancer_addr.as_str()) {
        Ok(hydromancer_id) => Ok(hydromancer_id),
        Err(_) => Err(StdError::generic_err(format!(
            "Hydromancer {} not found",
            hydromancer_addr
        ))),
    }
}

pub fn add_hydromancer(
    storage: &mut dyn Storage,
    hydromancer: &Hydromancer,
) -> Result<(), StdError> {
    HYDROMANCERS.save(storage, hydromancer.hydromancer_id, hydromancer)
}

pub fn hydromancer_exists(storage: &dyn Storage, hydromancer_id: HydromancerId) -> bool {
    HYDROMANCERS.has(storage, hydromancer_id)
}

pub fn add_vessel(
    storage: &mut dyn Storage,
    vessel: &Vessel,
    owner: &Addr,
) -> Result<(), StdError> {
    let vessel_id = vessel.hydro_lock_id;

    VESSELS.save(storage, vessel_id, vessel)?;

    let mut owner_vessels = OWNER_VESSELS
        .may_load(storage, owner.as_str())?
        .unwrap_or_default();

    owner_vessels.insert(vessel_id);

    OWNER_VESSELS.save(storage, owner.as_str(), &owner_vessels)?;

    let mut vessels_hydromancer = HYDROMANCER_VESSELS
        .may_load(storage, vessel.hydromancer_id)?
        .unwrap_or_default();

    vessels_hydromancer.insert(vessel_id);

    HYDROMANCER_VESSELS.save(storage, vessel.hydromancer_id, &vessels_hydromancer)?;

    if vessel.auto_maintenance {
        let mut vessels_class = AUTO_MAINTAINED_VESSELS_BY_CLASS
            .may_load(storage, vessel.class_period)?
            .unwrap_or_default();
        vessels_class.insert(vessel_id);
        AUTO_MAINTAINED_VESSELS_BY_CLASS.save(storage, vessel.class_period, &vessels_class)?;
    }

    TOKENIZED_SHARE_RECORDS.save(storage, vessel.tokenized_share_record_id, &vessel_id)?;

    Ok(())
}

pub fn is_tokenized_share_record_used(
    storage: &dyn Storage,
    tokenized_share_record_id: TokenizedShareRecordId,
) -> bool {
    TOKENIZED_SHARE_RECORDS.has(storage, tokenized_share_record_id)
}

pub fn add_vessel_to_harbor(
    storage: &mut dyn Storage,
    tranche_id: TrancheId,
    round_id: RoundId,
    proposal_id: HydroProposalId,
    vessel_harbor: &VesselHarbor,
) -> Result<(), StdError> {
    let vessels_harbor = VESSEL_TO_HARBOR
        .may_load(storage, (tranche_id, round_id, proposal_id))
        .unwrap_or_default();
    match vessels_harbor {
        Some(mut vessel_harbors) => {
            vessel_harbors.insert(vessel_harbor.clone());
            VESSEL_TO_HARBOR.save(
                storage,
                (tranche_id, round_id, proposal_id),
                &vessel_harbors,
            )?;
        }
        None => {
            let mut vessel_harbors = BTreeSet::new();
            vessel_harbors.insert(vessel_harbor.clone());
            VESSEL_TO_HARBOR.save(
                storage,
                (tranche_id, round_id, proposal_id),
                &vessel_harbors,
            )?;
        }
    }

    if vessel_harbor.user_control == true {
        let vessels_under_user_control = VESSELS_UNDER_USER_CONTROL
            .may_load(storage, (tranche_id, round_id))
            .unwrap_or_default();
        match vessels_under_user_control {
            Some(mut vessel_ids) => {
                vessel_ids.insert(vessel_harbor.hydro_lock_id);
                VESSELS_UNDER_USER_CONTROL.save(storage, (tranche_id, round_id), &vessel_ids)?;
            }
            None => {
                let mut vessel_ids = BTreeSet::new();
                vessel_ids.insert(vessel_harbor.hydro_lock_id);
                VESSELS_UNDER_USER_CONTROL.save(storage, (tranche_id, round_id), &vessel_ids)?;
            }
        }
    }

    Ok(())
}

pub fn get_harbor_of_vessel(
    storage: &dyn Storage,
    tranche_id: TrancheId,
    round_id: RoundId,
    hydro_lock_id: HydroLockId,
) -> Option<HydroProposalId> {
    let vessels_harbor_iter = VESSEL_TO_HARBOR
        .prefix((tranche_id, round_id))
        .range(storage, None, None, cosmwasm_std::Order::Ascending)
        .filter_map(|item| item.ok());
    for (harbor_id, harbors) in vessels_harbor_iter {
        if harbors.contains(&VesselHarbor {
            user_control: false,
            steerer_id: 0,
            hydro_lock_id,
        }) {
            return Some(harbor_id);
        }
    }
    None
}

pub fn remove_vessel_harbor(
    storage: &mut dyn Storage,
    tranche_id: TrancheId,
    round_id: RoundId,
    hydro_proposal_id: HydroLockId,
    hydro_lock_id: HydroLockId,
) -> Result<BTreeSet<VesselHarbor>, StdError> {
    VESSEL_TO_HARBOR.update(
        storage,
        (tranche_id, round_id, hydro_proposal_id),
        |vessels| match vessels {
            Some(mut vessels) => {
                vessels.remove(&VesselHarbor {
                    user_control: false,
                    steerer_id: 0,
                    hydro_lock_id,
                });
                Ok(vessels)
            }
            None => Err(StdError::generic_err("Vessel not found in harbor")),
        },
    )
}

pub fn is_vessel_under_user_control(
    storage: &dyn Storage,
    tranche_id: TrancheId,
    round_id: RoundId,
    hydro_lock_id: HydroLockId,
) -> bool {
    let vessels_under_user_control = VESSELS_UNDER_USER_CONTROL
        .may_load(storage, (tranche_id, round_id))
        .unwrap_or_default();

    match vessels_under_user_control {
        Some(vessel_ids) => vessel_ids.contains(&hydro_lock_id),
        None => false,
    }
}

pub fn get_vessel(storage: &dyn Storage, hydro_lock_id: HydroLockId) -> Result<Vessel, StdError> {
    VESSELS.load(storage, hydro_lock_id)
}

pub fn get_vessels_by_ids(
    storage: &dyn Storage,
    hydro_lock_ids: &[HydroLockId],
) -> Result<Vec<Vessel>, StdError> {
    hydro_lock_ids
        .iter()
        .map(|id| VESSELS.load(storage, *id))
        .collect()
}

pub fn get_vessels_by_owner(
    storage: &dyn Storage,
    owner: Addr,
    start_index: usize,
    limit: usize,
) -> Result<Vec<Vessel>, StdError> {
    // First try to load and handle the case where the owner has no vessels
    let vessel_ids: BTreeSet<u64> = OWNER_VESSELS
        .may_load(storage, owner.as_str())?
        .unwrap_or_default(); // Returns empty BTreeSet if not found

    vessel_ids
        .iter()
        .enumerate()
        .skip(start_index)
        .take(limit)
        .map(|id| {
            VESSELS.load(storage, *id.1).map_err(|e| {
                StdError::generic_err(format!("Failed to load vessel {}: {}", id.1, e))
            })
        })
        .collect()
}

pub fn get_vessels_by_hydromancer(
    storage: &dyn Storage,
    hydromancer_addr: Addr,
    start_index: usize,
    limit: usize,
) -> Result<Vec<Vessel>, StdError> {
    let hydromancer_id = get_hydromancer_id_by_address(storage, hydromancer_addr.clone())?;

    let vessel_ids = HYDROMANCER_VESSELS
        .may_load(storage, hydromancer_id)?
        .unwrap_or_default(); // Returns empty BTreeSet if not found

    vessel_ids
        .iter()
        .enumerate()
        .skip(start_index)
        .take(limit)
        .map(|id| {
            VESSELS.load(storage, *id.1).map_err(|e| {
                StdError::generic_err(format!("Failed to load vessel {}: {}", id.1, e))
            })
        })
        .collect()
}

pub fn get_vessels_id_by_class() -> Result<Map<u64, BTreeSet<HydroLockId>>, StdError> {
    Ok(AUTO_MAINTAINED_VESSELS_BY_CLASS)
}

pub fn modify_auto_maintenance(
    storage: &mut dyn Storage,
    hydro_lock_id: HydroLockId,
    auto_maintenance: bool,
) -> Result<(), ContractError> {
    let mut vessel = get_vessel(storage, hydro_lock_id)?;

    let old_auto_maintenance = vessel.auto_maintenance;

    // No change in auto_maintenance, nothing to do, return early
    if old_auto_maintenance == auto_maintenance {
        return Ok(());
    }

    vessel.auto_maintenance = auto_maintenance;
    VESSELS.save(storage, hydro_lock_id, &vessel)?;

    let mut auto_maintained_ids = AUTO_MAINTAINED_VESSELS_BY_CLASS
        .may_load(storage, vessel.class_period)?
        .unwrap_or_default();

    if old_auto_maintenance {
        auto_maintained_ids.remove(&hydro_lock_id);
    } else if auto_maintenance {
        auto_maintained_ids.insert(hydro_lock_id);
    }

    AUTO_MAINTAINED_VESSELS_BY_CLASS.save(storage, vessel.class_period, &auto_maintained_ids)?;

    Ok(())
}

pub fn remove_vessel(
    storage: &mut dyn Storage,
    owner: &Addr,
    hydro_lock_id: HydroLockId,
) -> Result<(), ContractError> {
    let vessel = get_vessel(storage, hydro_lock_id)?;

    VESSELS.remove(storage, hydro_lock_id);

    let mut owner_vessels = OWNER_VESSELS
        .may_load(storage, owner.as_str())?
        .unwrap_or_default();

    owner_vessels.remove(&hydro_lock_id);

    OWNER_VESSELS.save(storage, owner.as_str(), &owner_vessels)?;

    let mut vessels_hydromancer = HYDROMANCER_VESSELS
        .may_load(storage, vessel.hydromancer_id)?
        .unwrap_or_default();

    vessels_hydromancer.remove(&hydro_lock_id);

    HYDROMANCER_VESSELS.save(storage, vessel.hydromancer_id, &vessels_hydromancer)?;

    if vessel.auto_maintenance {
        let mut vessels_class = AUTO_MAINTAINED_VESSELS_BY_CLASS
            .may_load(storage, vessel.class_period)?
            .unwrap_or_default();

        vessels_class.remove(&hydro_lock_id);

        AUTO_MAINTAINED_VESSELS_BY_CLASS.save(storage, vessel.class_period, &vessels_class)?;
    }

    TOKENIZED_SHARE_RECORDS.remove(storage, vessel.tokenized_share_record_id);

    Ok(())
}

pub fn is_vessel_owned_by(
    storage: &dyn Storage,
    owner: &Addr,
    hydro_lock_id: HydroLockId,
) -> Result<bool, StdError> {
    let owner_vessels = OWNER_VESSELS
        .may_load(storage, owner.as_str())?
        .unwrap_or_default();
    Ok(owner_vessels.contains(&hydro_lock_id))
}

pub fn is_whitelisted_admin(storage: &dyn Storage, sender: &Addr) -> Result<bool, ContractError> {
    let whitelist_admins = WHITELIST_ADMINS.load(storage)?;
    Ok(whitelist_admins.contains(sender))
}

pub fn are_vessels_owned_by(
    storage: &dyn Storage,
    owner: &Addr,
    hydro_lock_ids: &[HydroLockId],
) -> Result<bool, StdError> {
    let owner_vessels = OWNER_VESSELS
        .may_load(storage, owner.as_str())?
        .unwrap_or_default();

    Ok(hydro_lock_ids
        .iter()
        .all(|&id_to_check| owner_vessels.contains(&id_to_check)))
}
