use std::collections::HashMap;

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Coin, Decimal, Timestamp, Uint128};

#[cw_serde]
pub struct ProposalToLockups {
    pub proposal_id: u64,
    pub lock_ids: Vec<u64>,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Refresh the lock duration of the specified lock ids, used by zephyrus to refresh period class period of vessels
    RefreshLockDuration {
        lock_ids: Vec<u64>,
        lock_duration: u64,
    },
    /// Unlock the specified lock ids, used by zephyrus to decommission vessels
    UnlockTokens { lock_ids: Option<Vec<u64>> },
    /// Vote the specified proposals, used by zephyrus to vote on proposals
    Vote {
        tranche_id: u64,
        proposals_votes: Vec<ProposalToLockups>,
    },
    /// Unvote the specified lock ids, used by zephyrus to unvote on proposals
    Unvote { tranche_id: u64, lock_ids: Vec<u64> },
    /// Claim the specified tribute, used by zephyrus to claim tribute
    ClaimTribute {
        round_id: u64,
        tranche_id: u64,
        tribute_id: u64,
        voter_address: String,
    },
}

/// Hydro contract query messages.
#[cw_serde]
pub enum HydroQueryMsg {
    /// Query the current round.
    CurrentRound {},
    /// Query the available tranches.
    Tranches {},
    /// Query the specific user lockups return SpecificUserLockupsResponse
    SpecificUserLockups { address: String, lock_ids: Vec<u64> },
    /// Query the specific user lockups with tranche infos return SpecificUserLockupsWithTrancheInfosResponse
    SpecificUserLockupsWithTrancheInfos { address: String, lock_ids: Vec<u64> },
    /// Query hydro constants return HydroConstantsResponse
    Constants {},
    /// Query the lockups info return LockupsInfoResponse
    /// Used to track time weighted shares of vessels with token group id and locked rounds
    LockupsInfo { lock_ids: Vec<u64> },
    /// Use to query the outstanding tribute claims by Zephyrusreturn OutstandingTributeClaimsResponse
    OutstandingTributeClaims {
        user_address: String,
        round_id: u64,
        tranche_id: u64,
    },
    /// Query the token info providers return TokenInfoProvidersResponse
    TokenInfoProviders {},
    /// Query the proposal return ProposalResponse
    Proposal {
        round_id: u64,
        tranche_id: u64,
        proposal_id: u64,
    },
    /// Query the round proposals return RoundProposalsResponse
    RoundProposals {
        round_id: u64,
        tranche_id: u64,
        start_from: u32,
        limit: u32,
    },
}

#[cw_serde]
pub enum DerivativeTokenInfoProviderQueryMsg {
    DenomInfo { round_id: u64 },
}

#[cw_serde]
pub enum TributeQueryMsg {
    ProposalTributes {
        round_id: u64,
        proposal_id: u64,
        start_from: u32,
        limit: u32,
    },
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
pub struct LockupsInfo {
    pub lock_id: u64,
    pub time_weighted_shares: Uint128,
    pub token_group_id: String,
    pub locked_rounds: u64,
}

#[cw_serde]
pub struct LockupsInfoResponse {
    pub lockups_info: Vec<LockupsInfo>,
}

#[cw_serde]
pub struct CurrentRoundResponse {
    pub round_id: u64,
    pub round_end: Timestamp,
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

#[cw_serde]
pub struct RoundWithBid {
    pub round_id: u64,
    pub proposal_id: u64,
    pub round_end: Timestamp,
}

// PerTrancheLockupInfo is used to store the lockup information for a specific tranche.
#[cw_serde]
pub struct PerTrancheLockupInfo {
    pub tranche_id: u64,
    // If this number is less or equal to the current round, it means the lockup can vote in the current round.
    pub next_round_lockup_can_vote: u64,
    // This is the proposal that the lockup is voting for in the current round, if any.
    // In particular, if the lockup is blocked from voting in the current round (because it voted for a
    // proposal with a long deployment duration in a previous round), this will be None.
    pub current_voted_on_proposal: Option<u64>,

    // This is the id of the proposal that the lockup is tied to because it has voted for a proposal with a long deployment duration.
    // In case the lockup can currently vote (and is not tied to a proposal), this will be None.
    // Note that None will also be returned if the lockup voted for a proposal that received a deployment with zero funds.
    pub tied_to_proposal: Option<u64>,

    /// This is the list of proposals that the lockup has been used to vote for in the past.
    /// It is used to show the history of the lockup upon transfer / selling on Marketplace.
    /// Note that this does not include the current voted on proposal, which is found in the current_voted_on_proposal field.
    pub historic_voted_on_proposals: Vec<RoundWithBid>,
}

#[cw_serde]
pub struct LockupWithPerTrancheInfo {
    pub lock_with_power: LockEntryWithPower,
    pub per_tranche_info: Vec<PerTrancheLockupInfo>,
}

#[cw_serde]
pub struct SpecificUserLockupsWithTrancheInfosResponse {
    pub lockups_with_per_tranche_infos: Vec<LockupWithPerTrancheInfo>,
}

#[cw_serde]
pub struct DenomInfoResponse {
    pub denom: String,
    pub token_group_id: String,
    pub ratio: Decimal,
}

#[cw_serde]
pub struct TokenInfoProviderDerivative {
    pub contract: String,
    pub cache: HashMap<u64, DenomInfoResponse>,
}

#[cw_serde]
pub struct TokenInfoProviderLSM {
    pub max_validator_shares_participating: u64,
    pub hub_connection_id: String,
    pub hub_transfer_channel_id: String,
    pub icq_update_period: u64,
}

#[cw_serde]
pub enum TokenInfoProvider {
    #[serde(rename = "lsm")]
    LSM(TokenInfoProviderLSM),
    Derivative(TokenInfoProviderDerivative),
}

#[cw_serde]
pub struct TokenInfoProvidersResponse {
    pub providers: Vec<TokenInfoProvider>,
}

#[cw_serde]
pub struct Proposal {
    pub round_id: u64,
    pub tranche_id: u64,
    pub proposal_id: u64,
    pub title: String,
    pub description: String,
    pub power: Uint128,
    pub percentage: Uint128,
    pub deployment_duration: u64, // number of rounds liquidity is allocated excluding voting round.
    pub minimum_atom_liquidity_request: Uint128,
}

#[cw_serde]
pub struct ProposalResponse {
    pub proposal: Proposal,
}

#[cw_serde]
pub struct RoundProposalsResponse {
    pub proposals: Vec<Proposal>,
}

#[cw_serde]
pub struct Tribute {
    pub round_id: u64,
    pub tranche_id: u64,
    pub proposal_id: u64,
    pub tribute_id: u64,
    pub depositor: Addr,
    pub funds: Coin,
    pub refunded: bool,
    pub creation_time: Timestamp,
    pub creation_round: u64,
}

#[cw_serde]
pub struct ProposalTributesResponse {
    pub tributes: Vec<Tribute>,
}
