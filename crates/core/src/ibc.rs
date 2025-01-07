use cosmos_sdk_proto::ibc::applications::transfer::v1::QueryDenomTraceRequest;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{CustomQuery, GrpcQuery, QuerierWrapper, QueryRequest, StdError};
use neutron_std::types::ibc::core::connection::v1::{ConnectionEnd, ConnectionQuerier};
use prost::Message;

#[cw_serde]
pub struct DenomTrace {
    pub path: String,
    pub base_denom: String,
}

pub trait QuerierExt {
    fn ibc_denom_trace(&self, ibc_denom: &str) -> Result<DenomTrace, StdError>;

    fn ibc_connection(&self, connection_id: &str) -> Result<ConnectionEnd, StdError>;
}

impl<C: CustomQuery> QuerierExt for QuerierWrapper<'_, C> {
    fn ibc_denom_trace(&self, ibc_denom: &str) -> Result<DenomTrace, StdError> {
        #[cw_serde]
        struct QueryDenomTraceResponse {
            denom_trace: Option<DenomTrace>,
        }

        let Some(("ibc", hash)) = ibc_denom.rsplit_once('/') else {
            return Err(StdError::generic_err("invalid ibc denom"));
        };

        let req = QueryDenomTraceRequest {
            hash: hash.to_owned(),
        };

        let res: QueryDenomTraceResponse = self.query(&QueryRequest::Grpc(GrpcQuery {
            path: "/ibc.applications.transfer.v1.Query/DenomTrace".to_owned(),
            data: req.encode_to_vec().into(),
        }))?;

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
