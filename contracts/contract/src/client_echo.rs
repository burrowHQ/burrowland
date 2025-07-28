use crate::*;

#[near_bindgen]
impl Contract {
    #[payable]
    pub fn append_client_echo_sender_whitelist(&mut self, sender_list: Vec<String>) {
        assert_one_yocto();
        self.assert_owner();
        let mut sender_whitelist: UnorderedSet<String> =
            if env::storage_has_key(CLIENT_ECHO_SENDER_WHITELIST.as_bytes()) {
                internal_get_client_echo_sender_whitelist()
            } else {
                UnorderedSet::new(CLIENT_ECHO_SENDER_WHITELIST.as_bytes())
            };
        for sender in sender_list {
            let is_success = sender_whitelist.insert(&sender);
            require!(is_success, format!("exist sender: {}", sender));
        }
        env::storage_write(
            CLIENT_ECHO_SENDER_WHITELIST.as_bytes(),
            &sender_whitelist.try_to_vec().unwrap(),
        );
    }

    #[payable]
    pub fn remove_client_echo_sender_whitelist(&mut self, sender_list: Vec<String>) {
        assert_one_yocto();
        self.assert_owner();
        let mut sender_whitelist = internal_get_client_echo_sender_whitelist();
        for sender in sender_list {
            let is_success = sender_whitelist.remove(&sender);
            require!(is_success, format!("sender {} not exist", sender));
        }
        env::storage_write(
            CLIENT_ECHO_SENDER_WHITELIST.as_bytes(),
            &sender_whitelist.try_to_vec().unwrap(),
        );
    }

    pub fn get_client_echo_sender_whitelist(&self) -> Vec<String> {
        let sender_whitelist = internal_get_client_echo_sender_whitelist();
        sender_whitelist.iter().collect()
    }
}

pub fn internal_get_client_echo_sender_whitelist() -> UnorderedSet<String> {
    let content =
        env::storage_read(CLIENT_ECHO_SENDER_WHITELIST.as_bytes()).expect("Empty storage");
    UnorderedSet::try_from_slice(&content)
        .expect("deserialize client echo sender whitelist failed.")
}

pub fn in_client_echo_sender_whitelist(sender_id: &str) -> bool {
    let sender_whitelist = internal_get_client_echo_sender_whitelist();
    let res = sender_whitelist.iter().any(|item| {
        if let Some(suffix) = item.strip_prefix("*.") {
            sender_id.ends_with(&format!(".{}", suffix))
        } else {
            sender_id == item
        }
    });
    res
}
