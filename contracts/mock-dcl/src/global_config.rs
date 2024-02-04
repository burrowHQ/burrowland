use crate::*;

#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct GlobalConfig {
    pub owner_id: AccountId,
    pub next_owner_id: Option<AccountId>,
    pub next_owner_accept_deadline: Option<u64>,
    pub wnear_id: AccountId,
    pub storage_price_per_slot: Balance,
    pub storage_for_asset: Balance,
}