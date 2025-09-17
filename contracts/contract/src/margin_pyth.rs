use std::collections::HashSet;
use crate::*;

#[near_bindgen]
impl Contract {

    /// Executes a given list margin actions on behalf of the predecessor account with pyth oracle price.
    /// - Requires one yoctoNEAR.
    #[payable]
    pub fn margin_execute_with_pyth(&mut self, actions: Vec<MarginAction>) {
        assert_one_yocto();
        let account_id = env::predecessor_account_id();

        // Set reliable liquidator context if caller is in whitelist
        self.is_reliable_liquidator_context = in_reliable_liquidator_whitelist(&account_id.to_string());

        let mut account = self.internal_unwrap_margin_account(&account_id);
        self.internal_margin_execute_with_pyth(&account_id, &mut account, actions);
        self.internal_set_margin_account(&account_id, account);
    }

    #[private]
    pub fn callback_margin_execute_with_pyth(&mut self, account_id: AccountId, margin_involved_tokens: Vec<TokenId>, all_promise_flags: Vec<String>, actions: Vec<MarginAction>, default_prices: HashMap<TokenId, Price>) {
        assert!(env::promise_results_count() == all_promise_flags.len() as u64, "Invalid promise count");
        let all_prices = self.generate_all_prices(margin_involved_tokens, all_promise_flags, default_prices);
        let mut account = self.internal_unwrap_margin_account(&account_id);
        self.internal_margin_execute(&account_id, &mut account, actions, all_prices);
        self.internal_set_margin_account(&account_id, account);
    }
}

impl Contract {
    pub fn internal_margin_execute_with_pyth(&mut self, account_id: &AccountId, account: &mut MarginAccount, actions: Vec<MarginAction>) {
        let margin_involved_tokens = self.margin_involved_tokens(&account, &actions);
        if margin_involved_tokens.len() > 0 {
            assert!(self.internal_config().enable_pyth_oracle, "Pyth oracle disabled");
            let (promise_token_ids, default_prices) = self.prepare_promise_tokens(&margin_involved_tokens);
            if promise_token_ids.len() > 0 {
                let (all_promise_flags, promise) = self.generate_flags_and_promise(&promise_token_ids);
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