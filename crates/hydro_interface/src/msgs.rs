use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Coin, Timestamp, Uint128};

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
