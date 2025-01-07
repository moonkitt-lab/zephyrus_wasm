use cosmwasm_std::{Coin, StdError};
use neutron_sdk::bindings::types::Height;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum TokenOwnershipProofError {
    #[error("{0}")]
    Utf8(#[from] std::str::Utf8Error),

    #[error("Query result and proof below the minimum height: {received:?} < {minimum:?}")]
    BelowMinimumHeight { received: Height, minimum: Height },

    #[error("Incorrect number of Key-Value pairs in query result")]
    IncorrectKvResultsLength,

    #[error("Owner in query result does not match escrow ICA address: received {query_result_owner}, expected {escrow_ica_address}")]
    OwnerDoesNotMatchIcaAddress {
        query_result_owner: String,
        escrow_ica_address: String,
    },
}

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    Payment(#[from] cw_utils::PaymentError),

    #[error("{0}")]
    ProtobufDecode(#[from] prost::DecodeError),

    #[error("{0}")]
    Neutron(#[from] neutron_sdk::NeutronError),

    #[error("{0}")]
    TokenOwnership(#[from] TokenOwnershipProofError),

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Hydromancer {hydromancer_id} not found")]
    HydromancerNotFound { hydromancer_id: u64 },

    #[error("Total shares error: {total_shares}")]
    TotalSharesError { total_shares: u8 },

    #[error("There is no vessel to auto maintain")]
    NoVesselsToAutoMaintain {},

    #[error("Paused")]
    Paused,

    #[error("The vessel cannot be decommissioned")]
    LockNotExpired {},

    #[error("Insufficient funds received to register ICA: {received} < {required}")]
    InsufficientIcaRegistrationFunds { received: Coin, required: Coin },

    #[error("Escrow ICA does not exist")]
    EscrowIcaDoesNotExist,

    #[error("Vessel cannot be sold")]
    VesselCannotBeSold,

    #[error("Sender is not the vessel owner")]
    SenderIsNotVesselOwner,

    #[error("No tokens received")]
    NoTokensReceived,

    #[error("Length of create vessel params does not match the number of tokens received: number of params received {params_len}, number of tokens received {funds_len}")]
    CreateVesselParamsLengthMismatch { params_len: usize, funds_len: usize },

    #[error("Invalid LSM token received: {0}")]
    InvalidLsmTokenReceived(String),

    #[error("Tokenized shares record with id {0} is already in use")]
    TokenizedShareRecordAlreadyInUse(u64),

    #[error("Tokenized shares record with id {0} is already in active use")]
    TokenizedShareRecordAlreadyInActiveUse(u64),
}
