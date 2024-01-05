use crate::{*, events::emit::{EventMarginOpen, EventMarginClose}};
use near_sdk::{promise_result_as_success, serde_json, PromiseOrValue};

// pub const GAS_FOR_FT_TRANSFER: Gas = Gas(Gas::ONE_TERA.0 * 10);
// pub const GAS_FOR_FT_TRANSFER_CALLBACK: Gas = Gas(Gas::ONE_TERA.0 * 5);
pub const GAS_FOR_FT_TRANSFER_CALL: Gas = Gas(20 * Gas::ONE_TERA.0);
// pub const GAS_FOR_FT_BALANCE_OF: Gas = Gas(10 * Gas::ONE_TERA.0);
pub const GAS_FOR_FT_TRANSFER_CALL_CALLBACK: Gas = Gas(20 * Gas::ONE_TERA.0);
// pub const GAS_FOR_TO_DISTRIBUTE_CALLBACK: Gas = Gas(20 * Gas::ONE_TERA.0);

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
    /// Used for convenient view
    pub open_ts: Timestamp,
    /// Record the unit accumulated holding-position interest when open
    pub uahpi_at_open: Balance,
    /// The capital of debt, used for calculate holding position fee
    pub debt_cap: Balance,

    pub margin_asset: TokenId,
    pub margin_shares: Shares,

    pub debt_asset: TokenId,
    pub debt_shares: Shares,

    pub position_asset: TokenId,
    pub position_amount: Balance,

    pub is_locking: bool,
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
            uahpi_at_open: 0_u128,
            debt_cap: 0_u128,
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
    pub(crate) fn get_mtp_margin_value(
        &self,
        mtp: &MarginTradingPosition,
        prices: &Prices,
    ) -> BigDecimal {
        let asset_margin = self.internal_unwrap_asset(&mtp.margin_asset);
        let margin_balance = asset_margin
            .supplied
            .shares_to_amount(mtp.margin_shares, false);
        BigDecimal::from_balance_price(
            margin_balance,
            prices.get_unwrap(&mtp.margin_asset),
            asset_margin.config.extra_decimals,
        )
    }

    pub(crate) fn get_mtp_position_value(
        &self,
        mtp: &MarginTradingPosition,
        prices: &Prices,
    ) -> BigDecimal {
        let asset_position = self.internal_unwrap_asset(&mtp.position_asset);
        BigDecimal::from_balance_price(
            mtp.position_amount,
            prices.get_unwrap(&mtp.position_asset),
            asset_position.config.extra_decimals,
        )
    }

    pub(crate) fn get_mtp_debt_value(
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

    pub(crate) fn get_mtp_fee_value(
        &self,
        mtp: &MarginTradingPosition,
        prices: &Prices,
    ) -> BigDecimal {
        let asset = self.internal_unwrap_asset(&mtp.debt_asset);
        let total_hp_fee = u128_ratio(
            mtp.debt_cap,
            asset.unit_acc_hp_interest - mtp.uahpi_at_open,
            UNIT,
        );
        if total_hp_fee > 0 {
            BigDecimal::from_balance_price(
                total_hp_fee,
                prices.get_unwrap(&mtp.debt_asset),
                asset.config.extra_decimals,
            )
        } else {
            BigDecimal::from(0_u128)
        }
    }

    pub(crate) fn is_mt_liquidatable(
        &self,
        mt: &MarginTradingPosition,
        prices: &Prices,
        safty_buffer: u32,
    ) -> bool {
        let total_cap =
            self.get_mtp_margin_value(&mt, prices) + self.get_mtp_position_value(&mt, prices);
        let total_debt = self.get_mtp_debt_value(&mt, prices);
        let total_hp_fee = self.get_mtp_fee_value(&mt, prices);
        total_cap.mul_ratio(safty_buffer) + total_cap < total_debt + total_hp_fee
    }

    pub(crate) fn get_mtp_lr(
        &self,
        mt: &MarginTradingPosition,
        prices: &Prices,
    ) -> Option<BigDecimal> {
        if mt.margin_shares.0 == 0 || mt.debt_shares.0 == 0 {
            None
        } else {
            Some(self.get_mtp_debt_value(&mt, prices) / self.get_mtp_margin_value(&mt, prices))
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
    ) -> EventMarginOpen {
        let pos_id = format!("{}_{}", account.account_id.clone(), ts);
        assert!(
            !account.margin_positions.contains_key(&pos_id),
            "Margin position already exist"
        );

        let asset_margin = self.internal_unwrap_asset(margin_id);
        let asset_position = self.internal_unwrap_asset(position_id);
        let mut asset_debt = self.internal_unwrap_asset(debt_id);
        let margin_config = self.internal_margin_config();

        // check legitimacy: assets legal; swap_indication matches;
        margin_config.check_pair(&debt_id, &position_id, &margin_id);
        let mut swap_detail = self.parse_swap_indication(swap_indication);
        let ft_debt_amount = debt_amount / 10u128.pow(asset_debt.config.extra_decimals as u32);
        assert!(
            swap_detail.verify_token_in(debt_id, ft_debt_amount),
            "token_in check failed"
        );
        let ft_position_amount =
            min_position_amount / 10u128.pow(asset_position.config.extra_decimals as u32);
        assert!(
            swap_detail.verify_token_out(position_id, ft_position_amount),
            "token_out check failed"
        );

        // check safty:
        //   min_position_amount reasonable
        assert!(
            is_min_amount_out_reasonable(
                debt_amount,
                &asset_debt,
                prices.get_unwrap(&debt_id),
                &asset_position,
                prices.get_unwrap(&debt_id),
                min_position_amount,
                margin_config.max_slippage_rate,
            ),
            "min_position_amount is too low"
        );
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
        assert!(
            !self.is_mt_liquidatable(&mt, prices, margin_config.min_safty_buffer),
            "Debt is too much"
        );
        //   leverage rate less than max leverage rate
        assert!(
            self.get_mtp_lr(&mt, prices).unwrap()
                <= BigDecimal::from(margin_config.max_leverage_rate as u32),
            "Leverage rate is too high"
        );

        // passes all check, start to open
        let event = EventMarginOpen {
            account_id: account.account_id.clone(),
            pos_id: pos_id.clone(),
            collateral_token_id: margin_id.clone(),
            collateral_amount: margin_amount,
            collateral_shares: mt.margin_shares,
            debt_token_id: debt_id.clone(),
            debt_amount,
            debt_shares: U128(0),
            position_token_id: position_id.clone(),
            position_amount: min_position_amount,
            open_fee: 0,
            holding_fee: 0,
        };
        account.withdraw_supply_shares(margin_id, &margin_shares);
        mt.debt_shares.0 = 0;
        mt.position_amount = 0;
        asset_debt.increase_margin_pending_debt(debt_amount, margin_config.pending_debt_scale);
        self.internal_set_asset(debt_id, asset_debt);
        // TODO: may need to change to store in an unorderedmap in user Account
        account.margin_positions.insert(pos_id.clone(), mt);

        // step 4: call dex to trade and wait for callback
        // organize swap action
        let swap_ref = SwapReference {
            account_id: account.account_id.clone(),
            pos_id: pos_id.clone(),
            amount_in: debt_amount.into(),
            op: format!("open"),
            liquidator_id: None,
        };
        swap_detail.set_client_echo(&swap_ref.to_msg_string());
        let swap_msg = swap_detail.to_msg_string();
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
            event
    }

    /// actual process for decreasing margin position
    fn process_decrease_margin_position(
        &mut self,
        account: &mut MarginAccount,
        pos_id: &String,
        position_amount: Balance,
        min_debt_amount: Balance,
        swap_indication: &SwapIndication,
        prices: &Prices,
        op: String,
        liquidator_id: Option<AccountId>,
    ) -> EventMarginClose {
        let mut mt = account
            .margin_positions
            .get_mut(pos_id)
            .expect("Position not exist");
        assert!(
            !mt.is_locking,
            "Position is currently waiting for a trading result."
        );
        let pre_position_amount = mt.position_amount;
        let mut asset_pos = self.internal_unwrap_asset(&mt.position_asset);
        let asset_debt = self.internal_unwrap_asset(&mt.debt_asset);
        let margin_config = self.internal_margin_config();

        //   check swap_indication
        let mut swap_detail = self.parse_swap_indication(swap_indication);
        let ft_position_amount =
            position_amount / 10u128.pow(asset_pos.config.extra_decimals as u32);
        assert!(
            swap_detail.verify_token_in(&mt.position_asset, ft_position_amount),
            "token_in check failed"
        );
        let ft_debt_amount = min_debt_amount / 10u128.pow(asset_debt.config.extra_decimals as u32);
        assert!(
            swap_detail.verify_token_out(&mt.debt_asset, ft_debt_amount),
            "token_out check failed"
        );

        //   min_debt_amount reasonable
        assert!(
            is_min_amount_out_reasonable(
                position_amount,
                &asset_pos,
                prices.get_unwrap(&mt.position_asset),
                &asset_debt,
                prices.get_unwrap(&mt.debt_asset),
                min_debt_amount,
                margin_config.max_slippage_rate,
            ),
            "min_debt_amount is too low"
        );

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

        if op == "close" || op == "liquidate" {
            //   ensure all debt would be repaid
            //   and take holding-position fee into account
            let total_debt_amount = asset_debt
                .margin_debt
                .shares_to_amount(mt.debt_shares, true);
            let hp_fee = u128_ratio(
                mt.debt_cap,
                asset_debt.unit_acc_hp_interest - mt.uahpi_at_open,
                UNIT,
            );
            if min_debt_amount < total_debt_amount + hp_fee {
                assert_eq!(
                    mt.margin_asset, mt.debt_asset,
                    "Can NOT trade under total debt when margin and debt asset are not the same"
                );
                let gap_shares = asset_debt
                    .supplied
                    .amount_to_shares(total_debt_amount + hp_fee - min_debt_amount, true);
                assert!(
                    mt.margin_shares.0 > gap_shares.0,
                    "Not all debt could be repaid"
                );
            }
        }

        if op == "liquidate" {
            assert!(
                self.is_mt_liquidatable(&mt, prices, margin_config.min_safty_buffer),
                "Margin position is not liquidatable"
            );
        } else if op == "forceclose" {
            assert!(
                self.is_mt_liquidatable(&mt, prices, 0),
                "Margin position is not forceclose-able"
            );
        }

        // prepare to close
        mt.is_locking = true;
        self.internal_set_asset(&mt.position_asset, asset_pos);
        // TODO: mt may be needed to change to store in an unorderedmap in user Account

        let event = EventMarginClose {
            account_id: account.account_id.clone(),
            pos_id: pos_id.clone(),
            liquidator_id: liquidator_id.clone(),
            position_token_id: mt.position_asset.clone(),
            position_amount,
            debt_token_id: mt.debt_asset.clone(),
            debt_amount: min_debt_amount,
        };

        // step 3: call dex to trade and wait for callback
        // organize swap action
        let swap_ref = SwapReference {
            account_id: account.account_id.clone(),
            pos_id: pos_id.clone(),
            amount_in: position_amount.into(),
            op,
            liquidator_id,
        };
        swap_detail.set_client_echo(&swap_ref.to_msg_string());
        let swap_msg = swap_detail.to_msg_string();
        ext_fungible_token::ext(mt.position_asset.clone())
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
        event
    }

    /// intent to reduce the position.
    /// sell postion to repay debt
    pub(crate) fn internal_margin_decrease_position(
        &mut self,
        account: &mut MarginAccount,
        pos_id: &String,
        position_amount: Balance,
        min_debt_amount: Balance,
        swap_indication: &SwapIndication,
        prices: &Prices,
    ) -> EventMarginClose {
        self.process_decrease_margin_position(
            account,
            pos_id,
            position_amount,
            min_debt_amount,
            swap_indication,
            prices,
            "decrease".to_string(),
            None,
        )
    }

    /// intent to close the position.
    /// sell postion to repay debt, if any debt remains,
    /// try to repay with margin if they are the same asset,
    /// otherwise, position stays unclosed.
    pub(crate) fn internal_margin_close_position(
        &mut self,
        account: &mut MarginAccount,
        pos_id: &String,
        position_amount: Balance,
        min_debt_amount: Balance,
        swap_indication: &SwapIndication,
        prices: &Prices,
    ) -> EventMarginClose {
        self.process_decrease_margin_position(
            account,
            pos_id,
            position_amount,
            min_debt_amount,
            swap_indication,
            prices,
            "close".to_string(),
            None,
        )
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
    ) -> EventMarginClose {
        let mut pos_owner = self.internal_unwrap_margin_account(pos_owner_id);
        self.process_decrease_margin_position(
            &mut pos_owner,
            pos_id,
            position_amount,
            min_debt_amount,
            swap_indication,
            prices,
            "liquidate".to_string(),
            Some(liquidator_id.clone()),
        )
    }

    pub(crate) fn internal_margin_forceclose_position(
        &mut self,
        pos_owner_id: &AccountId,
        pos_id: &String,
        position_amount: Balance,
        min_debt_amount: Balance,
        swap_indication: &SwapIndication,
        prices: &Prices,
    ) -> EventMarginClose {
        let mut pos_owner = self.internal_unwrap_margin_account(pos_owner_id);
        self.process_decrease_margin_position(
            &mut pos_owner,
            pos_id,
            position_amount,
            min_debt_amount,
            swap_indication,
            prices,
            "forceclose".to_string(),
            None,
        )
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
                let event = EventMarginOpen {
                    account_id: account_id.clone(),
                    pos_id: pos_id.clone(),
                    collateral_token_id: mt.margin_asset.clone(),
                    collateral_amount: 0,
                    collateral_shares: mt.margin_shares,
                    debt_token_id: mt.debt_asset.clone(),
                    debt_amount: amount_in.0,
                    debt_shares: U128(0),
                    position_token_id: mt.position_asset.clone(),
                    position_amount: pre_position_amount.0,
                    open_fee: 0,
                    holding_fee: 0,
                };
                let mut asset_debt = self.internal_unwrap_asset(&mt.debt_asset);
                asset_debt.margin_pending_debt -= amount_in.0;
                self.internal_set_asset(&mt.debt_asset, asset_debt);
                account.deposit_supply_shares(&mt.margin_asset.clone(), &mt.margin_shares.clone());
                account.margin_positions.remove(&pos_id);
                events::emit::margin_open_failed(event);
                
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
                events::emit::margin_decrease_failed(&account_id, &pos_id);
                // Note: hashmap item got from a get_mut doesn't need to insert back
                // account.margin_positions.insert(pos_id, mt.clone());
            }
            self.internal_set_margin_account(&account_id, account);
        }
    }
}
