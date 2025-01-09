use cosmwasm_std::StdError;
use thiserror::Error;

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

    #[error("Total shares error: {total_shares}")]
    TotalSharesError { total_shares: u8 },
    #[error("There is no vessel to auto maintain")]
    NoVesselsToAutoMaintain {},

    #[error("Paused")]
    Paused,
}
