use crate::*;
use near_sdk::{promise_result_as_success, is_promise_success, PromiseOrValue,serde_json};

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

#[derive(BorshSerialize, BorshDeserialize, Serialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct MarginTradingPosition {
    pub margin_asset: TokenId,
    pub margin_shares: Shares,
    pub debt_asset: TokenId,
    pub debt_shares: Shares,
    pub position_asset: TokenId,
    pub position_amount: Balance,
    // 0 - pre-open, 1 - running, 2 - adjusting
    pub stat: u8,
}

impl MarginTradingPosition {
    fn new(margin_asset: TokenId, debt_asset: TokenId, position_asset: TokenId) -> Self {
        MarginTradingPosition {
            margin_asset,
            margin_shares: U128(0),
            debt_asset,
            debt_shares: U128(0),
            position_asset,
            position_amount: 0,
            stat: 0,  // 0 - running, 1 - pre-open, 2 - adjusting
        }
    }

    pub fn is_empty(&self) -> bool {
        self.margin_shares.0 == 0 && self.debt_shares.0 == 0 && self.position_amount == 0 && self.stat == 0
    }

    pub fn is_no_borrowed(&self) -> bool {
        self.debt_shares.0 == 0
    }

    pub fn increase_collateral(&mut self, token_id: &TokenId, shares: Shares) {
        if self.margin_asset == token_id.clone() {
            self.margin_shares.0 += shares.0
        } else {
            env::panic_str("Margin asset unmatch");
        }
    }

    pub fn decrease_collateral(&mut self, token_id: &TokenId, shares: Shares) {
        if self.margin_asset == token_id.clone() {
            if self.margin_shares.0 >= shares.0 {
                self.margin_shares.0 -= shares.0
            } else {
                env::panic_str("Not enough margin to decrease");
            }
        } else {
            env::panic_str("Margin asset unmatch");
        }
    }

    pub fn increase_borrowed(&mut self, token_id: &TokenId, shares: Shares) {
        if self.debt_asset == token_id.clone() {
            self.debt_shares.0 += shares.0
        } else {
            env::panic_str("Debt asset unmatch");
        }
    }

    pub fn decrease_borrowed(&mut self, token_id: &TokenId, shares: Shares) {
        if self.debt_asset == token_id.clone() {
            if self.debt_shares.0 >= shares.0 {
                self.debt_shares.0 -= shares.0
            } else {
                env::panic_str("Not enough debt to decrease");
            }
        } else {
            env::panic_str("Debt asset unmatch");
        }
    }

    pub fn internal_unwrap_collateral(&self, token_id: &TokenId) -> Shares {
        self.margin_shares
    }

    pub fn internal_unwrap_borrowed(&self, token_id: &TokenId) -> Shares {
        self.debt_shares
    }
}

impl Contract {
    pub(crate) fn get_mtp_collateral_sum_with_volatility_ratio(
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
        )
        .mul_ratio(asset_margin.config.volatility_ratio);

        let asset_position = self.internal_unwrap_asset(&mtp.position_asset);
        let position_adjusted_value = BigDecimal::from_balance_price(
            mtp.position_amount,
            prices.get_unwrap(&mtp.position_asset),
            asset_position.config.extra_decimals,
        )
        .mul_ratio(asset_position.config.volatility_ratio);

        margin_adjusted_value + position_adjusted_value
    }

    pub(crate) fn get_mtp_borrowed_sum_with_volatility_ratio(
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
        .div_ratio(asset.config.volatility_ratio)
    }

    pub(crate) fn get_mtp_profit(
        &self,
        mtp: &MarginTradingPosition,
        prices: &Prices,
    ) -> Option<BigDecimal> {
        None
    }

    pub(crate) fn get_mtp_loss(
        &self,
        mtp: &MarginTradingPosition,
        prices: &Prices,
    ) -> Option<BigDecimal> {
        None
    }

    pub(crate) fn get_mtp_lr(
        &self,
        mtp: &MarginTradingPosition,
        prices: &Prices,
    ) -> Option<BigDecimal> {
        None
    }

    pub(crate) fn get_mtp_hf(
        &self,
        mtp: &MarginTradingPosition,
        prices: &Prices,
    ) -> Option<BigDecimal> {
        None
    }


    pub(crate) fn internal_open_margin(
        &mut self, 
        account: &mut Account,
        margin_id: &AccountId, 
        margin_amount: Balance,
        debt_id: &AccountId, 
        debt_amount: Balance, 
        position_id: &AccountId, 
        min_position_amount: Balance,
        market_route_id: u32,
        prices: &Prices
    ) {
        let pos_id = format!("{}_{}_{}", margin_id, debt_id, position_id);
        assert!(!account.positions.contains_key(&pos_id), "Margin position already exist");

        // step 0: check if pending_debt of debt asset has debt_amount room available
        // pending_debt should less than tbd_x% of available of this asset
        let mut asset_debt = self.internal_unwrap_asset(debt_id);
        let tbd_x = 10_u128;
        assert!(tbd_x * (asset_debt.margin_pending_debt + debt_amount) < asset_debt.available_amount(), "Pending debt will overflow");

        // step 1: create MTP with special status (pre-open)
        let asset_margin = self.internal_unwrap_asset(margin_id);
        let margin_shares = asset_margin.supplied.amount_to_shares(margin_amount, true);

        let mut account_asset = account.internal_unwrap_asset(margin_id);
        account_asset.withdraw_shares(margin_shares);
        account.internal_set_asset(margin_id, account_asset);

        let mut mt = MarginTradingPosition::new(
            margin_id.clone(),
            debt_id.clone(),
            position_id.clone(),
        );
        mt.stat = 1;
        mt.margin_shares = margin_shares;
        mt.debt_shares = asset_debt.margin_debt.amount_to_shares(debt_amount, true);
        mt.position_amount = min_position_amount;

        // step 2: check before call dex to trade
        // check if mt is valid
        let hf_nom = self.get_mtp_collateral_sum_with_volatility_ratio(&mt, prices);
        let hf_den = self.get_mtp_borrowed_sum_with_volatility_ratio(&mt, prices);
        let hf = hf_nom / hf_den;
        let tbd_min_open_margin_hf = 1.05_f64;
        assert!(hf>BigDecimal::from(tbd_min_open_margin_hf), "Invalid health factor");
        // TODO: check if the three tokens are legal for margin

        // step 3: pre-lending 
        mt.debt_shares.0 = 0;
        mt.position_amount = 0;
        asset_debt.margin_pending_debt += debt_amount;
        self.internal_set_asset(debt_id, asset_debt);
        account.positions.insert(pos_id.clone(), Position::MarginTradingPosition(mt));

        // step 4: call dex to trade and wait for callback
        // TODO: organize swap action
        let swap_msg = format!("");
        ext_fungible_token::ext(debt_id.clone())
            .with_attached_deposit(1)
            .with_static_gas(GAS_FOR_FT_TRANSFER_CALL)
            .ft_transfer_call(
                self.get_config().ref_exchange_id.clone(), 
                U128(debt_amount), 
                None, 
                swap_msg
            ).then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_FOR_FT_TRANSFER_CALL_CALLBACK)
                    .callback_dex_trade(account.account_id.clone(), pos_id.clone(), debt_amount.into(), format!("open"))
            );

    }
}

#[near_bindgen]
impl Contract {

    #[private]
    pub fn callback_dex_trade(&mut self, account_id: AccountId, pos_id: String, amount_in: U128, op: String) {
        let cross_call_result = promise_result_as_success().expect("ERR102_CROSS_CONTRACT_FAILED");
        let debt_used = serde_json::from_slice::<U128>(&cross_call_result).unwrap().0;
        if debt_used == 0 {
            // trading failed, revert margin operation
            let mut account = self.internal_unwrap_account(&account_id);
            let mut mt = if let Position::MarginTradingPosition(mt) = account.positions.get(&pos_id).unwrap() {
                mt.clone()
            } else {
                env::panic_str("Invalid position type")
            };
            if op == "open" {
                let mut asset_debt = self.internal_unwrap_asset(&mt.debt_asset);
                asset_debt.margin_pending_debt -= amount_in.0;
                self.internal_set_asset(&mt.debt_asset, asset_debt);

                let mut account_asset = account.internal_unwrap_asset(&mt.margin_asset);
                account_asset.deposit_shares(mt.margin_shares);
                account.internal_set_asset(&mt.margin_asset, account_asset);

                account.positions.remove(&pos_id);
            } else if op == "increase" {
                let mut asset_debt = self.internal_unwrap_asset(&mt.debt_asset);
                asset_debt.margin_pending_debt -= amount_in.0;
                self.internal_set_asset(&mt.debt_asset, asset_debt);

                mt.stat = 0;
                account.positions.insert(pos_id, Position::MarginTradingPosition(mt));
            } else if op == "decrease" {
                let mut asset_position = self.internal_unwrap_asset(&mt.position_asset);
                asset_position.margin_position += amount_in.0;
                self.internal_set_asset(&mt.debt_asset, asset_position);

                mt.stat = 0;
                account.positions.insert(pos_id, Position::MarginTradingPosition(mt));
            }
            self.internal_set_account(&account_id, account);

        }

    }
}
