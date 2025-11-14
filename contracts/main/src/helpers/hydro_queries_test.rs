#[cfg(test)]
mod tests {
    use cosmwasm_std::{testing::mock_env, Env};
    use zephyrus_core::state::{Constants, HydroConfig};

    use crate::{
        helpers::hydro_queries::{
            query_hydro_constants, query_hydro_current_round, query_hydro_lockups_shares,
            query_hydro_lockups_with_tranche_infos, query_hydro_specific_user_lockups,
            query_hydro_tranches,
        },
        testing::make_valid_addr,
        testing_mocks::{generate_deterministic_tws, mock_dependencies, mock_hydro_contract},
    };

    fn get_test_constants() -> Constants {
        Constants {
            default_hydromancer_id: 0,
            paused_contract: false,
            hydro_config: HydroConfig {
                hydro_contract_address: make_valid_addr("hydro"),
                hydro_tribute_contract_address: make_valid_addr("tribute"),
            },
            commission_rate: "0.1".parse().unwrap(),
            commission_recipient: make_valid_addr("commission_recipient"),
            min_tokens_per_vessel: 5_000_000,
        }
    }

    fn get_test_env() -> Env {
        let mut env = mock_env();
        env.contract.address = make_valid_addr("zephyrus_contract");
        env
    }

    #[test]
    fn test_query_hydro_current_round_success() {
        let deps = mock_dependencies();
        let constants = get_test_constants();

        let result = query_hydro_current_round(&deps.as_ref(), &constants);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1); // Default current_round in mock is 1
    }

    #[test]
    fn test_query_hydro_current_round_wrong_contract_fails() {
        let deps = mock_dependencies();
        let mut constants = get_test_constants();
        constants.hydro_config.hydro_contract_address = make_valid_addr("wrong_contract");

        let result = query_hydro_current_round(&deps.as_ref(), &constants);

        assert!(result.is_err());
    }

    #[test]
    fn test_query_hydro_constants_success() {
        let deps = mock_dependencies();
        let constants = get_test_constants();

        let result = query_hydro_constants(&deps.as_ref(), &constants);

        assert!(result.is_ok());
        let hydro_constants_response = result.unwrap();

        // Verify the mock data structure
        assert_eq!(hydro_constants_response.constants.round_length, 1_000_000);
        assert_eq!(
            hydro_constants_response.constants.lock_epoch_length,
            1_000_000
        );
        assert_eq!(
            hydro_constants_response.constants.max_locked_tokens,
            55_000_000_000
        );
        assert_eq!(hydro_constants_response.constants.known_users_cap, 0);
        assert!(!hydro_constants_response.constants.paused);
        assert_eq!(
            hydro_constants_response.constants.max_deployment_duration,
            3
        );

        // Verify round lock power schedule
        let schedule = &hydro_constants_response
            .constants
            .round_lock_power_schedule
            .round_lock_power_schedule;
        assert_eq!(schedule.len(), 3);
        assert_eq!(schedule[0].locked_rounds, 1);
        assert_eq!(schedule[1].locked_rounds, 2);
        assert_eq!(schedule[2].locked_rounds, 3);

        // Verify collection info
        assert_eq!(
            hydro_constants_response
                .constants
                .cw721_collection_info
                .name,
            "Hydro Lockups"
        );
        assert_eq!(
            hydro_constants_response
                .constants
                .cw721_collection_info
                .symbol,
            "hydro-lockups"
        );
    }

    #[test]
    fn test_query_hydro_constants_wrong_contract_fails() {
        let deps = mock_dependencies();
        let mut constants = get_test_constants();
        constants.hydro_config.hydro_contract_address = make_valid_addr("wrong_contract");

        let result = query_hydro_constants(&deps.as_ref(), &constants);

        assert!(result.is_err());
    }

    #[test]
    fn test_query_hydro_tranches_success() {
        let deps = mock_dependencies();
        let constants = get_test_constants();

        let result = query_hydro_tranches(&deps.as_ref(), &constants);

        assert!(result.is_ok());
        let tranches = result.unwrap();

        // Mock returns one tranche with id 1
        assert_eq!(tranches.len(), 1);
        assert_eq!(tranches[0], 1);
    }

    #[test]
    fn test_query_hydro_tranches_wrong_contract_fails() {
        let deps = mock_dependencies();
        let mut constants = get_test_constants();
        constants.hydro_config.hydro_contract_address = make_valid_addr("wrong_contract");

        let result = query_hydro_tranches(&deps.as_ref(), &constants);

        assert!(result.is_err());
    }

    #[test]
    fn test_query_hydro_lockups_shares_success() {
        let deps = mock_dependencies();
        let constants = get_test_constants();
        let vessel_ids = vec![1, 2, 3];

        let result = query_hydro_lockups_shares(&deps.as_ref(), &constants, vessel_ids.clone());

        assert!(result.is_ok());
        let lockups_shares_response = result.unwrap();

        // Mock creates one entry per vessel_id
        assert_eq!(lockups_shares_response.lockups.len(), vessel_ids.len());

        for (i, vessel_id) in vessel_ids.iter().enumerate() {
            let shares_info = &lockups_shares_response.lockups[i];
            let (token_group_id, tws) = generate_deterministic_tws(*vessel_id);
            assert_eq!(shares_info.lock_id, *vessel_id);
            assert_eq!(shares_info.time_weighted_shares.u128(), tws);
            assert_eq!(shares_info.token_group_id, token_group_id);
            assert_eq!(shares_info.locked_rounds_remaining, 1);
        }
    }

    #[test]
    fn test_query_hydro_lockups_shares_empty_list() {
        let deps = mock_dependencies();
        let constants = get_test_constants();
        let vessel_ids = vec![];

        let result = query_hydro_lockups_shares(&deps.as_ref(), &constants, vessel_ids);

        assert!(result.is_ok());
        let lockups_shares_response = result.unwrap();
        assert_eq!(lockups_shares_response.lockups.len(), 0);
    }

    #[test]
    fn test_query_hydro_lockups_shares_wrong_contract_fails() {
        let deps = mock_dependencies();
        let mut constants = get_test_constants();
        constants.hydro_config.hydro_contract_address = make_valid_addr("wrong_contract");
        let vessel_ids = vec![1, 2];

        let result = query_hydro_lockups_shares(&deps.as_ref(), &constants, vessel_ids);

        assert!(result.is_err());
        // Should contain the error message with vessel IDs
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Failed to get time weighted shares for vessels"));
        assert!(error_msg.contains("1,2")); // vessel IDs joined
    }

    #[test]
    fn test_query_hydro_specific_user_lockups_success() {
        let deps = mock_dependencies();
        let constants = get_test_constants();
        let env = get_test_env();
        let lock_ids = vec![1, 2, 3];

        let result =
            query_hydro_specific_user_lockups(&deps.as_ref(), &env, &constants, lock_ids.clone());

        assert!(result.is_ok());
        let specific_lockups_response = result.unwrap();

        // Mock creates one entry per lock_id
        assert_eq!(specific_lockups_response.lockups.len(), lock_ids.len());

        for (i, lock_id) in lock_ids.iter().enumerate() {
            let lockup = &specific_lockups_response.lockups[i];
            assert_eq!(lockup.lock_entry.lock_id, *lock_id);
            assert_eq!(lockup.lock_entry.owner, env.contract.address);
            assert_eq!(lockup.lock_entry.funds.amount.u128(), 5_000_000u128);
            assert_eq!(lockup.lock_entry.funds.denom, "uatom");
            assert_eq!(lockup.current_voting_power.u128(), 1000u128);
        }
    }

    #[test]
    fn test_query_hydro_specific_user_lockups_empty_list() {
        let deps = mock_dependencies();
        let constants = get_test_constants();
        let env = get_test_env();
        let lock_ids = vec![];

        let result = query_hydro_specific_user_lockups(&deps.as_ref(), &env, &constants, lock_ids);

        assert!(result.is_ok());
        let specific_lockups_response = result.unwrap();
        assert_eq!(specific_lockups_response.lockups.len(), 0);
    }

    #[test]
    fn test_query_hydro_specific_user_lockups_error_mode() {
        let mut deps = mock_dependencies();
        mock_hydro_contract(&mut deps, true); // error_specific_user_lockups = true

        let mut constants = get_test_constants();
        constants.hydro_config.hydro_contract_address = make_valid_addr("hydro_addr"); // Match mock_hydro_contract address
        let env = get_test_env();
        let lock_ids = vec![1, 2];

        let result = query_hydro_specific_user_lockups(&deps.as_ref(), &env, &constants, lock_ids);

        assert!(result.is_ok());
        let specific_lockups_response = result.unwrap();
        // In error mode, mock returns empty vec
        assert_eq!(specific_lockups_response.lockups.len(), 0);
    }

    #[test]
    fn test_query_hydro_specific_user_lockups_wrong_contract_fails() {
        let deps = mock_dependencies();
        let mut constants = get_test_constants();
        constants.hydro_config.hydro_contract_address = make_valid_addr("wrong_contract");
        let env = get_test_env();
        let lock_ids = vec![1, 2];

        let result = query_hydro_specific_user_lockups(&deps.as_ref(), &env, &constants, lock_ids);

        assert!(result.is_err());
    }

    #[test]
    fn test_query_hydro_lockups_with_tranche_infos_success() {
        let deps = mock_dependencies();
        let constants = get_test_constants();
        let env = get_test_env();
        let vessel_ids = vec![1, 2];

        let result =
            query_hydro_lockups_with_tranche_infos(&deps.as_ref(), &env, &constants, &vessel_ids);

        assert!(result.is_ok());
        let lockups_with_tranche_infos = result.unwrap();

        // Mock creates one entry per vessel_id
        assert_eq!(lockups_with_tranche_infos.len(), vessel_ids.len());

        for (i, vessel_id) in vessel_ids.iter().enumerate() {
            let lockup_info = &lockups_with_tranche_infos[i];

            // Verify lock entry
            assert_eq!(lockup_info.lock_with_power.lock_entry.lock_id, *vessel_id);
            assert_eq!(
                lockup_info.lock_with_power.lock_entry.funds.amount.u128(),
                1000u128
            );
            assert_eq!(lockup_info.lock_with_power.lock_entry.funds.denom, "uatom");
            assert_eq!(
                lockup_info.lock_with_power.current_voting_power.u128(),
                1000u128
            );

            // Verify per-tranche info
            assert_eq!(lockup_info.per_tranche_info.len(), 1);
            let tranche_info = &lockup_info.per_tranche_info[0];
            assert_eq!(tranche_info.tranche_id, 1);
            assert_eq!(tranche_info.next_round_lockup_can_vote, 2);
            assert!(tranche_info.current_voted_on_proposal.is_none());
            assert!(tranche_info.tied_to_proposal.is_none());
            assert!(tranche_info.historic_voted_on_proposals.is_empty());
        }
    }

    #[test]
    fn test_query_hydro_lockups_with_tranche_infos_empty_list() {
        let deps = mock_dependencies();
        let constants = get_test_constants();
        let env = get_test_env();
        let vessel_ids = vec![];

        let result =
            query_hydro_lockups_with_tranche_infos(&deps.as_ref(), &env, &constants, &vessel_ids);

        assert!(result.is_ok());
        let lockups_with_tranche_infos = result.unwrap();
        assert_eq!(lockups_with_tranche_infos.len(), 0);
    }

    #[test]
    fn test_query_hydro_lockups_with_tranche_infos_wrong_contract_fails() {
        let deps = mock_dependencies();
        let mut constants = get_test_constants();
        constants.hydro_config.hydro_contract_address = make_valid_addr("wrong_contract");
        let env = get_test_env();
        let vessel_ids = vec![1, 2];

        let result =
            query_hydro_lockups_with_tranche_infos(&deps.as_ref(), &env, &constants, &vessel_ids);

        assert!(result.is_err());
    }

    #[test]
    fn test_query_hydro_constants_with_default_values() {
        // Test that we get the default mock values when no custom constants are provided
        let deps = mock_dependencies();
        let constants = get_test_constants();

        let result = query_hydro_constants(&deps.as_ref(), &constants);

        assert!(result.is_ok());
        let hydro_constants_response = result.unwrap();

        // Verify default mock values from testing_mocks.rs
        assert_eq!(hydro_constants_response.constants.round_length, 1_000_000);
        assert_eq!(
            hydro_constants_response.constants.lock_epoch_length,
            1_000_000
        );
        assert_eq!(
            hydro_constants_response.constants.max_locked_tokens,
            55_000_000_000
        );
        assert_eq!(hydro_constants_response.constants.known_users_cap, 0);
        assert!(!hydro_constants_response.constants.paused);
        assert_eq!(
            hydro_constants_response.constants.max_deployment_duration,
            3
        );
        assert_eq!(
            hydro_constants_response
                .constants
                .cw721_collection_info
                .name,
            "Hydro Lockups"
        );
        assert_eq!(
            hydro_constants_response
                .constants
                .cw721_collection_info
                .symbol,
            "hydro-lockups"
        );

        // Verify default schedule has 3 entries
        let schedule = &hydro_constants_response
            .constants
            .round_lock_power_schedule
            .round_lock_power_schedule;
        assert_eq!(schedule.len(), 3);
        assert_eq!(schedule[0].locked_rounds, 1);
        assert_eq!(schedule[1].locked_rounds, 2);
        assert_eq!(schedule[2].locked_rounds, 3);
    }

    #[test]
    fn test_query_hydro_current_round_default_value() {
        // Test that we get the default current_round value from mocks
        let deps = mock_dependencies();
        let constants = get_test_constants();

        let result = query_hydro_current_round(&deps.as_ref(), &constants);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1); // Default value in testing_mocks.rs
    }

    #[test]
    fn test_integration_multiple_queries() {
        let deps = mock_dependencies();
        let constants = get_test_constants();
        let env = get_test_env();

        // Test multiple queries work together
        let current_round = query_hydro_current_round(&deps.as_ref(), &constants).unwrap();
        let tranches = query_hydro_tranches(&deps.as_ref(), &constants).unwrap();
        let hydro_constants = query_hydro_constants(&deps.as_ref(), &constants).unwrap();

        assert_eq!(current_round, 1);
        assert_eq!(tranches, vec![1]);
        assert_eq!(hydro_constants.constants.round_length, 1_000_000);

        // Test queries with vessel data
        let vessel_ids = vec![1, 2];
        let lockups_shares =
            query_hydro_lockups_shares(&deps.as_ref(), &constants, vessel_ids.clone()).unwrap();
        let specific_lockups =
            query_hydro_specific_user_lockups(&deps.as_ref(), &env, &constants, vessel_ids.clone())
                .unwrap();
        let lockups_with_tranche_infos =
            query_hydro_lockups_with_tranche_infos(&deps.as_ref(), &env, &constants, &vessel_ids)
                .unwrap();

        assert_eq!(lockups_shares.lockups.len(), 2);
        assert_eq!(specific_lockups.lockups.len(), 2);
        assert_eq!(lockups_with_tranche_infos.len(), 2);
    }
}
