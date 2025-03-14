use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Coin, Decimal, Order, StdError, StdResult, Storage};
use cw_storage_plus::{Item, Map};
use std::collections::BTreeSet;
use zephyrus_core::{
    msgs::{HydroProposalId, RoundId, TrancheId, UserId},
    state::{Constants, HydroLockId, HydromancerId, Vessel, VesselHarbor},
};

use crate::errors::ContractError;

#[cw_serde]
pub struct Hydromancer {
    pub hydromancer_id: u64,
    pub address: Addr,
    pub name: String,
    pub commission_rate: Decimal,
}

#[cw_serde]
pub struct User {
    pub user_id: UserId,
    pub address: Addr,
    pub claimable_rewards: Vec<Coin>,
}

pub type TokenizedShareRecordId = u64;

// Sequences
const USER_NEXT_ID: Item<UserId> = Item::new("user_next_id");
const HYDROMANCER_NEXT_ID: Item<HydromancerId> = Item::new("hydromancer_next_id");

const CONSTANTS: Item<Constants> = Item::new("constants");

// Every address in this list is an admin
const WHITELIST_ADMINS: Item<Vec<Addr>> = Item::new("whitelist_admins");

const USERS: Map<UserId, User> = Map::new("users");
const USERID_BY_ADDR: Map<&str, UserId> = Map::new("userid_address");

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

const VESSEL_TO_HARBOR: Map<((TrancheId, RoundId), HydroProposalId, HydroLockId), VesselHarbor> =
    Map::new("vessel_to_harbor");
const HARBOR_OF_VESSEL: Map<((TrancheId, RoundId), HydroLockId), HydroProposalId> =
    Map::new("harbor_of_vessel");
const VESSELS_UNDER_USER_CONTROL: Map<(TrancheId, RoundId), BTreeSet<HydroLockId>> =
    Map::new("vessels_under_user_control");

//Track time weighted shares
const HYDROMANCER_SHARES_BY_VALIDATOR: Map<((HydromancerId, TrancheId, RoundId), &str), u128> =
    Map::new("hydromancer_shares_by_validator");
const PROPOSAL_HYDROMANCER_SHARES_BY_VALIDATOR: Map<(HydroProposalId, HydromancerId, &str), u128> =
    Map::new("proposal_hydromancer_shares_by_validator");

const SHARES_UNDER_USER_CONTROL_BY_VALIDATOR: Map<(HydroProposalId, UserId, &str), u128> =
    Map::new("proposal_user_shares_by_validator");

const USER_HYDROMANCER_SHARES: Map<((UserId, HydromancerId, TrancheId), RoundId, &str), u128> =
    Map::new("hydromancer_shares");

pub fn get_hydromancer_shares_by_round(
    storage: &dyn Storage,
    hydromancer_id: HydromancerId,
    tranche_id: TrancheId,
    round_id: RoundId,
) -> Result<Vec<(String, u128)>, StdError> {
    HYDROMANCER_SHARES_BY_VALIDATOR
        .prefix((hydromancer_id, tranche_id, round_id))
        .range(storage, None, None, Order::Ascending)
        .map(|item| {
            let (key, value) = item?;
            Ok((key, value))
        })
        .collect::<StdResult<Vec<_>>>()
}

pub fn add_weighted_shares_to_user_hydromancer(
    storage: &mut dyn Storage,
    tranche_id: TrancheId,
    user_id: UserId,
    hydromancer_id: HydromancerId,
    round_id: RoundId,
    validator: &str,
    shares: u128,
) -> Result<(), StdError> {
    let current_shares = USER_HYDROMANCER_SHARES
        .load(
            storage,
            ((user_id, hydromancer_id, tranche_id), round_id, validator),
        )
        .unwrap_or_default();
    USER_HYDROMANCER_SHARES.save(
        storage,
        ((user_id, hydromancer_id, tranche_id), round_id, validator),
        &(current_shares + shares),
    )?;
    Ok(())
}

pub fn sub_weighted_shares_to_user_hydromancer(
    storage: &mut dyn Storage,
    user_id: UserId,
    tranche_id: TrancheId,
    hydromancer_id: HydromancerId,
    round_id: RoundId,
    validator: &str,
    shares: u128,
) -> Result<(), StdError> {
    let current_shares = USER_HYDROMANCER_SHARES
        .load(
            storage,
            ((user_id, hydromancer_id, tranche_id), round_id, validator),
        )
        .unwrap_or_default();
    if current_shares < shares {
        return Err(StdError::generic_err(format!(
            "Not enough shares to remove {} from user {} and validator {}",
            shares, user_id, validator
        )));
    }
    USER_HYDROMANCER_SHARES.save(
        storage,
        ((user_id, hydromancer_id, tranche_id), round_id, validator),
        &(current_shares - shares),
    )?;
    Ok(())
}

pub fn add_weighted_shares_under_user_control_for_proposal(
    storage: &mut dyn Storage,
    user_id: UserId,
    proposal_id: HydroProposalId,
    validator: &str,
    shares: u128,
) -> Result<(), StdError> {
    let current_shares = SHARES_UNDER_USER_CONTROL_BY_VALIDATOR
        .load(storage, (proposal_id, user_id, validator))
        .unwrap_or_default();
    SHARES_UNDER_USER_CONTROL_BY_VALIDATOR.save(
        storage,
        (proposal_id, user_id, validator),
        &(current_shares + shares),
    )?;
    Ok(())
}

pub fn sub_weighted_shares_under_user_control_for_proposal(
    storage: &mut dyn Storage,
    user_id: UserId,
    proposal_id: HydroProposalId,
    validator: &str,
    shares: u128,
) -> Result<(), StdError> {
    let current_shares = SHARES_UNDER_USER_CONTROL_BY_VALIDATOR
        .load(storage, (proposal_id, user_id, validator))
        .unwrap_or_default();
    if current_shares < shares {
        return Err(StdError::generic_err(format!(
            "Not enough shares to remove {} from user {} and validator {}",
            shares, user_id, validator
        )));
    }
    SHARES_UNDER_USER_CONTROL_BY_VALIDATOR.save(
        storage,
        (proposal_id, user_id, validator),
        &(current_shares - shares),
    )?;
    Ok(())
}

pub fn add_weighted_shares_to_hydromancer(
    storage: &mut dyn Storage,
    hydromancer_id: HydromancerId,
    tranche_id: TrancheId,
    round_id: RoundId,
    validator: &str,
    shares: u128,
) -> Result<(), StdError> {
    let current_shares = HYDROMANCER_SHARES_BY_VALIDATOR
        .load(storage, ((hydromancer_id, tranche_id, round_id), validator))
        .unwrap_or_default();
    HYDROMANCER_SHARES_BY_VALIDATOR.save(
        storage,
        ((hydromancer_id, tranche_id, round_id), validator),
        &(current_shares + shares),
    )?;
    Ok(())
}

pub fn sub_weighted_shares_to_hydromancer(
    storage: &mut dyn Storage,
    hydromancer_id: HydromancerId,
    tranche_id: TrancheId,
    round_id: RoundId,
    validator: &str,
    shares: u128,
) -> Result<(), StdError> {
    let current_shares = HYDROMANCER_SHARES_BY_VALIDATOR
        .load(storage, ((hydromancer_id, tranche_id, round_id), validator))
        .unwrap_or_default();
    if current_shares < shares {
        return Err(StdError::generic_err(format!(
            "Not enough shares to remove {} from hydromancer {} and validator {}",
            shares, hydromancer_id, validator
        )));
    }
    HYDROMANCER_SHARES_BY_VALIDATOR.save(
        storage,
        ((hydromancer_id, tranche_id, round_id), validator),
        &(current_shares - shares),
    )?;
    Ok(())
}

pub fn has_shares_for_hydromancer_and_round(
    storage: &mut dyn Storage,
    hydromancer_id: HydromancerId,
    tranche_id: TrancheId,
    round_id: RoundId,
) -> StdResult<bool> {
    let key_prefix = (hydromancer_id, tranche_id, round_id);

    // Vérifie s'il existe au moins une entrée avec ce (hydromancer_id, round_id)
    let has_data = HYDROMANCER_SHARES_BY_VALIDATOR
        .prefix(key_prefix)
        .keys(storage, None, None, cosmwasm_std::Order::Ascending)
        .next()
        .is_some();

    Ok(has_data)
}

pub fn add_weighted_shares_to_proposal_hydromancer(
    storage: &mut dyn Storage,
    hydromancer_id: HydromancerId,
    proposal_id: HydroProposalId,
    validator: &str,
    shares: u128,
) -> Result<(), StdError> {
    let current_shares = PROPOSAL_HYDROMANCER_SHARES_BY_VALIDATOR
        .load(storage, (proposal_id, hydromancer_id, validator))
        .unwrap_or_default();
    PROPOSAL_HYDROMANCER_SHARES_BY_VALIDATOR.save(
        storage,
        (proposal_id, hydromancer_id, validator),
        &(current_shares + shares),
    )?;
    Ok(())
}

pub fn sub_weighted_shares_to_proposal_hydromancer(
    storage: &mut dyn Storage,
    hydromancer_id: HydromancerId,
    proposal_id: HydroProposalId,
    validator: &str,
    shares: u128,
) -> Result<(), StdError> {
    let current_shares = PROPOSAL_HYDROMANCER_SHARES_BY_VALIDATOR
        .load(storage, (proposal_id, hydromancer_id, validator))
        .unwrap_or_default();
    if current_shares < shares {
        return Err(StdError::generic_err(format!(
            "Not enough shares to remove {} from hydromancer {} and validator {}",
            shares, hydromancer_id, validator
        )));
    }
    PROPOSAL_HYDROMANCER_SHARES_BY_VALIDATOR.save(
        storage,
        (proposal_id, hydromancer_id, validator),
        &(current_shares - shares),
    )?;
    Ok(())
}

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

pub fn get_vessel_harbor(
    storage: &dyn Storage,
    tranche_id: TrancheId,
    round_id: RoundId,
    hydro_lock_id: HydroLockId,
) -> Result<(VesselHarbor, HydroProposalId), StdError> {
    let proposal_id = HARBOR_OF_VESSEL.load(storage, ((tranche_id, round_id), hydro_lock_id))?;
    let vessel_harbor = VESSEL_TO_HARBOR.load(
        storage,
        ((tranche_id, round_id), proposal_id, hydro_lock_id),
    )?;
    Ok((vessel_harbor, proposal_id))
}

pub fn insert_new_user(storage: &mut dyn Storage, user_address: Addr) -> Result<UserId, StdError> {
    let user_id = get_user_id_by_address(storage, user_address.clone());
    match user_id {
        Ok(user_id) => Err(StdError::generic_err(format!(
            "User {} already exists with id {}",
            user_address, user_id
        ))),
        Err(_) => {
            //user id was not found, so we can create a new user
            let user_id = USER_NEXT_ID.may_load(storage)?.unwrap_or_default();

            let user = User {
                user_id,
                address: user_address.clone(),
                claimable_rewards: vec![],
            };
            USERS.save(storage, user_id, &user)?;

            USERID_BY_ADDR.save(storage, user_address.as_str(), &user_id)?;

            USER_NEXT_ID.save(storage, &(user_id + 1))?;

            Ok(user_id)
        }
    }
}

pub fn get_user_id_by_address(storage: &dyn Storage, user_addr: Addr) -> Result<UserId, StdError> {
    match USERID_BY_ADDR.load(storage, user_addr.as_str()) {
        Ok(user_id) => Ok(user_id),
        Err(_) => Err(StdError::generic_err(format!(
            "User {} not found",
            user_addr
        ))),
    }
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
    VESSEL_TO_HARBOR.save(
        storage,
        (
            (tranche_id, round_id),
            proposal_id,
            vessel_harbor.hydro_lock_id,
        ),
        vessel_harbor,
    )?;
    HARBOR_OF_VESSEL.save(
        storage,
        ((tranche_id, round_id), vessel_harbor.hydro_lock_id),
        &proposal_id,
    )?;
    if vessel_harbor.user_control {
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

pub fn get_vessel_to_harbor_by_harbor_id(
    storage: &dyn Storage,
    tranche_id: TrancheId,
    round_id: RoundId,
    hydro_proposal_id: HydroProposalId,
) -> Result<Vec<(HydroLockId, VesselHarbor)>, StdError> {
    VESSEL_TO_HARBOR
        .prefix(((tranche_id, round_id), hydro_proposal_id))
        .range(storage, None, None, Order::Ascending)
        .collect()
}

pub fn get_vessel_to_harbor(
    storage: &dyn Storage,
    tranche_id: TrancheId,
    round_id: RoundId,
    hydro_proposal_id: HydroProposalId,
    hydro_lock_id: HydroLockId,
) -> Result<VesselHarbor, StdError> {
    VESSEL_TO_HARBOR.load(
        storage,
        ((tranche_id, round_id), hydro_proposal_id, hydro_lock_id),
    )
}

pub fn get_harbor_of_vessel(
    storage: &dyn Storage,
    tranche_id: TrancheId,
    round_id: RoundId,
    hydro_lock_id: HydroLockId,
) -> Result<Option<HydroProposalId>, StdError> {
    HARBOR_OF_VESSEL.may_load(storage, ((tranche_id, round_id), hydro_lock_id))
}

pub fn remove_vessel_harbor(
    storage: &mut dyn Storage,
    tranche_id: TrancheId,
    round_id: RoundId,
    hydro_proposal_id: HydroLockId,
    hydro_lock_id: HydroLockId,
) -> Result<(), StdError> {
    let vessel_to_harbor = VESSEL_TO_HARBOR.load(
        storage,
        ((tranche_id, round_id), hydro_proposal_id, hydro_lock_id),
    )?;

    VESSEL_TO_HARBOR.remove(
        storage,
        ((tranche_id, round_id), hydro_proposal_id, hydro_lock_id),
    );
    HARBOR_OF_VESSEL.remove(storage, ((tranche_id, round_id), hydro_lock_id));
    if vessel_to_harbor.user_control {
        let mut vessels_under_user_control = VESSELS_UNDER_USER_CONTROL
            .may_load(storage, (tranche_id, round_id))?
            .unwrap_or_default();
        vessels_under_user_control.remove(&hydro_lock_id);
        VESSELS_UNDER_USER_CONTROL.save(
            storage,
            (tranche_id, round_id),
            &vessels_under_user_control,
        )?;
    }
    Ok(())
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

pub fn change_vessel_hydromancer(
    storage: &mut dyn Storage,
    tranche_id: TrancheId,
    hydro_lock_id: HydroLockId,
    current_round_id: RoundId,
    new_hydromancer_id: HydromancerId,
) -> Result<(), ContractError> {
    let mut vessel = get_vessel(storage, hydro_lock_id)?;

    let old_hydromancer_id = vessel.hydromancer_id;

    //if vesssel is under user control then it have to be removed from user control
    //we have to do it even if the new hydromancer is the same as the old one, because the user can give back control to hydromancer
    let mut vote_reseted = false;
    if is_vessel_under_user_control(storage, tranche_id, current_round_id, hydro_lock_id) {
        let hydro_proposal_id =
            get_harbor_of_vessel(storage, tranche_id, current_round_id, hydro_lock_id)?;

        if let Some(proposal_id) = hydro_proposal_id {
            remove_vessel_harbor(
                storage,
                tranche_id,
                current_round_id,
                proposal_id,
                hydro_lock_id,
            )?;
        }
        vote_reseted = true;
    }

    if old_hydromancer_id == new_hydromancer_id {
        return Ok(());
    }

    //new hydromancer is different from the old one so we have to reset the vote if it was not reseted before
    if !vote_reseted {
        let hydro_proposal_id =
            get_harbor_of_vessel(storage, tranche_id, current_round_id, hydro_lock_id)?;

        if let Some(proposal_id) = hydro_proposal_id {
            remove_vessel_harbor(
                storage,
                tranche_id,
                current_round_id,
                proposal_id,
                hydro_lock_id,
            )?;
        }
    }

    let mut old_hydromancer_vessels = HYDROMANCER_VESSELS
        .may_load(storage, old_hydromancer_id)?
        .unwrap_or_default();

    old_hydromancer_vessels.remove(&hydro_lock_id);

    HYDROMANCER_VESSELS.save(storage, old_hydromancer_id, &old_hydromancer_vessels)?;

    let mut new_hydromancer_vessels = HYDROMANCER_VESSELS
        .may_load(storage, new_hydromancer_id)?
        .unwrap_or_default();

    new_hydromancer_vessels.insert(hydro_lock_id);

    HYDROMANCER_VESSELS.save(storage, new_hydromancer_id, &new_hydromancer_vessels)?;

    vessel.hydromancer_id = new_hydromancer_id;

    VESSELS.save(storage, hydro_lock_id, &vessel)?;

    Ok(())
}

pub fn get_vessels_count_by_hydromancer(
    storage: &dyn Storage,
    hydromancer_id: HydromancerId,
) -> Result<usize, StdError> {
    let vessels = HYDROMANCER_VESSELS
        .load(storage, hydromancer_id)
        .unwrap_or_default();
    Ok(vessels.len())
}
