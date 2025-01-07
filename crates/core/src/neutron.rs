use cosmwasm_std::{Coin, CustomQuery, QuerierWrapper, StdError};
use neutron_std::{
    try_proto_to_cosmwasm_coins,
    types::{
        cosmos::base::query::v1beta1::PageRequest,
        neutron::{
            interchainqueries::InterchainqueriesQuerier, interchaintxs::v1::InterchaintxsQuerier,
        },
    },
};

pub trait QuerierExt {
    fn interchain_account_register_fee(&self) -> Result<Coin, StdError>;

    fn interchain_query_deposit(&self) -> Result<Coin, StdError>;

    fn last_registered_interchain_query_id(&self) -> Result<Option<u64>, StdError>;
}

impl<C: CustomQuery> QuerierExt for QuerierWrapper<'_, C> {
    fn interchain_account_register_fee(&self) -> Result<Coin, StdError> {
        InterchaintxsQuerier::new(self)
            .params()
            .map(|res| res.params.expect("params always present").register_fee)
            .and_then(try_proto_to_cosmwasm_coins)
            .map(|coins| {
                coins
                    .into_iter()
                    .next()
                    .expect("always a registration fee coin")
            })
    }

    fn interchain_query_deposit(&self) -> Result<Coin, StdError> {
        InterchainqueriesQuerier::new(self)
            .params()
            .map(|res| res.params.expect("params always present").query_deposit)
            .and_then(try_proto_to_cosmwasm_coins)
            .map(|coins| {
                coins
                    .into_iter()
                    .next()
                    .expect("always a query deposit coin")
            })
    }

    fn last_registered_interchain_query_id(&self) -> Result<Option<u64>, StdError> {
        let res = InterchainqueriesQuerier::new(self).registered_queries(
            Vec::new(),
            String::new(),
            Some(PageRequest {
                key: Vec::new(),
                offset: 0,
                limit: 1,
                count_total: false,
                reverse: true,
            }),
        )?;

        let Some(last_registered_query) = res.registered_queries.first() else {
            return Ok(None);
        };

        Ok(Some(last_registered_query.id))
    }
}
