use crate::*;

pub fn read_protocol_debts_from_storage() -> HashMap<TokenId, u128> {
    if let Some(content) = env::storage_read(PROTOCOL_DEBTS_KEY.as_bytes()) {
        HashMap::try_from_slice(&content).expect("deserialize protocol debts failed.")
    } else {
        HashMap::new()
    }
}

pub fn write_protocol_debts_to_storage(data: HashMap<TokenId, u128>) {
    env::storage_write(PROTOCOL_DEBTS_KEY.as_bytes(), &data.try_to_vec().unwrap());
}

#[near_bindgen]
impl Contract {
    pub fn list_protocol_debts(&self, token_ids: Vec<AccountId>) -> HashMap<AccountId, Option<U128>> {
        let protocol_debts = read_protocol_debts_from_storage();
        token_ids.into_iter().map(|token_id| (token_id.clone(), protocol_debts.get(&token_id).map(|v| U128(*v)))).collect()
    }

    pub fn get_all_protocol_debts(&self) -> HashMap<AccountId, U128> {
        let protocol_debts = read_protocol_debts_from_storage();
        protocol_debts.into_iter().map(|(k, v)| (k, U128(v))).collect()
    }
}
