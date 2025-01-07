#[cfg(test)]
mod tests {

    use cosmwasm_std::{
        testing::{message_info, mock_dependencies, mock_env, MockApi},
        Addr, Coin, Decimal,
    };
    use zephyrus_core::msgs::{BuildVesselParams, ExecuteMsg, InstantiateMsg};

    use crate::{
        contract::{execute, instantiate},
        errors::ContractError,
    };

    pub const IBC_DENOM_1: &str =
        "ibc/0EA38305D72BE22FD87E7C0D1002D36D59B59BC3C863078A54550F8E50C50EEE";

    pub fn get_address_as_str(mock_api: &MockApi, addr: &str) -> String {
        mock_api.addr_make(addr).to_string()
    }

    #[test]
    fn instantiate_test() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = message_info(&Addr::unchecked("sender"), &[]);
        let user_address = get_address_as_str(&deps.api, "addr0000");
        let msg = get_default_instantiate_msg(&deps, user_address);
        let res = instantiate(deps.as_mut(), env, info, msg);
        assert!(res.is_ok(), "error: {:?}", res);
    }

    fn get_default_instantiate_msg(
        deps: &cosmwasm_std::OwnedDeps<
            cosmwasm_std::MemoryStorage,
            MockApi,
            cosmwasm_std::testing::MockQuerier,
        >,
        user_address: String,
    ) -> InstantiateMsg {
        InstantiateMsg {
            whitelist_admins: vec![user_address.clone()],
            hydro_contract_address: get_address_as_str(&deps.api, "hydro_addr"),
            tribute_contract_address: get_address_as_str(&deps.api, "tribute_addr"),
            default_hydromancer_address: get_address_as_str(&deps.api, "hydromancer_addr"),
            default_hydromancer_name: get_address_as_str(&deps.api, "default_hydromancer_name"),
            default_hydromancer_commission_rate: Decimal::from_ratio(1u128, 100u128),
        }
    }

    #[test]
    fn pause_fail_not_admin() {
        let (mut deps, env) = (mock_dependencies(), mock_env());
        let admin_address = get_address_as_str(&deps.api, "addr0000");
        let info = message_info(&Addr::unchecked("sender"), &[]);
        let msg = get_default_instantiate_msg(&deps, admin_address.to_string());

        let res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg.clone());
        assert!(res.is_ok(), "error: {:?}", res);
        let info1 = message_info(&Addr::unchecked("sender"), &[]);

        let msg = ExecuteMsg::PauseContract {};

        let res = execute(deps.as_mut(), env.clone(), info1.clone(), msg);
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err().to_string(),
            ContractError::Unauthorized.to_string()
        );
    }

    #[test]
    fn unpause_fail_not_admin() {
        let (mut deps, env) = (mock_dependencies(), mock_env());
        let admin_address = get_address_as_str(&deps.api, "addr0000");
        let info = message_info(&Addr::unchecked("sender"), &[]);
        let msg = get_default_instantiate_msg(&deps, admin_address.to_string());

        let res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg.clone());
        assert!(res.is_ok(), "error: {:?}", res);
        let info1 = message_info(&Addr::unchecked("sender"), &[]);

        let msg = ExecuteMsg::UnpauseContract {};

        let res = execute(deps.as_mut(), env.clone(), info1.clone(), msg);
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err().to_string(),
            ContractError::Unauthorized.to_string()
        );
    }

    #[test]
    fn pause_basic_test() {
        let (mut deps, env) = (mock_dependencies(), mock_env());
        let admin_address = get_address_as_str(&deps.api, "addr0000");
        let info = message_info(&Addr::unchecked("sender"), &[]);
        let msg = get_default_instantiate_msg(&deps, admin_address.to_string());

        let res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg.clone());
        assert!(res.is_ok(), "error: {:?}", res);
        let info1 = message_info(&Addr::unchecked(admin_address.clone()), &[]);

        let msg_pause = ExecuteMsg::PauseContract {};

        let res = execute(deps.as_mut(), env.clone(), info1.clone(), msg_pause);
        assert!(res.is_ok(), "error: {:?}", res);

        //now every msg executed should be in error "ContractError::Paused"

        let info2 = message_info(
            &Addr::unchecked(admin_address.clone()),
            &[Coin::new(3000u64, IBC_DENOM_1.to_string())],
        );
        let msg_build_vessel = ExecuteMsg::BuildVessel {
            vessels: vec![BuildVesselParams {
                lock_duration: 1000,
                auto_maintenance: true,
                hydromancer_id: 0,
            }],
            receiver: None,
        };

        let res = execute(deps.as_mut(), env.clone(), info2.clone(), msg_build_vessel);
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err().to_string(),
            ContractError::Paused.to_string()
        );

        let info3 = message_info(&Addr::unchecked("sender"), &[]);
        let msg_auto_maintain = ExecuteMsg::AutoMaintain {};
        let res = execute(deps.as_mut(), env.clone(), info3.clone(), msg_auto_maintain);
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err().to_string(),
            ContractError::Paused.to_string()
        );

        let info4 = message_info(&Addr::unchecked("sender"), &[]);
        let msg_modify_automaintenance = ExecuteMsg::ModifyAutoMaintenance {
            hydro_lock_ids: vec![0],
            auto_maintenance: true,
        };
        let res = execute(
            deps.as_mut(),
            env.clone(),
            info4.clone(),
            msg_modify_automaintenance,
        );
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err().to_string(),
            ContractError::Paused.to_string()
        );

        let info5 = message_info(&Addr::unchecked("sender"), &[]);
        let msg_update_class = ExecuteMsg::UpdateVesselsClass {
            hydro_lock_ids: vec![1],
            hydro_lock_duration: 1000,
        };
        let res = execute(deps.as_mut(), env.clone(), info5.clone(), msg_update_class);
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err().to_string(),
            ContractError::Paused.to_string()
        );
    }

    #[test]
    fn fail_unpause_already_unpause_contract_test() {
        let (mut deps, env) = (mock_dependencies(), mock_env());
        let admin_address = get_address_as_str(&deps.api, "addr0000");
        let info = message_info(&Addr::unchecked("sender"), &[]);
        let msg = get_default_instantiate_msg(&deps, admin_address.to_string());

        let res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg.clone());
        assert!(res.is_ok(), "error: {:?}", res);
        let info1 = message_info(&Addr::unchecked(admin_address.clone()), &[]);

        let msg = ExecuteMsg::UnpauseContract {};

        let res = execute(deps.as_mut(), env.clone(), info1.clone(), msg);
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err().to_string(),
            "Generic error: Cannot unpause: Contract not paused"
        );
    }
}
