use crate::*;
use near_sdk::{is_promise_success, promise_result_as_success, serde_json, PromiseOrValue};

pub const GAS_FOR_FT_TRANSFER: Gas = Gas(Gas::ONE_TERA.0 * 10);
pub const GAS_FOR_FT_TRANSFER_CALLBACK: Gas = Gas(Gas::ONE_TERA.0 * 5);
pub const GAS_FOR_FT_TRANSFER_CALL: Gas = Gas(20 * Gas::ONE_TERA.0);
pub const GAS_FOR_FT_BALANCE_OF: Gas = Gas(10 * Gas::ONE_TERA.0);
pub const GAS_FOR_FT_TRANSFER_CALL_CALLBACK: Gas = Gas(20 * Gas::ONE_TERA.0);
pub const GAS_FOR_TO_DISTRIBUTE_CALLBACK: Gas = Gas(20 * Gas::ONE_TERA.0);

#[ext_contract(ext_fungible_token)]
pub trait FungibleToken {
    fn ft_transfer(&mut self, receiver_id: AccountId, amount: U128, memo: Option<String>);
    fn ft_transfer_call(
        &mut self,
        receiver_id: AccountId,
        amount: U128,
        memo: Option<String>,
        msg: String,
    ) -> PromiseOrValue<U128>;
    fn ft_balance_of(&self, account_id: AccountId) -> U128;
}

/// margin_asset == debt_asset or position asset
///
#[derive(BorshSerialize, BorshDeserialize, Serialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct MarginTradingPosition {
    pub open_ts: Timestamp,
    pub margin_asset: TokenId,
    pub margin_shares: Shares,
    pub debt_asset: TokenId,
    pub debt_shares: Shares,
    pub position_asset: TokenId,
    pub position_amount: Balance,
    pub is_locking: bool,
}

#[derive(Serialize, Deserialize, Clone)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
#[serde(crate = "near_sdk::serde")]
pub struct SwapIndication {
    pub dex_id: AccountId,
    /// Total supplied including collateral, but excluding reserved.
    pub swap_action_text: String,
}

impl MarginTradingPosition {
    fn new(
        open_ts: Timestamp,
        margin_asset: TokenId,
        margin_shares: Shares,
        debt_asset: TokenId,
        position_asset: TokenId,
    ) -> Self {
        MarginTradingPosition {
            open_ts,
            margin_asset,
            margin_shares,
            debt_asset,
            debt_shares: U128(0),
            position_asset,
            position_amount: 0,
            is_locking: true,
        }
    }
}

impl Contract {
    pub(crate) fn get_mtp_collateral_sum(
        &self,
        mtp: &MarginTradingPosition,
        prices: &Prices,
    ) -> BigDecimal {
        let asset_margin = self.internal_unwrap_asset(&mtp.margin_asset);
        let margin_balance = asset_margin
            .supplied
            .shares_to_amount(mtp.margin_shares, false);
        let margin_adjusted_value = BigDecimal::from_balance_price(
            margin_balance,
            prices.get_unwrap(&mtp.margin_asset),
            asset_margin.config.extra_decimals,
        );

        let asset_position = self.internal_unwrap_asset(&mtp.position_asset);
        let position_adjusted_value = BigDecimal::from_balance_price(
            mtp.position_amount,
            prices.get_unwrap(&mtp.position_asset),
            asset_position.config.extra_decimals,
        );

        margin_adjusted_value + position_adjusted_value
    }

    pub(crate) fn get_mtp_borrowed_sum(
        &self,
        mtp: &MarginTradingPosition,
        prices: &Prices,
    ) -> BigDecimal {
        let asset = self.internal_unwrap_asset(&mtp.debt_asset);
        let balance = asset.margin_debt.shares_to_amount(mtp.debt_shares, true);
        BigDecimal::from_balance_price(
            balance,
            prices.get_unwrap(&mtp.debt_asset),
            asset.config.extra_decimals,
        )
    }

    pub(crate) fn get_mtp_profit(
        &self,
        mtp: &MarginTradingPosition,
        prices: &Prices,
    ) -> Option<BigDecimal> {
        let asset_position = self.internal_unwrap_asset(&mtp.position_asset);
        let position_value = BigDecimal::from_balance_price(
            mtp.position_amount,
            prices.get_unwrap(&mtp.position_asset),
            asset_position.config.extra_decimals,
        );
        let asset_debt = self.internal_unwrap_asset(&mtp.debt_asset);
        let debt_balance = asset_debt
            .margin_debt
            .shares_to_amount(mtp.debt_shares, true);
        let debt_value = BigDecimal::from_balance_price(
            debt_balance,
            prices.get_unwrap(&mtp.debt_asset),
            asset_debt.config.extra_decimals,
        );
        if position_value >= debt_value {
            Some(position_value - debt_value)
        } else {
            None
        }
    }

    pub(crate) fn get_mtp_loss(
        &self,
        mtp: &MarginTradingPosition,
        prices: &Prices,
    ) -> Option<BigDecimal> {
        let asset_position = self.internal_unwrap_asset(&mtp.position_asset);
        let position_value = BigDecimal::from_balance_price(
            mtp.position_amount,
            prices.get_unwrap(&mtp.position_asset),
            asset_position.config.extra_decimals,
        );
        let asset_debt = self.internal_unwrap_asset(&mtp.debt_asset);
        let debt_balance = asset_debt
            .margin_debt
            .shares_to_amount(mtp.debt_shares, true);
        let debt_value = BigDecimal::from_balance_price(
            debt_balance,
            prices.get_unwrap(&mtp.debt_asset),
            asset_debt.config.extra_decimals,
        );
        if position_value < debt_value {
            Some(debt_value - position_value)
        } else {
            None
        }
    }

    pub(crate) fn get_mtp_lr(
        &self,
        mtp: &MarginTradingPosition,
        prices: &Prices,
    ) -> Option<BigDecimal> {
        if mtp.margin_shares.0 == 0 || mtp.debt_shares.0 == 0 {
            None
        } else {
            let asset_debt = self.internal_unwrap_asset(&mtp.debt_asset);
            let debt_balance = asset_debt
                .margin_debt
                .shares_to_amount(mtp.debt_shares, true);
            let debt_value = BigDecimal::from_balance_price(
                debt_balance,
                prices.get_unwrap(&mtp.debt_asset),
                asset_debt.config.extra_decimals,
            );
            let asset_margin = self.internal_unwrap_asset(&mtp.margin_asset);
            let margin_balance = asset_margin
                .supplied
                .shares_to_amount(mtp.margin_shares, false);
            let margin_value = BigDecimal::from_balance_price(
                margin_balance,
                prices.get_unwrap(&mtp.margin_asset),
                asset_margin.config.extra_decimals,
            );
            Some(debt_value / margin_value)
        }
    }
}

impl Contract {
    pub(crate) fn internal_margin_open_position(
        &mut self,
        ts: Timestamp,
        account: &mut MarginAccount,
        margin_id: &AccountId,
        margin_amount: Balance,
        debt_id: &AccountId,
        debt_amount: Balance,
        position_id: &AccountId,
        min_position_amount: Balance,
        swap_indication: &SwapIndication,
        prices: &Prices,
    ) {
        let pos_id = format!("{}_{}", account.account_id.clone(), ts);
        assert!(
            !account.margin_positions.contains_key(&pos_id),
            "Margin position already exist"
        );

        let asset_margin = self.internal_unwrap_asset(margin_id);
        let asset_position = self.internal_unwrap_asset(position_id);

        // check legitimacy: assets legal; swap_indication matches;
        // TODO:

        // check safty:  pending_debt available; min_position_amount reasonable; margin_hf more than 1 + safty_buffer_rate(10%)
        //   pending_debt available
        let mut asset_debt = self.internal_unwrap_asset(debt_id);
        let tbd_x = 10_u128; // means 10%
        assert!(
            tbd_x * (asset_debt.margin_pending_debt + debt_amount) < asset_debt.available_amount(),
            "Pending debt will overflow"
        );
        //   min_position_amount reasonable
        let debt_value = BigDecimal::from_balance_price(
            debt_amount,
            prices.get_unwrap(&debt_id),
            asset_debt.config.extra_decimals,
        );
        let et_position_amount = debt_value.to_balance_in_price(
            prices.get_unwrap(&position_id),
            asset_position.config.extra_decimals,
        );
        let tbd_slippage_rate = 1000_u128; // 10% slippage
        if min_position_amount < et_position_amount {
            assert!(
                min_position_amount >= et_position_amount * tbd_slippage_rate / MAX_RATIO as u128,
                "min_position_amount is too low"
            );
        }
        //   margin_hf more than 1 + safty_buffer_rate(10%)
        let margin_shares = asset_margin.supplied.amount_to_shares(margin_amount, false);
        let mut mt = MarginTradingPosition::new(
            ts,
            margin_id.clone(),
            margin_shares.clone(),
            debt_id.clone(),
            position_id.clone(),
        );
        mt.debt_shares = asset_debt.margin_debt.amount_to_shares(debt_amount, true);
        mt.position_amount = min_position_amount;
        let total_cap = self.get_mtp_collateral_sum(&mt, prices);
        let total_debt = self.get_mtp_borrowed_sum(&mt, prices);
        let tbd_safty_buffer = 1000_u32;
        assert!(
            total_cap.mul_ratio(tbd_safty_buffer) + total_cap > total_debt,
            "Debt is too much"
        );

        // passes all check, start to open
        // TODO: if margin asset is not come from user supply, should modify these code
        account.withdraw_supply_shares(margin_id, &margin_shares);
        mt.debt_shares.0 = 0;
        mt.position_amount = 0;
        asset_debt.margin_pending_debt += debt_amount;
        self.internal_set_asset(debt_id, asset_debt);
        // TODO: change to store in an unorderedmap in user Account
        account.margin_positions.insert(pos_id.clone(), mt);

        // step 4: call dex to trade and wait for callback
        // TODO: organize swap action
        let swap_msg = format!("");
        ext_fungible_token::ext(debt_id.clone())
            .with_attached_deposit(1)
            .with_static_gas(GAS_FOR_FT_TRANSFER_CALL)
            .ft_transfer_call(
                swap_indication.dex_id.clone(),
                U128(debt_amount),
                None,
                swap_msg,
            )
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_FOR_FT_TRANSFER_CALL_CALLBACK)
                    .callback_dex_trade(
                        account.account_id.clone(),
                        pos_id.clone(),
                        debt_amount.into(),
                        U128(0),
                        format!("open"),
                    ),
            );
    }

    /// intent to close the position.
    /// sell postion to repay debt, if any debt remains,
    /// try to repay with margin if they are the same asset,
    /// otherwise,position goes to a pending close state.
    pub(crate) fn internal_margin_close_position(
        &mut self,
        account: &mut MarginAccount,
        pos_id: &String,
        position_amount: Balance,
        min_debt_amount: Balance,
        swap_indication: &SwapIndication,
        prices: &Prices,
    ) {
        let mut mt = account
            .margin_positions
            .get_mut(pos_id)
            .expect("Position not exist");
        let pre_position_amount = mt.position_amount;

        let position_id = mt.position_asset.clone();
        let mut asset_pos = self.internal_unwrap_asset(&mt.position_asset);
        let asset_debt = self.internal_unwrap_asset(&mt.debt_asset);

        // check safty:  mt is running; min_debt_amount reasonable; enough position token; all debt would be repaid
        //   mt is running
        assert!(
            !mt.is_locking,
            "Position is currently waiting for trading result."
        );
        //   min_debt_amount reasonable
        let position_value = BigDecimal::from_balance_price(
            position_amount,
            prices.get_unwrap(&mt.position_asset),
            asset_pos.config.extra_decimals,
        );
        let et_debt_amount = position_value.to_balance_in_price(
            prices.get_unwrap(&mt.debt_asset),
            asset_debt.config.extra_decimals,
        );
        let tbd_slippage_rate = 1000_u128; // 10% slippage
        if min_debt_amount < et_debt_amount {
            assert!(
                min_debt_amount >= et_debt_amount * tbd_slippage_rate / MAX_RATIO as u128,
                "min_position_amount is too low"
            );
        }
        //   ensure enough position token to trade
        if position_amount > mt.position_amount {
            // try to add some of margin asset into trading
            assert_eq!(
                mt.margin_asset, mt.position_asset,
                "Not enough position asset balance"
            );
            let gap_shares = asset_pos
                .supplied
                .amount_to_shares(position_amount - mt.position_amount, true);
            mt.margin_shares
                .0
                .checked_sub(gap_shares.0)
                .expect("Not enough position asset balance");
            asset_pos
                .supplied
                .withdraw(gap_shares, position_amount - mt.position_amount);
            asset_pos.margin_position -= mt.position_amount;
            mt.position_amount = 0;
        } else {
            asset_pos.margin_position -= position_amount;
            mt.position_amount -= position_amount;
        }
        //   ensure all debt would be repaid
        let total_debt_amount = asset_debt
            .margin_debt
            .shares_to_amount(mt.debt_shares, true);
        if min_debt_amount < total_debt_amount {
            assert_eq!(
                mt.margin_asset, mt.debt_asset,
                "Can NOT trade under total debt when margin and debt asset are not the same"
            );
            let gap_shares = asset_debt
                .supplied
                .amount_to_shares(total_debt_amount - min_debt_amount, true);
            assert!(
                mt.margin_shares.0 > gap_shares.0,
                "Not all debt could be repaid"
            );
        }

        // prepare to close
        mt.is_locking = true;
        self.internal_set_asset(&mt.position_asset, asset_pos);
        // TODO: change to store in an unorderedmap in user Account
        // Note: hashmap item got from a get_mut doesn't need to insert back
        // account.margin_positions.insert(pos_id.clone(), mt.clone());

        // step 3: call dex to trade and wait for callback
        // TODO: organize swap action
        let swap_msg = format!("");
        ext_fungible_token::ext(position_id.clone())
            .with_attached_deposit(1)
            .with_static_gas(GAS_FOR_FT_TRANSFER_CALL)
            .ft_transfer_call(
                swap_indication.dex_id.clone(),
                U128(position_amount),
                None,
                swap_msg,
            )
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_FOR_FT_TRANSFER_CALL_CALLBACK)
                    .callback_dex_trade(
                        account.account_id.clone(),
                        pos_id.clone(),
                        position_amount.into(),
                        pre_position_amount.into(),
                        format!("decrease"),
                    ),
            );
    }

    pub(crate) fn internal_margin_liquidate_position(
        &mut self,
        liquidator_id: &AccountId,
        pos_owner_id: &AccountId,
        pos_id: &String,
        position_amount: Balance,
        min_debt_amount: Balance,
        swap_indication: &SwapIndication,
        prices: &Prices,
    ) {
        let mut pos_owner = self.internal_unwrap_margin_account(pos_owner_id);
        let mut mt = pos_owner
            .margin_positions
            .get_mut(pos_id)
            .expect("Position not exist");
        let pre_position_amount = mt.position_amount;

        let position_id = mt.position_asset.clone();
        let mut asset_pos = self.internal_unwrap_asset(&mt.position_asset);
        let asset_debt = self.internal_unwrap_asset(&mt.debt_asset);

        // check safty:  mt is running; mt is liquidatable; min_debt_amount reasonable; enough position token; all debt would be repaid
        //   mt is running
        assert!(
            !mt.is_locking,
            "Position is currently waiting for trading result."
        );
        //   mt is liquidatable
        let total_cap = self.get_mtp_collateral_sum(&mt, prices);
        let total_debt = self.get_mtp_borrowed_sum(&mt, prices);
        let tbd_safty_buffer = 1000_u32;
        assert!(
            total_cap.mul_ratio(tbd_safty_buffer) + total_cap < total_debt,
            "Margin position is not liquidatable"
        );
        //   min_debt_amount reasonable
        let position_value = BigDecimal::from_balance_price(
            position_amount,
            prices.get_unwrap(&mt.position_asset),
            asset_pos.config.extra_decimals,
        );
        let et_debt_amount = position_value.to_balance_in_price(
            prices.get_unwrap(&mt.debt_asset),
            asset_debt.config.extra_decimals,
        );
        let tbd_slippage_rate = 1000_u128; // 10% slippage
        if min_debt_amount < et_debt_amount {
            assert!(
                min_debt_amount >= et_debt_amount * tbd_slippage_rate / MAX_RATIO as u128,
                "min_position_amount is too low"
            );
        }
        //   ensure enough position token to trade
        if position_amount > mt.position_amount {
            // try to add some of margin asset into trading
            assert_eq!(
                mt.margin_asset, mt.position_asset,
                "Not enough position asset balance"
            );
            let gap_shares = asset_pos
                .supplied
                .amount_to_shares(position_amount - mt.position_amount, true);
            mt.margin_shares
                .0
                .checked_sub(gap_shares.0)
                .expect("Not enough position asset balance");
            asset_pos
                .supplied
                .withdraw(gap_shares, position_amount - mt.position_amount);
            asset_pos.margin_position -= mt.position_amount;
            mt.position_amount = 0;
        } else {
            asset_pos.margin_position -= position_amount;
            mt.position_amount -= position_amount;
        }
        //   ensure all debt would be repaid
        let total_debt_amount = asset_debt
            .margin_debt
            .shares_to_amount(mt.debt_shares, true);
        if min_debt_amount < total_debt_amount {
            assert_eq!(
                mt.margin_asset, mt.debt_asset,
                "Can NOT trade under total debt when margin and debt asset are not the same"
            );
            let gap_shares = asset_debt
                .supplied
                .amount_to_shares(total_debt_amount - min_debt_amount, true);
            assert!(
                mt.margin_shares.0 > gap_shares.0,
                "Not all debt could be repaid"
            );
        }

        // prepare to close
        mt.is_locking = true;
        self.internal_set_asset(&mt.position_asset, asset_pos);
        // TODO: change to store in an unorderedmap in user Account
        // Note: hashmap item got from a get_mut doesn't need to insert back
        // pos_owner.margin_positions.insert(pos_id.clone(), mt.clone());

        // step 3: call dex to trade and wait for callback
        // TODO: organize swap action
        let swap_msg = format!("");
        ext_fungible_token::ext(position_id.clone())
            .with_attached_deposit(1)
            .with_static_gas(GAS_FOR_FT_TRANSFER_CALL)
            .ft_transfer_call(
                swap_indication.dex_id.clone(),
                U128(position_amount),
                None,
                swap_msg,
            )
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_FOR_FT_TRANSFER_CALL_CALLBACK)
                    .callback_dex_trade(
                        pos_owner.account_id.clone(),
                        pos_id.clone(),
                        position_amount.into(),
                        pre_position_amount.into(),
                        format!("decrease"),
                    ),
            );
    }

    pub(crate) fn internal_margin_forceclose_position(
        &mut self,
        pos_owner_id: &AccountId,
        pos_id: &String,
        position_amount: Balance,
        min_debt_amount: Balance,
        swap_indication: &SwapIndication,
        prices: &Prices,
    ) {
        let mut pos_owner = self.internal_unwrap_margin_account(pos_owner_id);
        let mut mt = pos_owner
            .margin_positions
            .get_mut(pos_id)
            .expect("Position not exist");
        let pre_position_amount = mt.position_amount;

        let position_id = mt.position_asset.clone();
        let mut asset_pos = self.internal_unwrap_asset(&mt.position_asset);
        let asset_debt = self.internal_unwrap_asset(&mt.debt_asset);

        // check safty:  mt is running; mt is liquidatable; min_debt_amount reasonable; enough position token
        //   mt is running
        assert!(
            !mt.is_locking,
            "Position is currently waiting for trading result."
        );
        //   mt is liquidatable
        let total_cap = self.get_mtp_collateral_sum(&mt, prices);
        let total_debt = self.get_mtp_borrowed_sum(&mt, prices);
        assert!(
            total_cap < total_debt,
            "Margin position does NOT meet force close condition"
        );
        //   min_debt_amount reasonable
        let position_value = BigDecimal::from_balance_price(
            position_amount,
            prices.get_unwrap(&mt.position_asset),
            asset_pos.config.extra_decimals,
        );
        let et_debt_amount = position_value.to_balance_in_price(
            prices.get_unwrap(&mt.debt_asset),
            asset_debt.config.extra_decimals,
        );
        let tbd_slippage_rate = 1000_u128; // 10% slippage
        if min_debt_amount < et_debt_amount {
            assert!(
                min_debt_amount >= et_debt_amount * tbd_slippage_rate / MAX_RATIO as u128,
                "min_position_amount is too low"
            );
        }
        //   ensure enough position token to trade
        if position_amount > mt.position_amount {
            // try to add some of margin asset into trading
            assert_eq!(
                mt.margin_asset, mt.position_asset,
                "Not enough position asset balance"
            );
            let gap_shares = asset_pos
                .supplied
                .amount_to_shares(position_amount - mt.position_amount, true);
            mt.margin_shares
                .0
                .checked_sub(gap_shares.0)
                .expect("Not enough position asset balance");
            asset_pos
                .supplied
                .withdraw(gap_shares, position_amount - mt.position_amount);
            asset_pos.margin_position -= mt.position_amount;
            mt.position_amount = 0;
        } else {
            asset_pos.margin_position -= position_amount;
            mt.position_amount -= position_amount;
        }

        // prepare to forceclose
        mt.is_locking = true;
        self.internal_set_asset(&mt.position_asset, asset_pos);
        // TODO: change to store in an unorderedmap in user Account
        // Note: hashmap item got from a get_mut doesn't need to insert back
        // pos_owner.margin_positions.insert(pos_id.clone(), mt.clone());

        // step 3: call dex to trade and wait for callback
        // TODO: organize swap action
        let swap_msg = format!("");
        ext_fungible_token::ext(position_id.clone())
            .with_attached_deposit(1)
            .with_static_gas(GAS_FOR_FT_TRANSFER_CALL)
            .ft_transfer_call(
                swap_indication.dex_id.clone(),
                U128(position_amount),
                None,
                swap_msg,
            )
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_FOR_FT_TRANSFER_CALL_CALLBACK)
                    .callback_dex_trade(
                        pos_owner.account_id.clone(),
                        pos_id.clone(),
                        position_amount.into(),
                        pre_position_amount.into(),
                        format!("decrease"),
                    ),
            );
    }
}

impl Contract {
    pub(crate) fn on_open_trade_return(
        &mut self,
        account: &mut MarginAccount,
        pos_id: PosId,
        amount: Balance,
        amount_in: Balance,
    ) {
        let mut mt = account.margin_positions.get(&pos_id).unwrap().clone();
        let mut asset_debt = self.internal_unwrap_asset(&mt.debt_asset);
        asset_debt.margin_pending_debt -= amount_in;
        let debt_shares = asset_debt.margin_debt.amount_to_shares(amount_in, true);
        asset_debt.margin_debt.deposit(debt_shares, amount_in);
        self.internal_set_asset(&mt.debt_asset, asset_debt);

        let mut asset_position = self.internal_unwrap_asset(&mt.position_asset);
        asset_position.margin_position += amount;
        self.internal_set_asset(&mt.position_asset, asset_position);

        mt.debt_shares.0 += debt_shares.0;
        mt.position_amount += amount;
        mt.is_locking = false;
        account.margin_positions.insert(pos_id, mt.clone());
    }

    pub(crate) fn on_decrease_trade_return(
        &mut self,
        account: &mut MarginAccount,
        pos_id: PosId,
        amount: Balance,
        amount_in: Balance,
    ) {
        let mut mt = account.margin_positions.get(&pos_id).unwrap().clone();
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
        mt.is_locking = false;
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
    }

    pub(crate) fn on_liquidate_trade_return(
        &mut self,
        account: &mut MarginAccount,
        pos_id: PosId,
        amount: Balance,
        amount_in: Balance,
        liquidator_id: Option<AccountId>,
    ) {
        let mut mt = account.margin_positions.get(&pos_id).unwrap().clone();
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
        let debt_amount = asset_debt
            .margin_debt
            .shares_to_amount(mt.debt_shares, true);
        let (repay_amount, repay_shares, left_amount) = if amount >= debt_amount {
            (debt_amount, mt.debt_shares, amount - debt_amount)
        } else {
            (
                amount,
                asset_debt.margin_debt.amount_to_shares(amount, false),
                0,
            )
        };
        asset_debt.margin_debt.withdraw(repay_shares, repay_amount);
        mt.debt_shares.0 -= repay_shares.0;
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
            let remain_debt_balance = asset_debt
                .margin_debt
                .shares_to_amount(mt.debt_shares, true);
            let margin_shares_to_repay = asset_debt
                .supplied
                .amount_to_shares(remain_debt_balance, true);
            let (repay_debt_share, used_supply_share, repay_amount) =
                if margin_shares_to_repay <= mt.margin_shares {
                    (mt.debt_shares, margin_shares_to_repay, remain_debt_balance)
                } else {
                    // use all margin balance to repay
                    let margin_balance = asset_debt
                        .supplied
                        .shares_to_amount(mt.margin_shares, false);
                    let repay_debt_shares = asset_debt
                        .margin_debt
                        .amount_to_shares(margin_balance, false);
                    (repay_debt_shares, mt.margin_shares, margin_balance)
                };
            asset_debt
                .supplied
                .withdraw(used_supply_share, repay_amount);
            asset_debt
                .margin_debt
                .withdraw(repay_debt_share, repay_amount);
            mt.debt_shares.0 -= repay_debt_share.0;
            mt.margin_shares.0 -= used_supply_share.0;
        }
        mt.is_locking = false;
        account.margin_positions.insert(pos_id.clone(), mt.clone());
        // try to settle this position
        if mt.debt_shares.0 == 0 {
            // close this position and remaining asset goes back to liquidator's inner account
            // TODO: change to directly send assets back to liquidator
            if mt.margin_shares.0 > 0 {
                liquidator_account.deposit_supply_shares(&mt.margin_asset, &mt.margin_shares);
            }
            if mt.position_amount > 0 {
                let position_shares = asset_position
                    .supplied
                    .amount_to_shares(mt.position_amount, false);
                asset_position
                    .supplied
                    .deposit(position_shares, mt.position_amount);
                liquidator_account.deposit_supply_shares(&mt.position_asset, &position_shares);
            }
            account.margin_positions.remove(&pos_id);
        }
        self.internal_set_margin_account(
            &liquidator_account.account_id.clone(),
            liquidator_account,
        );
        self.internal_set_asset(&mt.debt_asset, asset_debt);
        self.internal_set_asset(&mt.position_asset, asset_position);
    }

    pub(crate) fn on_forceclose_trade_return(
        &mut self,
        account: &mut MarginAccount,
        pos_id: PosId,
        amount: Balance,
        amount_in: Balance,
    ) {
        let mut mt = account.margin_positions.get(&pos_id).unwrap().clone();
        let mut asset_debt = self.internal_unwrap_asset(&mt.debt_asset);
        let mut asset_position = self.internal_unwrap_asset(&mt.position_asset);
        let mut liquidator_account = self.internal_unwrap_margin_account(&self.internal_config().owner_id);
        // figure out actual repay amount and shares
        let debt_amount = asset_debt
            .margin_debt
            .shares_to_amount(mt.debt_shares, true);
        let (repay_amount, repay_shares, left_amount) = if amount >= debt_amount {
            (debt_amount, mt.debt_shares, amount - debt_amount)
        } else {
            (
                amount,
                asset_debt.margin_debt.amount_to_shares(amount, false),
                0,
            )
        };
        asset_debt.margin_debt.withdraw(repay_shares, repay_amount);
        mt.debt_shares.0 -= repay_shares.0;
        
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
            let remain_debt_balance = asset_debt
                .margin_debt
                .shares_to_amount(mt.debt_shares, true);
            let margin_shares_to_repay = asset_debt
                .supplied
                .amount_to_shares(remain_debt_balance, true);
            let (repay_debt_share, used_supply_share, repay_amount) =
                if margin_shares_to_repay <= mt.margin_shares {
                    (mt.debt_shares, margin_shares_to_repay, remain_debt_balance)
                } else {
                    // use all margin balance to repay
                    let margin_balance = asset_debt
                        .supplied
                        .shares_to_amount(mt.margin_shares, false);
                    let repay_debt_shares = asset_debt
                        .margin_debt
                        .amount_to_shares(margin_balance, false);
                    (repay_debt_shares, mt.margin_shares, margin_balance)
                };
            asset_debt
                .supplied
                .withdraw(used_supply_share, repay_amount);
            asset_debt
                .margin_debt
                .withdraw(repay_debt_share, repay_amount);
            mt.debt_shares.0 -= repay_debt_share.0;
            mt.margin_shares.0 -= used_supply_share.0;
        }
        // try to use protocol reserve to repay remaining debt
        if mt.debt_shares.0 > 0 {
            let remain_debt_balance = asset_debt
                .margin_debt
                .shares_to_amount(mt.debt_shares, true);
            if asset_debt.reserved > remain_debt_balance {
                asset_debt.reserved -= remain_debt_balance;
                asset_debt.margin_debt.withdraw(mt.debt_shares, remain_debt_balance);
                mt.debt_shares.0 = 0;
            }
        }
        mt.is_locking = false;
        account.margin_positions.insert(pos_id.clone(), mt.clone());
        // try to settle this position
        if mt.debt_shares.0 == 0 {
            // close this position and remaining asset goes back to liquidator's inner account
            if mt.margin_shares.0 > 0 {
                liquidator_account.deposit_supply_shares(&mt.margin_asset, &mt.margin_shares);
            }
            if mt.position_amount > 0 {
                let position_shares = asset_position
                    .supplied
                    .amount_to_shares(mt.position_amount, false);
                asset_position
                    .supplied
                    .deposit(position_shares, mt.position_amount);
                liquidator_account.deposit_supply_shares(&mt.position_asset, &position_shares);
            }
            account.margin_positions.remove(&pos_id);
        } else {
            // force close failed due to insufficient reserve
            env::log_str(&format!("Force close failed due to insufficient reserve, user {}, pos_id {}", account.account_id.clone(), pos_id.clone()));
        }
        self.internal_set_margin_account(
            &liquidator_account.account_id.clone(),
            liquidator_account,
        );
        self.internal_set_asset(&mt.debt_asset, asset_debt);
        self.internal_set_asset(&mt.position_asset, asset_position);
    }
}

#[near_bindgen]
impl Contract {
    #[private]
    pub fn callback_dex_trade(
        &mut self,
        account_id: AccountId,
        pos_id: PosId,
        amount_in: U128,
        pre_position_amount: U128,
        op: String,
    ) {
        let amount_in_used = if let Some(cross_call_result) = promise_result_as_success() {
            serde_json::from_slice::<U128>(&cross_call_result)
                .unwrap()
                .0
        } else {
            0_u128
        };
        if amount_in_used == 0 {
            // trading failed, revert margin operation
            let mut account = self.internal_unwrap_margin_account(&account_id);
            if op == "open" {
                let mt = account.margin_positions.get(&pos_id).unwrap();
                let mut asset_debt = self.internal_unwrap_asset(&mt.debt_asset);
                asset_debt.margin_pending_debt -= amount_in.0;
                self.internal_set_asset(&mt.debt_asset, asset_debt);
                account.deposit_supply_shares(&mt.margin_asset.clone(), &mt.margin_shares.clone());
                account.margin_positions.remove(&pos_id);
            } else if op == "decrease" {
                let mut mt = account.margin_positions.get_mut(&pos_id).unwrap();
                let mut asset_position = self.internal_unwrap_asset(&mt.position_asset);
                let amount_in: Balance = amount_in.into();
                let pre_position_amount: Balance = pre_position_amount.into();
                if amount_in > pre_position_amount {
                    asset_position.margin_position += pre_position_amount;
                    // re-deposit those gap to supply as margin
                    let gap = amount_in - pre_position_amount;
                    let gap_shares = asset_position.supplied.amount_to_shares(gap, false);
                    asset_position.supplied.deposit(gap_shares, gap);
                    mt.margin_shares.0 += gap_shares.0;
                } else {
                    asset_position.margin_position += amount_in;
                }
                self.internal_set_asset(&mt.position_asset, asset_position);
                mt.is_locking = false;
                mt.position_amount = pre_position_amount;
                // Note: hashmap item got from a get_mut doesn't need to insert back
                // account.margin_positions.insert(pos_id, mt.clone());
            }
            self.internal_set_margin_account(&account_id, account);
        }
    }
}
