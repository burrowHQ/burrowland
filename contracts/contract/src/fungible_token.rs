use crate::*;
use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
use near_sdk::json_types::U128;
use near_sdk::{is_promise_success, serde_json, PromiseOrValue};

#[ext_contract(ext_fungible_token)]
pub trait FungibleToken {
    fn ft_transfer(&mut self, receiver_id: AccountId, amount: U128, memo: Option<String>);
}

const GAS_FOR_FT_TRANSFER: Gas = Gas(Gas::ONE_TERA.0 * 10);
const GAS_FOR_AFTER_FT_TRANSFER: Gas = Gas(Gas::ONE_TERA.0 * 20);

#[derive(Deserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, Serialize))]
#[serde(crate = "near_sdk::serde")]
pub enum TokenReceiverMsg {
    Execute { actions: Vec<Action> },
    DepositToReserve,
    SwapOut {account_id: AccountId, pos_id: String, amount_in: U128, op: String, liquidator_id: Option<AccountId>},
}

#[near_bindgen]
impl FungibleTokenReceiver for Contract {
    /// Receives the transfer from the fungible token and executes a list of actions given in the
    /// message on behalf of the sender. The actions that can be executed should be limited to a set
    /// that doesn't require pricing.
    /// - Requires to be called by the fungible token account.
    fn ft_on_transfer(
        &mut self,
        sender_id: AccountId,
        amount: U128,
        msg: String,
    ) -> PromiseOrValue<U128> {
        let token_id = env::predecessor_account_id();
        let mut asset = self.internal_unwrap_asset(&token_id);
        assert!(
            asset.config.can_deposit,
            "Deposits for this asset are not enabled"
        );

        let amount = amount.0 * 10u128.pow(asset.config.extra_decimals as u32);

        // TODO: We need to be careful that only whitelisted tokens can call this method with a
        //     given set of actions. Or verify which actions are possible to do.
        let actions: Vec<Action> = if msg.is_empty() {
            vec![]
        } else {
            let token_receiver_msg: TokenReceiverMsg =
                serde_json::from_str(&msg).expect("Can't parse TokenReceiverMsg");
            match token_receiver_msg {
                TokenReceiverMsg::Execute { actions } => actions,
                TokenReceiverMsg::DepositToReserve => {
                    asset.reserved += amount;
                    self.internal_set_asset(&token_id, asset);
                    events::emit::deposit_to_reserve(&sender_id, amount, &token_id);
                    return PromiseOrValue::Value(U128(0));
                }
                TokenReceiverMsg::SwapOut { 
                    account_id, 
                    pos_id, 
                    amount_in, 
                    op, 
                    liquidator_id,
                } => {
                    let mut account = self.internal_unwrap_margin_account(&account_id);
                    let mut mt = account.margin_positions.get(&pos_id).unwrap().clone();
                    if op == "open" {
                        let mut asset_debt = self.internal_unwrap_asset(&mt.debt_asset);
                        asset_debt.margin_pending_debt -= amount_in.0;
                        let debt_shares = asset_debt.margin_debt.amount_to_shares(amount_in.0, true);
                        asset_debt.margin_debt.deposit(debt_shares, amount_in.0);
                        self.internal_set_asset(&mt.debt_asset, asset_debt);

                        let mut asset_position = self.internal_unwrap_asset(&mt.position_asset);
                        asset_position.margin_position += amount;
                        self.internal_set_asset(&mt.position_asset, asset_position);

                        mt.debt_shares.0 += debt_shares.0;
                        mt.position_amount += amount;
                        mt.is_locking = false;
                        account.margin_positions.insert(pos_id, mt.clone());
                    } else if op == "decrease" {
                        let mut asset_debt = self.internal_unwrap_asset(&mt.debt_asset);
                        let mut asset_position = self.internal_unwrap_asset(&mt.position_asset);
                        // figure out actual repay amount and shares
                        let debt_amount = asset_debt.margin_debt.shares_to_amount(mt.debt_shares, true);
                        let (repay_amount, repay_shares, left_amount) = if amount >= debt_amount {
                            (debt_amount, mt.debt_shares, amount - debt_amount)
                        } else {
                            (amount, asset_debt.margin_debt.amount_to_shares(amount, false), 0)
                        };
                        asset_debt.margin_debt.withdraw(repay_shares, repay_amount);
                        mt.debt_shares.0 -= repay_shares.0;
                        mt.is_locking = false;
                        // handle possible leftover debt asset, put them into user's supply
                        if left_amount > 0 {
                            let supply_shares = asset_debt.supplied.amount_to_shares(left_amount, false);
                            if supply_shares.0 > 0 {
                                asset_debt.supplied.deposit(supply_shares, left_amount);
                                account.deposit_supply_shares(&mt.debt_asset, &supply_shares);
                            }
                        }
                        // try to repay remaining debt from margin
                        if mt.debt_shares.0 > 0 && mt.debt_asset == mt.margin_asset {
                            let remain_debt_balance = asset_debt.margin_debt.shares_to_amount(mt.debt_shares, true);
                            let margin_shares_to_repay = asset_debt.supplied.amount_to_shares(remain_debt_balance, true);
                            let (repay_debt_share, used_supply_share, repay_amount) = if margin_shares_to_repay <= mt.margin_shares {
                                (mt.debt_shares, margin_shares_to_repay, remain_debt_balance)
                            } else {
                                // use all margin balance to repay
                                let margin_balance = asset_debt.supplied.shares_to_amount(mt.margin_shares, false);
                                let repay_debt_shares = asset_debt.margin_debt.amount_to_shares(margin_balance, false);
                                (repay_debt_shares, mt.margin_shares, margin_balance)
                            };
                            asset_debt.supplied.withdraw(used_supply_share, repay_amount);
                            asset_debt.margin_debt.withdraw(repay_debt_share, repay_amount);
                            mt.debt_shares.0 -= repay_debt_share.0;
                            mt.margin_shares.0 -= used_supply_share.0;
                        }
                        account.margin_positions.insert(pos_id.clone(), mt.clone());
                        // try to settle this position
                        if mt.debt_shares.0 == 0 {
                            // close this position and remaining asset goes back to user's inner account
                            // TODO: change to directly send assets back to user
                            if mt.margin_shares.0 > 0 {
                                account.deposit_supply_shares(&mt.margin_asset, &mt.margin_shares);
                            }
                            if mt.position_amount > 0 {
                                let position_shares = asset_position.supplied.amount_to_shares(mt.position_amount, false);
                                asset_position.supplied.deposit(position_shares, mt.position_amount);
                                account.deposit_supply_shares(&mt.position_asset, &position_shares);
                            }
                            account.margin_positions.remove(&pos_id);
                        }
                        self.internal_set_asset(&mt.debt_asset, asset_debt);
                        self.internal_set_asset(&mt.position_asset, asset_position);

                    } else if op == "liquidate" {
                        let mut asset_debt = self.internal_unwrap_asset(&mt.debt_asset);
                        let mut asset_position = self.internal_unwrap_asset(&mt.position_asset);
                        // figure out actual liquidator
                        let mut liquidator_account = if let Some(liquidator_account_id) = liquidator_id {
                            if let Some(x) = self.internal_get_margin_account(&liquidator_account_id) {
                                x
                            } else {
                                self.internal_unwrap_margin_account(&self.internal_config().owner_id)
                            }
                        } else {
                            self.internal_unwrap_margin_account(&self.internal_config().owner_id)
                        };
                        // figure out actual repay amount and shares
                        let debt_amount = asset_debt.margin_debt.shares_to_amount(mt.debt_shares, true);
                        let (repay_amount, repay_shares, left_amount) = if amount >= debt_amount {
                            (debt_amount, mt.debt_shares, amount - debt_amount)
                        } else {
                            (amount, asset_debt.margin_debt.amount_to_shares(amount, false), 0)
                        };
                        asset_debt.margin_debt.withdraw(repay_shares, repay_amount);
                        mt.debt_shares.0 -= repay_shares.0;
                        mt.is_locking = false;
                        // handle possible leftover debt asset, put them into liquidator's supply
                        if left_amount > 0 {
                            let supply_shares = asset_debt.supplied.amount_to_shares(left_amount, false);
                            if supply_shares.0 > 0 {
                                asset_debt.supplied.deposit(supply_shares, left_amount);
                                liquidator_account.deposit_supply_shares(&mt.debt_asset, &supply_shares);
                            }
                        }
                        // try to repay remaining debt from margin
                        if mt.debt_shares.0 > 0 && mt.debt_asset == mt.margin_asset {
                            let remain_debt_balance = asset_debt.margin_debt.shares_to_amount(mt.debt_shares, true);
                            let margin_shares_to_repay = asset_debt.supplied.amount_to_shares(remain_debt_balance, true);
                            let (repay_debt_share, used_supply_share, repay_amount) = if margin_shares_to_repay <= mt.margin_shares {
                                (mt.debt_shares, margin_shares_to_repay, remain_debt_balance)
                            } else {
                                // use all margin balance to repay
                                let margin_balance = asset_debt.supplied.shares_to_amount(mt.margin_shares, false);
                                let repay_debt_shares = asset_debt.margin_debt.amount_to_shares(margin_balance, false);
                                (repay_debt_shares, mt.margin_shares, margin_balance)
                            };
                            asset_debt.supplied.withdraw(used_supply_share, repay_amount);
                            asset_debt.margin_debt.withdraw(repay_debt_share, repay_amount);
                            mt.debt_shares.0 -= repay_debt_share.0;
                            mt.margin_shares.0 -= used_supply_share.0;
                        }
                        account.margin_positions.insert(pos_id.clone(), mt.clone());
                        // try to settle this position
                        if mt.debt_shares.0 == 0 {
                            // close this position and remaining asset goes back to liquidator's inner account
                            // TODO: change to directly send assets back to liquidator
                            if mt.margin_shares.0 > 0 {
                                liquidator_account.deposit_supply_shares(&mt.margin_asset, &mt.margin_shares);
                            }
                            if mt.position_amount > 0 {
                                let position_shares = asset_position.supplied.amount_to_shares(mt.position_amount, false);
                                asset_position.supplied.deposit(position_shares, mt.position_amount);
                                liquidator_account.deposit_supply_shares(&mt.position_asset, &position_shares);
                            }
                            account.margin_positions.remove(&pos_id);
                        }
                        self.internal_set_margin_account(&liquidator_account.account_id.clone(), liquidator_account);
                        self.internal_set_asset(&mt.debt_asset, asset_debt);
                        self.internal_set_asset(&mt.position_asset, asset_position);
                    }
                    self.internal_set_margin_account(&account_id, account);

                    return PromiseOrValue::Value(U128(0));
                }
            }
        };

        let mut account = self.internal_unwrap_account(&sender_id);
        self.internal_deposit(&mut account, &token_id, amount);
        events::emit::deposit(&sender_id, amount, &token_id);
        self.internal_execute(&sender_id, &mut account, actions, Prices::new());
        self.internal_set_account(&sender_id, account);

        PromiseOrValue::Value(U128(0))
    }
}

impl Contract {
    pub fn internal_ft_transfer(
        &mut self,
        account_id: &AccountId,
        token_id: &TokenId,
        amount: Balance,
    ) -> Promise {
        let asset = self.internal_unwrap_asset(token_id);
        let ft_amount = amount / 10u128.pow(asset.config.extra_decimals as u32);
        ext_fungible_token::ext(token_id.clone())
        .with_attached_deposit(ONE_YOCTO)
        .with_static_gas(GAS_FOR_FT_TRANSFER)
        .ft_transfer(
            account_id.clone(),
            ft_amount.into(),
            None,
        )
        .then(
            Self::ext(env::current_account_id())
                .with_static_gas(GAS_FOR_AFTER_FT_TRANSFER)
                .after_ft_transfer(
                    account_id.clone(),
                    token_id.clone(),
                    amount.into(),
                )
        )
    }
}

#[ext_contract(ext_self)]
trait ExtSelf {
    fn after_ft_transfer(&mut self, account_id: AccountId, token_id: TokenId, amount: U128)
        -> bool;
}

#[near_bindgen]
impl ExtSelf for Contract {
    #[private]
    fn after_ft_transfer(
        &mut self,
        account_id: AccountId,
        token_id: TokenId,
        amount: U128,
    ) -> bool {
        let promise_success = is_promise_success();
        if !promise_success {
            let mut account = self.internal_unwrap_account(&account_id);
            self.internal_deposit(&mut account, &token_id, amount.0);
            events::emit::withdraw_failed(&account_id, amount.0, &token_id);
            self.internal_set_account(&account_id, account);
        } else {
            events::emit::withdraw_succeeded(&account_id, amount.0, &token_id);
        }
        promise_success
    }
}
