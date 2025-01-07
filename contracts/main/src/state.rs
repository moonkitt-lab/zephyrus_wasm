use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Decimal, StdError, Storage};
use cw_storage_plus::{Item, Map};
use neutron_sdk::bindings::types::Height;
use std::collections::BTreeSet;
use zephyrus_core::msgs::{HydroLockId, HydromancerId, UserId, Vessel};

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
pub enum VesselOwnershipState {
    OwnedByUser { owner: String },
    OwnedByProtocol,
    TransferPending { next_owner: String },
}

pub type TokenizedShareRecordId = u64;

// Sequences
const USER_NEXT_ID: Item<UserId> = Item::new("user_next_id");
const HYDROMANCER_NEXT_ID: Item<HydromancerId> = Item::new("hydromancer_next_id");

// Every address in this list is an admin
const WHITELIST_ADMINS: Item<Vec<Addr>> = Item::new("whitelist_admins");

const HYDRO_CONFIG: Item<HydroConfig> = Item::new("hydro_config");

const HYDROMANCERS: Map<HydromancerId, Hydromancer> = Map::new("hydromancers");
const HYDROMANCERID_BY_ADDR: Map<&str, HydromancerId> = Map::new("hydromancerid_address");
const DEFAULT_HYDROMANCER_ID: Item<HydromancerId> = Item::new("default_hydromancer_id");

const VESSELS: Map<HydroLockId, Vessel> = Map::new("vessels");
const VESSEL_OWNERSHIP_STATE: Map<HydroLockId, VesselOwnershipState> =
    Map::new("vessel_ownership_state");
const VESSEL_TOKEN_OWNERSHIP_PROOF_MINIMUM_HEIGHT: Map<HydroLockId, Height> =
    Map::new("vessel_token_ownership_proof_minimum_height");

const TOKENIZED_SHARE_RECORDS: Map<TokenizedShareRecordId, bool> =
    Map::new("tokenized_share_records");

// Addr as &str when used as a key allows for less cloning
const OWNER_VESSELS: Map<&str, BTreeSet<HydroLockId>> = Map::new("owner_vessels");

const ESCROW_ICA_ADDRESS: Item<String> = Item::new("escrow_ica_address");

const HYDROMANCER_VESSELS: Map<HydromancerId, BTreeSet<HydroLockId>> =
    Map::new("hydromancer_vessels_ids");

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
        address: hydromancer_address.clone(),
        name: hydromancer_name,
        commission_rate: hydromancer_commission_rate,
    };
    HYDROMANCERS.save(storage, hydromancer_id, &hydromancer)?;

    HYDROMANCERID_BY_ADDR.save(storage, hydromancer_address.as_str(), &hydromancer_id)?;

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

pub fn hydromancer_exists(storage: &dyn Storage, hydromancer_id: HydromancerId) -> bool {
    HYDROMANCERS.has(storage, hydromancer_id)
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

    let mut vessels_hydromancer = HYDROMANCER_VESSELS
        .may_load(storage, vessel.hydromancer_id)?
        .unwrap_or_default();

    vessels_hydromancer.insert(vessel_id);

    HYDROMANCER_VESSELS.save(storage, vessel.hydromancer_id, &vessels_hydromancer)?;

    save_vessel_ownership_state(
        storage,
        vessel_id,
        VesselOwnershipState::OwnedByUser {
            owner: owner.as_str().to_owned(),
        },
    )?;

    TOKENIZED_SHARE_RECORDS.save(storage, vessel.tokenized_share_record_id, &true)?;

    Ok(())
}

pub fn get_vessel(storage: &dyn Storage, hydro_lock_id: HydroLockId) -> Result<Vessel, StdError> {
    VESSELS.load(storage, hydro_lock_id)
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

pub fn save_escrow_ica_address(
    storage: &mut dyn Storage,
    address: &String,
) -> Result<(), StdError> {
    ESCROW_ICA_ADDRESS.save(storage, address)
}

pub fn get_escrow_ica_address(storage: &dyn Storage) -> Result<Option<String>, StdError> {
    ESCROW_ICA_ADDRESS.may_load(storage)
}

pub fn save_vessel_token_ownership_proof_minimum_height(
    storage: &mut dyn Storage,
    hydro_lock_id: HydroLockId,
    height: Height,
) -> Result<(), StdError> {
    VESSEL_TOKEN_OWNERSHIP_PROOF_MINIMUM_HEIGHT.save(storage, hydro_lock_id, &height)
}

pub fn get_vessel_token_ownership_proof_minimum_height(
    storage: &dyn Storage,
    hydro_lock_id: HydroLockId,
) -> Result<Option<Height>, StdError> {
    VESSEL_TOKEN_OWNERSHIP_PROOF_MINIMUM_HEIGHT.may_load(storage, hydro_lock_id)
}

pub fn save_vessel_ownership_state(
    storage: &mut dyn Storage,
    hydro_lock_id: HydroLockId,
    state: VesselOwnershipState,
) -> Result<(), StdError> {
    VESSEL_OWNERSHIP_STATE.save(storage, hydro_lock_id, &state)
}

pub fn get_vessel_ownership_state(
    storage: &dyn Storage,
    hydro_lock_id: HydroLockId,
) -> Result<VesselOwnershipState, StdError> {
    VESSEL_OWNERSHIP_STATE
        .may_load(storage, hydro_lock_id)
        .map(|res| res.expect("vessel ownership status always set"))
}

pub fn is_tokenized_share_record_active(
    storage: &dyn Storage,
    tokenized_share_record_id: TokenizedShareRecordId,
) -> Result<bool, StdError> {
    TOKENIZED_SHARE_RECORDS
        .may_load(storage, tokenized_share_record_id)
        .map(Option::unwrap_or_default)
}

/// invariants:
/// - the vessel must have an owner
/// - the owner it must be a user, not the protocol
pub fn transfer_vessel_ownership_to_protocol(
    storage: &mut dyn Storage,
    hydro_lock_id: u64,
    escrow_ica_address: &str,
) -> Result<(), StdError> {
    let VesselOwnershipState::OwnedByUser { owner } = VESSEL_OWNERSHIP_STATE
        .may_load(storage, hydro_lock_id)?
        .expect("the vessel must have an owner")
    else {
        panic!("the owner of the vessel {hydro_lock_id} is not a user");
    };

    let mut owner_vessels = OWNER_VESSELS
        .may_load(storage, &owner)?
        .expect("the user must own at least this vessel");

    owner_vessels.remove(&hydro_lock_id);

    OWNER_VESSELS.save(storage, &owner, &owner_vessels)?;

    let mut protocol_owned_vessels = OWNER_VESSELS
        .may_load(storage, escrow_ica_address)?
        .unwrap_or_default();

    protocol_owned_vessels.insert(hydro_lock_id);

    OWNER_VESSELS.save(storage, escrow_ica_address, &protocol_owned_vessels)?;

    save_vessel_ownership_state(
        storage,
        hydro_lock_id,
        VesselOwnershipState::OwnedByProtocol,
    )?;

    Ok(())
}
