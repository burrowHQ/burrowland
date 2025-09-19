use crate::*;

#[near_bindgen]
impl Contract {
    #[payable]
    pub fn append_reliable_liquidator_whitelist(&mut self, liquidator_list: Vec<String>) {
        assert_one_yocto();
        self.assert_owner();
        let mut liquidator_whitelist: HashSet<String> =
            if env::storage_has_key(RELIABLE_LIQUIDATOR_WHITELIST.as_bytes()) {
                internal_get_reliable_liquidator_whitelist()
            } else {
                HashSet::new()
            };
        for liquidator in liquidator_list {
            let is_success = liquidator_whitelist.insert(liquidator.clone());
            require!(is_success, format!("exist liquidator: {}", liquidator));
        }
        env::storage_write(
            RELIABLE_LIQUIDATOR_WHITELIST.as_bytes(),
            &liquidator_whitelist.try_to_vec().unwrap(),
        );
    }

    #[payable]
    pub fn remove_reliable_liquidator_whitelist(&mut self, liquidator_list: Vec<String>) {
        assert_one_yocto();
        self.assert_owner();
        let mut liquidator_whitelist = internal_get_reliable_liquidator_whitelist();
        for liquidator in liquidator_list {
            let is_success = liquidator_whitelist.remove(&liquidator);
            require!(is_success, format!("liquidator {} not exist", liquidator));
        }
        env::storage_write(
            RELIABLE_LIQUIDATOR_WHITELIST.as_bytes(),
            &liquidator_whitelist.try_to_vec().unwrap(),
        );
    }

    pub fn get_reliable_liquidator_whitelist(&self) -> Vec<String> {
        if env::storage_has_key(RELIABLE_LIQUIDATOR_WHITELIST.as_bytes()) {
            let liquidator_whitelist = internal_get_reliable_liquidator_whitelist();
            liquidator_whitelist.iter().cloned().collect()
        } else {
            vec![]
        }
    }
}

pub fn internal_get_reliable_liquidator_whitelist() -> HashSet<String> {
    let content =
        env::storage_read(RELIABLE_LIQUIDATOR_WHITELIST.as_bytes()).expect("Empty storage(RELIABLE_LIQUIDATOR_WHITELIST)");
    HashSet::try_from_slice(&content)
        .expect("deserialize reliable liquidator whitelist failed.")
}

pub fn in_reliable_liquidator_whitelist(liquidator_id: &str) -> bool {
    if !env::storage_has_key(RELIABLE_LIQUIDATOR_WHITELIST.as_bytes()) {
        return false;
    }

    let liquidator_whitelist = internal_get_reliable_liquidator_whitelist();
    let res = liquidator_whitelist.iter().any(|item| {
        if let Some(suffix) = item.strip_prefix("*.") {
            liquidator_id.ends_with(&format!(".{}", suffix))
        } else {
            liquidator_id == item
        }
    });
    res
}

#[cfg(test)]
mod tests {
    use super::*;
    use unit_env::*;

    #[test]
    fn test_append_reliable_liquidator_whitelist() {
        let mut test_env = init_unit_env();

        // Test adding liquidators to whitelist
        let liquidators = vec!["alice.near".to_string(), "bob.near".to_string()];
        test_env.contract.append_reliable_liquidator_whitelist(liquidators.clone());

        let whitelist = test_env.contract.get_reliable_liquidator_whitelist();
        assert_eq!(whitelist.len(), 2);
        assert!(whitelist.contains(&"alice.near".to_string()));
        assert!(whitelist.contains(&"bob.near".to_string()));
    }

    #[test]
    fn test_append_reliable_liquidator_whitelist_with_wildcard() {
        let mut test_env = init_unit_env();

        // Test adding wildcard pattern
        let liquidators = vec!["*.liquidator.near".to_string(), "specific.near".to_string()];
        test_env.contract.append_reliable_liquidator_whitelist(liquidators);

        let whitelist = test_env.contract.get_reliable_liquidator_whitelist();
        assert_eq!(whitelist.len(), 2);
        assert!(whitelist.contains(&"*.liquidator.near".to_string()));
        assert!(whitelist.contains(&"specific.near".to_string()));
    }

    #[test]
    #[should_panic(expected = "exist liquidator: alice.near")]
    fn test_append_duplicate_liquidator_fails() {
        let mut test_env = init_unit_env();

        // Add liquidator first time
        test_env.contract.append_reliable_liquidator_whitelist(vec!["alice.near".to_string()]);

        // Try to add the same liquidator again - should fail
        test_env.contract.append_reliable_liquidator_whitelist(vec!["alice.near".to_string()]);
    }

    #[test]
    fn test_remove_reliable_liquidator_whitelist() {
        let mut test_env = init_unit_env();

        // First add liquidators
        let liquidators = vec!["alice.near".to_string(), "bob.near".to_string(), "charlie.near".to_string()];
        test_env.contract.append_reliable_liquidator_whitelist(liquidators);

        // Remove one liquidator
        test_env.contract.remove_reliable_liquidator_whitelist(vec!["bob.near".to_string()]);

        let whitelist = test_env.contract.get_reliable_liquidator_whitelist();
        assert_eq!(whitelist.len(), 2);
        assert!(whitelist.contains(&"alice.near".to_string()));
        assert!(whitelist.contains(&"charlie.near".to_string()));
        assert!(!whitelist.contains(&"bob.near".to_string()));
    }

    #[test]
    #[should_panic(expected = "liquidator nonexistent.near not exist")]
    fn test_remove_nonexistent_liquidator_fails() {
        let mut test_env = init_unit_env();

        // Add some liquidators
        test_env.contract.append_reliable_liquidator_whitelist(vec!["alice.near".to_string()]);

        // Try to remove a liquidator that doesn't exist - should fail
        test_env.contract.remove_reliable_liquidator_whitelist(vec!["nonexistent.near".to_string()]);
    }

    #[test]
    fn test_get_empty_whitelist() {
        let test_env = init_unit_env();

        // Test getting whitelist when it's empty
        let whitelist = test_env.contract.get_reliable_liquidator_whitelist();
        assert_eq!(whitelist.len(), 0);
    }

    #[test]
    fn test_in_reliable_liquidator_whitelist_exact_match() {
        let mut test_env = init_unit_env();

        // Add specific liquidators
        test_env.contract.append_reliable_liquidator_whitelist(vec![
            "alice.near".to_string(),
            "bob.liquidator.near".to_string()
        ]);

        // Test exact matches
        assert!(in_reliable_liquidator_whitelist("alice.near"));
        assert!(in_reliable_liquidator_whitelist("bob.liquidator.near"));
        assert!(!in_reliable_liquidator_whitelist("charlie.near"));
        assert!(!in_reliable_liquidator_whitelist("alice.test.near"));
    }

    #[test]
    fn test_in_reliable_liquidator_whitelist_wildcard_match() {
        let mut test_env = init_unit_env();

        // Add wildcard patterns
        test_env.contract.append_reliable_liquidator_whitelist(vec![
            "*.liquidator.near".to_string(),
            "*.exchange.near".to_string(),
            "specific.near".to_string()
        ]);

        // Test wildcard matches
        assert!(in_reliable_liquidator_whitelist("alice.liquidator.near"));
        assert!(in_reliable_liquidator_whitelist("bob.liquidator.near"));
        assert!(in_reliable_liquidator_whitelist("any.exchange.near"));
        assert!(in_reliable_liquidator_whitelist("specific.near"));

        // Test non-matches
        assert!(!in_reliable_liquidator_whitelist("alice.near"));
        assert!(!in_reliable_liquidator_whitelist("liquidator.near")); // Should not match without prefix
        assert!(!in_reliable_liquidator_whitelist("alice.liquidator.testnet"));
        assert!(!in_reliable_liquidator_whitelist("alice.test.near"));
    }

    #[test]
    fn test_in_reliable_liquidator_whitelist_complex_patterns() {
        let mut test_env = init_unit_env();

        // Add mixed patterns
        test_env.contract.append_reliable_liquidator_whitelist(vec![
            "exact.near".to_string(),
            "*.protocol.near".to_string(),
            "*.subdomain.protocol.near".to_string()
        ]);

        // Test various patterns
        assert!(in_reliable_liquidator_whitelist("exact.near"));
        assert!(in_reliable_liquidator_whitelist("alice.protocol.near"));
        assert!(in_reliable_liquidator_whitelist("bob.subdomain.protocol.near"));
        assert!(in_reliable_liquidator_whitelist("test.subdomain.protocol.near"));

        // Test edge cases
        assert!(!in_reliable_liquidator_whitelist("protocol.near")); // No prefix for wildcard
        assert!(!in_reliable_liquidator_whitelist("someother.near")); // Different exact match
        assert!(!in_reliable_liquidator_whitelist("alice.protocol.testnet")); // Wrong suffix
    }

    #[test]
    fn test_reliable_liquidator_context_behavior() {
        let mut test_env = init_unit_env();

        // Add a reliable liquidator to whitelist
        test_env.contract.append_reliable_liquidator_whitelist(vec!["reliable.liquidator.near".to_string()]);

        // Test that context starts as false
        assert!(!test_env.contract.is_reliable_liquidator_context);

        // Verify the liquidator is in whitelist
        assert!(in_reliable_liquidator_whitelist("reliable.liquidator.near"));
        assert!(!in_reliable_liquidator_whitelist("regular.user.near"));
    }

    #[test]
    fn test_multiple_operations() {
        let mut test_env = init_unit_env();

        // Add multiple liquidators
        test_env.contract.append_reliable_liquidator_whitelist(vec![
            "alice.near".to_string(),
            "*.liquidator.near".to_string()
        ]);

        // Verify initial state
        assert!(in_reliable_liquidator_whitelist("alice.near"));
        assert!(in_reliable_liquidator_whitelist("bob.liquidator.near"));

        // Add more liquidators
        test_env.contract.append_reliable_liquidator_whitelist(vec![
            "charlie.near".to_string(),
            "*.exchange.near".to_string()
        ]);

        // Verify all are present
        let whitelist = test_env.contract.get_reliable_liquidator_whitelist();
        assert_eq!(whitelist.len(), 4);
        assert!(in_reliable_liquidator_whitelist("alice.near"));
        assert!(in_reliable_liquidator_whitelist("charlie.near"));
        assert!(in_reliable_liquidator_whitelist("test.liquidator.near"));
        assert!(in_reliable_liquidator_whitelist("test.exchange.near"));

        // Remove some liquidators
        test_env.contract.remove_reliable_liquidator_whitelist(vec![
            "alice.near".to_string(),
            "*.exchange.near".to_string()
        ]);

        // Verify final state
        let final_whitelist = test_env.contract.get_reliable_liquidator_whitelist();
        assert_eq!(final_whitelist.len(), 2);
        assert!(!in_reliable_liquidator_whitelist("alice.near"));
        assert!(in_reliable_liquidator_whitelist("charlie.near"));
        assert!(in_reliable_liquidator_whitelist("test.liquidator.near"));
        assert!(!in_reliable_liquidator_whitelist("test.exchange.near"));
    }
}