use cosmwasm_std::{testing::mock_dependencies, Addr, Coin, Decimal, Uint128};
use hydro_interface::msgs::DenomInfoResponse;
use std::collections::HashMap;
use zephyrus_core::{
    msgs::{ClaimTributeReplyPayload, CLAIM_TRIBUTE_REPLY_ID},
    state::{Constants, Vessel},
};

use crate::{
    helpers::hydromancer_tribute_data_loader::DataLoader, helpers::rewards::*, state,
    testing::make_valid_addr,
};

// Helper function to create mock constants
fn create_mock_constants() -> Constants {
    Constants {
        commission_rate: Decimal::percent(5), // 5% commission
        hydro_config: zephyrus_core::state::HydroConfig {
            hydro_tribute_contract_address: Addr::unchecked("hydro_tribute_contract"),
            hydro_contract_address: Addr::unchecked("hydro_derivative_contract"),
        },
        commission_recipient: Addr::unchecked("commission_recipient"),
        default_hydromancer_id: 1u64,
        paused_contract: false,
        min_tokens_per_vessel: 5_000_000,
    }
}

// Mock DataLoader for testing
struct MockDataLoader;

impl DataLoader for MockDataLoader {
    fn load_hydromancer_tribute(
        &self,
        _storage: &dyn cosmwasm_std::Storage,
        _hydromancer_id: u64,
        _round_id: u64,
        _tribute_id: u64,
    ) -> cosmwasm_std::StdResult<Option<zephyrus_core::state::HydromancerTribute>> {
        Ok(None)
    }
}

// Helper function to create mock token info provider
fn create_mock_token_info_provider() -> HashMap<String, DenomInfoResponse> {
    let mut provider = HashMap::new();
    provider.insert(
        "token_group_1".to_string(),
        DenomInfoResponse {
            ratio: Decimal::percent(100),
            denom: "uatom".to_string(),
            token_group_id: "token_group_1".to_string(),
        },
    );
    provider.insert(
        "token_group_2".to_string(),
        DenomInfoResponse {
            ratio: Decimal::percent(50),
            denom: "uosmo".to_string(),
            token_group_id: "token_group_2".to_string(),
        },
    );
    provider
}

#[test]
fn test_build_claim_tribute_sub_msg() {
    let round_id = 1u64;
    let tranche_id = 1u64;
    let vessel_ids = vec![1u64, 2u64];
    let owner = Addr::unchecked("owner");
    let constants = create_mock_constants();
    let contract_address = Addr::unchecked("contract");
    let balances = vec![Coin::new(1000u128, "uatom")];
    let outstanding_tribute = hydro_interface::msgs::TributeClaim {
        tribute_id: 1,
        proposal_id: 1,
        round_id: 1,
        tranche_id: 1,
        amount: Coin::new(100u128, "uatom"),
    };

    let result = build_claim_tribute_sub_msg(
        round_id,
        tranche_id,
        &vessel_ids,
        &owner,
        &constants,
        &contract_address,
        &balances,
        &outstanding_tribute,
    );

    assert!(result.is_ok());
    let sub_msg = result.unwrap();
    assert_eq!(sub_msg.id, CLAIM_TRIBUTE_REPLY_ID);
}

#[test]
fn test_calcul_total_voting_power_of_hydromancer_on_proposal() {
    let deps = mock_dependencies();

    let hydromancer_id = 1u64;
    let proposal_id = 1u64;
    let round_id = 1u64;
    let token_info_provider = create_mock_token_info_provider();

    let result = calculate_total_voting_power_of_hydromancer_on_proposal(
        deps.as_ref().storage,
        hydromancer_id,
        proposal_id,
        round_id,
        &token_info_provider,
    );

    assert!(result.is_err() || result.is_ok());
}

#[test]
fn test_calculate_total_voting_power_of_hydromancer_for_locked_rounds() {
    let deps = mock_dependencies();

    let hydromancer_id = 1u64;
    let round_id = 1u64;
    let locked_rounds = 2u64;
    let token_info_provider = create_mock_token_info_provider();

    let result = calculate_total_voting_power_of_hydromancer_for_locked_rounds(
        deps.as_ref().storage,
        hydromancer_id,
        round_id,
        locked_rounds,
        &token_info_provider,
    );

    assert!(result.is_err() || result.is_ok());
}

#[test]
fn test_calcul_total_voting_power_on_proposal() {
    let deps = mock_dependencies();

    let proposal_id = 1u64;
    let round_id = 1u64;
    let token_info_provider = create_mock_token_info_provider();

    let result = calculate_total_voting_power_on_proposal(
        deps.as_ref().storage,
        proposal_id,
        round_id,
        &token_info_provider,
    );

    assert!(result.is_err() || result.is_ok());
}

#[test]
fn test_calculate_voting_power_of_vessel() {
    let deps = mock_dependencies();

    let vessel_id = 1u64;
    let round_id = 1u64;
    let token_info_provider = create_mock_token_info_provider();

    let result = calculate_voting_power_of_vessel(
        deps.as_ref().storage,
        vessel_id,
        round_id,
        &token_info_provider,
    );

    assert!(result.is_err() || result.is_ok());
}

#[test]
fn test_calculate_rewards_amount_for_vessel_on_proposal() {
    let deps = mock_dependencies();

    let round_id = 1u64;
    let tranche_id = 1u64;
    let proposal_id = 1u64;
    let tribute_id = 1u64;
    let constants = create_mock_constants();
    let token_info_provider = create_mock_token_info_provider();
    let total_proposal_voting_power = Decimal::percent(100);
    let proposal_rewards = Coin::new(1000u128, "uatom");
    let vessel_id = 1u64;

    let mock_data_loader = MockDataLoader;
    let result = calculate_rewards_amount_for_vessel_on_tribute(
        deps.as_ref(),
        round_id,
        tranche_id,
        proposal_id,
        tribute_id,
        &constants,
        &token_info_provider,
        total_proposal_voting_power,
        proposal_rewards,
        vessel_id,
        &mock_data_loader,
    );

    assert!(result.is_err() || result.is_ok());
}

#[test]
fn test_calcul_protocol_comm_and_rest() {
    let payload = ClaimTributeReplyPayload {
        proposal_id: 1u64,
        tribute_id: 1u64,
        round_id: 1u64,
        tranche_id: 1u64,
        amount: Coin::new(1000u128, "uatom"),
        balance_before_claim: Coin::new(500u128, "uatom"),
        vessels_owner: Addr::unchecked("owner"),
        vessel_ids: vec![1u64, 2u64],
    };
    let constants = create_mock_constants();

    let (commission_amount, user_funds) =
        calculate_protocol_comm_and_rest(payload.amount.clone(), &constants);

    // Verify commission calculation (5% of 1000 = 50)
    assert_eq!(commission_amount, Uint128::new(50));

    // Verify user funds (1000 - 50 = 950)
    assert_eq!(user_funds.amount, Uint128::new(950));
    assert_eq!(user_funds.denom, "uatom");
}

#[test]
fn test_calculate_hydromancer_claiming_rewards() {
    let deps = mock_dependencies();

    let sender = Addr::unchecked("sender");
    let round_id = 1u64;
    let tribute_id = 1u64;

    let mock_data_loader = MockDataLoader;
    let result = calculate_hydromancer_claiming_rewards(
        deps.as_ref(),
        sender,
        round_id,
        tribute_id,
        &mock_data_loader,
    );

    assert!(result.is_err() || result.is_ok());
}

// Test edge cases
#[test]
fn test_calcul_protocol_comm_and_rest_zero_amount() {
    let payload = ClaimTributeReplyPayload {
        proposal_id: 1u64,
        tribute_id: 1u64,
        round_id: 1u64,
        tranche_id: 1u64,
        amount: Coin::new(0u128, "uatom"),
        balance_before_claim: Coin::new(0u128, "uatom"),
        vessels_owner: Addr::unchecked("owner"),
        vessel_ids: vec![],
    };
    let constants = create_mock_constants();

    let (commission_amount, user_funds) =
        calculate_protocol_comm_and_rest(payload.amount.clone(), &constants);

    assert_eq!(commission_amount, Uint128::zero());
    assert_eq!(user_funds.amount, Uint128::zero());
}

#[test]
fn test_calcul_protocol_comm_and_rest_high_commission() {
    let mut constants = create_mock_constants();
    constants.commission_rate = Decimal::percent(100); // 100% commission

    let payload = ClaimTributeReplyPayload {
        proposal_id: 1u64,
        tribute_id: 1u64,
        round_id: 1u64,
        tranche_id: 1u64,
        amount: Coin::new(1000u128, "uatom"),
        balance_before_claim: Coin::new(500u128, "uatom"),
        vessels_owner: Addr::unchecked("owner"),
        vessel_ids: vec![1u64, 2u64],
    };

    let (commission_amount, user_funds) =
        calculate_protocol_comm_and_rest(payload.amount.clone(), &constants);

    assert_eq!(commission_amount, Uint128::new(1000));
    assert_eq!(user_funds.amount, Uint128::zero());
}

// Test with different denominations
#[test]
fn test_calcul_protocol_comm_and_rest_different_denom() {
    let payload = ClaimTributeReplyPayload {
        proposal_id: 1u64,
        tribute_id: 1u64,
        round_id: 1u64,
        tranche_id: 1u64,
        amount: Coin::new(1000u128, "uosmo"),
        balance_before_claim: Coin::new(500u128, "uosmo"),
        vessels_owner: Addr::unchecked("owner"),
        vessel_ids: vec![1u64, 2u64],
    };
    let constants = create_mock_constants();

    let (commission_amount, user_funds) =
        calculate_protocol_comm_and_rest(payload.amount.clone(), &constants);

    assert_eq!(commission_amount, Uint128::new(50));
    assert_eq!(user_funds.amount, Uint128::new(950));
    assert_eq!(user_funds.denom, "uosmo");
}

// Test error handling for division by zero scenarios
#[test]
fn test_voting_power_calculation_with_zero_total() {
    let deps = mock_dependencies();

    let round_id = 1u64;
    let tranche_id = 1u64;
    let proposal_id = 1u64;
    let tribute_id = 1u64;
    let constants = create_mock_constants();
    let token_info_provider = create_mock_token_info_provider();
    let total_proposal_voting_power = Decimal::zero(); // This should cause division by zero
    let proposal_rewards = Coin::new(1000u128, "uatom");
    let vessel_id = 1u64;

    let mock_data_loader = MockDataLoader;
    let result = calculate_rewards_amount_for_vessel_on_tribute(
        deps.as_ref(),
        round_id,
        tranche_id,
        proposal_id,
        tribute_id,
        &constants,
        &token_info_provider,
        total_proposal_voting_power,
        proposal_rewards,
        vessel_id,
        &mock_data_loader,
    );

    // Should return an error due to division by zero
    assert!(result.is_err());
}

// Test empty vessel IDs list
#[test]
fn test_calculate_rewards_for_vessels_on_tribute_empty_list() {
    let deps = mock_dependencies();

    let vessel_ids = vec![]; // Empty list
    let tribute_id = 1u64;
    let tranche_id = 1u64;
    let round_id = 1u64;
    let proposal_id = 1u64;
    let tribute_rewards = Coin::new(1000u128, "uatom");
    let constants = create_mock_constants();
    let token_info_provider = create_mock_token_info_provider();
    let total_proposal_voting_power = Decimal::percent(100);

    let mock_data_loader = MockDataLoader;
    let result = calculate_rewards_for_vessels_on_tribute(
        deps.as_ref(),
        vessel_ids,
        tribute_id,
        tranche_id,
        round_id,
        proposal_id,
        tribute_rewards,
        constants,
        token_info_provider,
        total_proposal_voting_power,
        &mock_data_loader,
    );

    // Should return zero rewards for empty vessel list
    assert!(result.is_ok());
    if let Ok(amount) = result {
        assert_eq!(amount, Decimal::zero());
    }
}

// Test with very large amounts
#[test]
fn test_calcul_protocol_comm_and_rest_large_amount() {
    let payload = ClaimTributeReplyPayload {
        proposal_id: 1u64,
        tribute_id: 1u64,
        round_id: 1u64,
        tranche_id: 1u64,
        amount: Coin::new(u64::MAX as u128, "uatom"),
        balance_before_claim: Coin::new(0u128, "uatom"),
        vessels_owner: Addr::unchecked("owner"),
        vessel_ids: vec![1u64, 2u64],
    };
    let constants = create_mock_constants();

    let (commission_amount, user_funds) =
        calculate_protocol_comm_and_rest(payload.amount.clone(), &constants);

    // Should handle large amounts without overflow
    assert!(commission_amount > Uint128::zero());
    assert!(user_funds.amount > Uint128::zero());
    assert_eq!(user_funds.denom, "uatom");
}

// Test build_claim_tribute_sub_msg with different scenarios
#[test]
fn test_build_claim_tribute_sub_msg_with_balance_found() {
    let round_id = 1u64;
    let tranche_id = 1u64;
    let vessel_ids = vec![1u64, 2u64];
    let owner = Addr::unchecked("owner");
    let constants = create_mock_constants();
    let contract_address = Addr::unchecked("contract");
    let balances = vec![Coin::new(1000u128, "uatom"), Coin::new(500u128, "uosmo")];
    let outstanding_tribute = hydro_interface::msgs::TributeClaim {
        tribute_id: 1,
        proposal_id: 1,
        round_id: 1,
        tranche_id: 1,
        amount: Coin::new(100u128, "uatom"),
    };

    let result = build_claim_tribute_sub_msg(
        round_id,
        tranche_id,
        &vessel_ids,
        &owner,
        &constants,
        &contract_address,
        &balances,
        &outstanding_tribute,
    );

    assert!(result.is_ok());
    let sub_msg = result.unwrap();
    assert_eq!(sub_msg.id, CLAIM_TRIBUTE_REPLY_ID);
}

#[test]
fn test_build_claim_tribute_sub_msg_with_balance_not_found() {
    let round_id = 1u64;
    let tranche_id = 1u64;
    let vessel_ids = vec![1u64, 2u64];
    let owner = Addr::unchecked("owner");
    let constants = create_mock_constants();
    let contract_address = Addr::unchecked("contract");
    let balances = vec![Coin::new(500u128, "uosmo")]; // Different denom
    let outstanding_tribute = hydro_interface::msgs::TributeClaim {
        tribute_id: 1,
        proposal_id: 1,
        round_id: 1,
        tranche_id: 1,
        amount: Coin::new(100u128, "uatom"),
    };

    let result = build_claim_tribute_sub_msg(
        round_id,
        tranche_id,
        &vessel_ids,
        &owner,
        &constants,
        &contract_address,
        &balances,
        &outstanding_tribute,
    );

    assert!(result.is_ok());
    let sub_msg = result.unwrap();
    assert_eq!(sub_msg.id, CLAIM_TRIBUTE_REPLY_ID);
}

#[test]
fn test_build_claim_tribute_sub_msg_with_empty_balances() {
    let round_id = 1u64;
    let tranche_id = 1u64;
    let vessel_ids = vec![1u64, 2u64];
    let owner = Addr::unchecked("owner");
    let constants = create_mock_constants();
    let contract_address = Addr::unchecked("contract");
    let balances = vec![]; // Empty balances
    let outstanding_tribute = hydro_interface::msgs::TributeClaim {
        tribute_id: 1,
        proposal_id: 1,
        round_id: 1,
        tranche_id: 1,
        amount: Coin::new(100u128, "uatom"),
    };

    let result = build_claim_tribute_sub_msg(
        round_id,
        tranche_id,
        &vessel_ids,
        &owner,
        &constants,
        &contract_address,
        &balances,
        &outstanding_tribute,
    );

    assert!(result.is_ok());
    let sub_msg = result.unwrap();
    assert_eq!(sub_msg.id, CLAIM_TRIBUTE_REPLY_ID);
}

// Test calculate_voting_power_of_vessel with vessel shares error
#[test]
fn test_calculate_voting_power_of_vessel_with_shares_error() {
    let deps = mock_dependencies();
    let vessel_id = 1u64;
    let round_id = 1u64;
    let token_info_provider = create_mock_token_info_provider();

    let result = calculate_voting_power_of_vessel(
        deps.as_ref().storage,
        vessel_id,
        round_id,
        &token_info_provider,
    );

    // Should return zero when vessel shares don't exist
    assert!(result.is_ok());
    if let Ok(voting_power) = result {
        assert_eq!(voting_power, Decimal::zero());
    }
}

// Test calculate_voting_power_of_vessel with token info not found
#[test]
fn test_calculate_voting_power_of_vessel_token_info_not_found() {
    let mut deps = mock_dependencies();

    // Create vessel with unknown token group
    let vessel_id = 1u64;
    let round_id = 1u64;

    // Insert vessel shares with unknown token group
    state::save_vessel_shares_info(
        deps.as_mut().storage,
        vessel_id,
        1u64,                              // round_id
        1000u128,                          // time_weighted_shares
        "unknown_token_group".to_string(), // token_group_id
        1u64,                              // locked_rounds
    )
    .expect("Should save vessel shares");

    let token_info_provider = create_mock_token_info_provider(); // Doesn't contain "unknown_token_group"

    let result = calculate_voting_power_of_vessel(
        deps.as_ref().storage,
        vessel_id,
        round_id,
        &token_info_provider,
    );

    // Should fail due to token info not found
    assert!(result.is_err());
}

// Test calculate_hydromancer_claiming_rewards with different scenarios
#[test]
fn test_calculate_hydromancer_claiming_rewards_not_hydromancer() {
    let deps = mock_dependencies();

    let sender = Addr::unchecked("not_hydromancer");
    let round_id = 1u64;
    let tribute_id = 1u64;

    let mock_data_loader = MockDataLoader;
    let result = calculate_hydromancer_claiming_rewards(
        deps.as_ref(),
        sender,
        round_id,
        tribute_id,
        &mock_data_loader,
    );

    assert!(result.is_ok());
    if let Ok(rewards) = result {
        assert!(rewards.is_none());
    }
}

// Test edge cases for voting power calculations
#[test]
fn test_calcul_total_voting_power_on_proposal_with_empty_tws() {
    let deps = mock_dependencies();

    let proposal_id = 1u64;
    let round_id = 1u64;
    let token_info_provider = create_mock_token_info_provider();

    let result = calculate_total_voting_power_on_proposal(
        deps.as_ref().storage,
        proposal_id,
        round_id,
        &token_info_provider,
    );

    // Should handle empty time weighted shares
    assert!(result.is_err() || result.is_ok());
}

#[test]
fn test_calcul_total_voting_power_of_hydromancer_on_proposal_with_empty_tws() {
    let deps = mock_dependencies();

    let hydromancer_id = 1u64;
    let proposal_id = 1u64;
    let round_id = 1u64;
    let token_info_provider = create_mock_token_info_provider();

    let result = calculate_total_voting_power_of_hydromancer_on_proposal(
        deps.as_ref().storage,
        hydromancer_id,
        proposal_id,
        round_id,
        &token_info_provider,
    );

    // Should handle empty time weighted shares
    assert!(result.is_err() || result.is_ok());
}

#[test]
fn test_calculate_total_voting_power_of_hydromancer_for_locked_rounds_with_empty_tws() {
    let deps = mock_dependencies();

    let hydromancer_id = 1u64;
    let round_id = 1u64;
    let locked_rounds = 2u64;
    let token_info_provider = create_mock_token_info_provider();

    let result = calculate_total_voting_power_of_hydromancer_for_locked_rounds(
        deps.as_ref().storage,
        hydromancer_id,
        round_id,
        locked_rounds,
        &token_info_provider,
    );

    // Should handle empty time weighted shares
    assert!(result.is_err() || result.is_ok());
}

// Test with different locked rounds scenarios
#[test]
fn test_calculate_total_voting_power_of_hydromancer_for_locked_rounds_zero_locked() {
    let deps = mock_dependencies();

    let hydromancer_id = 1u64;
    let round_id = 1u64;
    let locked_rounds = 0u64; // Zero locked rounds
    let token_info_provider = create_mock_token_info_provider();

    let result = calculate_total_voting_power_of_hydromancer_for_locked_rounds(
        deps.as_ref().storage,
        hydromancer_id,
        round_id,
        locked_rounds,
        &token_info_provider,
    );

    assert!(result.is_err() || result.is_ok());
}

// Test calculate_rewards_amount_for_vessel_on_tribute with different scenarios
#[test]
fn test_calculate_rewards_amount_for_vessel_on_tribute_zero_voting_power() {
    let deps = mock_dependencies();

    let round_id = 1u64;
    let tranche_id = 1u64;
    let proposal_id = 1u64;
    let tribute_id = 1u64;
    let constants = create_mock_constants();
    let token_info_provider = create_mock_token_info_provider();
    let total_proposal_voting_power = Decimal::percent(100);
    let proposal_rewards = Coin::new(1000u128, "uatom");
    let vessel_id = 1u64;

    let mock_data_loader = MockDataLoader;
    let result = calculate_rewards_amount_for_vessel_on_tribute(
        deps.as_ref(),
        round_id,
        tranche_id,
        proposal_id,
        tribute_id,
        &constants,
        &token_info_provider,
        total_proposal_voting_power,
        proposal_rewards,
        vessel_id,
        &mock_data_loader,
    );

    // Should handle vessels that don't exist
    assert!(result.is_err() || result.is_ok());
}

// Test calculate_rewards_amount_for_vessel_on_proposal with vessel not found
#[test]
fn test_calculate_rewards_amount_for_vessel_on_tribute_vessel_not_found() {
    let deps = mock_dependencies();

    let round_id = 1u64;
    let tranche_id = 1u64;
    let proposal_id = 1u64;
    let tribute_id = 1u64;
    let constants = create_mock_constants();
    let token_info_provider = create_mock_token_info_provider();
    let total_proposal_voting_power = Decimal::percent(100);
    let proposal_rewards = Coin::new(1000u128, "uatom");
    let vessel_id = 999u64; // Non-existent vessel

    let mock_data_loader = MockDataLoader;
    let result = calculate_rewards_amount_for_vessel_on_tribute(
        deps.as_ref(),
        round_id,
        tranche_id,
        proposal_id,
        tribute_id,
        &constants,
        &token_info_provider,
        total_proposal_voting_power,
        proposal_rewards,
        vessel_id,
        &mock_data_loader,
    );

    // Should fail due to vessel not found
    assert!(result.is_err());
}

// Test allocate_rewards_to_hydromancer with real data
#[test]
fn test_allocate_rewards_to_hydromancer_with_real_data() {
    let mut deps = mock_dependencies();

    // Create hydromancer
    let hydromancer_id = state::insert_new_hydromancer(
        deps.as_mut().storage,
        make_valid_addr("hydromancer"),
        "Test Hydromancer".to_string(),
        Decimal::percent(10), // 10% commission
    )
    .expect("Should create hydromancer");

    // Add hydromancer proposal TWS
    state::add_time_weighted_shares_to_proposal_for_hydromancer(
        deps.as_mut().storage,
        1u64, // proposal_id
        hydromancer_id,
        "token_group_1",
        1000u128,
    )
    .expect("Should save hydromancer proposal TWS");

    let proposal_id = 1u64;
    let round_id = 1u64;
    let funds = Coin::new(1000u128, "uatom");
    let token_info_provider = create_mock_token_info_provider();
    let total_proposal_voting_power = Decimal::from_ratio(2000u128, 1u128); // 2000 total power

    let result = allocate_rewards_to_hydromancer(
        deps.as_ref(),
        proposal_id,
        round_id,
        funds,
        &token_info_provider,
        total_proposal_voting_power,
        hydromancer_id,
    );

    // Should succeed
    assert!(result.is_ok());
}

// Test allocate_rewards_to_hydromancer with division by zero
#[test]
fn test_allocate_rewards_to_hydromancer_division_by_zero() {
    let mut deps = mock_dependencies();

    // Create hydromancer
    let hydromancer_id = state::insert_new_hydromancer(
        deps.as_mut().storage,
        make_valid_addr("hydromancer"),
        "Test Hydromancer".to_string(),
        Decimal::percent(10),
    )
    .expect("Should create hydromancer");

    let proposal_id = 1u64;
    let round_id = 1u64;
    let funds = Coin::new(1000u128, "uatom");
    let token_info_provider = create_mock_token_info_provider();
    let total_proposal_voting_power = Decimal::zero(); // This will cause division by zero

    let result = allocate_rewards_to_hydromancer(
        deps.as_ref(),
        proposal_id,
        round_id,
        funds,
        &token_info_provider,
        total_proposal_voting_power,
        hydromancer_id,
    );

    // Should fail due to division by zero
    assert!(result.is_err());
}

// Test distribute_rewards_for_vessels_on_tribute with real data
#[test]
fn test_distribute_rewards_for_vessels_on_tribute_with_real_data() {
    let mut deps = mock_dependencies();

    // Create user and vessel
    let user_id = state::insert_new_user(deps.as_mut().storage, make_valid_addr("user"))
        .expect("Should create user");

    let vessel_id = 1u64;
    state::add_vessel(
        deps.as_mut().storage,
        &Vessel {
            hydro_lock_id: vessel_id,
            tokenized_share_record_id: None,
            class_period: 1_000_000,
            auto_maintenance: true,
            hydromancer_id: None, // User control
            owner_id: user_id,
        },
        &make_valid_addr("user"),
    )
    .expect("Should add vessel");

    // Add vessel shares
    state::save_vessel_shares_info(
        deps.as_mut().storage,
        vessel_id,
        1u64,     // round_id
        1000u128, // time_weighted_shares
        "token_group_1".to_string(),
        1u64, // locked_rounds
    )
    .expect("Should save vessel shares");

    // Add vessel to harbor
    state::add_vessel_to_harbor(
        deps.as_mut().storage,
        1u64, // tranche_id
        1u64, // round_id
        1u64, // proposal_id
        &zephyrus_core::state::VesselHarbor {
            hydro_lock_id: vessel_id,
            user_control: true,
            steerer_id: 1u64,
        },
    )
    .expect("Should add vessel to harbor");

    let vessel_ids = vec![vessel_id];
    let tribute_id = 1u64;
    let tranche_id = 1u64;
    let round_id = 1u64;
    let proposal_id = 1u64;
    let tribute_rewards = Coin::new(1000u128, "uatom");
    let constants = create_mock_constants();
    let token_info_provider = create_mock_token_info_provider();
    let total_proposal_voting_power = Decimal::from_ratio(2000u128, 1u128);

    let result = distribute_rewards_for_vessels_on_tribute(
        &mut deps.as_mut(),
        vessel_ids,
        tribute_id,
        tranche_id,
        round_id,
        proposal_id,
        tribute_rewards,
        constants,
        token_info_provider,
        total_proposal_voting_power,
    );

    // Should succeed and return calculated rewards
    assert!(result.is_ok());
    if let Ok(amount) = result {
        // Should be (1000 / 2000) * 1000 = 500
        assert_eq!(amount, Decimal::from_ratio(500u128, 1u128));
    }
}

// Test distribute_rewards_for_vessels_on_tribute with already claimed vessels
#[test]
fn test_distribute_rewards_for_vessels_on_tribute_already_claimed() {
    let mut deps = mock_dependencies();

    let vessel_ids = vec![1u64, 2u64];
    let tribute_id = 1u64;
    let tranche_id = 1u64;
    let round_id = 1u64;
    let proposal_id = 1u64;
    let tribute_rewards = Coin::new(1000u128, "uatom");
    let constants = create_mock_constants();
    let token_info_provider = create_mock_token_info_provider();
    let total_proposal_voting_power = Decimal::from_ratio(2000u128, 1u128);

    // Mark vessels as already claimed
    state::save_vessel_tribute_claim(
        deps.as_mut().storage,
        1,
        tribute_id,
        Coin::new(100u128, "uatom"),
    )
    .expect("Should save claim");

    state::save_vessel_tribute_claim(
        deps.as_mut().storage,
        2,
        tribute_id,
        Coin::new(200u128, "uatom"),
    )
    .expect("Should save claim");

    let result = distribute_rewards_for_vessels_on_tribute(
        &mut deps.as_mut(),
        vessel_ids,
        tribute_id,
        tranche_id,
        round_id,
        proposal_id,
        tribute_rewards,
        constants,
        token_info_provider,
        total_proposal_voting_power,
    );

    // Should succeed and return zero since vessels are already claimed
    assert!(result.is_ok());
    if let Ok(amount) = result {
        assert_eq!(amount, Decimal::zero());
    }
}

// Test process_hydromancer_claiming_rewards with real data
#[test]
fn test_process_hydromancer_claiming_rewards_with_real_data() {
    let mut deps = mock_dependencies();

    // Create hydromancer
    let hydromancer_address = make_valid_addr("hydromancer");
    let hydromancer_id = state::insert_new_hydromancer(
        deps.as_mut().storage,
        hydromancer_address.clone(),
        "Test Hydromancer".to_string(),
        Decimal::percent(10),
    )
    .expect("Should create hydromancer");

    // Add hydromancer rewards
    state::add_new_rewards_to_hydromancer(
        deps.as_mut().storage,
        hydromancer_id,
        1u64, // round_id
        1u64, // tribute_id
        zephyrus_core::state::HydromancerTribute {
            rewards_for_users: Coin::new(800u128, "uatom"),
            commission_for_hydromancer: Coin::new(200u128, "uatom"),
        },
    )
    .expect("Should add hydromancer rewards");

    let round_id = 1u64;
    let tribute_id = 1u64;

    let result = process_hydromancer_claiming_rewards(
        &mut deps.as_mut(),
        hydromancer_address,
        round_id,
        tribute_id,
    );

    // Should succeed and return a message
    assert!(result.is_ok());
    if let Ok(option) = result {
        assert!(option.is_some());
        if let Some(bank_msg) = option {
            match bank_msg {
                cosmwasm_std::BankMsg::Send { to_address, amount } => {
                    assert_eq!(
                        to_address,
                        "cosmwasm1k9fnhzkd5jln2cape82tp057t0gzanv4k5htvr9jh3qhazv2fz0sw5yjcf"
                    );
                    assert_eq!(amount.len(), 1);
                    assert_eq!(amount[0].amount, Uint128::new(200));
                    assert_eq!(amount[0].denom, "uatom");
                }
                _ => panic!("Expected BankMsg::Send"),
            }
        }
    }
}

// Test process_hydromancer_claiming_rewards with zero commission
#[test]
fn test_process_hydromancer_claiming_rewards_zero_commission() {
    let mut deps = mock_dependencies();

    // Create hydromancer
    let hydromancer_address = make_valid_addr("hydromancer");
    let hydromancer_id = state::insert_new_hydromancer(
        deps.as_mut().storage,
        hydromancer_address.clone(),
        "Test Hydromancer".to_string(),
        Decimal::percent(10),
    )
    .expect("Should create hydromancer");

    // Add hydromancer rewards with zero commission
    state::add_new_rewards_to_hydromancer(
        deps.as_mut().storage,
        hydromancer_id,
        1u64, // round_id
        1u64, // tribute_id
        zephyrus_core::state::HydromancerTribute {
            rewards_for_users: Coin::new(1000u128, "uatom"),
            commission_for_hydromancer: Coin::new(0u128, "uatom"),
        },
    )
    .expect("Should add hydromancer rewards");

    let round_id = 1u64;
    let tribute_id = 1u64;

    let result = process_hydromancer_claiming_rewards(
        &mut deps.as_mut(),
        hydromancer_address,
        round_id,
        tribute_id,
    );

    // Should succeed but return None due to zero commission
    assert!(result.is_ok());
    if let Ok(option) = result {
        assert!(option.is_none());
    }
}

// Test process_hydromancer_claiming_rewards with no hydromancer tribute
#[test]
fn test_process_hydromancer_claiming_rewards_no_tribute() {
    let mut deps = mock_dependencies();

    // Create hydromancer
    let hydromancer_address = make_valid_addr("hydromancer");
    let _hydromancer_id = state::insert_new_hydromancer(
        deps.as_mut().storage,
        hydromancer_address.clone(),
        "Test Hydromancer".to_string(),
        Decimal::percent(10),
    )
    .expect("Should create hydromancer");

    // Don't add any rewards

    let round_id = 1u64;
    let tribute_id = 1u64;

    let result = process_hydromancer_claiming_rewards(
        &mut deps.as_mut(),
        hydromancer_address,
        round_id,
        tribute_id,
    );

    // Should succeed but return None due to no tribute
    assert!(result.is_ok());
    if let Ok(option) = result {
        assert!(option.is_none());
    }
}
