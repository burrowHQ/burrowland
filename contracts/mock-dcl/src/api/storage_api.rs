use crate::*;
use near_contract_standards::storage_management::{
    StorageBalance, StorageBalanceBounds, StorageManagement,
};

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(test, derive(Debug))]
pub struct UserStorageDetail {
    pub sponsor_id: AccountId,
    pub locked_near: U128,
    pub storage_for_asset: U128,
    pub slot_price: U128,
    pub max_slots: u32,
    pub cur_order_slots: u32,
    pub cur_liquidity_slots: u32,
}

#[near_bindgen]
impl Contract {
    pub fn get_user_storage_detail(&self, user_id: AccountId) -> Option<UserStorageDetail> {
        if let Some(user) = self.internal_get_user(&user_id) {
            let global_config = self.internal_get_global_config();
            let storage_price_per_slot = global_config.storage_price_per_slot;
            let storage_for_asset = global_config.storage_for_asset;
            Some(UserStorageDetail{
                sponsor_id: user.sponsor_id,
                locked_near: U128(user.locked_near_for_storage),
                storage_for_asset: U128(storage_for_asset),
                slot_price: U128(storage_price_per_slot),
                max_slots: ((user.locked_near_for_storage - storage_for_asset) / storage_price_per_slot) as u32,
                cur_order_slots: user.order_keys.len() as u32,
                cur_liquidity_slots: user.liquidity_keys.len() as u32,
            })
        } else {
            None
        }
    }
}

/// Implements users storage management for the pool.
#[near_bindgen]
impl StorageManagement for Contract {
    #[payable]
    fn storage_deposit(
        &mut self,
        account_id: Option<AccountId>,
        registration_only: Option<bool>,
    ) -> StorageBalance {
        self.assert_contract_running();

        let amount = env::attached_deposit();
        let account_id = account_id.unwrap_or_else(env::predecessor_account_id);
        let caller_id = env::predecessor_account_id();
        let already_registered = self.data().users.contains_key(&account_id);
        let registration_only = registration_only.unwrap_or_default();
        if amount < STORAGE_BALANCE_MIN_BOUND && !already_registered {
            env::panic_str(E102_INSUFFICIENT_STORAGE);
        }

        if already_registered {
            if amount > 0 {
                if registration_only {
                    Promise::new(env::predecessor_account_id()).transfer(amount);
                } else {
                    let mut user = self.internal_unwrap_user(&account_id);
                    if caller_id == account_id && account_id != user.sponsor_id {
                        require!(amount >= user.locked_near_for_storage);
                        if user.sponsor_id != env::current_account_id() {
                            Promise::new(user.sponsor_id).transfer(user.locked_near_for_storage);
                        }
                        user.sponsor_id = caller_id;
                        user.locked_near_for_storage = amount;
                    } else {
                        user.locked_near_for_storage += amount;
                    }
                    Event::AppendUserStorage {
                        operator: &env::predecessor_account_id(),
                        user: &account_id,
                        amount: &U128(amount),
                    }.emit();
                    self.internal_set_user(&account_id, user);
                }
            }
        } else {
            let actual_amount = 
            if registration_only {
                self.internal_set_user(&account_id, User::new(&account_id, &caller_id, STORAGE_BALANCE_MIN_BOUND));
                let refund = amount - STORAGE_BALANCE_MIN_BOUND;
                if refund > 0 {
                    Promise::new(env::predecessor_account_id()).transfer(refund);
                }
                STORAGE_BALANCE_MIN_BOUND
            } else {
                self.internal_set_user(&account_id, User::new(&account_id, &caller_id, amount));
                amount
            };
            self.data_mut().user_count += 1;
            Event::InitUserStorage {
                operator: &env::predecessor_account_id(),
                user: &account_id,
                amount: &U128(actual_amount),
            }.emit();
        }
        self.storage_balance_of(account_id).unwrap()
    }

    #[payable]
    fn storage_withdraw(
        &mut self,
        amount: Option<U128>,
    ) -> StorageBalance {
        assert_one_yocto();
        self.assert_contract_running();

        let account_id = env::predecessor_account_id();
        let mut user = self.internal_unwrap_user(&account_id);
        let receiver_id = user.sponsor_id.clone();
        let global_config = self.internal_get_global_config();
        let storage_price_per_slot = global_config.storage_price_per_slot;
        let available_slots = user.get_available_slots(storage_price_per_slot, global_config.storage_for_asset);

        let max_amount = available_slots as u128 * storage_price_per_slot;
        let withdraw_amount = if let Some(a) = amount {
            if a.0 > max_amount { max_amount } else { a.0 }
        } else {
            max_amount
        };

        user.locked_near_for_storage -= withdraw_amount;
        
        Event::WithdrawUserStorage {
            operator: &account_id,
            receiver: &receiver_id,
            amount: &U128(withdraw_amount),
            remain: &U128(user.locked_near_for_storage),
        }.emit();

        self.internal_set_user(&account_id, user);

        if withdraw_amount > 0 && receiver_id != env::current_account_id() {
            Promise::new(receiver_id).transfer(withdraw_amount);
        }

        self.storage_balance_of(account_id).unwrap()
    }

    #[payable]
    fn storage_unregister(&mut self, #[allow(unused_variables)] force: Option<bool>) -> bool {
        assert_one_yocto();
        self.assert_contract_running();

        // force option is useless, leave it for compatible consideration.
        // User can NOT unregister if there is still have liquidity, order and asset remain!
        let account_id = env::predecessor_account_id();
        if let Some(user) = self.internal_get_user(&account_id) {
            require!(user.is_empty(), E103_STILL_HAS_REWARD);
            self.data_mut().users.remove(&account_id);
            self.data_mut().user_count -= 1;
            if user.sponsor_id != env::current_account_id() {
                Promise::new(user.sponsor_id.clone()).transfer(user.locked_near_for_storage);
            }
            Event::UnregisterUserStorage {
                operator: &account_id,
                sponsor: &user.sponsor_id,
                amount: &U128(user.locked_near_for_storage),
            }.emit();
            true
        } else {
            false
        }
    }

    fn storage_balance_bounds(&self) -> StorageBalanceBounds {
        StorageBalanceBounds {
            min: U128(STORAGE_BALANCE_MIN_BOUND),
            max: None,
        }
    }

    fn storage_balance_of(&self, account_id: AccountId) -> Option<StorageBalance> {
        if let Some(user) = self.internal_get_user(&account_id) {
            let global_config = self.internal_get_global_config();
            let storage_price_per_slot = global_config.storage_price_per_slot;
            let available_slots = user.get_available_slots(storage_price_per_slot, global_config.storage_for_asset);
            let available_amount = if available_slots > 0 { 
                available_slots as u128 * storage_price_per_slot 
            } else { 
                0u128 
            };
            Some(StorageBalance {
                total: U128(user.locked_near_for_storage),
                available: U128(available_amount),
            })
        } else {
            None
        }
    }
}
