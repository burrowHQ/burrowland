use near_sdk::{AccountId, Gas, ext_contract, Promise};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::serde::{Serialize, Deserialize};
use near_sdk::json_types::U128;
use uint::construct_uint;


pub const TGAS: u64 = 1_000_000_000_000;
pub const GAS_FOR_ASSET_TRANSFER: Gas = Gas(20 * TGAS);
pub const GAS_FOR_ASSET_TRANSFER_CALL: Gas = Gas(45 * TGAS);
pub const GAS_FOR_RESOLVE_ASSET_TRANSFER: Gas = Gas(10 * TGAS);
pub const GAS_FOR_NEAR_WITHDRAW: Gas = Gas(20 * TGAS);
pub const GAS_FOR_RESOLVE_NEAR_WITHDRAW: Gas = Gas(10 * TGAS);

construct_uint! {
    #[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
    #[serde(crate = "near_sdk::serde")]
	pub struct U256(4);
}

construct_uint! {
    #[derive(BorshDeserialize, BorshSerialize)]
	pub struct U512(8);
}

pub mod u256_dec_format {
    use near_sdk::serde::de;
    use near_sdk::serde::{Deserialize, Deserializer, Serializer};
    use super::U256;

    pub fn serialize<S>(num: &U256, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&num.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<U256, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(de::Error::custom)
    }
}

pub mod u128_dec_format {
    use near_sdk::serde::de;
    use near_sdk::serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(num: &u128, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&num.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u128, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(de::Error::custom)
    }
}

pub mod u64_dec_format {
    use near_sdk::serde::de;
    use near_sdk::serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(num: &u64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&num.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u64, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(de::Error::custom)
    }
}

#[ext_contract(ext_fungible_token)]
pub trait FungibleToken {
    fn ft_transfer(&mut self, receiver_id: AccountId, amount: U128, memo: Option<String>);
    fn ft_transfer_call(&mut self, receiver_id: AccountId, amount: U128, memo: Option<String>, msg: String);
}

#[ext_contract(ext_wrap_near)]
pub trait WrapNear {
    fn near_withdraw(&mut self, amount: U128) -> Promise;
}