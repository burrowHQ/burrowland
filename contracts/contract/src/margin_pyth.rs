use std::{collections::HashSet, convert::TryFrom};

use near_sdk::{serde_json, PromiseResult};

use crate::*;

pub const GAS_FOR_CALLBACK_MARGIN_EXECUTE_WITH_PYTH: Gas = Gas(200 * Gas::ONE_TERA.0);

#[near_bindgen]
impl Contract {
    #[private]
    pub fn callback_margin_execute_with_pyth(&mut self, account_id: AccountId, all_promise_flags: Vec<String>, actions: Vec<MarginAction>, default_prices: HashMap<TokenId, Price>) {
        assert!(env::promise_results_count() == all_promise_flags.len() as u64, "Invalid promise count");
        let config = self.internal_config();
        let mut all_prices = Prices::from_prices(default_prices);
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
        let mut account = self.internal_unwrap_margin_account(&account_id);
        self.internal_margin_execute(&account_id, &mut account, actions, all_prices);
        self.internal_set_margin_account(&account_id, account);
    }

    #[payable]
    pub fn margin_execute_with_pyth(&mut self, actions: Vec<MarginAction>) {
        assert_one_yocto();
        let account_id = env::predecessor_account_id();
        let mut account = self.internal_unwrap_margin_account(&account_id);
        self.internal_margin_execute_with_pyth(&account_id, &mut account, actions);
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
            for token_id in margin_involved_tokens {
                let token_pyth_info = self.get_pyth_info_by_token(&token_id);
                if token_pyth_info.default_price.is_some() {
                    default_prices.insert(token_id, token_pyth_info.default_price.unwrap());
                } else {
                    promise_token_ids.push(token_id);
                }
            }
            if promise_token_ids.len() > 0 {
                let (mut all_promise_flags, mut promise) = token_involved_promises(
                    &pyth_oracle_account_id, &self.get_pyth_info_by_token(&promise_token_ids[0]), &promise_token_ids[0]);
                for token_id in promise_token_ids[1..].iter() {
                    let (token_promise_flags, token_promise) = token_involved_promises(
                        &pyth_oracle_account_id, self.get_pyth_info_by_token(&token_id), token_id);
                    all_promise_flags.extend(token_promise_flags);
                    promise = promise.and(token_promise);
                }
                assert!(all_promise_flags.len() <= GET_PRICE_PROMISES_LIMIT, "Too many promises to get prices");
                promise.then(
                    Self::ext(env::current_account_id())
                        .with_static_gas(GAS_FOR_CALLBACK_MARGIN_EXECUTE_WITH_PYTH)
                        .callback_margin_execute_with_pyth(account_id.clone(), all_promise_flags, actions, default_prices)
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
                MarginAction::OpenPosition { token_c_id, token_c_amount: _, token_d_id, token_d_amount: _, token_p_id: _, min_token_p_amount: _, swap_indication: _ } => {
                    tokens.insert(token_c_id.clone());
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
                    tokens.insert(mt.token_c_id.clone());
                    tokens.insert(mt.token_d_id.clone());
                    None
                }
                MarginAction::ForceCloseMTPosition { pos_owner_id, pos_id, token_p_amount: _, min_token_d_amount: _, swap_indication: _ } => {
                    let pos_owner_account = self.internal_get_margin_account(pos_owner_id).expect("Margin account not exist");
                    let mt = pos_owner_account.margin_positions.get(pos_id).expect("Position not exist");
                    tokens.insert(mt.token_c_id.clone());
                    tokens.insert(mt.token_d_id.clone());
                    None
                }
                _ => None
            };
            if let Some(pos_id) = pos_id {
                let mt = account.margin_positions.get(pos_id).expect("Position not exist");
                tokens.insert(mt.token_c_id.clone());
                tokens.insert(mt.token_d_id.clone());
            }
        });
        tokens.into_iter().collect()
    }
}