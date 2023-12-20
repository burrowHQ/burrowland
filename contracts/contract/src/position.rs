use crate::*;

#[derive(BorshSerialize, BorshDeserialize, Serialize, Clone, Default)]
#[serde(crate = "near_sdk::serde")]
pub struct RegularPosition {
    pub collateral: HashMap<TokenId, Shares>,
    pub borrowed: HashMap<TokenId, Shares>,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct LPTokenPosition {
    pub lpt_id: String,
    pub collateral: Shares,
    pub borrowed: HashMap<TokenId, Shares>,
}

impl LPTokenPosition {
    fn new(lpt_id: String) -> Self {
        LPTokenPosition {
            lpt_id,
            collateral: U128(0),
            borrowed: HashMap::new()
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub enum Position {
    RegularPosition(RegularPosition),
    LPTokenPosition(LPTokenPosition),
    MarginTradingPosition(MarginTradingPosition),
}

impl Position {

    pub fn new(position: &String) -> Self {
        if position.starts_with(SHADOW_V1_TOKEN_PREFIX) {
            Position::LPTokenPosition(LPTokenPosition::new(position.clone()))
        } else {
            Position::RegularPosition(RegularPosition::default())
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Position::RegularPosition(regular_position) => {
                regular_position.collateral.is_empty() && regular_position.borrowed.is_empty()
            }
            Position::LPTokenPosition(lp_token_position) => {
                lp_token_position.collateral.0 == 0 && lp_token_position.borrowed.is_empty()
            }
            Position::MarginTradingPosition(mt_position) => {
                mt_position.is_empty()
            }
        }
    }

    pub fn is_no_borrowed(&self) -> bool {
        match self {
            Position::RegularPosition(regular_position) => {
                regular_position.borrowed.is_empty()
            }
            Position::LPTokenPosition(lp_token_position) => {
                lp_token_position.borrowed.is_empty()
            }
            Position::MarginTradingPosition(mt_position) => {
                mt_position.is_no_borrowed()
            }
        }
    }

    pub fn increase_collateral(&mut self, token_id: &TokenId, shares: Shares){
        match self {
            Position::RegularPosition(regular_position) => {
                regular_position.collateral
                    .entry(token_id.clone())
                    .or_insert_with(|| 0.into())
                    .0 += shares.0;
            }
            Position::LPTokenPosition(lp_token_position) => {
                lp_token_position.collateral = U128(lp_token_position.collateral.0 + shares.0);
            }
            Position::MarginTradingPosition(mt_position) => {
                mt_position.increase_collateral(token_id, shares)
            }
        }
    }

    pub fn decrease_collateral(&mut self, token_id: &TokenId, shares: Shares) {
        match self {
            Position::RegularPosition(regular_position) => {
                let current_collateral = regular_position.collateral.get(token_id).unwrap();
                if let Some(new_balance) = current_collateral.0.checked_sub(shares.0) {
                    if new_balance > 0 {
                        regular_position.collateral
                            .insert(token_id.clone(), Shares::from(new_balance));
                    } else {
                        regular_position.collateral.remove(token_id);
                    }
                } else {
                    env::panic_str("Not enough collateral balance");
                }
            }
            Position::LPTokenPosition(lp_token_position) => {
                lp_token_position.collateral = U128(lp_token_position.collateral.0 - shares.0);
            }
            Position::MarginTradingPosition(mt_position) => {
                mt_position.decrease_collateral(token_id, shares)
            }
        }
    }

    pub fn increase_borrowed(&mut self, token_id: &TokenId, shares: Shares) {
        match self {
            Position::RegularPosition(regular_position) => {
                regular_position.borrowed
                    .entry(token_id.clone())
                    .or_insert_with(|| 0.into())
                    .0 += shares.0;
            }
            Position::LPTokenPosition(lp_token_position) => {
                lp_token_position.borrowed
                    .entry(token_id.clone())
                    .or_insert_with(|| 0.into())
                    .0 += shares.0;
            }
            Position::MarginTradingPosition(mt_position) => {
                mt_position.increase_borrowed(token_id, shares)
            }
        }
    }

    pub fn decrease_borrowed(&mut self, token_id: &TokenId, shares: Shares) {
        match self {
            Position::RegularPosition(regular_position) => {
                let current_borrowed = regular_position.borrowed.get(token_id).unwrap();
                if let Some(new_balance) = current_borrowed.0.checked_sub(shares.0) {
                    if new_balance > 0 {
                        regular_position.borrowed
                            .insert(token_id.clone(), Shares::from(new_balance));
                    } else {
                        regular_position.borrowed.remove(token_id);
                    }
                } else {
                    env::panic_str("Not enough borrowed balance");
                }
            }
            Position::LPTokenPosition(lp_token_position) => {
                let current_borrowed = lp_token_position.borrowed.get(token_id).unwrap();
                if let Some(new_balance) = current_borrowed.0.checked_sub(shares.0) {
                    if new_balance > 0 {
                        lp_token_position.borrowed
                            .insert(token_id.clone(), Shares::from(new_balance));
                    } else {
                        lp_token_position.borrowed.remove(token_id);
                    }
                } else {
                    env::panic_str("Not enough borrowed balance");
                }
            }
            Position::MarginTradingPosition(mt_position) => {
                mt_position.decrease_borrowed(token_id, shares)
            }
        }
    }

    pub fn internal_unwrap_collateral(&self, token_id: &TokenId) -> Shares {
        match self {
            Position::RegularPosition(regular_position) => {
                *regular_position
                    .collateral
                    .get(token_id)
                    .expect("Collateral asset not found")
            }
            Position::LPTokenPosition(lp_token_position) => {
                lp_token_position.collateral
            }
            Position::MarginTradingPosition(mt_position) => {
                mt_position.internal_unwrap_collateral(token_id)
            }
        }
    }
    pub fn internal_unwrap_borrowed(&self, token_id: &TokenId) -> Shares {
        match self {
            Position::RegularPosition(regular_position) => {
                *regular_position
                    .borrowed
                    .get(token_id)
                    .expect("Borrowed asset not found")
            }
            Position::LPTokenPosition(lp_token_position) => {
                *lp_token_position
                    .borrowed
                    .get(token_id)
                    .expect("Borrowed asset not found")
            }
            Position::MarginTradingPosition(mt_position) => {
                mt_position.internal_unwrap_borrowed(token_id)
            }
        }
    }
}

impl Contract {
    pub fn get_collateral_sum_with_volatility_ratio(&self, position_info: &Position, prices: &Prices) -> BigDecimal {
        match position_info {
            Position::RegularPosition(regular_position) => {
                regular_position
                .collateral
                .iter()
                .fold(BigDecimal::zero(), |sum, (token_id, shares)| {
                    let asset = self.internal_unwrap_asset(&token_id);
                    let balance = asset.supplied.shares_to_amount(*shares, false);
                    sum + BigDecimal::from_balance_price(
                        balance,
                        prices.get_unwrap(&token_id),
                        asset.config.extra_decimals,
                    )
                    .mul_ratio(asset.config.volatility_ratio)
                })
            }
            Position::LPTokenPosition(lp_token_position) => {
                let collateral_asset = self.internal_unwrap_asset(&AccountId::new_unchecked(lp_token_position.lpt_id.clone()));
                let collateral_shares = lp_token_position.collateral;
                let collateral_balance = collateral_asset.supplied.shares_to_amount(collateral_shares, false);
                let unit_share_tokens = self.last_lp_token_infos.get(&lp_token_position.lpt_id).expect("lp_token_infos not found");
                let config = self.internal_config();
                assert!(env::block_timestamp() - unit_share_tokens.timestamp <= to_nano(config.lp_tokens_info_valid_duration_sec), "LP token info timestamp is too stale");
                let unit_share = 10u128.pow(unit_share_tokens.decimals as u32);
                unit_share_tokens.tokens
                    .iter()
                    .fold(BigDecimal::zero(), |sum, unit_share_token_value|{
                        let token_asset = self.internal_unwrap_asset(&unit_share_token_value.token_id);
                        let token_stdd_amount = unit_share_token_value.amount.0 * 10u128.pow(token_asset.config.extra_decimals as u32);
                        let token_balance = u128_ratio(token_stdd_amount, collateral_balance, 10u128.pow(collateral_asset.config.extra_decimals as u32) * unit_share);
                        sum + BigDecimal::from_balance_price(
                            token_balance,
                            prices.get_unwrap(&unit_share_token_value.token_id),
                            token_asset.config.extra_decimals,
                        )
                        .mul_ratio(token_asset.config.volatility_ratio)
                    }).mul_ratio(collateral_asset.config.volatility_ratio)
            }
            Position::MarginTradingPosition(mt_position) => {
                self.get_mtp_collateral_sum_with_volatility_ratio(mt_position, prices)
            }
        }
    }

    pub fn get_borrowed_sum_with_volatility_ratio(&self, position_info: &Position, prices: &Prices) -> BigDecimal {
        match position_info {
            Position::RegularPosition(regular_position) => {
                regular_position
                .borrowed
                .iter()
                .fold(BigDecimal::zero(), |sum, (token_id, shares)| {
                    let asset = self.internal_unwrap_asset(&token_id);
                    let balance = asset.borrowed.shares_to_amount(*shares, true);
                    sum + BigDecimal::from_balance_price(
                        balance,
                        prices.get_unwrap(&token_id),
                        asset.config.extra_decimals,
                    )
                    .div_ratio(asset.config.volatility_ratio)
                })
            }
            Position::LPTokenPosition(lp_token_position) => {
                lp_token_position
                .borrowed
                .iter()
                .fold(BigDecimal::zero(), |sum, (token_id, shares)| {
                    let asset = self.internal_unwrap_asset(&token_id);
                    let balance = asset.borrowed.shares_to_amount(*shares, true);
                    sum + BigDecimal::from_balance_price(
                        balance,
                        prices.get_unwrap(&token_id),
                        asset.config.extra_decimals,
                    )
                    .div_ratio(asset.config.volatility_ratio)
                })
            }
            Position::MarginTradingPosition(mt_position) => {
                self.get_mtp_borrowed_sum_with_volatility_ratio(mt_position, prices)
            }
        }
    }
}