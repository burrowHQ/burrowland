use crate::*;

#[derive(BorshSerialize, BorshDeserialize, Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct User {
    /// A copy of an user ID. Saves one storage_read when iterating on users.
    pub user_id: AccountId,
    pub sponsor_id: AccountId,
    pub locked_near_for_storage: Balance,
    #[serde(skip_serializing)]
    pub liquidity_keys: UnorderedSet<LptId>,
    #[serde(skip_serializing)]
    pub order_keys: UnorderedMap<UserOrderKey, OrderId>,
    #[serde(skip_serializing)]
    pub history_orders: Vector<UserOrder>,
    #[serde(with = "u64_dec_format")]
    pub completed_order_count: u64,
    #[serde(skip_serializing)]
    pub assets: UnorderedMap<AccountId, Balance>,
    #[serde(skip_serializing)]
    pub mft_assets: UnorderedMap<MftId, Balance>,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub enum VUser {
    V0(UserV0),
    Current(User),
}

impl From<VUser> for User {
    fn from(v: VUser) -> Self {
        match v {
            VUser::V0(c) => c.into(),
            VUser::Current(c) => c,
        }
    }
}

impl From<User> for VUser {
    fn from(c: User) -> Self {
        VUser::Current(c)
    }
}

impl User {
    pub fn new(user_id: &AccountId, sponsor_id: &AccountId, locked_near_for_storage: Balance) -> Self {
        User {
            user_id: user_id.clone(),
            sponsor_id: sponsor_id.clone(),
            locked_near_for_storage,
            liquidity_keys: UnorderedSet::new(StorageKeys::UserLiquidityKey {
                account_id: user_id.clone(),
            }),
            order_keys: UnorderedMap::new(StorageKeys::UserOrderKey {
                account_id: user_id.clone(),
            }),
            history_orders: Vector::new(StorageKeys::UserOrderHistory {
                account_id: user_id.clone(),
            }),
            completed_order_count: 0_u64,
            assets: UnorderedMap::new(StorageKeys::UserAsset {
                account_id: user_id.clone(),
            }),
            mft_assets: UnorderedMap::new(StorageKeys::UserMftAsset {
                account_id: user_id.clone(),
            }),
        }
    }

    pub fn get_available_slots(&self, storage_price_per_slot: Balance, storage_for_asset: Balance) -> u64 {
        let max_slots = ((self.locked_near_for_storage - storage_for_asset) / storage_price_per_slot) as u64;
        let cur_slots = self.order_keys.len() + self.liquidity_keys.len();

        if max_slots > cur_slots { max_slots - cur_slots } else { 0u64 }
    }

    pub fn is_empty(&self) -> bool {
        self.assets.len() == 0 && self.mft_assets.len() == 0 && self.liquidity_keys.len() == 0 && self.order_keys.len() == 0
    }
}

impl Contract {
    pub fn internal_get_user(&self, user_id: &AccountId) -> Option<User> {
        self.data().users.get(user_id).map(|o| o.into())
    }

    pub fn internal_unwrap_user(&self, user_id: &AccountId) -> User {
        self.internal_get_user(user_id)
            .expect(E100_ACC_NOT_REGISTERED)
    }

    pub fn internal_set_user(&mut self, user_id: &AccountId, user: User) {
        self.data_mut().users.insert(user_id, &user.into());
    }
}
