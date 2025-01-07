use cosmwasm_std::{CustomQuery, QuerierWrapper, StdError};
use neutron_std::types::ibc::{
    applications::transfer::v1::TransferQuerier,
    core::connection::v1::{ConnectionEnd, ConnectionQuerier},
};

pub use neutron_std::types::ibc::applications::transfer::v1::DenomTrace;

pub trait QuerierExt {
    fn ibc_denom_trace(&self, ibc_denom: &str) -> Result<DenomTrace, StdError>;

    fn ibc_connection(&self, connection_id: &str) -> Result<ConnectionEnd, StdError>;
}

impl<C: CustomQuery> QuerierExt for QuerierWrapper<'_, C> {
    fn ibc_denom_trace(&self, ibc_denom: &str) -> Result<DenomTrace, StdError> {
        let Some(("ibc", hash)) = ibc_denom.rsplit_once('/') else {
            return Err(StdError::generic_err("invalid ibc denom"));
        };

        let res = TransferQuerier::new(self).denom_trace(hash.to_owned())?;

        let denom_trace = res
            .denom_trace
            .ok_or_else(|| StdError::not_found(format!("denom trace for {hash}")))?;

        Ok(denom_trace)
    }

    fn ibc_connection(&self, connection_id: &str) -> Result<ConnectionEnd, StdError> {
        ConnectionQuerier::new(self)
            .connection(connection_id.to_owned())
            .and_then(|res| {
                res.connection.ok_or_else(|| {
                    StdError::not_found(format!("no connection end found for {connection_id}"))
                })
            })
    }
}
