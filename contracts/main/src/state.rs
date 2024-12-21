use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Decimal, DepsMut, StdError, Storage};
use cw_storage_plus::{Item, Map};
use std::collections::BTreeSet;

use crate::errors::ContractError;

#[cw_serde]
pub struct HydroConfig {
    pub hydro_contract_address: Addr,
    pub hydro_tribute_contract_address: Addr,
}
#[cw_serde]
pub struct Hydromancer {
    pub hydromancer_id: u64,
    pub address: Addr,
    pub name: String,
    pub commission_rate: Decimal,
}

#[cw_serde]
pub struct Vessel {
    pub hydro_lock_id: HydroLockId,
    pub class_period: u64,
    pub auto_maintenance: bool,
    pub hydromancer_id: u64,
}

pub type UserId = u64;
pub type HydromancerId = u64;
pub type HydroLockId = u64; // This doesn't use a sequence, as we use lock_id returned by Hydro

// Sequences
const USER_NEXT_ID: Item<UserId> = Item::new("user_next_id");
const HYDROMANCER_NEXT_ID: Item<HydromancerId> = Item::new("hydromancer_next_id");

// Every address in this list is an admin
const WHITELIST_ADMINS: Item<Vec<Addr>> = Item::new("whitelist_admins");

const HYDRO_CONFIG: Item<HydroConfig> = Item::new("hydro_config");

const HYDROMANCERS: Map<HydromancerId, Hydromancer> = Map::new("hydromancers");
const DEFAULT_HYDROMANCER_ID: Item<HydromancerId> = Item::new("default_hydromancer_id");

const VESSELS: Map<HydroLockId, Vessel> = Map::new("vessels");
// Addr as &str when used as a key allows for less cloning
const OWNER_VESSELS: Map<&str, BTreeSet<HydroLockId>> = Map::new("owner_vessels");

pub fn initialize_sequences(storage: &mut dyn Storage) -> Result<(), StdError> {
    USER_NEXT_ID.save(storage, &0)?;
    HYDROMANCER_NEXT_ID.save(storage, &0)?;
    Ok(())
}

pub fn update_whitelist_admins(
    storage: &mut dyn Storage,
    whitelist_admins: Vec<Addr>,
) -> Result<(), StdError> {
    WHITELIST_ADMINS.save(storage, &whitelist_admins)?;
    Ok(())
}

pub fn update_hydro_config(
    storage: &mut dyn Storage,
    hydro_config: HydroConfig,
) -> Result<(), StdError> {
    HYDRO_CONFIG.save(storage, &hydro_config)?;
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
        address: hydromancer_address,
        name: hydromancer_name,
        commission_rate: hydromancer_commission_rate,
    };
    HYDROMANCERS.save(storage, hydromancer_id, &hydromancer)?;

    HYDROMANCER_NEXT_ID.save(storage, &(hydromancer_id + 1))?;

    Ok(hydromancer_id)
}

pub fn save_default_hydroamancer_id(
    storage: &mut dyn Storage,
    default_hydromancer_id: HydromancerId,
) -> Result<(), StdError> {
    DEFAULT_HYDROMANCER_ID.save(storage, &default_hydromancer_id)?;
    Ok(())
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

pub fn add_hydromancer(
    storage: &mut dyn Storage,
    hydromancer: &Hydromancer,
) -> Result<(), StdError> {
    HYDROMANCERS.save(storage, hydromancer.hydromancer_id, hydromancer)
}

pub fn get_hydro_config(storage: &dyn Storage) -> Result<HydroConfig, StdError> {
    HYDRO_CONFIG.load(storage)
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

    Ok(())
}

pub fn get_vessel(storage: &dyn Storage, hydro_lock_id: HydroLockId) -> Result<Vessel, StdError> {
    VESSELS.load(storage, hydro_lock_id)
}
