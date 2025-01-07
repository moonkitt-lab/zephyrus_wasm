pub mod msgs;

use cosmwasm_std::{Addr, CustomQuery, QuerierWrapper, StdError};

pub trait QuerierExt {
    fn hydro_hub_connection_id(&self, hydro_contract: &Addr) -> Result<String, StdError>;
}

impl<C: CustomQuery> QuerierExt for QuerierWrapper<'_, C> {
    fn hydro_hub_connection_id(&self, hydro_contract: &Addr) -> Result<String, StdError> {
        let res: msgs::ConstantsResponse =
            self.query_wasm_smart(hydro_contract, &msgs::QueryMsg::Constants {})?;

        Ok(res.constants.hub_connection_id)
    }
}
