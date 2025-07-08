use cosmwasm_std::StdError;
use thiserror::Error;
use zephyrus_core::{
    msgs::{HydroLockId, HydromancerId},
    state::UserId,
};

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized,
    // Add any other custom errors you like here.
    // Look at https://docs.rs/thiserror/1.0.21/thiserror/ for details.
    #[error("Custom Error val: {msg:?}")]
    CustomError { msg: String },

    #[error("Hydromancer {hydromancer_id} not found")]
    HydromancerNotFound { hydromancer_id: u64 },

    #[error("Vessel {vessel_id} is controlled by {vessel_hydromancer_id} not {hydromancer_id}")]
    InvalidHydromancerId {
        vessel_id: HydroLockId,
        hydromancer_id: HydromancerId,
        vessel_hydromancer_id: HydromancerId,
    },

    #[error("Vessel {vessel_id} is owned by {vessel_user_id} not {user_id}")]
    InvalidUserId {
        vessel_id: HydroLockId,
        user_id: UserId,
        vessel_user_id: UserId,
    },

    #[error("Total shares error: {total_shares}")]
    TotalSharesError { total_shares: u8 },

    #[error("There is no vessel to auto maintain")]
    NoVesselsToAutoMaintain {},

    #[error("Paused")]
    Paused,
    #[error("The vessel cannot be decommissioned")]
    LockNotExpired {},

    #[error("No tokens received")]
    NoTokensReceived,

    #[error("Length of create vessel params does not match the number of tokens received: number of params received {params_len}, number of tokens received {funds_len}")]
    CreateVesselParamsLengthMismatch { params_len: usize, funds_len: usize },

    #[error("Can't vote with vessel {vessel_id} because it's under user control")]
    VesselUnderUserControl { vessel_id: u64 },

    #[error("Invalid LSM token received: {0}")]
    InvalidLsmTokenReceived(String),

    #[error("Tokenized shares record with id {0} is already in use")]
    TokenizedShareRecordAlreadyInUse(u64),

    #[error("Lockup {id} not owned by Zephyrus")]
    LockupNotOwned { id: String },

    #[error("NFT not accepted")]
    NftNotAccepted,

    #[error("Duplicate Harbor ID: {harbor_id}")]
    VoteDuplicatedHarborId { harbor_id: u64 },

    #[error("Duplicate Vessel ID: {vessel_id}")]
    VoteDuplicatedVesselId { vessel_id: u64 },

    #[error("Vessel {vessel_id} is under hydromancer control")]
    VesselUnderHydromancerControl { vessel_id: u64 },
}
