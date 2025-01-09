#[cfg(test)]
mod tests {

    use cosmwasm_std::{
        testing::{message_info, mock_dependencies, mock_env, MockApi},
        Addr, Decimal, Response,
    };
    use zephyrus_core::msgs::InstantiateMsg;

    use crate::contract::instantiate;

    pub fn get_address_as_str(mock_api: &MockApi, addr: &str) -> String {
        mock_api.addr_make(addr).to_string()
    }

    #[test]
    fn instantiate_test() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = message_info(&Addr::unchecked("sender"), &[]);
        let user_address = get_address_as_str(&deps.api, "addr0000");
        let msg = InstantiateMsg {
            whitelist_admins: vec![user_address.clone()],

            hydro_contract_address: get_address_as_str(&deps.api, "hydro_addr"),
            tribute_contract_address: get_address_as_str(&deps.api, "tribute_addr"),
            default_hydromancer_address: get_address_as_str(&deps.api, "hydromancer_addr"),
            default_hydromancer_name: get_address_as_str(&deps.api, "default_hydromancer_name"),
            default_hydromancer_commission_rate: Decimal::from_ratio(1u128, 100u128),
        };
        let res = instantiate(deps.as_mut(), env, info, msg)
            .map_err(|e| e.to_string())
            .unwrap();
        assert_eq!(res, Response::default());
    }
}
