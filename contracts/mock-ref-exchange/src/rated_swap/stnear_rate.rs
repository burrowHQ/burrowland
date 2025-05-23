use super::{rate::RateTrait, PRECISION};
use crate::errors::ERR126_FAILED_TO_PARSE_RESULT;
use crate::utils::{GAS_FOR_BASIC_OP, NO_DEPOSIT};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::{
    env, ext_contract, json_types::U128, serde_json::from_slice, AccountId, Balance, Promise,
};

// default expire time is 24 hours
const EXPIRE_TS: u64 = 24 * 3600 * 10u64.pow(9);

#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct StnearRate {
    /// *
    pub stored_rates: Balance,
    /// *
    pub rates_updated_at: u64,
    /// *
    pub contract_id: AccountId,
}

#[allow(unused)]
#[ext_contract(ext_metapool)]
pub trait ExtMetapool {
    //https://github.com/Narwallets/meta-pool/blob/40636d9004d28dc9cb214b9703692061c93f613c/metapool/src/owner.rs#L254
    fn get_st_near_price(&self) -> U128;
}

impl RateTrait for StnearRate {
    fn are_actual(&self) -> bool {
        env::block_timestamp() <= self.rates_updated_at + EXPIRE_TS  
    }
    fn get(&self) -> Balance {
        self.stored_rates
    }
    fn last_update_ts(&self) -> u64 {
        self.rates_updated_at
    }
    fn async_update(&self) -> Promise {
        ext_metapool::ext(self.contract_id.clone())
            .with_attached_deposit(NO_DEPOSIT)
            .with_static_gas(GAS_FOR_BASIC_OP)
            .get_st_near_price()
    }
    fn set(&mut self, cross_call_result: &Vec<u8>) -> u128 {
        if let Ok(U128(price)) = from_slice::<U128>(cross_call_result) {
            self.stored_rates = price;
            self.rates_updated_at = env::block_timestamp();
            price
        } else {
            env::panic_str(ERR126_FAILED_TO_PARSE_RESULT);
        }
    }
}

impl StnearRate {
    pub fn new(contract_id: AccountId) -> Self {
        Self {
            stored_rates: PRECISION, 
            rates_updated_at: 0,
            contract_id,
        }
    }
}

