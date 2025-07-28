use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Coin, Decimal, Order, StdError, StdResult, Storage, Uint128};
use cw_storage_plus::{Bound, Item, Map};
use std::collections::BTreeSet;
use zephyrus_core::{
    msgs::{HydroProposalId, RoundId, TrancheId, TributeId, UserId},
    state::{
        Constants, HydroLockId, HydromancerId, HydromancerTribute, Vessel, VesselHarbor,
        VesselSharesInfo,
    },
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
const HYDROMANCER_TW_SHARES_BY_TOKEN_GROUP_ID: Map<((HydromancerId, RoundId), u64, &str), u128> =
    Map::new("hydromancer_tw_shares_by_token_group_id");
const PROPOSAL_HYDROMANCER_TW_SHARES_BY_TOKEN_GROUP_ID: Map<
    (HydroProposalId, HydromancerId, &str),
    u128,
> = Map::new("proposal_hydromancer_tw_shares_by_token_group_id");

const PROPOSAL_TOTAL_TW_SHARES_BY_TOKEN_GROUP_ID: Map<(HydroProposalId, &str), u128> =
    Map::new("proposal_total_tw_shares_by_token_group_id");

const VESSEL_SHARES_INFO: Map<(RoundId, HydroLockId), VesselSharesInfo> =
    Map::new("vessel_shares_info");

// Track hydromancers with completed TWS per round for efficient checking
const HYDROMANCER_TWS_COMPLETED_PER_ROUND: Map<(RoundId, HydromancerId), bool> =
    Map::new("hydromancer_tws_completed_per_round");

const HYDROMANCER_REWARDS_BY_TRIBUTE: Map<(HydromancerId, RoundId, TributeId), HydromancerTribute> =
    Map::new("hydromancer_rewards_by_tribute");

// Importantly, the VESSEL_TRIBUTE_CLAIMS for a lock_id and tribute_id being present at all means the user has claimed that tribute.
// VESSEL_TRIBUTE_CLAIMS: key(hydro_lock_id, tribute_id) -> amount_claimed
// Kept for historical information
pub const VESSEL_TRIBUTE_CLAIMS: Map<(HydroLockId, TributeId), Coin> =
    Map::new("vessel_tribute_claims");

// Insert new rewards to hydromancer
// If the hydromancer already has a reward for the tribute => error
// If the hydromancer doesn't have a reward for the tribute => insert new reward
pub fn add_new_rewards_to_hydromancer(
    storage: &mut dyn Storage,
    hydromancer_id: HydromancerId,
    round_id: RoundId,
    tribute_id: TributeId,
    hydromancer_tribute: HydromancerTribute,
) -> StdResult<()> {
    let tribute_reward =
        HYDROMANCER_REWARDS_BY_TRIBUTE.may_load(storage, (hydromancer_id, round_id, tribute_id))?;
    if tribute_reward.is_some() {
        return Err(StdError::generic_err("Tribute reward already exists"));
    }
    HYDROMANCER_REWARDS_BY_TRIBUTE.save(
        storage,
        (hydromancer_id, round_id, tribute_id),
        &hydromancer_tribute,
    )
}

pub fn save_vessel_tribute_claim(
    storage: &mut dyn Storage,
    hydro_lock_id: HydroLockId,
    tribute_id: TributeId,
    amount: Coin,
) -> StdResult<()> {
    VESSEL_TRIBUTE_CLAIMS.save(storage, (hydro_lock_id, tribute_id), &amount)
}

pub fn is_vessel_tribute_claimed(
    storage: &dyn Storage,
    hydro_lock_id: HydroLockId,
    tribute_id: TributeId,
) -> bool {
    VESSEL_TRIBUTE_CLAIMS.has(storage, (hydro_lock_id, tribute_id))
}

pub fn get_hydromancer_rewards_by_tribute(
    storage: &dyn Storage,
    hydromancer_id: HydromancerId,
    round_id: RoundId,
    tribute_id: TributeId,
) -> StdResult<Option<HydromancerTribute>> {
    HYDROMANCER_REWARDS_BY_TRIBUTE.may_load(storage, (hydromancer_id, round_id, tribute_id))
}
pub fn initialize_sequences(storage: &mut dyn Storage) -> StdResult<()> {
    USER_NEXT_ID.save(storage, &0)?;
    HYDROMANCER_NEXT_ID.save(storage, &0)
}

pub fn update_constants(storage: &mut dyn Storage, constants: Constants) -> StdResult<()> {
    CONSTANTS.save(storage, &constants)
}

pub fn get_constants(storage: &dyn Storage) -> StdResult<Constants> {
    CONSTANTS.load(storage)
}

pub fn update_whitelist_admins(
    storage: &mut dyn Storage,
    whitelist_admins: Vec<Addr>,
) -> StdResult<()> {
    WHITELIST_ADMINS.save(storage, &whitelist_admins)
}

pub fn get_vessel_harbor(
    storage: &dyn Storage,
    tranche_id: TrancheId,
    round_id: RoundId,
    hydro_lock_id: HydroLockId,
) -> StdResult<(VesselHarbor, HydroProposalId)> {
    let proposal_id = HARBOR_OF_VESSEL.load(storage, ((tranche_id, round_id), hydro_lock_id))?;
    let vessel_harbor = VESSEL_TO_HARBOR.load(
        storage,
        ((tranche_id, round_id), proposal_id, hydro_lock_id),
    )?;
    Ok((vessel_harbor, proposal_id))
}

pub fn insert_new_user(storage: &mut dyn Storage, user_address: Addr) -> StdResult<UserId> {
    // Check if user already exists
    if let Ok(user_id) = get_user_id_by_address(storage, user_address.clone()) {
        return Err(StdError::generic_err(format!(
            "User {} already exists with id {}",
            user_address, user_id
        )));
    }

    // User doesn't exist, create new one
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

pub fn get_user_id_by_address(storage: &dyn Storage, user_addr: Addr) -> StdResult<UserId> {
    USERID_BY_ADDR.load(storage, user_addr.as_str())
}

pub fn insert_new_hydromancer(
    storage: &mut dyn Storage,
    hydromancer_address: Addr,
    hydromancer_name: String,
    hydromancer_commission_rate: Decimal,
) -> StdResult<HydromancerId> {
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
) -> StdResult<Hydromancer> {
    HYDROMANCERS.load(storage, hydromancer_id)
}

pub fn get_hydromancer_id_by_address(
    storage: &dyn Storage,
    hydromancer_addr: Addr,
) -> StdResult<HydromancerId> {
    HYDROMANCERID_BY_ADDR.load(storage, hydromancer_addr.as_str())
}

/// Get user ID by address
pub fn get_user_id(storage: &dyn Storage, user_addr: &Addr) -> Result<UserId, ContractError> {
    let user_id = USERID_BY_ADDR.load(storage, user_addr.as_str())?;
    Ok(user_id)
}

pub fn add_vessel(storage: &mut dyn Storage, vessel: &Vessel, owner: &Addr) -> StdResult<()> {
    let vessel_id = vessel.hydro_lock_id;

    VESSELS.save(storage, vessel_id, vessel)?;

    let mut owner_vessels = OWNER_VESSELS
        .may_load(storage, owner.as_str())?
        .unwrap_or_default();

    owner_vessels.insert(vessel_id);

    OWNER_VESSELS.save(storage, owner.as_str(), &owner_vessels)?;
    if let Some(hydromancer_id) = vessel.hydromancer_id {
        let mut vessels_hydromancer = HYDROMANCER_VESSELS
            .may_load(storage, hydromancer_id)?
            .unwrap_or_default();

        vessels_hydromancer.insert(vessel_id);

        HYDROMANCER_VESSELS.save(storage, hydromancer_id, &vessels_hydromancer)?;
    }

    if vessel.auto_maintenance {
        let mut vessels_class = AUTO_MAINTAINED_VESSELS_BY_CLASS
            .may_load(storage, vessel.class_period)?
            .unwrap_or_default();
        vessels_class.insert(vessel_id);
        AUTO_MAINTAINED_VESSELS_BY_CLASS.save(storage, vessel.class_period, &vessels_class)?;
    }

    if vessel.tokenized_share_record_id.is_some() {
        TOKENIZED_SHARE_RECORDS.save(
            storage,
            vessel.tokenized_share_record_id.unwrap(),
            &vessel_id,
        )?;
    }

    Ok(())
}

pub fn save_vessel_shares_info(
    storage: &mut dyn Storage,
    vessel_id: HydroLockId,
    round_id: RoundId,
    time_weighted_shares: u128,
    token_group_id: String,
    locked_rounds: u64,
) -> StdResult<()> {
    let vessel_shares_info = VesselSharesInfo {
        time_weighted_shares,
        token_group_id,
        locked_rounds,
    };
    VESSEL_SHARES_INFO.save(storage, (round_id, vessel_id), &vessel_shares_info)
}

pub fn get_vessel_shares_info(
    storage: &dyn Storage,
    round_id: RoundId,
    hydro_lock_id: HydroLockId,
) -> StdResult<VesselSharesInfo> {
    VESSEL_SHARES_INFO.load(storage, (round_id, hydro_lock_id))
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
) -> StdResult<()> {
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

        let mut vessel_ids = vessels_under_user_control.unwrap_or_default();
        vessel_ids.insert(vessel_harbor.hydro_lock_id);
        VESSELS_UNDER_USER_CONTROL.save(storage, (tranche_id, round_id), &vessel_ids)?;
    }

    Ok(())
}

pub fn get_vessel_to_harbor_by_harbor_id(
    storage: &dyn Storage,
    tranche_id: TrancheId,
    round_id: RoundId,
    hydro_proposal_id: HydroProposalId,
) -> StdResult<Vec<(HydroLockId, VesselHarbor)>> {
    VESSEL_TO_HARBOR
        .prefix(((tranche_id, round_id), hydro_proposal_id))
        .range(storage, None, None, Order::Ascending)
        .collect()
}

pub fn get_harbor_of_vessel(
    storage: &dyn Storage,
    tranche_id: TrancheId,
    round_id: RoundId,
    hydro_lock_id: HydroLockId,
) -> StdResult<Option<HydroProposalId>> {
    HARBOR_OF_VESSEL.may_load(storage, ((tranche_id, round_id), hydro_lock_id))
}

pub fn remove_vessel_harbor(
    storage: &mut dyn Storage,
    tranche_id: TrancheId,
    round_id: RoundId,
    hydro_proposal_id: HydroLockId,
    hydro_lock_id: HydroLockId,
) -> StdResult<()> {
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

pub fn is_vessel_used_under_user_control(
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

pub fn get_vessel(storage: &dyn Storage, hydro_lock_id: HydroLockId) -> StdResult<Vessel> {
    VESSELS.load(storage, hydro_lock_id)
}

pub fn vessel_exists(storage: &dyn Storage, hydro_lock_id: HydroLockId) -> bool {
    VESSELS.has(storage, hydro_lock_id)
}

pub fn get_vessels_by_ids(
    storage: &dyn Storage,
    hydro_lock_ids: &[HydroLockId],
) -> StdResult<Vec<Vessel>> {
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
) -> StdResult<Vec<Vessel>> {
    let vessel_ids: BTreeSet<u64> = OWNER_VESSELS
        .may_load(storage, owner.as_str())?
        .unwrap_or_default();

    vessel_ids
        .iter()
        .skip(start_index)
        .take(limit)
        .map(|&vessel_id| {
            VESSELS.load(storage, vessel_id).map_err(|e| {
                StdError::generic_err(format!("Failed to load vessel {}: {}", vessel_id, e))
            })
        })
        .collect()
}

pub fn get_vessels_by_hydromancer(
    storage: &dyn Storage,
    hydromancer_id: u64,
    start_index: usize,
    limit: usize,
) -> StdResult<Vec<Vessel>> {
    let vessel_ids = HYDROMANCER_VESSELS
        .may_load(storage, hydromancer_id)?
        .unwrap_or_default(); // Returns empty BTreeSet if not found

    vessel_ids
        .iter()
        .skip(start_index)
        .take(limit)
        .map(|&id| VESSELS.load(storage, id))
        .collect()
}

pub fn get_vessel_ids_auto_maintained_by_class() -> StdResult<Map<u64, BTreeSet<HydroLockId>>> {
    Ok(AUTO_MAINTAINED_VESSELS_BY_CLASS)
}

pub fn modify_auto_maintenance(
    storage: &mut dyn Storage,
    hydro_lock_id: HydroLockId,
    auto_maintenance: bool,
) -> StdResult<()> {
    let mut vessel = get_vessel(storage, hydro_lock_id)?;

    // No change in auto_maintenance, nothing to do, return early
    if vessel.auto_maintenance == auto_maintenance {
        return Ok(());
    }

    vessel.auto_maintenance = auto_maintenance;
    VESSELS.save(storage, hydro_lock_id, &vessel)?;

    // Here we know we need to change, as vessel.auto_maintenance != auto_maintenance
    AUTO_MAINTAINED_VESSELS_BY_CLASS.update(
        storage,
        vessel.class_period,
        |existing| -> StdResult<BTreeSet<u64>> {
            let mut auto_maintained_ids = existing.unwrap_or_default();

            if auto_maintenance {
                auto_maintained_ids.insert(hydro_lock_id);
            } else {
                auto_maintained_ids.remove(&hydro_lock_id);
            }

            Ok(auto_maintained_ids)
        },
    )?;

    Ok(())
}

pub fn remove_vessel(
    storage: &mut dyn Storage,
    owner: &Addr,
    hydro_lock_id: HydroLockId,
) -> StdResult<()> {
    let vessel = get_vessel(storage, hydro_lock_id)?;

    VESSELS.remove(storage, hydro_lock_id);

    // Update owner vessels
    OWNER_VESSELS.update(
        storage,
        owner.as_str(),
        |existing| -> StdResult<BTreeSet<u64>> {
            let mut owner_vessels = existing.unwrap_or_default();
            owner_vessels.remove(&hydro_lock_id);
            Ok(owner_vessels)
        },
    )?;

    // Update hydromancer vessels if assigned
    if let Some(hydromancer_id) = vessel.hydromancer_id {
        HYDROMANCER_VESSELS.update(
            storage,
            hydromancer_id,
            |existing| -> StdResult<BTreeSet<u64>> {
                let mut vessels_hydromancer = existing.unwrap_or_default();
                vessels_hydromancer.remove(&hydro_lock_id);
                Ok(vessels_hydromancer)
            },
        )?;
    }

    // Update auto-maintained vessels if applicable
    if vessel.auto_maintenance {
        AUTO_MAINTAINED_VESSELS_BY_CLASS.update(
            storage,
            vessel.class_period,
            |existing| -> StdResult<BTreeSet<u64>> {
                let mut vessels_class = existing.unwrap_or_default();
                vessels_class.remove(&hydro_lock_id);
                Ok(vessels_class)
            },
        )?;
    }

    // Remove tokenized share record if it exists
    if let Some(record_id) = vessel.tokenized_share_record_id {
        TOKENIZED_SHARE_RECORDS.remove(storage, record_id);
    }

    Ok(())
}

pub fn is_vessel_owned_by(
    storage: &dyn Storage,
    owner: &Addr,
    hydro_lock_id: HydroLockId,
) -> StdResult<bool> {
    let owner_vessels = OWNER_VESSELS
        .may_load(storage, owner.as_str())?
        .unwrap_or_default();

    Ok(owner_vessels.contains(&hydro_lock_id))
}

pub fn are_vessels_owned_by(
    storage: &dyn Storage,
    owner: &Addr,
    hydro_lock_ids: &[HydroLockId],
) -> StdResult<bool> {
    let owner_vessels = OWNER_VESSELS
        .may_load(storage, owner.as_str())?
        .unwrap_or_default();

    Ok(hydro_lock_ids.iter().all(|id| owner_vessels.contains(id)))
}

pub fn are_vessels_controlled_by_hydromancer(
    storage: &dyn Storage,
    hydromancer_id: u64,
    vessel_ids: &[u64],
) -> StdResult<bool> {
    let hydromancer_vessels = HYDROMANCER_VESSELS
        .may_load(storage, hydromancer_id)?
        .unwrap_or_default();

    Ok(vessel_ids.iter().all(|id| hydromancer_vessels.contains(id)))
}

pub fn extract_vessels_not_controlled_by_hydromancer(
    storage: &dyn Storage,
    hydromancer_id: u64,
    vessel_ids: &[u64],
) -> StdResult<Vec<u64>> {
    let controlled_vessels = HYDROMANCER_VESSELS
        .may_load(storage, hydromancer_id)?
        .unwrap_or_default();

    Ok(vessel_ids
        .iter()
        .filter(|&&vessel_id| !controlled_vessels.contains(&vessel_id))
        .copied()
        .collect())
}

pub fn is_whitelisted_admin(storage: &dyn Storage, sender: &Addr) -> StdResult<bool> {
    let whitelist_admins = WHITELIST_ADMINS.load(storage)?;
    Ok(whitelist_admins.contains(sender))
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

    match old_hydromancer_id {
        Some(old_hydromancer_id) => {
            if old_hydromancer_id == new_hydromancer_id {
                return Ok(());
            }
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

            vessel.hydromancer_id = Some(new_hydromancer_id);

            VESSELS.save(storage, hydro_lock_id, &vessel)?;

            Ok(())
        }
        None => {
            // Vessel has no hydromancer, it's under user control for this round, new hydromancer will be set and user vote will be reseted
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
            let mut new_hydromancer_vessels = HYDROMANCER_VESSELS
                .may_load(storage, new_hydromancer_id)?
                .unwrap_or_default();

            new_hydromancer_vessels.insert(hydro_lock_id);

            HYDROMANCER_VESSELS.save(storage, new_hydromancer_id, &new_hydromancer_vessels)?;

            vessel.hydromancer_id = Some(new_hydromancer_id);

            VESSELS.save(storage, hydro_lock_id, &vessel)?;
            Ok(())
        }
    }
}

// === PURE DATABASE OPERATIONS FOR VESSEL-HYDROMANCER MAPPINGS ===

/// Save a vessel to storage
pub fn save_vessel(
    storage: &mut dyn Storage,
    vessel_id: HydroLockId,
    vessel: &Vessel,
) -> Result<(), ContractError> {
    VESSELS.save(storage, vessel_id, vessel)?;
    Ok(())
}

/// Add vessel to hydromancer's vessel set
pub fn add_vessel_to_hydromancer(
    storage: &mut dyn Storage,
    hydromancer_id: HydromancerId,
    vessel_id: HydroLockId,
) -> Result<(), ContractError> {
    let mut hydromancer_vessels = HYDROMANCER_VESSELS
        .may_load(storage, hydromancer_id)?
        .unwrap_or_default();
    hydromancer_vessels.insert(vessel_id);
    HYDROMANCER_VESSELS.save(storage, hydromancer_id, &hydromancer_vessels)?;
    Ok(())
}

/// Remove vessel from hydromancer's vessel set
pub fn remove_vessel_from_hydromancer(
    storage: &mut dyn Storage,
    hydromancer_id: HydromancerId,
    vessel_id: HydroLockId,
) -> Result<(), ContractError> {
    let mut hydromancer_vessels = HYDROMANCER_VESSELS
        .may_load(storage, hydromancer_id)?
        .unwrap_or_default();
    hydromancer_vessels.remove(&vessel_id);
    HYDROMANCER_VESSELS.save(storage, hydromancer_id, &hydromancer_vessels)?;
    Ok(())
}

/// Check if hydromancer exists
pub fn hydromancer_exists(
    storage: &dyn Storage,
    hydromancer_id: HydromancerId,
) -> Result<bool, ContractError> {
    Ok(HYDROMANCERS.has(storage, hydromancer_id))
}

/// Iterate over vessels with a predicate and pagination
pub fn iterate_vessels_with_predicate<F>(
    storage: &dyn Storage,
    start_from_vessel_id: Option<HydroLockId>,
    limit: usize,
    predicate: F,
) -> Result<Vec<(HydroLockId, Vessel)>, ContractError>
where
    F: Fn(&Vessel) -> bool,
{
    let start_bound = start_from_vessel_id.map(Bound::exclusive);
    let iter = VESSELS.range(storage, start_bound, None, Order::Ascending);

    let mut results = Vec::new();

    for item in iter {
        let (vessel_id, vessel) = item?;

        if predicate(&vessel) {
            results.push((vessel_id, vessel));

            // Stop when we have enough results
            if results.len() >= limit {
                break;
            }
        }
    }

    Ok(results)
}

pub fn get_hydromancer_time_weighted_shares_by_round(
    storage: &dyn Storage,
    round_id: RoundId,
    hydromancer_id: HydromancerId,
) -> StdResult<Vec<((u64, String), u128)>> {
    let prefix_key = (hydromancer_id, round_id);
    HYDROMANCER_TW_SHARES_BY_TOKEN_GROUP_ID
        .sub_prefix(prefix_key)
        .range(storage, None, None, Order::Ascending)
        .collect()
}

pub fn add_time_weighted_shares_to_hydromancer(
    storage: &mut dyn Storage,
    hydromancer_id: HydromancerId,
    round_id: RoundId,
    token_group_id: &str,
    locked_rounds: u64,
    shares: u128,
) -> StdResult<()> {
    HYDROMANCER_TW_SHARES_BY_TOKEN_GROUP_ID.update(
        storage,
        ((hydromancer_id, round_id), locked_rounds, token_group_id),
        |current_shares| -> Result<_, StdError> { Ok(current_shares.unwrap_or_default() + shares) },
    )?;
    Ok(())
}

pub fn substract_time_weighted_shares_from_hydromancer(
    storage: &mut dyn Storage,
    hydromancer_id: HydromancerId,
    round_id: RoundId,
    token_group_id: &str,
    locked_rounds: u64,
    shares: u128,
) -> StdResult<()> {
    HYDROMANCER_TW_SHARES_BY_TOKEN_GROUP_ID.update(
        storage,
        ((hydromancer_id, round_id), locked_rounds, token_group_id),
        |current_shares| -> Result<_, StdError> { Ok(current_shares.unwrap_or_default() - shares) },
    )?;
    Ok(())
}

pub fn get_proposal_time_weighted_shares(
    storage: &dyn Storage,
    proposal_id: HydroProposalId,
) -> StdResult<Vec<(String, u128)>> {
    let prefix = proposal_id;
    PROPOSAL_TOTAL_TW_SHARES_BY_TOKEN_GROUP_ID
        .prefix(prefix)
        .range(storage, None, None, Order::Ascending)
        .collect()
}

pub fn add_time_weighted_shares_to_proposal(
    storage: &mut dyn Storage,
    proposal_id: HydroProposalId,
    token_group_id: &str,
    time_weighted_shares: u128,
) -> StdResult<()> {
    PROPOSAL_TOTAL_TW_SHARES_BY_TOKEN_GROUP_ID.update(
        storage,
        (proposal_id, token_group_id),
        |current_shares| -> Result<_, StdError> {
            Ok(current_shares.unwrap_or_default() + time_weighted_shares)
        },
    )?;
    Ok(())
}

pub fn substract_time_weighted_shares_from_proposal(
    storage: &mut dyn Storage,
    proposal_id: HydroProposalId,
    token_group_id: &str,
    time_weighted_shares: u128,
) -> StdResult<()> {
    PROPOSAL_TOTAL_TW_SHARES_BY_TOKEN_GROUP_ID.update(
        storage,
        (proposal_id, token_group_id),
        |current_shares| -> Result<_, StdError> {
            Ok(current_shares.unwrap_or_default() - time_weighted_shares)
        },
    )?;
    Ok(())
}

pub fn get_hydromancer_proposal_time_weighted_shares(
    storage: &dyn Storage,
    proposal_id: HydroProposalId,
    hydromancer_id: HydromancerId,
) -> StdResult<Vec<(String, u128)>> {
    let prefix = (proposal_id, hydromancer_id);
    PROPOSAL_HYDROMANCER_TW_SHARES_BY_TOKEN_GROUP_ID
        .prefix(prefix)
        .range(storage, None, None, Order::Ascending)
        .collect()
}

pub fn add_time_weighted_shares_to_proposal_for_hydromancer(
    storage: &mut dyn Storage,
    proposal_id: HydroProposalId,
    hydromancer_id: HydromancerId,
    token_group_id: &str,
    time_weighted_shares: u128,
) -> StdResult<()> {
    PROPOSAL_HYDROMANCER_TW_SHARES_BY_TOKEN_GROUP_ID.update(
        storage,
        (proposal_id, hydromancer_id, token_group_id),
        |current_shares| -> Result<_, StdError> {
            Ok(current_shares.unwrap_or_default() + time_weighted_shares)
        },
    )?;
    Ok(())
}

pub fn substract_time_weighted_shares_from_proposal_for_hydromancer(
    storage: &mut dyn Storage,
    proposal_id: HydroProposalId,
    hydromancer_id: HydromancerId,
    token_group_id: &str,
    time_weighted_shares: u128,
) -> StdResult<()> {
    PROPOSAL_HYDROMANCER_TW_SHARES_BY_TOKEN_GROUP_ID.update(
        storage,
        (proposal_id, hydromancer_id, token_group_id),
        |current_shares| -> Result<_, StdError> {
            Ok(current_shares.unwrap_or_default() - time_weighted_shares)
        },
    )?;
    Ok(())
}

pub fn take_control_of_vessels(storage: &mut dyn Storage, vessel_id: HydroLockId) -> StdResult<()> {
    let mut vessel = get_vessel(storage, vessel_id)?;
    vessel.hydromancer_id = None;
    VESSELS.save(storage, vessel_id, &vessel)
}

pub fn is_hydromancer_tws_complete(
    storage: &dyn Storage,
    round_id: RoundId,
    hydromancer_id: HydromancerId,
) -> bool {
    HYDROMANCER_TWS_COMPLETED_PER_ROUND.has(storage, (round_id, hydromancer_id))
}

pub fn mark_hydromancer_tws_complete(
    storage: &mut dyn Storage,
    round_id: RoundId,
    hydromancer_id: HydromancerId,
) -> StdResult<()> {
    HYDROMANCER_TWS_COMPLETED_PER_ROUND.save(storage, (round_id, hydromancer_id), &true)
}

pub fn get_all_hydromancers(storage: &dyn Storage) -> Result<Vec<HydromancerId>, StdError> {
    HYDROMANCERS
        .keys(storage, None, None, cosmwasm_std::Order::Ascending)
        .collect()
}

pub fn has_vessel_shares_info(
    storage: &dyn Storage,
    round_id: RoundId,
    hydro_lock_id: HydroLockId,
) -> bool {
    VESSEL_SHARES_INFO.has(storage, (round_id, hydro_lock_id))
}
