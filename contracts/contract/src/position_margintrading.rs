use crate::*;

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
}
