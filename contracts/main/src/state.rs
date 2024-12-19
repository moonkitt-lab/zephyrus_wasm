use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Decimal, DepsMut, Storage};
use cw_storage_plus::{Item, Map};

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

//sequences
const USER_ID: Item<u64> = Item::new("user_id");

const HYDROMANCER_ID: Item<u64> = Item::new("hydromancer_id");

// Every address in this list is an admin
const WHITELIST_ADMINS: Item<Vec<Addr>> = Item::new("whitelist_admins");

const HYDRO_CONFIG: Item<HydroConfig> = Item::new("hydro_config");

const HYDROMANCERS: Map<u64, Hydromancer> = Map::new("hydromancers");

const DEFAULT_HYDROMANCER_ID: Item<u64> = Item::new("default_hydromancer_id");

pub fn initialize_sequences(storage: &mut dyn Storage) -> Result<(), cosmwasm_std::StdError> {
    USER_ID.save(storage, &0)?;
    HYDROMANCER_ID.save(storage, &0)?;
    Ok(())
}
pub fn update_whitelist_admins(
    storage: &mut dyn Storage,
    whitelist_admins: Vec<Addr>,
) -> Result<(), cosmwasm_std::StdError> {
    WHITELIST_ADMINS.save(storage, &whitelist_admins)?;
    Ok(())
}

pub fn update_hydro_config(
    storage: &mut dyn Storage,
    hydro_config: HydroConfig,
) -> Result<(), cosmwasm_std::StdError> {
    HYDRO_CONFIG.save(storage, &hydro_config)?;
    Ok(())
}

pub fn insert_new_hydromancer(
    storage: &mut dyn Storage,
    hydromancer_address: Addr,
    hydromancer_name: String,
    hydromancer_commission_rate: Decimal,
) -> Result<u64, cosmwasm_std::StdError> {
    let mut hydromancer_id = HYDROMANCER_ID.load(storage)?;
    hydromancer_id += 1;
    let hydromancer = Hydromancer {
        hydromancer_id,
        address: hydromancer_address,
        name: hydromancer_name,
        commission_rate: hydromancer_commission_rate,
    };
    HYDROMANCER_ID.save(storage, &hydromancer_id)?;
    HYDROMANCERS.save(storage, hydromancer_id, &hydromancer)?;
    Ok(hydromancer_id)
}

pub fn save_default_hydroamancer_id(
    storage: &mut dyn Storage,
    default_hydromancer_id: u64,
) -> Result<(), cosmwasm_std::StdError> {
    DEFAULT_HYDROMANCER_ID.save(storage, &default_hydromancer_id)?;
    Ok(())
}

pub fn get_hydromancer(
    storage: &dyn Storage,
    hydromancer_id: u64,
) -> Result<Hydromancer, cosmwasm_std::StdError> {
    HYDROMANCERS.load(storage, hydromancer_id)
}

pub fn get_hydro_config(storage: &dyn Storage) -> Result<HydroConfig, cosmwasm_std::StdError> {
    HYDRO_CONFIG.load(storage)
}
