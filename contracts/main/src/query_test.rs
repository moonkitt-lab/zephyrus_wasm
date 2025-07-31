#[cfg(test)]
mod tests {
    use cosmwasm_std::testing::mock_env;
    use zephyrus_core::msgs::{ConstantsResponse, QueryMsg, VesselHarborResponse, VesselsResponse};
    use zephyrus_core::state::{Vessel, VesselHarbor};

    use crate::{query::query, state, testing::make_valid_addr, testing_mocks::mock_dependencies};
    use cosmwasm_std::{Decimal, MessageInfo};
    use zephyrus_core::msgs::InstantiateMsg;

    fn init_contract(
        deps: &mut cosmwasm_std::OwnedDeps<
            cosmwasm_std::testing::MockStorage,
            cosmwasm_std::testing::MockApi,
            crate::testing_mocks::MockQuerier,
        >,
    ) {
        use crate::contract::instantiate;
        let _ = instantiate(
            deps.as_mut(),
            mock_env(),
            MessageInfo {
                sender: make_valid_addr("deployer"),
                funds: vec![],
            },
            InstantiateMsg {
                hydro_contract_address: make_valid_addr("hydro").into_string(),
                tribute_contract_address: make_valid_addr("tribute").into_string(),
                whitelist_admins: vec![make_valid_addr("admin").into_string()],
                default_hydromancer_name: make_valid_addr("zephyrus").into_string(),
                default_hydromancer_commission_rate: "0.1".parse().unwrap(),
                default_hydromancer_address: make_valid_addr("zephyrus").into_string(),
                commission_rate: "0.1".parse().unwrap(),
                commission_recipient: make_valid_addr("commission_recipient").into_string(),
            },
        );
    }

    fn setup_test_data(
        deps: &mut cosmwasm_std::OwnedDeps<
            cosmwasm_std::testing::MockStorage,
            cosmwasm_std::testing::MockApi,
            crate::testing_mocks::MockQuerier,
        >,
    ) {
        init_contract(deps);

        // Add test users
        let user1 = make_valid_addr("user1");
        let user2 = make_valid_addr("user2");

        let user1_id = state::insert_new_user(deps.as_mut().storage, user1.clone()).unwrap();
        let user2_id = state::insert_new_user(deps.as_mut().storage, user2.clone()).unwrap();

        // Add test hydromancer
        let hydromancer_id = state::insert_new_hydromancer(
            deps.as_mut().storage,
            make_valid_addr("test_hydromancer"),
            "Test Hydromancer".to_string(),
            Decimal::percent(15),
        )
        .unwrap();

        // Add test vessels
        state::add_vessel(
            deps.as_mut().storage,
            &Vessel {
                hydro_lock_id: 1,
                tokenized_share_record_id: None,
                class_period: 1_000_000,
                auto_maintenance: true,
                hydromancer_id: Some(0), // Default hydromancer
                owner_id: user1_id,
            },
            &user1,
        )
        .unwrap();

        state::add_vessel(
            deps.as_mut().storage,
            &Vessel {
                hydro_lock_id: 2,
                tokenized_share_record_id: None,
                class_period: 2_000_000,
                auto_maintenance: false,
                hydromancer_id: Some(hydromancer_id),
                owner_id: user2_id,
            },
            &user2,
        )
        .unwrap();

        state::add_vessel(
            deps.as_mut().storage,
            &Vessel {
                hydro_lock_id: 3,
                tokenized_share_record_id: None,
                class_period: 3_000_000,
                auto_maintenance: true,
                hydromancer_id: None, // Under user control
                owner_id: user1_id,
            },
            &user1,
        )
        .unwrap();

        // Add vessel harbor data for testing
        state::add_vessel_to_harbor(
            deps.as_mut().storage,
            1, // tranche_id
            1, // round_id
            1, // proposal_id
            &VesselHarbor {
                hydro_lock_id: 1,
                steerer_id: 0,
                user_control: false,
            },
        )
        .unwrap();

        state::add_vessel_to_harbor(
            deps.as_mut().storage,
            1, // tranche_id
            1, // round_id
            2, // proposal_id
            &VesselHarbor {
                hydro_lock_id: 3,
                steerer_id: user1_id,
                user_control: true,
            },
        )
        .unwrap();
    }

    #[test]
    fn test_query_vessels_by_owner() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        setup_test_data(&mut deps);

        let user1 = make_valid_addr("user1");
        let msg = QueryMsg::VesselsByOwner {
            owner: user1.to_string(),
            start_index: None,
            limit: None,
        };

        let result = query(deps.as_ref(), env, msg);
        assert!(result.is_ok());

        let binary = result.unwrap();
        let response: VesselsResponse = cosmwasm_std::from_json(&binary).unwrap();
        assert_eq!(response.vessels.len(), 2); // user1 owns vessels 1 and 3
        assert_eq!(response.start_index, 0);
        assert_eq!(response.limit, 100); // Default pagination limit
        assert_eq!(response.total, 2);

        // Check vessel IDs
        let vessel_ids: Vec<u64> = response.vessels.iter().map(|v| v.hydro_lock_id).collect();
        assert!(vessel_ids.contains(&1));
        assert!(vessel_ids.contains(&3));
    }

    #[test]
    fn test_query_vessels_by_owner_with_pagination() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        setup_test_data(&mut deps);

        let user1 = make_valid_addr("user1");
        let msg = QueryMsg::VesselsByOwner {
            owner: user1.to_string(),
            start_index: Some(1),
            limit: Some(1),
        };

        let result = query(deps.as_ref(), env, msg);
        assert!(result.is_ok());

        let binary = result.unwrap();
        let response: VesselsResponse = cosmwasm_std::from_json(&binary).unwrap();
        assert_eq!(response.vessels.len(), 1);
        assert_eq!(response.start_index, 1);
        assert_eq!(response.limit, 1);
        assert_eq!(response.total, 1);
    }

    #[test]
    fn test_query_vessels_by_owner_invalid_address() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        setup_test_data(&mut deps);

        let msg = QueryMsg::VesselsByOwner {
            owner: "invalid_address".to_string(),
            start_index: None,
            limit: None,
        };

        let result = query(deps.as_ref(), env, msg);
        assert!(result.is_err());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Error decoding bech32"));
    }

    #[test]
    fn test_query_vessels_by_owner_no_vessels() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        setup_test_data(&mut deps);

        let empty_user = make_valid_addr("empty_user");
        let msg = QueryMsg::VesselsByOwner {
            owner: empty_user.to_string(),
            start_index: None,
            limit: None,
        };

        let result = query(deps.as_ref(), env, msg);
        assert!(result.is_ok());

        let binary = result.unwrap();
        let response: VesselsResponse = cosmwasm_std::from_json(&binary).unwrap();
        assert_eq!(response.vessels.len(), 0);
        assert_eq!(response.total, 0);
    }

    #[test]
    fn test_query_vessels_by_hydromancer() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        setup_test_data(&mut deps);

        let hydromancer_address = make_valid_addr("test_hydromancer");
        let msg = QueryMsg::VesselsByHydromancer {
            hydromancer_addr: hydromancer_address.to_string(),
            start_index: None,
            limit: None,
        };

        let result = query(deps.as_ref(), env, msg);
        assert!(result.is_ok());

        let binary = result.unwrap();
        let response: VesselsResponse = cosmwasm_std::from_json(&binary).unwrap();
        assert_eq!(response.vessels.len(), 1); // Only vessel 2 is controlled by test_hydromancer
        assert_eq!(response.vessels[0].hydro_lock_id, 2);
        assert_eq!(response.start_index, 0);
        assert_eq!(response.limit, 100);
        assert_eq!(response.total, 1);
    }

    #[test]
    fn test_query_vessels_by_hydromancer_default() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        setup_test_data(&mut deps);

        let default_hydromancer = make_valid_addr("zephyrus");
        let msg = QueryMsg::VesselsByHydromancer {
            hydromancer_addr: default_hydromancer.to_string(),
            start_index: None,
            limit: None,
        };

        let result = query(deps.as_ref(), env, msg);
        assert!(result.is_ok());

        let binary = result.unwrap();
        let response: VesselsResponse = cosmwasm_std::from_json(&binary).unwrap();
        assert_eq!(response.vessels.len(), 1); // Only vessel 1 is controlled by default hydromancer
        assert_eq!(response.vessels[0].hydro_lock_id, 1);
    }

    #[test]
    fn test_query_vessels_by_hydromancer_invalid_address() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        setup_test_data(&mut deps);

        let msg = QueryMsg::VesselsByHydromancer {
            hydromancer_addr: "invalid_address".to_string(),
            start_index: None,
            limit: None,
        };

        let result = query(deps.as_ref(), env, msg);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Error decoding bech32"));
    }

    #[test]
    fn test_query_vessels_by_hydromancer_not_found() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        setup_test_data(&mut deps);

        let non_existent_hydromancer = make_valid_addr("non_existent");
        let msg = QueryMsg::VesselsByHydromancer {
            hydromancer_addr: non_existent_hydromancer.to_string(),
            start_index: None,
            limit: None,
        };

        let result = query(deps.as_ref(), env, msg);
        assert!(result.is_err());
    }

    #[test]
    fn test_query_constants() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        setup_test_data(&mut deps);

        let msg = QueryMsg::Constants {};

        let result = query(deps.as_ref(), env, msg);
        assert!(result.is_ok());

        let binary = result.unwrap();
        let response: ConstantsResponse = cosmwasm_std::from_json(&binary).unwrap();
        assert_eq!(response.constants.default_hydromancer_id, 0);
        assert_eq!(response.constants.paused_contract, false);
        assert_eq!(
            response.constants.hydro_config.hydro_contract_address,
            make_valid_addr("hydro")
        );
        assert_eq!(
            response
                .constants
                .hydro_config
                .hydro_tribute_contract_address,
            make_valid_addr("tribute")
        );
    }

    #[test]
    fn test_query_vessels_harbor() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        setup_test_data(&mut deps);

        let msg = QueryMsg::VesselsHarbor {
            tranche_id: 1,
            round_id: 1,
            lock_ids: vec![1, 3],
        };

        let result = query(deps.as_ref(), env, msg);
        assert!(result.is_ok());

        let binary = result.unwrap();
        let response: VesselHarborResponse = cosmwasm_std::from_json(&binary).unwrap();
        assert_eq!(response.vessels_harbor_info.len(), 2);

        // Check vessel 1 (should have harbor info)
        let vessel_1_info = response
            .vessels_harbor_info
            .iter()
            .find(|info| info.vessel_id == 1)
            .unwrap();
        assert!(vessel_1_info.vessel_to_harbor.is_some());
        assert!(vessel_1_info.harbor_id.is_some());
        assert_eq!(vessel_1_info.harbor_id.unwrap(), 1);

        // Check vessel 3 (should have harbor info)
        let vessel_3_info = response
            .vessels_harbor_info
            .iter()
            .find(|info| info.vessel_id == 3)
            .unwrap();
        assert!(vessel_3_info.vessel_to_harbor.is_some());
        assert!(vessel_3_info.harbor_id.is_some());
        assert_eq!(vessel_3_info.harbor_id.unwrap(), 2);
    }

    #[test]
    fn test_query_vessels_harbor_vessel_not_in_harbor() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        setup_test_data(&mut deps);

        let msg = QueryMsg::VesselsHarbor {
            tranche_id: 1,
            round_id: 1,
            lock_ids: vec![2], // vessel_id 2 is not in any harbor
        };

        let result = query(deps.as_ref(), env, msg);
        assert!(result.is_ok());

        let binary = result.unwrap();
        let response: VesselHarborResponse = cosmwasm_std::from_json(&binary).unwrap();
        assert_eq!(response.vessels_harbor_info.len(), 1);

        let vessel_info = &response.vessels_harbor_info[0];
        assert_eq!(vessel_info.vessel_id, 2);
        assert!(vessel_info.vessel_to_harbor.is_none());
        assert!(vessel_info.harbor_id.is_none());
    }

    #[test]
    fn test_query_vessels_harbor_vessel_not_exists() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        setup_test_data(&mut deps);

        let msg = QueryMsg::VesselsHarbor {
            tranche_id: 1,
            round_id: 1,
            lock_ids: vec![999], // Non-existent vessel
        };

        let result = query(deps.as_ref(), env, msg);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Vessel 999 does not exist"));
    }

    #[test]
    fn test_query_vessels_harbor_duplicate_vessel_ids() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        setup_test_data(&mut deps);

        let msg = QueryMsg::VesselsHarbor {
            tranche_id: 1,
            round_id: 1,
            lock_ids: vec![1, 1], // Duplicate vessel IDs
        };

        let result = query(deps.as_ref(), env, msg);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Duplicate"));
    }

    #[test]
    fn test_query_vessels_harbor_empty_vessel_list() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        setup_test_data(&mut deps);

        let msg = QueryMsg::VesselsHarbor {
            tranche_id: 1,
            round_id: 1,
            lock_ids: vec![], // Empty vessel list
        };

        let result = query(deps.as_ref(), env, msg);
        assert!(result.is_ok());

        let binary = result.unwrap();
        let response: VesselHarborResponse = cosmwasm_std::from_json(&binary).unwrap();
        assert_eq!(response.vessels_harbor_info.len(), 0);
    }

    #[test]
    fn test_pagination_limits() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        setup_test_data(&mut deps);

        let user1 = make_valid_addr("user1");

        // Test with limit exceeding MAX_PAGINATION_LIMIT
        let msg = QueryMsg::VesselsByOwner {
            owner: user1.to_string(),
            start_index: None,
            limit: Some(2000), // Exceeds MAX_PAGINATION_LIMIT of 1000
        };

        let result = query(deps.as_ref(), env, msg);
        assert!(result.is_ok());

        let binary = result.unwrap();
        let response: VesselsResponse = cosmwasm_std::from_json(&binary).unwrap();
        assert_eq!(response.limit, 1000); // Should be capped at MAX_PAGINATION_LIMIT
    }

    #[test]
    fn test_pagination_edge_cases() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        setup_test_data(&mut deps);

        let user1 = make_valid_addr("user1");

        // Test with start_index beyond available vessels
        let msg = QueryMsg::VesselsByOwner {
            owner: user1.to_string(),
            start_index: Some(10), // Beyond available vessels
            limit: None,
        };

        let result = query(deps.as_ref(), env, msg);
        assert!(result.is_ok());

        let binary = result.unwrap();
        let response: VesselsResponse = cosmwasm_std::from_json(&binary).unwrap();
        assert_eq!(response.vessels.len(), 0);
        assert_eq!(response.start_index, 10);
        assert_eq!(response.total, 0);
    }
}
