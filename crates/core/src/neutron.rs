use cosmos_sdk_proto::cosmos::base::query::v1beta1::PageRequest;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Coin, CustomQuery, GrpcQuery, QuerierWrapper, QueryRequest, StdError, Uint64};
use neutron_sdk::proto_types::neutron::interchainqueries::QueryRegisteredQueriesRequest;

use prost::Message;

#[cw_serde]
pub struct InterchainTxsParams {
    pub msg_submit_tx_max_messages: Uint64,
    pub register_fee: Vec<Coin>,
}

impl InterchainTxsParams {
    pub const QUERY_PATH: &'static str = "/neutron.interchaintxs.v1.Query/Params";
}

#[cw_serde]
pub struct QueryInterchainTxParamsResponse {
    pub params: InterchainTxsParams,
}

#[cw_serde]
pub struct IcqParams {
    pub query_submit_timeout: String,
    pub query_deposit: Vec<Coin>,
    pub tx_query_removal_limit: String,
}

impl IcqParams {
    pub const QUERY_PATH: &'static str = "/neutron.interchainqueries.Query/Params";
}

#[cw_serde]
pub struct QueryIcqParamsResponse {
    pub params: IcqParams,
}

pub trait QuerierExt {
    fn interchain_account_register_fee(&self) -> Result<Coin, StdError>;

    fn interchain_query_deposit(&self) -> Result<Coin, StdError>;

    fn last_registered_interchain_query_id(&self) -> Result<Option<u64>, StdError>;
}

impl<C: CustomQuery> QuerierExt for QuerierWrapper<'_, C> {
    fn interchain_account_register_fee(&self) -> Result<Coin, StdError> {
        let res: QueryInterchainTxParamsResponse = self.query(&QueryRequest::Grpc(GrpcQuery {
            path: InterchainTxsParams::QUERY_PATH.to_owned(),
            data: vec![].into(),
        }))?;

        let coin = res.params.register_fee.into_iter().next().unwrap();

        Ok(coin)
    }

    fn interchain_query_deposit(&self) -> Result<Coin, StdError> {
        let res: QueryIcqParamsResponse = self.query(&QueryRequest::Grpc(GrpcQuery {
            path: IcqParams::QUERY_PATH.to_owned(),
            data: vec![].into(),
        }))?;

        let coin = res.params.query_deposit.into_iter().next().unwrap();

        Ok(coin)
    }

    fn last_registered_interchain_query_id(&self) -> Result<Option<u64>, StdError> {
        #[cw_serde]
        struct RegisteredQuery {
            id: u64,
        }

        #[cw_serde]
        struct QueryRegisteredQueriesResponse {
            registered_queries: Vec<RegisteredQuery>,
        }

        let req = QueryRegisteredQueriesRequest {
            owners: Vec::new(),
            connection_id: String::new(),
            pagination: Some(PageRequest {
                key: Vec::new(),
                offset: 0,
                limit: 1,
                count_total: false,
                reverse: true,
            }),
        };

        let res: QueryRegisteredQueriesResponse = self.query(&QueryRequest::Grpc(GrpcQuery {
            path: "/neutron.interchainqueries.Query/RegisteredQueries".to_owned(),
            data: req.encode_to_vec().into(),
        }))?;

        let Some(last_registered_query) = res.registered_queries.first() else {
            return Ok(None);
        };

        Ok(Some(last_registered_query.id))
    }
}
