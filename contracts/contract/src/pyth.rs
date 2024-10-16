use near_sdk::{serde_json, require, PromiseResult};

use crate::*;

/// set static gas consumption for quering pyth
pub const GAS_FOR_GET_PRICE: Gas = Gas(5 * Gas::ONE_TERA.0);
/// set static gas consumption for generating promises
pub const GAS_FOR_BUFFER: Gas = Gas(40 * Gas::ONE_TERA.0);
/// set MAX PROMISE numbers in case out of gas
pub const GET_PRICE_PROMISES_LIMIT: usize = 20;

pub const FLAG_PARTITION: &str = "@";

pub const ONE_NEAR: u128 = 10u128.pow(24);

#[ext_contract(ext_pyth)]
pub trait Pyth {
    fn get_price_no_older_than(&self, price_id: PriceIdentifier, age: u64) -> Option<Price>;
}

#[derive(BorshDeserialize, BorshSerialize, Deserialize, Serialize, Clone)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
#[serde(crate = "near_sdk::serde")]
pub struct TokenPythInfo {
    pub decimals: u8,
    pub fraction_digits: u8,
    pub price_identifier: PriceIdentifier,
    pub extra_call: Option<String>,
    pub default_price: Option<Price>
}


#[derive(BorshDeserialize, BorshSerialize, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
#[serde(crate = "near_sdk::serde")]
pub struct PythPrice {
    pub price: I64,
    /// Confidence interval around the price
    pub conf: U64,
    /// The exponent
    pub expo: i32,
    /// Unix timestamp of when this price was computed
    pub publish_time: i64,
}

#[derive(BorshDeserialize, BorshSerialize, PartialEq, Eq, Hash, Clone)]
#[repr(transparent)]
pub struct PriceIdentifier(pub [u8; 32]);

impl<'de> near_sdk::serde::Deserialize<'de> for PriceIdentifier {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: near_sdk::serde::Deserializer<'de>,
    {
        /// A visitor that deserializes a hex string into a 32 byte array.
        struct IdentifierVisitor;

        impl<'de> near_sdk::serde::de::Visitor<'de> for IdentifierVisitor {
            /// Target type for either a hex string or a 32 byte array.
            type Value = [u8; 32];

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a hex string")
            }

            // When given a string, attempt a standard hex decode.
            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: near_sdk::serde::de::Error,
            {
                if value.len() != 64 {
                    return Err(E::custom(format!(
                        "expected a 64 character hex string, got {}",
                        value.len()
                    )));
                }
                let mut bytes = [0u8; 32];
                hex::decode_to_slice(value, &mut bytes).map_err(E::custom)?;
                Ok(bytes)
            }
        }

        deserializer
            .deserialize_any(IdentifierVisitor)
            .map(PriceIdentifier)
    }
}

impl near_sdk::serde::Serialize for PriceIdentifier {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: near_sdk::serde::Serializer,
    {
        serializer.serialize_str(&hex::encode(&self.0))
    }
}

impl std::string::ToString for PriceIdentifier {
    fn to_string(&self) -> String {
        hex::encode(&self.0)
    }
}

impl std::fmt::Debug for PriceIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(&self.0))
    }
}

impl Contract {

    pub fn get_pyth_info_by_token(&self, token_id: &TokenId) -> &TokenPythInfo {
        self.token_pyth_info.get(&token_id).expect(format!("Missing {} token pyth info", token_id).as_str())
    }

    pub fn token_involved_promises(&self, pyth_oracle_account_id: &AccountId, promise_token_ids: &Vec<AccountId>) -> (Vec<String>, Vec<Promise>) {
        let mut promises_flags = vec![];
        let mut promises = vec![];
        let config = self.internal_config();
        for token_id in promise_token_ids.iter() {
            let token_pyth_info = self.get_pyth_info_by_token(&token_id);
            let price_identifier = token_pyth_info.price_identifier.clone();
            if !promises_flags.contains(&price_identifier.to_string()) {
                promises_flags.push(price_identifier.to_string());
                promises.push(ext_pyth::ext(pyth_oracle_account_id.clone())
                    .with_static_gas(GAS_FOR_GET_PRICE)
                    .get_price_no_older_than(price_identifier, config.pyth_price_valid_duration_sec as u64));
            }
            
            if let Some(extra_call) = token_pyth_info.extra_call.as_ref() {
                if !promises_flags.contains(extra_call) {
                    promises_flags.push(extra_call.clone());
                    promises.push(Promise::new(token_id.clone())
                        .function_call(extra_call.clone(), vec![], 0, GAS_FOR_GET_PRICE));
                }
            }
        }
        (promises_flags, promises)
    }

    pub fn prepare_promise_tokens(&self, involved_tokens: &Vec<TokenId>) -> (Vec<AccountId>, HashMap<TokenId, Price>) {
        let mut default_prices: HashMap<TokenId, Price> = HashMap::new();
        let mut promise_token_ids = vec![];
        for token_id in involved_tokens {
            let token_pyth_info = self.get_pyth_info_by_token(token_id);
            if token_pyth_info.default_price.is_some() {
                default_prices.insert(token_id.clone(), token_pyth_info.default_price.unwrap());
            } else {
                promise_token_ids.push(token_id.clone());
            }
        }
        (promise_token_ids, default_prices)
    }

    pub fn generate_flags_and_promise(&self, promise_token_ids: &Vec<AccountId>) -> (Vec<String>, Promise) {
        let (all_promise_flags, mut promises) = self.token_involved_promises(&self.internal_config().pyth_oracle_account_id, promise_token_ids);
        assert!(all_promise_flags.len() <= promises.len());
        assert!(all_promise_flags.len() <= GET_PRICE_PROMISES_LIMIT, "Too many promises to get prices");
        let mut promise = promises.remove(0);
        for p in promises.into_iter() {
            promise = promise.and(p);
        }
        (all_promise_flags, promise)
    }

    pub fn generate_all_prices(&mut self, involved_tokens: Vec<TokenId>, all_promise_flags: Vec<String>, default_prices: HashMap<TokenId, Price>) -> Prices {
        let config = self.internal_config();
        let mut all_prices = Prices::new();
        let mut all_cross_call_results = HashMap::new();
        for (index, flag) in all_promise_flags.into_iter().enumerate() {
            match env::promise_result(index as u64) {
                PromiseResult::Successful(cross_call_result) => {
                    all_cross_call_results.insert(flag, cross_call_result);
                },
                _ => env::panic_str(format!("{} cross call failed!", flag).as_str()),
            }
        }
        for token_id in involved_tokens {
            if let Some(token_price) = default_prices.get(&token_id) {
                all_prices.prices.insert(token_id, *token_price);
            } else {
                let token_pyth_info = self.get_pyth_info_by_token(&token_id);
                let price_identifier = token_pyth_info.price_identifier.to_string();
                let pyth_price_bytes = all_cross_call_results.get(&price_identifier).expect(format!("Missing {} price cross_call_result", price_identifier).as_str());
                let pyth_price = serde_json::from_slice::<Option<PythPrice>>(pyth_price_bytes)
                    .expect(format!("{} cross_call_result not Option<PythPrice>", price_identifier).as_str())
                    .expect(format!("Missing {} price", price_identifier).as_str());
                assert!(pyth_price.publish_time > 0 && sec_to_nano(pyth_price.publish_time as u32 + config.pyth_price_valid_duration_sec) >= env::block_timestamp(), "Pyth {} publish_time is too stale", price_identifier);
                let mut token_price = pyth_price_to_price_oracle_price(self.get_pyth_info_by_token(&token_id), &pyth_price);
                if let Some(extra_call) = token_pyth_info.extra_call.as_ref() {
                    let extra_call_bytes = all_cross_call_results.get(extra_call).expect(format!("Missing {} extra_call cross_call_result", price_identifier).as_str());
                    let extra_call_amount = serde_json::from_slice::<U128>(&extra_call_bytes).expect(format!("{} extra_call not U128", extra_call).as_str()).0;
                    self.update_staking_token_price_record(&token_id, extra_call_amount, format!("The {} {} return value is out of the valid range", token_id, extra_call));
                    token_price.multiplier = u128_ratio(token_price.multiplier, extra_call_amount, ONE_NEAR);
                }
                all_prices.prices.insert(token_id, token_price);
            }
        }
        all_prices
    }

    pub fn update_staking_token_price_record(&mut self, token_id: &TokenId, price: u128, err_msg: String) {
        if let Some(max_change_rate) = self.internal_unwrap_asset(token_id).config.max_change_rate {
            if let Some(&U128(last_staking_token_price)) = self.last_staking_token_prices.get(token_id) {
                assert!(last_staking_token_price <= price
                    && last_staking_token_price + u128_ratio(last_staking_token_price, max_change_rate as _, MAX_RATIO as _) >= price, "{}", err_msg);
            }
        }
        self.last_staking_token_prices.insert(token_id.clone(), price.into());
    }
}

pub fn pyth_price_to_price_oracle_price(token_info: &TokenPythInfo, pyth_price: &PythPrice) -> Price {
    require!(pyth_price.price.0 > 0, "Invalid Pyth Price");
    let mut multiplier = BigDecimal::from(pyth_price.price.0 as Balance);
    if pyth_price.expo > 0 {
        multiplier = multiplier * BigDecimal::from(10u128.pow(pyth_price.expo.abs() as u32));
    } else {
        multiplier = multiplier / BigDecimal::from(10u128.pow(pyth_price.expo.abs() as u32));
    }
    
    Price {
        multiplier: (multiplier * BigDecimal::from(10u128.pow(token_info.fraction_digits as u32))).round_down_u128(),
        decimals: token_info.decimals + token_info.fraction_digits
    }
}
