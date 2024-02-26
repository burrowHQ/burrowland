use crate::{*, events::emit::{EventDataMarginOpen, EventDataMarginDecrease}};
use near_sdk::{promise_result_as_success, serde_json, PromiseOrValue};


pub const GAS_FOR_FT_TRANSFER_CALL: Gas = Gas(100 * Gas::ONE_TERA.0);
pub const GAS_FOR_FT_TRANSFER_CALL_CALLBACK: Gas = Gas(20 * Gas::ONE_TERA.0);

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

    pub token_c_id: TokenId,
    pub token_c_shares: Shares,

    pub token_d_id: TokenId,
    pub token_d_shares: Shares,

    pub token_p_id: TokenId,
    pub token_p_amount: Balance,

    pub is_locking: bool,
}

impl MarginTradingPosition {
    fn new(
        open_ts: Timestamp,
        token_c_id: TokenId,
        token_c_shares: Shares,
        token_d_id: TokenId,
        token_p_id: TokenId,
    ) -> Self {
        MarginTradingPosition {
            open_ts,
            uahpi_at_open: 0_u128,
            debt_cap: 0_u128,
            token_c_id,
            token_c_shares,
            token_d_id,
            token_d_shares: U128(0),
            token_p_id,
            token_p_amount: 0,
            is_locking: true,
        }
    }
}

impl Contract {
    pub(crate) fn get_mtp_collateral_value(
        &self,
        mtp: &MarginTradingPosition,
        prices: &Prices,
    ) -> BigDecimal {
        let asset_c = self.internal_unwrap_asset(&mtp.token_c_id);
        let balance_c = asset_c
            .supplied
            .shares_to_amount(mtp.token_c_shares, false);
        BigDecimal::from_balance_price(
            balance_c,
            prices.get_unwrap(&mtp.token_c_id),
            asset_c.config.extra_decimals,
        )
    }

    pub(crate) fn get_mtp_position_value(
        &self,
        mtp: &MarginTradingPosition,
        prices: &Prices,
    ) -> BigDecimal {
        let asset_p = self.internal_unwrap_asset(&mtp.token_p_id);
        BigDecimal::from_balance_price(
            mtp.token_p_amount,
            prices.get_unwrap(&mtp.token_p_id),
            asset_p.config.extra_decimals,
        )
    }

    pub(crate) fn get_mtp_debt_value(
        &self,
        mtp: &MarginTradingPosition,
        prices: &Prices,
    ) -> BigDecimal {
        let asset_d = self.internal_unwrap_asset(&mtp.token_d_id);
        let balance_d = asset_d.margin_debt.shares_to_amount(mtp.token_d_shares, true);
        BigDecimal::from_balance_price(
            balance_d,
            prices.get_unwrap(&mtp.token_d_id),
            asset_d.config.extra_decimals,
        )
    }

    pub(crate) fn get_mtp_hp_fee_value(
        &self,
        mtp: &MarginTradingPosition,
        prices: &Prices,
    ) -> BigDecimal {
        let asset_d = self.internal_unwrap_asset(&mtp.token_d_id);
        let total_hp_fee = u128_ratio(
            mtp.debt_cap,
            asset_d.unit_acc_hp_interest - mtp.uahpi_at_open,
            UNIT,
        );
        if total_hp_fee > 0 {
            BigDecimal::from_balance_price(
                total_hp_fee,
                prices.get_unwrap(&mtp.token_d_id),
                asset_d.config.extra_decimals,
            )
        } else {
            BigDecimal::from(0_u128)
        }
    }

    pub(crate) fn is_mt_liquidatable(
        &self,
        mt: &MarginTradingPosition,
        prices: &Prices,
        safty_buffer_rate: u32,
    ) -> bool {
        let total_cap =
            self.get_mtp_collateral_value(&mt, prices) + self.get_mtp_position_value(&mt, prices);
        let total_debt = self.get_mtp_debt_value(&mt, prices);
        let total_hp_fee = self.get_mtp_hp_fee_value(&mt, prices);
        total_cap >= total_debt + total_hp_fee && 
            total_cap - total_cap.mul_ratio(safty_buffer_rate) < total_debt + total_hp_fee
    }

    pub(crate) fn is_mt_forcecloseable(
        &self,
        mt: &MarginTradingPosition,
        prices: &Prices,
    ) -> bool {
        let total_cap =
            self.get_mtp_collateral_value(&mt, prices) + self.get_mtp_position_value(&mt, prices);
        let total_debt = self.get_mtp_debt_value(&mt, prices);
        let total_hp_fee = self.get_mtp_hp_fee_value(&mt, prices);
        total_cap < total_debt + total_hp_fee
    }

    pub(crate) fn get_mtp_lr(
        &self,
        mt: &MarginTradingPosition,
        prices: &Prices,
    ) -> Option<BigDecimal> {
        if mt.token_c_shares.0 == 0 || mt.token_d_shares.0 == 0 {
            None
        } else {
            Some(self.get_mtp_debt_value(&mt, prices) / self.get_mtp_collateral_value(&mt, prices))
        }
    }
}

impl Contract {
    pub(crate) fn internal_margin_open_position(
        &mut self,
        ts: Timestamp,
        account: &mut MarginAccount,
        token_c_id: &AccountId,
        token_c_amount: Balance,
        token_d_id: &AccountId,
        token_d_amount: Balance,
        token_p_id: &AccountId,
        min_token_p_amount: Balance,
        swap_indication: &SwapIndication,
        prices: &Prices,
    ) -> EventDataMarginOpen {
        let pos_id = format!("{}_{}", account.account_id.clone(), ts);
        assert!(
            !account.margin_positions.contains_key(&pos_id),
            "Margin position already exist"
        );

        let asset_c = self.internal_unwrap_asset(token_c_id);
        let asset_p = self.internal_unwrap_asset(token_p_id);
        let mut asset_d = self.internal_unwrap_asset(token_d_id);
        let margin_config = self.internal_margin_config();

        // check legitimacy: assets legal; swap_indication matches;
        margin_config.check_pair(&token_d_id, &token_p_id, &token_c_id);
        let mut swap_detail = self.parse_swap_indication(swap_indication);
        let ft_d_amount = token_d_amount / 10u128.pow(asset_d.config.extra_decimals as u32);
        assert!(
            swap_detail.verify_token_in(token_d_id, ft_d_amount),
            "token_in check failed"
        );
        let ft_p_amount =
            min_token_p_amount / 10u128.pow(asset_p.config.extra_decimals as u32);
        assert!(
            swap_detail.verify_token_out(token_p_id, ft_p_amount),
            "token_out check failed"
        );

        // check safty:
        //   min_position_amount reasonable
        assert!(
            is_min_amount_out_reasonable(
                token_d_amount,
                &asset_d,
                prices.get_unwrap(&token_d_id),
                &asset_p,
                prices.get_unwrap(&token_p_id),
                min_token_p_amount,
                margin_config.max_slippage_rate,
            ),
            "min_position_amount is too low"
        );
        //   margin_hf more than 1 + safty_buffer_rate(10%)
        let mut mt = MarginTradingPosition::new(
            ts,
            token_c_id.clone(),
            asset_c.supplied.amount_to_shares(token_c_amount, false),
            token_d_id.clone(),
            token_p_id.clone(),
        );
        mt.token_d_shares = asset_d.margin_debt.amount_to_shares(token_d_amount, true);
        mt.token_p_amount = min_token_p_amount;
        assert!(
            !self.is_mt_liquidatable(&mt, prices, margin_config.min_safty_buffer),
            "Debt is too much"
        );
        assert!(
            !self.is_mt_forcecloseable(&mt, prices),
            "Debt is too much"
        );
        //   leverage rate less than max leverage rate
        assert!(
            self.get_mtp_lr(&mt, prices).unwrap()
                <= BigDecimal::from(margin_config.max_leverage_rate as u32),
            "Leverage rate is too high"
        );

        // passes all check, start to open
        let event = EventDataMarginOpen {
            account_id: account.account_id.clone(),
            pos_id: pos_id.clone(),
            token_c_id: token_c_id.clone(),
            token_c_amount,
            token_c_shares: mt.token_c_shares,
            token_d_id: token_d_id.clone(),
            token_d_amount,
            token_p_id: token_p_id.clone(),
            token_p_amount: min_token_p_amount,
        };
        account.withdraw_supply_shares(token_c_id, &mt.token_c_shares);
        mt.token_d_shares.0 = 0;
        mt.token_p_amount = 0;
        asset_d.increase_margin_pending_debt(token_d_amount, margin_config.pending_debt_scale);
        self.internal_set_asset(token_d_id, asset_d);
        // TODO: may need to change to store in an unorderedmap in user Account
        account.margin_positions.insert(pos_id.clone(), mt);

        // step 4: call dex to trade and wait for callback
        // organize swap action
        let swap_ref = SwapReference {
            account_id: account.account_id.clone(),
            pos_id: pos_id.clone(),
            amount_in: token_d_amount.into(),
            op: format!("open"),
            liquidator_id: None,
        };
        swap_detail.set_client_echo(&swap_ref.to_msg_string());
        let swap_msg = swap_detail.to_msg_string();
        ext_fungible_token::ext(token_d_id.clone())
            .with_attached_deposit(1)
            .with_static_gas(GAS_FOR_FT_TRANSFER_CALL)
            .ft_transfer_call(
                swap_indication.dex_id.clone(),
                U128(ft_d_amount),
                None,
                swap_msg,
            )
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_FOR_FT_TRANSFER_CALL_CALLBACK)
                    .callback_dex_trade(
                        account.account_id.clone(),
                        pos_id.clone(),
                        token_d_amount.into(),
                        U128(0),
                        format!("open"),
                    ),
            );
            event
    }

    /// actual process for decreasing margin position
    pub(crate) fn process_decrease_margin_position(
        &mut self,
        account: &mut MarginAccount,
        pos_id: &String,
        token_p_amount: Balance,
        min_token_d_amount: Balance,
        swap_indication: &SwapIndication,
        prices: &Prices,
        op: String,
        liquidator_id: Option<AccountId>,
    ) -> EventDataMarginDecrease {
        let mut mt = account
            .margin_positions
            .get_mut(pos_id)
            .expect("Position not exist");
        assert!(
            !mt.is_locking,
            "Position is currently waiting for a trading result."
        );
        let pre_token_p_amount = mt.token_p_amount;
        let mut asset_p = self.internal_unwrap_asset(&mt.token_p_id);
        let asset_d = self.internal_unwrap_asset(&mt.token_d_id);
        let margin_config = self.internal_margin_config();

        //   check swap_indication
        let mut swap_detail = self.parse_swap_indication(swap_indication);
        let ft_p_amount =
            token_p_amount / 10u128.pow(asset_p.config.extra_decimals as u32);
        assert!(
            swap_detail.verify_token_in(&mt.token_p_id, ft_p_amount),
            "token_in check failed"
        );
        let ft_d_amount = min_token_d_amount / 10u128.pow(asset_d.config.extra_decimals as u32);
        assert!(
            swap_detail.verify_token_out(&mt.token_d_id, ft_d_amount),
            "token_out check failed"
        );

        //   min_debt_amount reasonable
        assert!(
            is_min_amount_out_reasonable(
                token_p_amount,
                &asset_p,
                prices.get_unwrap(&mt.token_p_id),
                &asset_d,
                prices.get_unwrap(&mt.token_d_id),
                min_token_d_amount,
                margin_config.max_slippage_rate,
            ),
            "min_debt_amount is too low"
        );

        if op == "close" || op == "liquidate" {
            //   ensure all debt would be repaid
            //   and take holding-position fee into account
            let total_debt_amount = asset_d
                .margin_debt
                .shares_to_amount(mt.token_d_shares, true);
            let hp_fee = u128_ratio(
                mt.debt_cap,
                asset_d.unit_acc_hp_interest - mt.uahpi_at_open,
                UNIT,
            );
            if min_token_d_amount < total_debt_amount + hp_fee {
                assert_eq!(
                    mt.token_c_id, mt.token_d_id,
                    "Can NOT trade under total debt when margin and debt asset are not the same"
                );
                let gap_shares = asset_d
                    .supplied
                    .amount_to_shares(total_debt_amount + hp_fee - min_token_d_amount, true);
                assert!(
                    mt.token_c_shares.0 > gap_shares.0,
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
                self.is_mt_forcecloseable(&mt, prices),
                "Margin position is not forceclose-able"
            );
        }

        //   ensure enough position token to trade
        if token_p_amount > mt.token_p_amount {
            // try to add some of margin asset into trading
            assert_eq!(
                mt.token_c_id, mt.token_p_id,
                "Not enough position asset balance"
            );
            let gap_shares = asset_p
                .supplied
                .amount_to_shares(token_p_amount - mt.token_p_amount, true);
            mt.token_c_shares
                .0
                .checked_sub(gap_shares.0)
                .expect("Not enough position asset balance");
            asset_p
                .supplied
                .withdraw(gap_shares, token_p_amount - mt.token_p_amount);
            asset_p.margin_position -= mt.token_p_amount;
            mt.token_p_amount = 0;
        } else {
            asset_p.margin_position -= token_p_amount;
            mt.token_p_amount -= token_p_amount;
        }

        // prepare to close
        mt.is_locking = true;
        self.internal_set_asset(&mt.token_p_id, asset_p);
        // TODO: mt may be needed to change to store in an unorderedmap in user Account

        let event = EventDataMarginDecrease {
            account_id: account.account_id.clone(),
            pos_id: pos_id.clone(),
            liquidator_id: liquidator_id.clone(),
            token_p_id: mt.token_p_id.clone(),
            token_p_amount,
            token_d_id: mt.token_d_id.clone(),
            token_d_amount: min_token_d_amount,
        };

        // step 3: call dex to trade and wait for callback
        // organize swap action
        let swap_ref = SwapReference {
            account_id: account.account_id.clone(),
            pos_id: pos_id.clone(),
            amount_in: token_p_amount.into(),
            op,
            liquidator_id,
        };
        swap_detail.set_client_echo(&swap_ref.to_msg_string());
        let swap_msg = swap_detail.to_msg_string();
        ext_fungible_token::ext(mt.token_p_id.clone())
            .with_attached_deposit(1)
            .with_static_gas(GAS_FOR_FT_TRANSFER_CALL)
            .ft_transfer_call(
                swap_indication.dex_id.clone(),
                U128(ft_p_amount),
                None,
                swap_msg,
            )
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_FOR_FT_TRANSFER_CALL_CALLBACK)
                    .callback_dex_trade(
                        account.account_id.clone(),
                        pos_id.clone(),
                        token_p_amount.into(),
                        pre_token_p_amount.into(),
                        format!("decrease"),
                    ),
            );
        event
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
        pre_token_p_amount: U128,
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
                let mt = account.margin_positions.get(&pos_id).unwrap().clone();
                let mut asset_d = self.internal_unwrap_asset(&mt.token_d_id);
                asset_d.margin_pending_debt -= amount_in.0;
                self.internal_set_asset(&mt.token_d_id, asset_d);
                account.deposit_supply_shares(&mt.token_c_id, &mt.token_c_shares);
                account.margin_positions.remove(&pos_id);
                events::emit::margin_open_failed(&account_id, &pos_id);
                
            } else if op == "decrease" {
                let mut mt = account.margin_positions.get_mut(&pos_id).unwrap();
                let mut asset_p = self.internal_unwrap_asset(&mt.token_p_id);
                let amount_in: Balance = amount_in.into();
                let pre_token_p_amount: Balance = pre_token_p_amount.into();
                if amount_in > pre_token_p_amount {
                    asset_p.margin_position += pre_token_p_amount;
                    // re-deposit those gap to supply as margin
                    let gap = amount_in - pre_token_p_amount;
                    let gap_shares = asset_p.supplied.amount_to_shares(gap, false);
                    asset_p.supplied.deposit(gap_shares, gap);
                    mt.token_c_shares.0 += gap_shares.0;
                } else {
                    asset_p.margin_position += amount_in;
                }
                self.internal_set_asset(&mt.token_p_id, asset_p);
                mt.is_locking = false;
                mt.token_p_amount = pre_token_p_amount;
                events::emit::margin_decrease_failed(&account_id, &pos_id);
                // Note: hashmap item got from a get_mut doesn't need to insert back
                // account.margin_positions.insert(pos_id, mt.clone());
            }
            self.internal_set_margin_account(&account_id, account);
        }
    }
}
