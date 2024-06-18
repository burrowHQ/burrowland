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

#[derive(Deserialize, Serialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
#[serde(crate = "near_sdk::serde")]
pub enum TokenReceiverMsg {
    Execute { actions: Vec<Action> },
    ExecuteWithPyth { actions: Vec<Action> },
    DepositToReserve,
    DepositToMargin,
    MarginExecute { actions: Vec<MarginAction> },
    MarginExecuteWithPyth { actions: Vec<MarginAction> },
    SwapReference { swap_ref: SwapReference },
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
        let (actions, with_pyth) = if msg.is_empty() {
            (vec![], false)
        } else {
            let token_receiver_msg: TokenReceiverMsg =
                serde_json::from_str(&msg).expect("Can't parse TokenReceiverMsg");
            match token_receiver_msg {
                TokenReceiverMsg::Execute { actions } => (actions, false),
                TokenReceiverMsg::ExecuteWithPyth { actions } => (actions, true),
                TokenReceiverMsg::DepositToReserve => {
                    asset.reserved += amount;
                    self.internal_set_asset(&token_id, asset);
                    events::emit::deposit_to_reserve(&sender_id, amount, &token_id);
                    return PromiseOrValue::Value(U128(0));
                }
                TokenReceiverMsg::DepositToMargin => {
                    // keep this independent deposit to support price oracle flow
                    let mut account = self.internal_unwrap_margin_account(&sender_id);
                    self.internal_margin_deposit(&mut account, &token_id, amount);
                    events::emit::margin_deposit(&sender_id, amount, &token_id);
                    self.internal_set_margin_account(&sender_id, account);
                    return PromiseOrValue::Value(U128(0));
                }
                TokenReceiverMsg::MarginExecute { actions } => {
                    let mut account = self.internal_unwrap_margin_account(&sender_id);
                    self.internal_margin_deposit(&mut account, &token_id, amount);
                    events::emit::margin_deposit(&sender_id, amount, &token_id);
                    self.internal_margin_execute(&sender_id, &mut account, actions, Prices::new());
                    self.internal_set_margin_account(&sender_id, account);
                    return PromiseOrValue::Value(U128(0));
                }
                TokenReceiverMsg::MarginExecuteWithPyth { actions } => {
                    let mut account = self.internal_unwrap_margin_account(&sender_id);
                    self.internal_margin_deposit(&mut account, &token_id, amount);
                    events::emit::margin_deposit(&sender_id, amount, &token_id);
                    self.internal_margin_execute_with_pyth(&sender_id, &mut account, actions);
                    self.internal_set_margin_account(&sender_id, account);
                    return PromiseOrValue::Value(U128(0));
                }
                TokenReceiverMsg::SwapReference { swap_ref } => {
                    let config = self.internal_config();
                    assert!(sender_id == config.ref_exchange_id || sender_id == config.dcl_id.expect("Missing dcl id"), "Not allow");
                    let mut account = self.internal_unwrap_margin_account(&swap_ref.account_id);
                    if swap_ref.op == "open" {
                        self.on_open_trade_return(&mut account, amount, &swap_ref);
                    } else if swap_ref.op == "decrease"
                        || swap_ref.op == "close"
                        || swap_ref.op == "liquidate"
                        || swap_ref.op == "forceclose"
                    {
                        let event = self.on_decrease_trade_return(&mut account, amount, &swap_ref);
                        events::emit::margin_decrease_succeeded(&swap_ref.op, event);
                    }
                    self.internal_set_margin_account(&swap_ref.account_id, account);
                    return PromiseOrValue::Value(U128(0));
                }
            }
        };

        let mut account = self.internal_unwrap_account(&sender_id);
        self.internal_deposit(&mut account, &token_id, amount);
        events::emit::deposit(&sender_id, amount, &token_id);
        if with_pyth {
            self.internal_execute_with_pyth(&sender_id, &mut account, actions);
        } else {
            self.internal_execute(&sender_id, &mut account, actions, Prices::new());
        }
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
        is_margin_asset: bool,
    ) -> Promise {
        let asset = self.internal_unwrap_asset(token_id);
        let ft_amount = amount / 10u128.pow(asset.config.extra_decimals as u32);
        ext_fungible_token::ext(token_id.clone())
            .with_attached_deposit(ONE_YOCTO)
            .with_static_gas(GAS_FOR_FT_TRANSFER)
            .ft_transfer(account_id.clone(), ft_amount.into(), None)
            .then(
                if is_margin_asset {
                    Self::ext(env::current_account_id())
                        .with_static_gas(GAS_FOR_AFTER_FT_TRANSFER)
                        .after_margin_asset_ft_transfer(account_id.clone(), token_id.clone(), amount.into())
                } else {
                    Self::ext(env::current_account_id())
                        .with_static_gas(GAS_FOR_AFTER_FT_TRANSFER)
                        .after_ft_transfer(account_id.clone(), token_id.clone(), amount.into())
                }
            )
    }
}

#[ext_contract(ext_self)]
trait ExtSelf {
    fn after_ft_transfer(&mut self, account_id: AccountId, token_id: TokenId, amount: U128)
        -> bool;
    fn after_margin_asset_ft_transfer(&mut self, account_id: AccountId, token_id: TokenId, amount: U128)
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
            self.internal_deposit_without_asset_basic_check(&mut account, &token_id, amount.0);
            events::emit::withdraw_failed(&account_id, amount.0, &token_id);
            self.internal_set_account(&account_id, account);
        } else {
            events::emit::withdraw_succeeded(&account_id, amount.0, &token_id);
        }
        promise_success
    }

    #[private]
    fn after_margin_asset_ft_transfer(
        &mut self,
        account_id: AccountId,
        token_id: TokenId,
        amount: U128,
    ) -> bool {
        let promise_success = is_promise_success();
        if !promise_success {
            let mut margin_account = self.internal_unwrap_margin_account(&account_id);
            self.internal_margin_deposit_without_asset_basic_check(&mut margin_account, &token_id, amount.0);
            events::emit::margin_asset_withdraw_failed(&account_id, amount.0, &token_id);
            self.internal_set_margin_account(&account_id, margin_account);
        } else {
            events::emit::margin_asset_withdraw_succeeded(&account_id, amount.0, &token_id);
        }
        promise_success
    }
}
