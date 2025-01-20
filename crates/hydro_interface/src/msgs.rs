use cosmwasm_schema::cw_serde;
use cosmwasm_std::Timestamp;

#[cw_serde]
pub struct ProposalToLockups {
    pub proposal_id: u64,
    pub lock_ids: Vec<u64>,
}

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
    Vote {
        tranche_id: u64,
        proposals_votes: Vec<ProposalToLockups>,
    },
}

#[cw_serde]
pub enum HydroQueryMsg {
    CurrentRound {},
}

#[cw_serde]
pub struct CurrentRoundResponse {
    pub round_id: u64,
    pub round_end: Timestamp,
}
