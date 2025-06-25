#[cfg(test)]
mod tests {
    use crate::{
        contract::{execute, instantiate},
        errors::ContractError,
    };
    use cosmwasm_std::{
        testing::{message_info, mock_dependencies, mock_env, MockApi, MockQuerier},
        to_json_binary, Addr, Binary, Coin, ContractResult, Decimal, Empty, MemoryStorage,
        MessageInfo, OwnedDeps, SystemError, SystemResult, WasmQuery,
    };
    use serde_json;
    use zephyrus_core::msgs::{
        BuildVesselParams, Cw721ReceiveMsg, ExecuteMsg, InstantiateMsg, VesselInfo,
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
        let msg = InstantiateMsg {
            whitelist_admins: vec![user_address.clone()],

            hydro_contract_address: get_address_as_str(&deps.api, "hydro_addr"),
            tribute_contract_address: get_address_as_str(&deps.api, "tribute_addr"),
            default_hydromancer_address: get_address_as_str(&deps.api, "hydromancer_addr"),
            default_hydromancer_name: get_address_as_str(&deps.api, "default_hydromancer_name"),
            default_hydromancer_commission_rate: Decimal::from_ratio(1u128, 100u128),
        };
        msg
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

    #[test]
    fn test_cw721_receive_nft_fail_collection_not_accepted() {
        let (mut deps, env) = (mock_dependencies(), mock_env());
        let admin_address = get_address_as_str(&deps.api, "addr0000");
        let info = message_info(&Addr::unchecked("sender"), &[]);
        let msg = get_default_instantiate_msg(&deps, admin_address.to_string());
        let fake_nft_contract_address = deps.api.addr_make("fake_nft_contract_address");
        let sender = deps.api.addr_make("sender");

        let _res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg.clone());
        let info = MessageInfo {
            sender: fake_nft_contract_address.clone(),
            funds: vec![],
        };
        let receive_msg = Cw721ReceiveMsg {
            sender: sender.to_string(),
            token_id: "1".to_string(),
            msg: Binary::from("{}".as_bytes()),
        };
        let msg = ExecuteMsg::ReceiveNft(receive_msg);
        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg);
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err().to_string(),
            ContractError::NftNotAccepted.to_string()
        );
    }

    #[test]
    fn test_cw721_receive_nft_fail_bad_period() {
        let (mut deps, env) = (mock_dependencies(), mock_env());
        let admin_address = get_address_as_str(&deps.api, "addr0000");
        let info = message_info(&Addr::unchecked("sender"), &[]);
        let msg = get_default_instantiate_msg(&deps, admin_address.to_string());
        let hydro_contract = deps.api.addr_make("hydro_addr");
        let sender = deps.api.addr_make("sender");

        let res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg.clone());
        assert!(res.is_ok());
        mock_hydro_contract(&mut deps, false);
        let info = MessageInfo {
            sender: hydro_contract.clone(),
            funds: vec![],
        };
        let vessel_info = VesselInfo {
            owner: sender.to_string(),
            auto_maintenance: true,
            hydromancer_id: 0,
            class_period: 31,
        };
        let receive_msg = Cw721ReceiveMsg {
            sender: sender.to_string(),
            token_id: "1".to_string(),
            msg: to_json_binary(&vessel_info).unwrap(),
        };
        let msg = ExecuteMsg::ReceiveNft(receive_msg);
        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg);
        assert!(res.is_err());
        assert!(res
            .unwrap_err()
            .to_string()
            .contains("Lock duration must be one of: [10, 20, 30]; but was: 31"));
    }

    #[test]
    fn test_cw721_receive_nft_fail_not_owner() {
        let (mut deps, env) = (mock_dependencies(), mock_env());
        let admin_address = get_address_as_str(&deps.api, "addr0000");
        let info = message_info(&Addr::unchecked("sender"), &[]);
        let msg = get_default_instantiate_msg(&deps, admin_address.to_string());
        let hydro_contract = deps.api.addr_make("hydro_addr");
        let sender = deps.api.addr_make("sender");

        let res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg.clone());
        assert!(res.is_ok());
        mock_hydro_contract(&mut deps, true);
        let info = MessageInfo {
            sender: hydro_contract.clone(),
            funds: vec![],
        };
        let vessel_info = VesselInfo {
            owner: sender.to_string(),
            auto_maintenance: true,
            hydromancer_id: 0,
            class_period: 30,
        };

        let receive_msg = Cw721ReceiveMsg {
            sender: sender.to_string(),
            token_id: "2".to_string(),
            msg: to_json_binary(&vessel_info).unwrap(),
        };
        let msg = ExecuteMsg::ReceiveNft(receive_msg);

        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg);
        assert!(res.is_err());
        assert!(res
            .unwrap_err()
            .to_string()
            .contains("Lockup 2 not owned by Zephyrus"));
    }

    #[test]
    fn test_cw721_receive_nft_succeed() {
        let (mut deps, env) = (mock_dependencies(), mock_env());
        let admin_address = get_address_as_str(&deps.api, "addr0000");
        let info = message_info(&Addr::unchecked("sender"), &[]);
        let msg = get_default_instantiate_msg(&deps, admin_address.to_string());
        let hydro_contract = deps.api.addr_make("hydro_addr");
        let sender = deps.api.addr_make("sender");

        let res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg.clone());
        assert!(res.is_ok());
        mock_hydro_contract(&mut deps, false);
        let info = MessageInfo {
            sender: hydro_contract.clone(),
            funds: vec![],
        };
        let vessel_info = VesselInfo {
            owner: sender.to_string(),
            auto_maintenance: true,
            hydromancer_id: 0,
            class_period: 30,
        };
        let receive_msg = Cw721ReceiveMsg {
            sender: sender.to_string(),
            token_id: "1".to_string(),
            msg: to_json_binary(&vessel_info).unwrap(),
        };
        let msg = ExecuteMsg::ReceiveNft(receive_msg);

        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg);
        assert!(res.is_ok());
    }

    fn mock_hydro_contract(
        deps: &mut OwnedDeps<MemoryStorage, MockApi, MockQuerier<Empty>>,
        error_specific_user_lockups: bool,
    ) {
        let hydro_constants_response = r#"{"constants":{"round_length":10,"lock_epoch_length":10,"first_round_start":"1000000000000000000","max_locked_tokens":10,"known_users_cap":10,"paused":false,"max_deployment_duration":10,"round_lock_power_schedule":{"round_lock_power_schedule":[{"locked_rounds":1,"power_scaling_factor":"1"},{"locked_rounds":2,"power_scaling_factor":"1.25"},{"locked_rounds":3,"power_scaling_factor":"1.5"}]},"cw721_collection_info":{"name":"hydro","symbol":"test"}}}"#;
        // Mock the Hydro contract responses
        deps.querier.update_wasm(
            move |msg| {
                let msg_str = match msg {
                    WasmQuery::Smart { msg, .. } => String::from_utf8(msg.to_vec()).unwrap(),
                    WasmQuery::Raw { .. } => "".to_string(),
                    WasmQuery::ContractInfo { .. } => "".to_string(),
                    _ => "".to_string(),
                };
                if msg_str.contains("constants") {
                    let response = serde_json::from_str::<hydro_interface::msgs::HydroConstantsResponse>(hydro_constants_response).unwrap();
                    SystemResult::Ok(ContractResult::Ok(to_json_binary(&response).unwrap()))
                } else if msg_str.contains("specific_user_lockups") {
                   if ! error_specific_user_lockups {
                    let lockup_response = r#"{"lockups":[{"lock_entry":{"lock_id":1,"owner":"addr0000","funds":{"denom":"uatom","amount":"1000"},"lock_start":"1000000000000000000","lock_end":"2000000000000000000"},"current_voting_power":"1000"}]}"#;
                    let response = serde_json::from_str::<hydro_interface::msgs::SpecificUserLockupsResponse>(lockup_response).unwrap();
                    SystemResult::Ok(ContractResult::Ok(to_json_binary(&response).unwrap()))
                   } else {
                    let lockup_response = r#"{"lockups":[]}"#;
                    let response = serde_json::from_str::<hydro_interface::msgs::SpecificUserLockupsResponse>(lockup_response).unwrap();
                    SystemResult::Ok(ContractResult::Ok(to_json_binary(&response).unwrap()))
                   }
                } else {
                    SystemResult::Err(SystemError::Unknown {})
                }
            });
    }
}
