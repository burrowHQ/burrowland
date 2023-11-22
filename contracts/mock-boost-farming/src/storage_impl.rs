use crate::*;
use near_contract_standards::storage_management::{
    StorageBalance, StorageBalanceBounds, StorageManagement,
};

// use std::convert::TryInto;

// use near_sdk::json_types::{ValidAccountId, U128};
// use near_sdk::{assert_one_yocto, env, near_bindgen, Promise};




/// Implements users storage management for the pool.
#[near_bindgen]
impl StorageManagement for Contract {
    #[allow(unused_variables)]
    #[payable]
    fn storage_deposit(
        &mut self,
        account_id: Option<AccountId>,
        registration_only: Option<bool>,
    ) -> StorageBalance {
        require!(self.data().state == RunningState::Running, E004_CONTRACT_PAUSED);

        let amount = env::attached_deposit();
        let account_id = account_id.unwrap_or_else(|| env::predecessor_account_id());
        let sponsor_id = env::predecessor_account_id();
        let already_registered = self.data().farmers.contains_key(&account_id);
        if amount < STORAGE_BALANCE_MIN_BOUND && !already_registered {
            env::panic_str(E102_INSUFFICIENT_STORAGE);
        }

        if already_registered {
            if amount > 0 {
                Promise::new(env::predecessor_account_id()).transfer(amount);
            }
        } else {            
            self.data_mut().farmers.insert(&account_id, &Farmer::new(&account_id, &sponsor_id).into());
            self.data_mut().farmer_count += 1;
            let refund = amount - STORAGE_BALANCE_MIN_BOUND;
            if refund > 0 {
                Promise::new(env::predecessor_account_id()).transfer(refund);
            }
        }
        self.storage_balance_of(account_id).unwrap()
    }

    #[allow(unused_variables)]
    #[payable]
    fn storage_withdraw(&mut self, amount: Option<U128>) -> StorageBalance {
        assert_one_yocto();
        require!(self.data().state == RunningState::Running, E004_CONTRACT_PAUSED);
        env::panic_str(E006_NOT_IMPLEMENTED);
    }

    #[allow(unused_variables)]
    #[payable]
    fn storage_unregister(&mut self, force: Option<bool>) -> bool {
        assert_one_yocto();
        require!(self.data().state == RunningState::Running, E004_CONTRACT_PAUSED);

        // force option is useless, leave it for compatible consideration.
        // User should withdraw all his rewards and seeds token before unregister!

        let account_id = env::predecessor_account_id();
        if let Some(farmer) = self.internal_get_farmer(&account_id) {
            
            require!(
                farmer.rewards.is_empty(),
                E103_STILL_HAS_REWARD
            );
            require!(
                farmer.seeds.is_empty() && farmer.vseeds.is_empty(),
                E104_STILL_HAS_SEED
            );

            self.data_mut().farmers.remove(&account_id);
            self.data_mut().farmer_count -= 1;
            Promise::new(farmer.sponsor_id.clone()).transfer(STORAGE_BALANCE_MIN_BOUND);
            true
        } else {
            false
        }
    }

    fn storage_balance_bounds(&self) -> StorageBalanceBounds {
        StorageBalanceBounds {
            min: U128(STORAGE_BALANCE_MIN_BOUND),
            max: Some(U128(STORAGE_BALANCE_MIN_BOUND)),
        }
    }

    fn storage_balance_of(&self, account_id: AccountId) -> Option<StorageBalance> {
        if self.data().farmers.contains_key(&account_id) {
            Some(StorageBalance {
                total: U128(STORAGE_BALANCE_MIN_BOUND),
                available: U128(0),
            })
        }else{
            None
        }
    }
}