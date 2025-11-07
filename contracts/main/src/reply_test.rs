#[cfg(test)]
mod tests {
    use cosmwasm_std::{Attribute, Coin, Event, Reply, SubMsgResponse, SubMsgResult};

    // Helper function to create a reply with attributes
    fn create_reply_with_attributes(id: u64, attributes: Vec<(&str, &str)>) -> Reply {
        let attrs: Vec<Attribute> = attributes
            .iter()
            .map(|(k, v)| Attribute {
                key: k.to_string(),
                value: v.to_string(),
            })
            .collect();

        let event = Event::new("wasm").add_attributes(attrs);

        // data is deprecated in SubMsgResponse
        #[allow(deprecated)]
        Reply {
            id,
            payload: cosmwasm_std::Binary::default(),
            gas_used: 0,
            result: SubMsgResult::Ok(SubMsgResponse {
                events: vec![event],
                data: None,
                msg_responses: vec![],
            }),
        }
    }

    #[test]
    fn test_parse_locks_skipped_reply_empty() {
        let reply = create_reply_with_attributes(1, vec![("locks_skipped", "")]);

        let result = super::super::reply::parse_locks_skipped_reply(&reply);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Vec::<u64>::new());
    }

    #[test]
    fn test_parse_locks_skipped_reply_single() {
        let reply = create_reply_with_attributes(1, vec![("locks_skipped", "42")]);

        let result = super::super::reply::parse_locks_skipped_reply(&reply);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![42]);
    }

    #[test]
    fn test_parse_locks_skipped_reply_multiple() {
        let reply = create_reply_with_attributes(1, vec![("locks_skipped", "1,2,3,4,5")]);

        let result = super::super::reply::parse_locks_skipped_reply(&reply);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_parse_locks_skipped_reply_with_spaces() {
        let reply = create_reply_with_attributes(1, vec![("locks_skipped", "1, 2, 3, 4, 5")]);

        let result = super::super::reply::parse_locks_skipped_reply(&reply);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_parse_locks_skipped_reply_missing_attribute() {
        let reply = create_reply_with_attributes(1, vec![("other_attribute", "value")]);

        let result = super::super::reply::parse_locks_skipped_reply(&reply);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_locks_skipped_reply_invalid_format() {
        let reply = create_reply_with_attributes(1, vec![("locks_skipped", "abc,def")]);

        let result = super::super::reply::parse_locks_skipped_reply(&reply);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_unlocked_lock_ids_reply_empty() {
        let reply = create_reply_with_attributes(1, vec![("unlocked_lock_ids", "")]);

        let result = super::super::reply::parse_unlocked_lock_ids_reply(&reply);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Vec::<u64>::new());
    }

    #[test]
    fn test_parse_unlocked_lock_ids_reply_single() {
        let reply = create_reply_with_attributes(1, vec![("unlocked_lock_ids", "100")]);

        let result = super::super::reply::parse_unlocked_lock_ids_reply(&reply);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![100]);
    }

    #[test]
    fn test_parse_unlocked_lock_ids_reply_multiple() {
        let reply = create_reply_with_attributes(1, vec![("unlocked_lock_ids", "10,20,30,40")]);

        let result = super::super::reply::parse_unlocked_lock_ids_reply(&reply);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![10, 20, 30, 40]);
    }

    #[test]
    fn test_parse_unlocked_token_from_reply_empty() {
        let reply = create_reply_with_attributes(1, vec![("unlocked_tokens", "")]);

        let result = super::super::reply::parse_unlocked_token_from_reply(&reply);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![]);
    }

    #[test]
    fn test_parse_unlocked_token_from_reply_single() {
        let reply = create_reply_with_attributes(1, vec![("unlocked_tokens", "1000uatom")]);

        let result = super::super::reply::parse_unlocked_token_from_reply(&reply);
        assert!(result.is_ok());
        let coins = result.unwrap();
        assert_eq!(coins.len(), 1);
        assert_eq!(coins[0], Coin::new(1000u128, "uatom"));
    }

    #[test]
    fn test_parse_unlocked_token_from_reply_multiple() {
        let reply =
            create_reply_with_attributes(1, vec![("unlocked_tokens", "1000uatom, 2000uosmo")]);

        let result = super::super::reply::parse_unlocked_token_from_reply(&reply);
        assert!(result.is_ok());
        let coins = result.unwrap();
        assert_eq!(coins.len(), 2);
        assert_eq!(coins[0], Coin::new(1000u128, "uatom"));
        assert_eq!(coins[1], Coin::new(2000u128, "uosmo"));
    }

    #[test]
    fn test_parse_unlocked_token_from_reply_multiple_same_denom() {
        let reply =
            create_reply_with_attributes(1, vec![("unlocked_tokens", "1000uatom, 500uatom")]);

        let result = super::super::reply::parse_unlocked_token_from_reply(&reply);
        assert!(result.is_ok());
        let coins = result.unwrap();
        assert_eq!(coins.len(), 2);
        assert_eq!(coins[0], Coin::new(1000u128, "uatom"));
        assert_eq!(coins[1], Coin::new(500u128, "uatom"));
    }

    #[test]
    fn test_parse_unlocked_token_from_reply_missing_attribute() {
        let reply = create_reply_with_attributes(1, vec![("other_attribute", "value")]);

        let result = super::super::reply::parse_unlocked_token_from_reply(&reply);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_unlocked_token_from_reply_invalid_format() {
        let reply = create_reply_with_attributes(1, vec![("unlocked_tokens", "invalid")]);

        let result = super::super::reply::parse_unlocked_token_from_reply(&reply);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_unlocked_token_from_reply_complex() {
        let reply = create_reply_with_attributes(
            1,
            vec![(
                "unlocked_tokens",
                "1000uatom, 2000uosmo, 3000ibc/ABC123, 4000factory/contract/subdenom",
            )],
        );

        let result = super::super::reply::parse_unlocked_token_from_reply(&reply);
        assert!(result.is_ok());
        let coins = result.unwrap();
        assert_eq!(coins.len(), 4);
        assert_eq!(coins[0], Coin::new(1000u128, "uatom"));
        assert_eq!(coins[1], Coin::new(2000u128, "uosmo"));
        assert_eq!(coins[2], Coin::new(3000u128, "ibc/ABC123"));
        assert_eq!(coins[3], Coin::new(4000u128, "factory/contract/subdenom"));
    }

    #[test]
    fn test_parse_reply_with_error_result() {
        let reply = Reply {
            id: 1,
            payload: cosmwasm_std::Binary::default(),
            gas_used: 0,
            result: SubMsgResult::Err("execution failed".to_string()),
        };

        let result = super::super::reply::parse_locks_skipped_reply(&reply);
        assert!(result.is_err());

        let result2 = super::super::reply::parse_unlocked_lock_ids_reply(&reply);
        assert!(result2.is_err());

        let result3 = super::super::reply::parse_unlocked_token_from_reply(&reply);
        assert!(result3.is_err());
    }

    #[test]
    fn test_parse_u64_list_large_numbers() {
        let reply =
            create_reply_with_attributes(1, vec![("locks_skipped", "18446744073709551615,1,0")]);

        let result = super::super::reply::parse_locks_skipped_reply(&reply);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![18446744073709551615, 1, 0]);
    }
}
