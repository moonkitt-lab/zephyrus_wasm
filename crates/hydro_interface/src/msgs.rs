use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Timestamp, Uint128};

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
    Unvote {
        tranche_id: u64,
        lock_ids: Vec<u64>,
    },
}

#[cw_serde]
pub enum HydroQueryMsg {
    CurrentRound {},
    TimeWeightedSharesVotingPower {
        time_weighted_shares: u128,
        validator: String,
        round_id: u64,
    },
    CurrentRoundTimeWeightedShares {
        owner: String,
        lock_ids: Vec<u64>,
    },
}

#[cw_serde]
pub struct CurrentRoundResponse {
    pub round_id: u64,
    pub round_end: Timestamp,
}

#[cw_serde]
pub struct LockTimeWeightedShare {
    pub lock_id: u64,
    pub validator: String,
    pub time_weighted_share: u128,
}

#[cw_serde]
pub struct CurrentRoundTimeWeightedSharesResponse {
    pub round_id: u64,
    pub lock_time_weighted_shares: Vec<LockTimeWeightedShare>,
}

#[cw_serde]
pub struct TimeWeightedSharesVotingPowerResponse {
    pub voting_power: u128,
}
