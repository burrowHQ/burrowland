use crate::*;
use near_sdk::json_types::U64;

#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(test, derive(Deserialize, Clone, Debug))]
pub struct Metadata {
    pub version: String,
    pub owner_id: AccountId,
    pub farming_contract_id: AccountId,
    pub next_owner_id: Option<AccountId>,
    pub next_owner_accept_deadline: Option<u64>,
    pub wnear_id: AccountId,
    pub state: RunningState,
    pub operators: Vec<AccountId>,
    pub protocol_fee_rate: u32,  // in BPs, 5000 means 50%
    pub pool_count: U64,
    pub user_count: U64,
}

#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(test, derive(Deserialize, Clone))]
pub struct StorageReport {
    pub storage: U64,
    pub locking_near: U128,
}

#[near_bindgen]
impl Contract {
    //******** Contract Concern */
    pub fn get_metadata(&self) -> Metadata {
        let global_config = self.internal_get_global_config();
        Metadata {
            version: env!("CARGO_PKG_VERSION").to_string(),
            owner_id: global_config.owner_id.clone(),
            farming_contract_id: self.data().farming_contract_id.clone(),
            next_owner_id: global_config.next_owner_id.clone(),
            next_owner_accept_deadline: global_config.next_owner_accept_deadline.clone(),
            wnear_id: global_config.wnear_id.clone(),
            state: self.data().state.clone(),
            operators: self.data().operators.to_vec(),
            protocol_fee_rate: self.data().protocol_fee_rate,
            pool_count: self.data().pools.len().into(),
            user_count: self.data().user_count.into(),
        }
    }

    pub fn get_contract_storage_report(&self) -> StorageReport {
        let su = env::storage_usage();
        StorageReport {
            storage: U64(su),
            locking_near: U128(su as Balance * env::storage_byte_cost()),
        }
    }

    pub fn get_frozenlist_tokens(&self) -> Vec<AccountId> {
        self.data().frozenlist.to_vec()
    }

    pub fn list_mft_supply(&self, from_index: Option<u64>, limit: Option<u64>) -> HashMap<MftId, U128> {
        let keys = self.data().mft_supply.keys_as_vector();

        let from_index = from_index.unwrap_or(0);
        let limit = limit.unwrap_or(keys.len());

        (from_index..std::cmp::min(from_index + limit, keys.len()))
            .map(|index| {
                (
                    keys.get(index).unwrap(),
                    self.data().mft_supply.get(&keys.get(index).unwrap()).unwrap().into(),
                )
            })
            .collect()
    }

    pub fn list_vip_users(&self, from_index: Option<u64>, limit: Option<u64>) -> HashMap<AccountId, HashMap<PoolId, u32>> {
        let keys = self.data().vip_users.keys_as_vector();
        let from_index = from_index.unwrap_or(0);
        let limit = limit.unwrap_or(keys.len());

        (from_index..std::cmp::min(from_index + limit, keys.len()))
            .map(|index| {
                (
                    keys.get(index).unwrap(),
                    self.data().vip_users.get(&keys.get(index).unwrap()).unwrap().into(),
                )
            })
            .collect()
    }

    pub fn get_vip_user_discount(&self, user_id: AccountId) -> Option<HashMap<PoolId, u32>> {
        self.data().vip_users.get(&user_id)
    }
}