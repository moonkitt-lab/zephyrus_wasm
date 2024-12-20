use cosmwasm_schema::cw_serde;

#[cw_serde]
pub enum ExecuteMsg {
    LockTokens {
        lock_duration: u64,
    },
    RefreshLockDuration {
        lock_ids: Vec<u64>,
        lock_duration: u64,
    },
}

#[cw_serde]
pub enum QueryMsg {}
