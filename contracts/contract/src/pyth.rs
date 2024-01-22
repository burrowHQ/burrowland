use std::{collections::HashSet, convert::TryFrom};

use near_sdk::{serde_json, require, PromiseResult};

use crate::*;

pub const GAS_FOR_CALLBACK_EXECUTE_WITH_PYTH: Gas = Gas(200 * Gas::ONE_TERA.0);

pub const EXTRA_CALL_GET_ST_NEAR_PRICE: &str = "get_st_near_price";
pub const EXTRA_CALL_GET_NEARX_PRICE: &str = "get_nearx_price";
pub const EXTRA_CALL_FT_PRICE: &str = "ft_price";

pub const GET_PRICE_PROMISES_LIMIT: usize = 10;

pub const FLAG_PARTITION: &str = "@";

pub const ONE_NEAR: u128 = 10u128.pow(24);

#[ext_contract(ext_pyth)]
pub trait Pyth {
    fn get_price(&self, price_identifier: PriceIdentifier) -> Option<Price>;
}

#[ext_contract(ext_price_extra_call)]
pub trait PriceExtraCall {
    fn get_nearx_price(&self) -> U128;
    fn ft_price(&self) -> U128;
    fn get_st_near_price(&self) -> U128;
}

#[derive(BorshDeserialize, BorshSerialize, Deserialize, Serialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct TokenPythInfo {
    pub decimals: u8,
    pub fraction_digits: u8,
    pub price_identifier: PriceIdentifier,
    pub extra_call: Option<String>
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

#[near_bindgen]
impl Contract {

    #[payable]
    pub fn execute_with_pyth(&mut self, actions: Vec<Action>) {
        assert_one_yocto();
        let account_id = env::predecessor_account_id();
        let mut account = self.internal_unwrap_account(&account_id);
        self.internal_execute_with_pyth(&account_id, &mut account, actions);
        self.internal_set_account(&account_id, account);
    }

    #[private]
    pub fn callback_execute_with_pyth(&mut self, account_id: AccountId, all_promise_flags: Vec<String>, actions: Vec<Action>) {
        assert!(env::promise_results_count() == all_promise_flags.len() as u64, "Invalid promise count");
        let mut account = self.internal_unwrap_account(&account_id);
        let config = self.internal_config();
        let mut all_prices = Prices::new();
        for (index, flag) in all_promise_flags.into_iter().enumerate() {
            if flag.contains(FLAG_PARTITION){
                let token_id = AccountId::try_from(flag.split(FLAG_PARTITION).collect::<Vec<&str>>()[0].to_string()).unwrap();
                let price_amount = match env::promise_result(index as u64) {
                    PromiseResult::Successful(cross_call_result) => {
                        serde_json::from_slice::<U128>(&cross_call_result)
                            .expect(format!("{} cross_call_result not U128", flag).as_str()).0
                    },
                    _ => env::panic_str(format!("{} get price failed!", flag).as_str()),
                };
                let price = all_prices.prices.get_mut(&token_id).unwrap();
                price.multiplier = u128_ratio(price.multiplier, price_amount, ONE_NEAR);
            } else {
                match env::promise_result(index as u64) {
                    PromiseResult::Successful(cross_call_result) => {
                        let pyth_price = serde_json::from_slice::<Option<PythPrice>>(&cross_call_result)
                            .expect(format!("{} cross_call_result not Option<PythPrice>", flag).as_str())
                            .expect(format!("Missing {} price", flag).as_str());
                        assert!(pyth_price.publish_time > 0 && sec_to_nano(pyth_price.publish_time as u32 + config.pyth_price_valid_duration_sec) >= env::block_timestamp(), "Pyth {} publish_time is too stale", flag);
                        let token_id = AccountId::try_from(flag.clone()).expect(format!("Flag {} is not a valid token ID", flag).as_str());
                        let token_price = pyth_price_to_price_oracle_price(self.get_pyth_info_by_token(&token_id), &pyth_price);
                        all_prices.prices.insert(token_id, token_price);
                    },
                    _ => env::panic_str(format!("{} get price failed!", flag).as_str()),
                };
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
            let (mut all_promise_flags, mut promise) = token_involved_promises(
                    &pyth_oracle_account_id, &self.get_pyth_info_by_token(&involved_tokens[0]), &involved_tokens[0]);
            for token in involved_tokens[1..].iter() {
                let (token_promise_flags, token_promise) = token_involved_promises(
                    &pyth_oracle_account_id, &self.get_pyth_info_by_token(token), token);
                all_promise_flags.extend(token_promise_flags);
                promise = promise.and(token_promise);
            }
            assert!(all_promise_flags.len() <= GET_PRICE_PROMISES_LIMIT, "Too many promises to get prices");
            promise.then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_FOR_CALLBACK_EXECUTE_WITH_PYTH)
                    .callback_execute_with_pyth(account_id.clone(), all_promise_flags, actions)
            );
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
                Action::Liquidate { account_id: _, in_assets, out_assets, position: _, min_token_amounts: _ } => {
                    in_assets.iter().for_each(|asset_amount| {
                        tokens.insert(asset_amount.token_id.clone());
                    });
                    out_assets.iter().for_each(|asset_amount| {
                        let token_id_string = asset_amount.token_id.to_string();
                        if token_id_string.starts_with(SHADOW_V1_TOKEN_PREFIX) {
                            let lpt_info = self.last_lp_token_infos.get(&token_id_string).expect("lp_token_infos not found");
                            lpt_info.tokens.iter().for_each(|token|{
                                tokens.insert(token.token_id.clone());
                            });
                        } else {
                            tokens.insert(asset_amount.token_id.clone());
                        }
                    });
                }
                Action::ForceClose { account_id, position, min_token_amounts: _ } => {
                    let position = position.clone().unwrap_or(REGULAR_POSITION.to_string());
                    let liquidation_account = self.internal_unwrap_account(account_id);
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

fn token_involved_promises(pyth_oracle_account_id: &AccountId, token_pyth_info: &TokenPythInfo, token_id: &TokenId) -> (Vec<String>, Promise) {
    let mut promise_flags = vec![];
    let mut promise = ext_pyth::ext(pyth_oracle_account_id.clone())
        .get_price(token_pyth_info.price_identifier.clone());
    promise_flags.push(token_id.to_string());

    if let Some(extra_call) = token_pyth_info.extra_call.as_ref() {
        match extra_call.as_str() {
            EXTRA_CALL_GET_ST_NEAR_PRICE => {
                promise = promise.and(ext_price_extra_call::ext(token_id.clone())
                    .get_st_near_price());
                promise_flags.push(format!("{}{}{}", token_id.to_string(), FLAG_PARTITION, EXTRA_CALL_GET_ST_NEAR_PRICE));
            }
            EXTRA_CALL_FT_PRICE => {
                promise = promise.and(ext_price_extra_call::ext(token_id.clone())
                    .ft_price());
                promise_flags.push(format!("{}{}{}", token_id.to_string(), FLAG_PARTITION, EXTRA_CALL_FT_PRICE));
            }
            EXTRA_CALL_GET_NEARX_PRICE => {
                promise = promise.and(ext_price_extra_call::ext(token_id.clone())
                    .get_nearx_price());
                promise_flags.push(format!("{}{}{}", token_id.to_string(), FLAG_PARTITION, EXTRA_CALL_GET_NEARX_PRICE));
            }
            _ => unimplemented!()
        }
    }
    (promise_flags, promise)
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
