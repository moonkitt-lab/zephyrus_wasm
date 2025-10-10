#[cfg(test)]
mod tests {
    use super::super::hydromancer_tribute_data_loader::*;
    use cosmwasm_std::testing::MockStorage;
    use std::collections::HashMap;
    use zephyrus_core::state::HydromancerTribute;

    #[test]
    fn test_in_memory_data_loader_empty() {
        let loader = InMemoryDataLoader {
            hydromancer_tributes: HashMap::new(),
        };

        let storage = MockStorage::new();
        let result = loader.load_hydromancer_tribute(&storage, 1, 1, 1);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn test_in_memory_data_loader_with_data() {
        let mut tributes = HashMap::new();
        let tribute = HydromancerTribute {
            rewards_for_users: cosmwasm_std::coin(1000, "uatom"),
            commission_for_hydromancer: cosmwasm_std::coin(100, "uatom"),
        };

        tributes.insert((1, 1, 1), tribute.clone());

        let loader = InMemoryDataLoader {
            hydromancer_tributes: tributes,
        };

        let storage = MockStorage::new();
        let result = loader.load_hydromancer_tribute(&storage, 1, 1, 1);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some(tribute));
    }

    #[test]
    fn test_in_memory_data_loader_missing_key() {
        let mut tributes = HashMap::new();
        let tribute = HydromancerTribute {
            rewards_for_users: cosmwasm_std::coin(1000, "uatom"),
            commission_for_hydromancer: cosmwasm_std::coin(100, "uatom"),
        };

        tributes.insert((1, 1, 1), tribute);

        let loader = InMemoryDataLoader {
            hydromancer_tributes: tributes,
        };

        let storage = MockStorage::new();
        // Query for a different key
        let result = loader.load_hydromancer_tribute(&storage, 2, 2, 2);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn test_in_memory_data_loader_multiple_tributes() {
        let mut tributes = HashMap::new();

        for hydromancer_id in 1..=3 {
            for round_id in 1..=2 {
                for tribute_id in 1..=2 {
                    let amount = (hydromancer_id * round_id * tribute_id * 100) as u128;
                    let tribute = HydromancerTribute {
                        rewards_for_users: cosmwasm_std::coin(amount, "uatom"),
                        commission_for_hydromancer: cosmwasm_std::coin(amount / 10, "uatom"),
                    };
                    tributes.insert((hydromancer_id, round_id, tribute_id), tribute);
                }
            }
        }

        let loader = InMemoryDataLoader {
            hydromancer_tributes: tributes.clone(),
        };

        let storage = MockStorage::new();

        // Test multiple queries
        for hydromancer_id in 1..=3 {
            for round_id in 1..=2 {
                for tribute_id in 1..=2 {
                    let result = loader.load_hydromancer_tribute(
                        &storage,
                        hydromancer_id,
                        round_id,
                        tribute_id,
                    );

                    assert!(result.is_ok());
                    let loaded_tribute = result.unwrap();
                    assert!(loaded_tribute.is_some());
                    assert_eq!(
                        loaded_tribute.unwrap(),
                        tributes[&(hydromancer_id, round_id, tribute_id)]
                    );
                }
            }
        }
    }

    #[test]
    fn test_in_memory_data_loader_different_tributes() {
        let mut tributes = HashMap::new();

        let tribute1 = HydromancerTribute {
            rewards_for_users: cosmwasm_std::coin(5000, "uatom"),
            commission_for_hydromancer: cosmwasm_std::coin(500, "uatom"),
        };

        let tribute2 = HydromancerTribute {
            rewards_for_users: cosmwasm_std::coin(3000, "uosmo"),
            commission_for_hydromancer: cosmwasm_std::coin(300, "uosmo"),
        };

        tributes.insert((1, 1, 1), tribute1.clone());
        tributes.insert((1, 1, 2), tribute2.clone());

        let loader = InMemoryDataLoader {
            hydromancer_tributes: tributes,
        };

        let storage = MockStorage::new();

        let result1 = loader.load_hydromancer_tribute(&storage, 1, 1, 1);
        assert!(result1.is_ok());
        assert_eq!(result1.unwrap(), Some(tribute1));

        let result2 = loader.load_hydromancer_tribute(&storage, 1, 1, 2);
        assert!(result2.is_ok());
        assert_eq!(result2.unwrap(), Some(tribute2));
    }
}
