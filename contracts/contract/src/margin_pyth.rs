use std::collections::HashSet;

use near_sdk::{serde_json, PromiseResult};

use crate::*;

#[near_bindgen]
impl Contract {

    #[payable]
    pub fn margin_execute_with_pyth(&mut self, actions: Vec<MarginAction>) {
        assert_one_yocto();
        let account_id = env::predecessor_account_id();
        let mut account = self.internal_unwrap_margin_account(&account_id);
        self.internal_margin_execute_with_pyth(&account_id, &mut account, actions);
        self.internal_set_margin_account(&account_id, account);
    }

    #[private]
    pub fn callback_margin_execute_with_pyth(&mut self, account_id: AccountId, margin_involved_tokens: Vec<TokenId>, all_promise_flags: Vec<String>, actions: Vec<MarginAction>, default_prices: HashMap<TokenId, Price>) {
        assert!(env::promise_results_count() == all_promise_flags.len() as u64, "Invalid promise count");
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
        for token_id in margin_involved_tokens {
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
        let mut account = self.internal_unwrap_margin_account(&account_id);
        self.internal_margin_execute(&account_id, &mut account, actions, all_prices);
        self.internal_set_margin_account(&account_id, account);
    }
}

impl Contract {
    pub fn internal_margin_execute_with_pyth(&mut self, account_id: &AccountId, account: &mut MarginAccount, actions: Vec<MarginAction>) {
        let pyth_oracle_account_id = self.internal_config().pyth_oracle_account_id;
        let margin_involved_tokens = self.margin_involved_tokens(&account, &actions);
        if margin_involved_tokens.len() > 0 {
            assert!(self.internal_config().enable_pyth_oracle, "Pyth oracle disabled");
            let mut default_prices: HashMap<TokenId, Price> = HashMap::new();
            let mut promise_token_ids = vec![];
            for token_id in margin_involved_tokens.iter() {
                let token_pyth_info = self.get_pyth_info_by_token(token_id);
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

                let callback_gas = env::prepaid_gas() - (GAS_FOR_GET_PRICE) * all_promise_flags.len() as u64 - GAS_FOR_BUFFER;
                promise.then(
                    Self::ext(env::current_account_id())
                        .with_static_gas(callback_gas)
                        .callback_margin_execute_with_pyth(account_id.clone(), margin_involved_tokens, all_promise_flags, actions, default_prices)
                );
            } else {
                self.internal_margin_execute(&account_id, account, actions, Prices::from_prices(default_prices));
            }
        } else {
            self.internal_margin_execute(&account_id, account, actions, Prices::new());
        }
    }

    pub fn margin_involved_tokens(&self, account: &MarginAccount, actions: &Vec<MarginAction>) -> Vec<TokenId> {
        let mut tokens = HashSet::new();
        actions.iter().for_each(|action|{
            let pos_id = match action {
                MarginAction::DecreaseCollateral { pos_id, amount: _ } => {
                    Some(pos_id)
                }
                MarginAction::OpenPosition { token_c_id: _, token_c_amount: _, token_d_id, token_d_amount: _, token_p_id, min_token_p_amount: _, swap_indication: _ } => {
                    tokens.insert(token_p_id.clone());
                    tokens.insert(token_d_id.clone());
                    None
                },
                MarginAction::DecreaseMTPosition { pos_id, token_p_amount: _, min_token_d_amount: _, swap_indication: _ }=> {
                    Some(pos_id)
                }
                MarginAction::CloseMTPosition { pos_id, token_p_amount: _, min_token_d_amount: _, swap_indication: _ } => {
                    Some(pos_id)
                }
                MarginAction::LiquidateMTPosition { pos_owner_id, pos_id, token_p_amount: _, min_token_d_amount: _, swap_indication: _ } => {
                    let pos_owner_account = self.internal_get_margin_account(pos_owner_id).expect("Margin account not exist");
                    let mt = pos_owner_account.margin_positions.get(pos_id).expect("Position not exist");
                    tokens.insert(mt.token_p_id.clone());
                    tokens.insert(mt.token_d_id.clone());
                    None
                }
                MarginAction::ForceCloseMTPosition { pos_owner_id, pos_id, token_p_amount: _, min_token_d_amount: _, swap_indication: _ } => {
                    let pos_owner_account = self.internal_get_margin_account(pos_owner_id).expect("Margin account not exist");
                    let mt = pos_owner_account.margin_positions.get(pos_id).expect("Position not exist");
                    tokens.insert(mt.token_p_id.clone());
                    tokens.insert(mt.token_d_id.clone());
                    None
                }
                _ => None
            };
            if let Some(pos_id) = pos_id {
                let mt = account.margin_positions.get(pos_id).expect("Position not exist");
                tokens.insert(mt.token_p_id.clone());
                tokens.insert(mt.token_d_id.clone());
            }
        });
        tokens.into_iter().collect()
    }
}