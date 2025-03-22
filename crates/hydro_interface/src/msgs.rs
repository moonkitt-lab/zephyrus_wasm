use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Coin, Decimal, Timestamp};

#[cw_serde]
pub struct ProposalToLockups {
    pub proposal_id: u64,
    pub lock_ids: Vec<u64>,
}

#[cw_serde]
pub enum HydroExecuteMsg {
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
    ClaimTribute {
        round_id: u64,
        tranche_id: u64,
        tribute_id: u64,
        voter_address: String,
    },
}

#[cw_serde]
pub enum HydroQueryMsg {
    CurrentRound {},
    Tranches {},
    ValidatorPowerRatio { validator: String, round_id: u64 },
    CurrentRoundTimeWeightedShares { owner: String, lock_ids: Vec<u64> },
}
#[cw_serde]
pub enum TributeQueryMsg {
    // Returns all tributes for a certain round and tranche
    //  that a certain user address is able to claim, but has not claimed yet.
    OutstandingTributeClaims {
        user_address: String,
        round_id: u64,
        tranche_id: u64,
        start_from: u32,
        limit: u32,
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

#[cw_serde]
pub struct ValidatorPowerRatioResponse {
    pub ratio: Decimal,
}

#[cw_serde]
pub struct TributeClaim {
    pub round_id: u64,
    pub tranche_id: u64,
    pub proposal_id: u64,
    pub tribute_id: u64,
    pub amount: Coin,
}

#[cw_serde]
pub struct OutstandingTributeClaimsResponse {
    pub claims: Vec<TributeClaim>,
}

#[cw_serde]
pub struct Tranche {
    pub id: u64,
    pub name: String,
    pub metadata: String,
}

#[cw_serde]
pub struct TranchesResponse {
    pub tranches: Vec<Tranche>,
}
