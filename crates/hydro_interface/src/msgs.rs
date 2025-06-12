use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Coin, Decimal, Timestamp, Uint128};

#[cw_serde]
pub enum ExecuteMsg {
    LockTokens {
        lock_duration: u64,
    },
    RefreshLockDuration {
        lock_ids: Vec<u64>,
        lock_duration: u64,
    },
    UnlockTokens {
        lock_ids: Option<Vec<u64>>,
    },
}

#[cw_serde]
pub enum QueryMsg {
    SpecificUserLockups { address: String, lock_ids: Vec<u64> },
    Constants {},
}

#[cw_serde]
pub struct SpecificUserLockupsResponse {
    pub lockups: Vec<LockEntryWithPower>,
}

#[cw_serde]
pub struct LockEntryWithPower {
    pub lock_entry: LockEntryV2,
    pub current_voting_power: Uint128,
}

#[cw_serde]
pub struct LockEntryV2 {
    pub lock_id: u64,
    pub owner: Addr,
    pub funds: Coin,
    pub lock_start: Timestamp,
    pub lock_end: Timestamp,
}

#[cw_serde]
pub struct HydroConstantsResponse {
    pub constants: HydroConstants,
}

#[cw_serde]
pub struct HydroConstants {
    pub round_length: u64,
    pub lock_epoch_length: u64,
    pub first_round_start: Timestamp,
    // The maximum number of tokens that can be locked by any users (currently known and the future ones)
    pub max_locked_tokens: u128,
    // The maximum number of tokens (out of the max_locked_tokens) that is reserved for locking only
    // for currently known users. This field is intended to be set to some value greater than zero at
    // the begining of the round, and such Constants would apply only for a predefined period of time.
    // After this period has expired, a new Constants would be activated that would set this value to
    // zero, which would allow any user to lock any amount that possibly wasn't filled, but was reserved
    // for this cap.
    pub known_users_cap: u128,
    pub paused: bool,
    pub max_deployment_duration: u64,
    pub round_lock_power_schedule: RoundLockPowerSchedule,
    pub cw721_collection_info: CollectionInfo,
}

#[cw_serde]
pub struct RoundLockPowerSchedule {
    pub round_lock_power_schedule: Vec<LockPowerEntry>,
}

#[cw_serde]
pub struct LockPowerEntry {
    pub locked_rounds: u64,
    pub power_scaling_factor: Decimal,
}

#[cw_serde]
pub struct CollectionInfo {
    pub name: String,
    pub symbol: String,
}
