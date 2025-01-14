use crate::helpers::vectors::{compare_coin_vectors, compare_u64_vectors};
use cosmwasm_std::{Coin, Uint128};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_coin_vectors() {
        // Test case 1: Equal vectors
        let hydro = vec![
            Coin {
                denom: "uatom".to_string(),
                amount: Uint128::new(100),
            },
            Coin {
                denom: "uatom".to_string(),
                amount: Uint128::new(50),
            },
        ];

        let received = vec![Coin {
            denom: "uatom".to_string(),
            amount: Uint128::new(150),
        }];

        assert!(compare_coin_vectors(hydro, received));

        // Test case 2: Different amounts
        let hydro = vec![Coin {
            denom: "uatom".to_string(),
            amount: Uint128::new(100),
        }];

        let received = vec![Coin {
            denom: "uatom".to_string(),
            amount: Uint128::new(150),
        }];

        assert!(!compare_coin_vectors(hydro, received));
    }

    #[test]
    fn test_compare_u64_vectors() {
        // Test case 1: Equal vectors in different order
        let vec1 = vec![3, 1, 4, 1, 5];
        let vec2 = vec![1, 3, 5, 1, 4];
        assert!(compare_u64_vectors(vec1, vec2));

        // Test case 2: Different vectors
        let vec1 = vec![1, 2, 3];
        let vec2 = vec![1, 2, 4];
        assert!(!compare_u64_vectors(vec1, vec2));

        // Test case 3: Different lengths
        let vec1 = vec![1, 2, 3];
        let vec2 = vec![1, 2];
        assert!(!compare_u64_vectors(vec1, vec2));

        // Test case 4: Empty vectors
        let vec1: Vec<u64> = vec![];
        let vec2: Vec<u64> = vec![];
        assert!(compare_u64_vectors(vec1, vec2));

        // Test case 5: Vectors with duplicates
        let vec1 = vec![1, 2, 2, 3];
        let vec2 = vec![2, 1, 3, 2];
        assert!(compare_u64_vectors(vec1, vec2));
    }
}
