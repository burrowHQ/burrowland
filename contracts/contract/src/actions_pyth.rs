use std::collections::HashSet;

use crate::*;

#[near_bindgen]
impl Contract {

    /// Executes a given list actions on behalf of the predecessor account with pyth oracle price.
    /// - Requires one yoctoNEAR.
    #[payable]
    pub fn execute_with_pyth(&mut self, actions: Vec<Action>) {
        assert_one_yocto();
        let account_id = env::predecessor_account_id();

        // move to internal_execute()
        // // Set reliable liquidator context if caller is in whitelist
        // self.is_reliable_liquidator_context = in_reliable_liquidator_whitelist(&account_id.to_string());

        let mut account = self.internal_unwrap_account(&account_id);
        self.internal_execute_with_pyth(&account_id, &mut account, actions);
        self.internal_set_account(&account_id, account);
    }

    #[private]
    pub fn callback_execute_with_pyth(&mut self, account_id: AccountId, involved_tokens: Vec<TokenId>, all_promise_flags: Vec<String>, actions: Vec<Action>, default_prices: HashMap<TokenId, Price>) {
        assert!(env::promise_results_count() == all_promise_flags.len() as u64, "Invalid promise count");
        let all_prices = self.generate_all_prices(involved_tokens, all_promise_flags, default_prices);
        let mut account = self.internal_unwrap_account(&account_id);
        self.internal_execute(&account_id, &mut account, actions, all_prices);
        self.internal_set_account(&account_id, account);
    }
}

impl Contract {
    pub fn internal_execute_with_pyth(&mut self, account_id: &AccountId, account: &mut Account, actions: Vec<Action>) {
        let involved_tokens: Vec<AccountId> = self.involved_tokens(&account, &actions);
        if involved_tokens.len() > 0 {
            assert!(self.internal_config().enable_pyth_oracle, "Pyth oracle disabled");
            let (promise_token_ids, default_prices) = self.prepare_promise_tokens(&involved_tokens);
            if promise_token_ids.len() > 0 {
                let (all_promise_flags, promise) = self.generate_flags_and_promise(&promise_token_ids);
                let callback_gas = env::prepaid_gas() - (GAS_FOR_GET_PRICE) * all_promise_flags.len() as u64 - GAS_FOR_BUFFER;
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