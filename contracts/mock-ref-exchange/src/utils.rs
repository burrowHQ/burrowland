use std::collections::{HashSet, HashMap};

use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::LookupMap;
use near_sdk::json_types::U128;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{ext_contract, AccountId, Balance, Gas};
use uint::construct_uint;
use crate::errors::*;

/// Attach no deposit.
pub const NO_DEPOSIT: u128 = 0;

/// 10T gas for basic operation
pub const GAS_FOR_BASIC_OP: Gas = Gas(10_000_000_000_000);

/// hotfix_insuffient_gas_for_mft_resolve_transfer.
pub const GAS_FOR_RESOLVE_TRANSFER: Gas = Gas(20_000_000_000_000);

pub const GAS_FOR_FT_TRANSFER_CALL: Gas = Gas(25_000_000_000_000 + GAS_FOR_RESOLVE_TRANSFER.0);

/// Amount of gas for fungible token transfers, increased to 20T to support AS token contracts.
pub const GAS_FOR_FT_TRANSFER: Gas = Gas(20_000_000_000_000);

/// Fee divisor, allowing to provide fee in bps.
pub const FEE_DIVISOR: u32 = 10_000;
pub const MAX_ADMIN_FEE_BPS: u32 = 8_000;

/// Initial shares supply on deposit of liquidity.
pub const INIT_SHARES_SUPPLY: u128 = 1_000_000_000_000_000_000_000_000;

construct_uint! {
    /// 256-bit unsigned integer.
    pub struct U256(4);
}

construct_uint! {
    /// 384-bit unsigned integer.
    pub struct U384(6);
}

/// Volume of swap on the given token.
#[derive(Clone, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct SwapVolume {
    pub input: U128,
    pub output: U128,
}

impl Default for SwapVolume {
    fn default() -> Self {
        Self {
            input: U128(0),
            output: U128(0),
        }
    }
}

/// Adds given value to item stored in the given key in the LookupMap collection.
pub fn add_to_collection(c: &mut LookupMap<AccountId, Balance>, key: &AccountId, value: Balance) {
    let prev_value = c.get(key).unwrap_or(0);
    c.insert(key, &(prev_value + value));
}

/// Checks if there are any duplicates in the given list of tokens.
pub fn check_token_duplicates(tokens: &[AccountId]) {
    let token_set: HashSet<_> = tokens.iter().map(|a| a.as_ref()).collect();
    assert_eq!(token_set.len(), tokens.len(), "{}", ERR92_TOKEN_DUPLICATES);
}

/// Newton's method of integer square root.
pub fn integer_sqrt(value: U256) -> U256 {
    let mut guess: U256 = (value + U256::one()) >> 1;
    let mut res = value;
    while guess < res {
        res = guess;
        guess = (value / guess + guess) >> 1;
    }
    res
}

pub fn u128_ratio(a: u128, num: u128, denom: u128) -> u128 {
    (U256::from(a) * U256::from(num) / U256::from(denom)).as_u128()
}

pub struct TokenCache(pub HashMap<AccountId, u128>);

impl TokenCache {
    pub fn new() -> Self {
        TokenCache(HashMap::new())
    }

    pub fn add(&mut self, token_id: &AccountId, amount: u128) {
        self.0.entry(token_id.clone()).and_modify(|v| *v += amount).or_insert(amount);
    }

    pub fn sub(&mut self, token_id: &AccountId, amount: u128) {
        if amount != 0 {
            if let Some(prev) = self.0.remove(token_id) {
                assert!(amount <= prev, "{}", ERR22_NOT_ENOUGH_TOKENS);
                let remain = prev - amount;
                if remain > 0 {
                    self.0.insert(token_id.clone(), remain);
                }
            } else {
                panic!("{}", ERR22_NOT_ENOUGH_TOKENS);
            }
        }
    }
}

impl From<TokenCache> for HashMap<AccountId, U128> {
    fn from(v: TokenCache) -> Self {
        v.0.into_iter().map(|(k, v)| (k, U128(v))).collect()
    }
}

#[allow(unused)]
#[ext_contract(ext_fungible_token)]
pub trait FungibleToken {
    fn ft_transfer(&mut self, receiver_id: AccountId, amount: U128, memo: Option<String>);
    fn ft_transfer_call(&mut self, receiver_id: AccountId, amount: U128, memo: Option<String>, msg: String);
}

pub fn nano_to_sec(nano: u64) -> u32 {
    (nano / 10u64.pow(9)) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sqrt() {
        assert_eq!(integer_sqrt(U256::from(0)), 0.into());
        assert_eq!(integer_sqrt(U256::from(4)), 2.into());
        assert_eq!(
            integer_sqrt(U256::from(1_516_156_330_329u128)),
            U256::from(1_231_323)
        );
    }
}
