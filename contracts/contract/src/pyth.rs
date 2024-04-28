use std::collections::HashSet;

use near_sdk::{serde_json, require, PromiseResult};

use crate::*;

pub const GAS_FOR_CALLBACK_EXECUTE_WITH_PYTH: Gas = Gas(200 * Gas::ONE_TERA.0);
pub const GAS_FOR_GET_PRICE: Gas = Gas(3 * Gas::ONE_TERA.0);
pub const GAS_FOR_BUFFER: Gas = Gas(30 * Gas::ONE_TERA.0);

pub const GET_PRICE_PROMISES_LIMIT: usize = 20;

pub const FLAG_PARTITION: &str = "@";

pub const ONE_NEAR: u128 = 10u128.pow(24);

#[ext_contract(ext_pyth)]
pub trait Pyth {
    fn get_price(&self, price_identifier: PriceIdentifier) -> Option<Price>;
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

#[near_bindgen]
impl Contract {

    /// Executes a given list actions on behalf of the predecessor account with pyth oracle price.
    /// - Requires one yoctoNEAR.
    #[payable]
    pub fn execute_with_pyth(&mut self, actions: Vec<Action>) {
        assert_one_yocto();
        let account_id = env::predecessor_account_id();
        let mut account = self.internal_unwrap_account(&account_id);
        self.internal_execute_with_pyth(&account_id, &mut account, actions);
        self.internal_set_account(&account_id, account);
    }

    #[private]
    pub fn callback_execute_with_pyth(&mut self, account_id: AccountId, involved_tokens: Vec<TokenId>, all_promise_flags: Vec<String>, actions: Vec<Action>, default_prices: HashMap<TokenId, Price>) {
        assert!(env::promise_results_count() == all_promise_flags.len() as u64, "Invalid promise count");
        let mut account = self.internal_unwrap_account(&account_id);
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
                    if let Some(max_change_rate) = self.internal_unwrap_asset(&token_id).config.max_change_rate {
                        if let Some(&U128(last_staking_token_price)) = self.last_staking_token_prices.get(&token_id) {
                            assert!(last_staking_token_price <= extra_call_amount
                                && last_staking_token_price + u128_ratio(last_staking_token_price, max_change_rate as _, MAX_RATIO as _) >= extra_call_amount, "{} {} Invaild", token_id, extra_call);
                        }
                    }
                    self.last_staking_token_prices.insert(token_id.clone(), extra_call_amount.into());
                    token_price.multiplier = u128_ratio(token_price.multiplier, extra_call_amount, ONE_NEAR);
                }
                all_prices.prices.insert(token_id, token_price);
            }
        }
        self.internal_execute(&account_id, &mut account, actions, all_prices);
        self.internal_set_account(&account_id, account);
    }
}

impl Contract {

    pub fn internal_execute_with_pyth(&mut self, account_id: &AccountId, account: &mut Account, actions: Vec<Action>) {
        let pyth_oracle_account_id = self.internal_config().pyth_oracle_account_id;
        let involved_tokens = self.involved_tokens(&account, &actions);
        if involved_tokens.len() > 0 {
            assert!(self.internal_config().enable_pyth_oracle, "Pyth oracle disabled");
            let mut default_prices: HashMap<TokenId, Price> = HashMap::new();
            let mut promise_token_ids = vec![];
            for token_id in involved_tokens.iter() {
                let token_pyth_info = self.get_pyth_info_by_token(&token_id);
                if token_pyth_info.default_price.is_some() {
                    default_prices.insert(token_id.clone(), token_pyth_info.default_price.unwrap());
                } else {
                    promise_token_ids.push(token_id.clone());
                }
            }
            if promise_token_ids.len() > 0 {
                let (all_promise_flags, mut promises) = self.token_involved_promises(&pyth_oracle_account_id, &promise_token_ids);
                assert!(all_promise_flags.len() <= promises.len());
                assert!(all_promise_flags.len() <= GET_PRICE_PROMISES_LIMIT, "Too many promises to get prices");
                let mut promise = promises.remove(0);
                for p in promises.into_iter() {
                    promise = promise.and(p);
                }

                let callback_gas = env::prepaid_gas() - env::used_gas() - (GAS_FOR_GET_PRICE) * all_promise_flags.len() as u64 - GAS_FOR_BUFFER;
                promise.then(
                    Self::ext(env::current_account_id())
                        .with_static_gas(callback_gas)
                        .callback_execute_with_pyth(account_id.clone(), involved_tokens, all_promise_flags, actions, default_prices)
                );
            } else {
                self.internal_execute(account_id, account, actions, Prices::from_prices(default_prices));
            }
        } else {
            self.internal_execute(account_id, account, actions, Prices::new());
        }
    }

    pub fn get_pyth_info_by_token(&self, token_id: &TokenId) -> &TokenPythInfo {
        self.token_pyth_info.get(&token_id).expect(format!("Missing {} token pyth info", token_id).as_str())
    }
    
    pub fn involved_tokens(&self, account: &Account, actions: &Vec<Action>) -> Vec<TokenId> {
        let mut positions = HashSet::new();
        let mut tokens = HashSet::new();
        actions.iter().for_each(|action|{
            match action {
                Action::IncreaseCollateral(asset_amount) => {
                    if account.positions.get(&REGULAR_POSITION.to_string()).is_none() && actions.iter().any(|a| matches!(a, Action::Borrow(_))) {
                        tokens.insert(asset_amount.token_id.clone());
                    }
                }
                Action::PositionIncreaseCollateral{ position, asset_amount: _ } => {
                    if account.positions.get(position).is_none() && actions.iter().any(|a| matches!(a, Action::PositionBorrow{..})){
                        let lpt_info = self.last_lp_token_infos.get(position).expect("lp_token_infos not found");
                        lpt_info.tokens.iter().for_each(|token|{
                            tokens.insert(token.token_id.clone());
                        });
                    }
                }
                Action::DecreaseCollateral(_) => {
                    positions.insert(REGULAR_POSITION.to_string());
                }
                Action::PositionDecreaseCollateral { position, asset_amount: _ } => {
                    positions.insert(position.clone());
                }
                Action::Borrow(asset_amount) => {
                    tokens.insert(asset_amount.token_id.clone());
                    positions.insert(REGULAR_POSITION.to_string());
                }
                Action::PositionBorrow { position, asset_amount } => {
                    tokens.insert(asset_amount.token_id.clone());
                    positions.insert(position.clone());
                }
                Action::Liquidate { account_id, in_assets: _, out_assets: _, position, min_token_amounts: _ } => {
                    let position = position.clone().unwrap_or(REGULAR_POSITION.to_string());
                    let liquidation_account = self.internal_get_account(&account_id, true).expect("Account is not registered");
                    tokens.extend(get_account_position_involved_tokens(&self.last_lp_token_infos, &liquidation_account, &position));
                }
                Action::ForceClose { account_id, position, min_token_amounts: _ } => {
                    let position = position.clone().unwrap_or(REGULAR_POSITION.to_string());
                    let liquidation_account = self.internal_get_account(&account_id, true).expect("Account is not registered");
                    tokens.extend(get_account_position_involved_tokens(&self.last_lp_token_infos, &liquidation_account, &position));
                }
                _ => {}
            }
        });
        positions.into_iter().for_each(|position|{
            tokens.extend(get_account_position_involved_tokens(&self.last_lp_token_infos, account, &position));
        });
        tokens.into_iter().collect()
    }

    pub fn token_involved_promises(&self, pyth_oracle_account_id: &AccountId, promise_token_ids: &Vec<AccountId>) -> (Vec<String>, Vec<Promise>) {
        let mut promises_flags = vec![];
        let mut promises = vec![];
        for token_id in promise_token_ids.iter() {
            let token_pyth_info = self.get_pyth_info_by_token(&token_id);
            let price_identifier = token_pyth_info.price_identifier.clone();
            if !promises_flags.contains(&price_identifier.to_string()) {
                promises_flags.push(price_identifier.to_string());
                promises.push(ext_pyth::ext(pyth_oracle_account_id.clone())
                    .with_static_gas(GAS_FOR_GET_PRICE)
                    .get_price(price_identifier));
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

fn get_account_position_involved_tokens(last_lp_token_infos: &HashMap<String, UnitShareTokens>, account: &Account, position: &String) -> HashSet<TokenId> {
    let mut tokens = HashSet::new();
    if let Some(position_info) = account.positions.get(position) {
        match position_info {
            Position::RegularPosition(regular_position) => {
                regular_position.collateral.iter().for_each(|(token_id, _)|{
                    tokens.insert(token_id.clone());
                });
                regular_position.borrowed.iter().for_each(|(token_id, _)|{
                    tokens.insert(token_id.clone());
                });
            }
            Position::LPTokenPosition(lp_token_position) => {
                let lpt_info = last_lp_token_infos.get(&lp_token_position.lpt_id).expect("lp_token_infos not found");
                lpt_info.tokens.iter().for_each(|token|{
                    tokens.insert(token.token_id.clone());
                });
                lp_token_position.borrowed.iter().for_each(|(token_id, _)|{
                    tokens.insert(token_id.clone());
                });
            }
        }
    }
    tokens
}
