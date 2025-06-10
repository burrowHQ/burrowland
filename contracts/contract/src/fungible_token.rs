use crate::*;
use near_contract_standards::fungible_token::core::ext_ft_core;
use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
use near_sdk::json_types::U128;
use near_sdk::{is_promise_success, serde_json, PromiseOrValue};

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

        let (actions, with_pyth) = if msg.is_empty() {
            (vec![], false)
        } else {
            let token_receiver_msg: TokenReceiverMsg =
                serde_json::from_str(&msg).expect("Can't parse TokenReceiverMsg");
            match token_receiver_msg {
                TokenReceiverMsg::Execute { actions } => (actions, false),
                TokenReceiverMsg::ExecuteWithPyth { actions } => (actions, true),
                TokenReceiverMsg::DepositToReserve => {
                    let mut protocol_debts = read_protocol_debts_from_storage();
                    let amount_to_reserved = if let Some(debt) = protocol_debts.remove(&token_id) {
                        let repay_amount = std::cmp::min(amount, debt);
                        let remain_debt = debt - repay_amount;
                        if remain_debt > 0 {
                            protocol_debts.insert(token_id.clone(), remain_debt);
                        }
                        write_protocol_debts_to_storage(protocol_debts);
                        events::emit::repay_protocol_debts(&token_id, repay_amount);
                        amount - repay_amount
                    } else {
                        amount
                    };
                    asset.reserved += amount_to_reserved;
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
                    let mut account = self.internal_unwrap_margin_account(&swap_ref.account_id);
                    let action_ts = account.position_latest_actions.remove(&swap_ref.pos_id).expect("There is no action for the position").0;
                    if sender_id == config.owner_id {
                        require!(env::block_timestamp() - action_ts >= sec_to_nano(self.internal_margin_config().max_position_action_wait_sec), "Please wait for the position action");
                    } else {
                        require!(sender_id == config.ref_exchange_id || sender_id == config.dcl_id.expect("Missing dcl id"), "Not allow");
                    }
                    if swap_ref.op == "open" {
                        self.on_open_trade_return(account, amount, &swap_ref);
                    } else if swap_ref.op == "decrease"
                        || swap_ref.op == "close"
                        || swap_ref.op == "liquidate"
                        || swap_ref.op == "forceclose"
                    {
                        let event = self.on_decrease_trade_return(account, amount, &swap_ref);
                        events::emit::margin_decrease_succeeded(&swap_ref.op, event);
                    }
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
        ft_amount: Balance,
        is_margin_asset: bool,
    ) -> Promise {
        ext_ft_core::ext(token_id.clone())
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

    pub fn internal_ft_transfer_call(
        &mut self,
        account_id: &AccountId,
        token_id: &TokenId,
        amount: Balance,
        ft_amount: Balance,
        client_echo: String,
    ) {
        ext_ft_core::ext(token_id.clone())
            .with_attached_deposit(ONE_YOCTO)
            .with_static_gas(Gas::ONE_TERA * 30)
            .ft_transfer_call(account_id.clone(), ft_amount.into(), None, client_echo)
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(Gas::ONE_TERA * 15)
                    .with_unused_gas_weight(0)
                    .after_ft_transfer_call(account_id.clone(), token_id.clone(), ft_amount.into(), amount.into())
            );
    }
}

#[allow(unused)]
#[ext_contract(ext_self)]
trait ExtSelf {
    fn after_ft_transfer(&mut self, account_id: AccountId, token_id: TokenId, amount: U128)
        -> bool;
    fn after_ft_transfer_call(
        &mut self,
        account_id: AccountId,
        token_id: TokenId,
        ft_amount: U128,
        amount: U128,
    );
    fn after_margin_asset_ft_transfer(
        &mut self,
        account_id: AccountId,
        token_id: TokenId,
        amount: U128,
    ) -> bool;
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
    fn after_ft_transfer_call(
        &mut self,
        account_id: AccountId,
        token_id: TokenId,
        ft_amount: U128,
        amount: U128,
    ) {
        let remain_ft_amount = match promise_result_as_success() {
            Some(result_bytes) => {
                let used_amount = serde_json::from_slice::<U128>(&result_bytes).unwrap();
                ft_amount.0 - used_amount.0
            }
            None => ft_amount.0,
        };
        if remain_ft_amount == 0 {
            events::emit::withdraw_succeeded(&account_id, amount.0, &token_id);
        } else {
            let remain_amount = u128_ratio(amount.0, remain_ft_amount, ft_amount.0);
            let mut account = self.internal_unwrap_account(&account_id);
            self.internal_deposit_without_asset_basic_check(&mut account, &token_id, remain_amount);
            self.internal_set_account(&account_id, account);
            if remain_ft_amount == ft_amount.0 {
                events::emit::withdraw_failed(&account_id, amount.0, &token_id);
            } else {
                events::emit::withdraw_failed(&account_id, remain_amount, &token_id);
                events::emit::withdraw_succeeded(&account_id, amount.0 - remain_amount, &token_id);
            }
        }
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
