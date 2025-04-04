use crate::*;

const MAX_POS: u32 = 10000;
const MAX_RATIO: u32 = 10000;

/// Represents an asset config.
/// Example:
/// 25% reserve, 80% target utilization, 12% target APR, 250% max APR, 60% vol
/// no extra decimals, can be deposited, withdrawn, used as a collateral, borrowed
/// JSON:
/// ```json
/// {
///   "reserve_ratio": 2500,
///   "release_ratio": 0,
///   "target_utilization": 8000,
///   "target_utilization_rate": "1000000000003593629036885046",
///   "max_utilization_rate": "1000000000039724853136740579",
///   "volatility_ratio": 6000,
///   "extra_decimals": 0,
///   "can_deposit": true,
///   "can_withdraw": true,
///   "can_use_as_collateral": true,
///   "can_borrow": true,
///   "net_tvl_multiplier": 0,
///   "max_change_rate": None,
///   "supplied_limit": "340282366920938463463374607431768211455",
///   "borrowed_limit": "340282366920938463463374607431768211455",
/// }
/// ```
#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
#[serde(crate = "near_sdk::serde")]
pub struct AssetConfig {
    /// The ratio of interest that is reserved by the protocol (multiplied by 10000).
    /// E.g. 2500 means 25% from borrowed interests goes to the reserve.
    pub reserve_ratio: u32,
    /// The ratio of reserved interest that belongs to the protocol (multiplied by 10000).
    /// E.g. 2500 means 25% from reserved interests goes to the prot.
    pub prot_ratio: u32,
    /// Target utilization ratio (multiplied by 10000).
    /// E.g. 8000 means the protocol targets 80% of assets are borrowed.
    pub target_utilization: u32,
    /// The compounding rate at target utilization ratio.
    /// Use `apr_to_rate.py` script to compute the value for a given APR.
    /// Given as a decimal string. E.g. "1000000000003593629036885046" for 12% APR.
    pub target_utilization_rate: LowU128,
    /// The compounding rate at 100% utilization.
    /// Use `apr_to_rate.py` script to compute the value for a given APR.
    /// Given as a decimal string. E.g. "1000000000039724853136740579" for 250% APR.
    pub max_utilization_rate: LowU128,
    /// The compounding rate when holding a margin position.
    /// Use `apr_to_rate.py` script to compute the value for a given APR.
    /// Given as a decimal string. E.g. "1000000000003593629036885046" for 12% APR.
    pub holding_position_fee_rate: LowU128,
    /// Volatility ratio (multiplied by 10000).
    /// It defines which percentage collateral that covers borrowing as well as which percentage of
    /// borrowed asset can be taken.
    /// E.g. 6000 means 60%. If an account has 100 $ABC in collateral and $ABC is at 10$ per token,
    /// the collateral value is 1000$, but the borrowing power is 60% or $600.
    /// Now if you're trying to borrow $XYZ and it's volatility ratio is 80%, then you can only
    /// borrow less than 80% of $600 = $480 of XYZ before liquidation can begin.
    pub volatility_ratio: u32,
    /// The amount of extra decimals to use for the fungible token. For example, if the asset like
    /// USDT has `6` decimals in the metadata, the `extra_decimals` can be set to `12`, to make the
    /// inner balance of USDT at `18` decimals.
    pub extra_decimals: u8,
    /// Whether the deposits of this assets are enabled.
    pub can_deposit: bool,
    /// Whether the withdrawals of this assets are enabled.
    pub can_withdraw: bool,
    /// Whether this assets can be used as collateral.
    pub can_use_as_collateral: bool,
    /// Whether this assets can be borrowed.
    pub can_borrow: bool,
    /// NetTvl asset multiplier (multiplied by 10000).
    /// Default multiplier is 10000, means the asset weight shouldn't be changed.
    /// Example: a multiplier of 5000 means the asset in TVL should only counted as 50%, e.g. if an
    /// asset is not useful for borrowing, but only useful as a collateral.
    pub net_tvl_multiplier: u32,
    /// The price change obtained in two consecutive retrievals cannot exceed this ratio.
    pub max_change_rate: Option<u32>,
    /// Allowed supplied upper limit of assets
    pub supplied_limit: Option<U128>,
    /// Allowed borrowed upper limit of assets
    pub borrowed_limit: Option<U128>,
    /// Allowed minimum borrowed amount
    pub min_borrowed_amount: Option<U128>,
}

impl AssetConfig {
    pub fn assert_valid(&self) {
        assert!(self.reserve_ratio <= MAX_RATIO);
        assert!(self.prot_ratio <= MAX_RATIO);
        assert!(self.target_utilization < MAX_POS);
        assert!(self.target_utilization_rate.0 >= BIG_DIVISOR, "Invalid target_utilization_rate");
        assert!(self.max_utilization_rate.0 >= BIG_DIVISOR, "Invalid max_utilization_rate");
        assert!(self.holding_position_fee_rate.0 >= BIG_DIVISOR, "Invalid holding_position_fee_rate");
        assert!(self.target_utilization_rate.0 <= self.max_utilization_rate.0);
        // The volatility ratio can't be 100% to avoid free liquidations of such assets.
        assert!(self.volatility_ratio < MAX_RATIO);
        assert!(self.net_tvl_multiplier <= MAX_RATIO);
        assert!(self.max_change_rate.is_none() || self.max_change_rate.unwrap() <= MAX_RATIO);
        assert!(self.supplied_limit.is_some());
        assert!(self.borrowed_limit.is_some());
        assert!(self.borrowed_limit.unwrap() <= self.supplied_limit.unwrap());
        assert!(self.min_borrowed_amount.is_some());
    }

    pub fn get_rate(
        &self,
        borrowed_balance: Balance,
        total_supplied_balance: Balance,
    ) -> BigDecimal {
        if total_supplied_balance == 0 {
            BigDecimal::one()
        } else {
            let pos = BigDecimal::from(borrowed_balance).div_u128(total_supplied_balance);
            let target_utilization = BigDecimal::from_ratio(self.target_utilization);
            if pos < target_utilization {
                BigDecimal::one()
                    + pos * (BigDecimal::from(self.target_utilization_rate) - BigDecimal::one())
                        / target_utilization
            } else {
                BigDecimal::from(self.target_utilization_rate)
                    + (pos - target_utilization)
                        * (BigDecimal::from(self.max_utilization_rate)
                            - BigDecimal::from(self.target_utilization_rate))
                        / BigDecimal::from_ratio(MAX_POS - self.target_utilization)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ONE_NEAR: u128 = 10u128.pow(24);

    fn test_config() -> AssetConfig {
        AssetConfig {
            reserve_ratio: 2500,
            prot_ratio: 0,
            target_utilization: 8000,
            target_utilization_rate: 1000000000003593629036885046u128.into(),
            max_utilization_rate: 1000000000039724853136740579u128.into(),
            holding_position_fee_rate: U128(1000000000000000000000000000),
            volatility_ratio: 6000,
            extra_decimals: 0,
            can_deposit: true,
            can_withdraw: true,
            can_use_as_collateral: true,
            can_borrow: true,
            net_tvl_multiplier: 10000,
            max_change_rate: None,
            supplied_limit: None,
            borrowed_limit: None,
            min_borrowed_amount: None,
        }
    }

    #[test]
    fn test_get_rate_and_apr() {
        let config = test_config();
        let rate = config.get_rate(81 * ONE_NEAR, 100 * ONE_NEAR);
        println!("Rate: {}", rate);

        let apr = rate.pow(MS_PER_YEAR) - BigDecimal::one();
        println!("APR: {}", apr)
    }
}
