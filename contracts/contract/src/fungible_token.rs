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
    SwapOut {account_id: AccountId, pos_id: String, amount_in: U128, op: String},
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
                    op 
                } => {
                    let mut account = self.internal_unwrap_account(&account_id);
                    let mut mt = if let Position::MarginTradingPosition(mt) = account.positions.get(&pos_id).unwrap() {
                        mt.clone()
                    } else {
                        env::panic_str("Invalid position type")
                    };
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
                        mt.stat = 0;
                        account.positions.insert(pos_id, Position::MarginTradingPosition(mt));
                    } else if op == "increase" {
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
                        mt.stat = 0;
                        account.positions.insert(pos_id, Position::MarginTradingPosition(mt));
                    } else if op == "decrease" {
                        let mut asset_debt = self.internal_unwrap_asset(&mt.debt_asset);
                        let debt_amount = asset_debt.margin_debt.shares_to_amount(mt.debt_shares, true);
                        let (repay_amount, left_amount) = if amount > debt_amount {
                            (debt_amount, amount - debt_amount)
                        } else {
                            (amount, 0)
                        };
                        let repay_shares = asset_debt.margin_debt.amount_to_shares(repay_amount, false);
                        asset_debt.margin_debt.withdraw(repay_shares, repay_amount);
                        let supply_shares = asset_debt.supplied.amount_to_shares(left_amount, false);
                        if supply_shares.0 > 0 {
                            asset_debt.supplied.deposit(supply_shares, left_amount);
                        }
                        self.internal_set_asset(&mt.debt_asset, asset_debt);

                        let mut asset_position = self.internal_unwrap_asset(&mt.position_asset);
                        asset_position.margin_position -= amount_in.0;
                        self.internal_set_asset(&mt.position_asset, asset_position);

                        if supply_shares.0 > 0 {
                            let mut account_asset = account.internal_unwrap_asset(&mt.debt_asset);
                            account_asset.deposit_shares(supply_shares);
                            account.internal_set_asset(&mt.debt_asset, account_asset);
                        }
                        mt.debt_shares.0 -= repay_shares.0;
                        mt.position_amount -= amount_in.0;
                        mt.stat = 0;
                        account.positions.insert(pos_id, Position::MarginTradingPosition(mt));
                    }
                    self.internal_set_account(&account_id, account);

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
