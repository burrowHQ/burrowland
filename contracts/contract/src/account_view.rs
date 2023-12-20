use crate::*;

#[derive(Serialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, Deserialize))]
#[serde(crate = "near_sdk::serde")]
pub struct AssetView {
    pub token_id: TokenId,
    #[serde(with = "u128_dec_format")]
    pub balance: Balance,
    /// The number of shares this account holds in the corresponding asset pool
    pub shares: Shares,
    /// The current APR for this asset (either supply or borrow APR).
    pub apr: BigDecimal,
}

#[derive(Serialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, Deserialize))]
#[serde(crate = "near_sdk::serde")]
pub struct AccountDetailedView {
    pub account_id: AccountId,
    /// A list of assets that are supplied by the account (but not used a collateral).
    pub supplied: Vec<AssetView>,
    /// A list of assets that are used as a collateral.
    pub collateral: Vec<AssetView>,
    /// A list of assets that are borrowed.
    pub borrowed: Vec<AssetView>,
    /// Account farms
    pub farms: Vec<AccountFarmView>,
    /// Whether the account has assets, that can be farmed.
    pub has_non_farmed_assets: bool,
    /// Staking of booster token.
    pub booster_staking: Option<BoosterStaking>,
}

#[derive(Serialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, Deserialize))]
#[serde(crate = "near_sdk::serde")]
pub struct AccountFarmView {
    pub farm_id: FarmId,
    pub rewards: Vec<AccountFarmRewardView>,
}

#[derive(Serialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, Deserialize))]
#[serde(crate = "near_sdk::serde")]
pub struct AccountFarmRewardView {
    pub reward_token_id: TokenId,
    pub asset_farm_reward: AssetFarmReward,
    #[serde(with = "u128_dec_format")]
    pub boosted_shares: Balance,
    #[serde(with = "u128_dec_format")]
    pub unclaimed_amount: Balance,
}

#[derive(Serialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, Deserialize))]
#[serde(crate = "near_sdk::serde")]
pub enum EPositionView {
    RegularPosition(PositionView),
    LPTokenPosition(PositionView),
    MarginTradingPosition(MTPositionView),
}

#[derive(Serialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, Deserialize))]
#[serde(crate = "near_sdk::serde")]
pub struct PositionView {
    pub collateral: Vec<AssetView>,
    pub borrowed: Vec<AssetView>,
}

#[derive(Serialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, Deserialize))]
#[serde(crate = "near_sdk::serde")]
pub struct MTPositionView {
    pub margin_token_id: TokenId,
    pub margin_shares: Shares,
    pub debt_token_id: TokenId,
    pub debt_shares: Shares,
    pub position_token_id: TokenId,
    #[serde(with = "u128_dec_format")]
    pub position_balance: Balance,
    pub stat: u8,
}

#[derive(Serialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, Deserialize))]
#[serde(crate = "near_sdk::serde")]
pub struct AccountAllPositionsDetailedView {
    pub account_id: AccountId,
    /// A list of assets that are supplied by the account (but not used a collateral).
    pub supplied: Vec<AssetView>,
    pub positions: HashMap<String, EPositionView>,
    /// Account farms
    pub farms: Vec<AccountFarmView>,
    /// Whether the account has assets, that can be farmed.
    pub has_non_farmed_assets: bool,
    /// Staking of booster token.
    pub booster_staking: Option<BoosterStaking>,
    pub is_locked: bool
}

impl Contract {
    pub fn account_into_regular_position_detailed_view(&self, account: Account) -> AccountDetailedView {
        let mut potential_farms = account.get_all_potential_farms();
        let farms = account
            .farms
            .keys()
            .cloned()
            .map(|farm_id| {
                // Remove already active farm.
                potential_farms.remove(&farm_id);
                let mut asset_farm = self.internal_unwrap_asset_farm(&farm_id, true);
                let (account_farm, new_rewards, inactive_rewards) =
                    self.internal_account_farm_claim(&account, &farm_id, &asset_farm);
                AccountFarmView {
                    farm_id,
                    rewards: account_farm
                        .rewards
                        .into_iter()
                        .map(|(token_id, AccountFarmReward { boosted_shares, .. })| {
                            (token_id, boosted_shares)
                        })
                        .chain(inactive_rewards)
                        .map(|(reward_token_id, boosted_shares)| {
                            let asset_farm_reward = asset_farm
                                .rewards
                                .remove(&reward_token_id)
                                .or_else(|| {
                                    asset_farm
                                        .internal_get_inactive_asset_farm_reward(&reward_token_id)
                                })
                                .unwrap();
                            let unclaimed_amount = new_rewards
                                .iter()
                                .find(|(token_id, _)| token_id == &reward_token_id)
                                .map(|(_, amount)| *amount)
                                .unwrap_or(0);
                            AccountFarmRewardView {
                                reward_token_id,
                                asset_farm_reward,
                                boosted_shares,
                                unclaimed_amount,
                            }
                        })
                        .collect(),
                }
            })
            .collect();
        if potential_farms.contains(&FarmId::NetTvl) && self.get_account_tvl_shares(&account) == 0 {
            potential_farms.remove(&FarmId::NetTvl);
        }
        // Check whether some asset can be farmed, but not farming yet.
        let has_non_farmed_assets = potential_farms
            .into_iter()
            .any(|farm_id| self.asset_farms.contains_key(&farm_id));
        let position_info = if let Some(Position::RegularPosition(position_info)) = account.positions.get(&REGULAR_POSITION.to_string()) {
            position_info.clone()
        } else {
            RegularPosition::default()
        };
        AccountDetailedView {
            account_id: account.account_id,
            supplied: account
                .supplied
                .into_iter()
                .map(|(token_id, shares)| self.get_asset_view(token_id, shares, false))
                .collect(),
            collateral: position_info
                .collateral
                .into_iter()
                .map(|(token_id, shares)| self.get_asset_view(token_id, shares, false))
                .collect(),
            borrowed: position_info
                .borrowed
                .into_iter()
                .map(|(token_id, shares)| self.get_asset_view(token_id, shares, true))
                .collect(),
            farms,
            has_non_farmed_assets,
            booster_staking: account.booster_staking,
        }
    }

    pub fn account_into_all_positions_detailed_view(&self, account: Account) -> AccountAllPositionsDetailedView {
        let mut potential_farms = account.get_all_potential_farms();
        let farms = account
            .farms
            .keys()
            .cloned()
            .map(|farm_id| {
                // Remove already active farm.
                potential_farms.remove(&farm_id);
                let mut asset_farm = self.internal_unwrap_asset_farm(&farm_id, true);
                let (account_farm, new_rewards, inactive_rewards) =
                    self.internal_account_farm_claim(&account, &farm_id, &asset_farm);
                AccountFarmView {
                    farm_id,
                    rewards: account_farm
                        .rewards
                        .into_iter()
                        .map(|(token_id, AccountFarmReward { boosted_shares, .. })| {
                            (token_id, boosted_shares)
                        })
                        .chain(inactive_rewards)
                        .map(|(reward_token_id, boosted_shares)| {
                            let asset_farm_reward = asset_farm
                                .rewards
                                .remove(&reward_token_id)
                                .or_else(|| {
                                    asset_farm
                                        .internal_get_inactive_asset_farm_reward(&reward_token_id)
                                })
                                .unwrap();
                            let unclaimed_amount = new_rewards
                                .iter()
                                .find(|(token_id, _)| token_id == &reward_token_id)
                                .map(|(_, amount)| *amount)
                                .unwrap_or(0);
                            AccountFarmRewardView {
                                reward_token_id,
                                asset_farm_reward,
                                boosted_shares,
                                unclaimed_amount,
                            }
                        })
                        .collect(),
                }
            })
            .collect();
        if potential_farms.contains(&FarmId::NetTvl) && self.get_account_tvl_shares(&account) == 0 {
            potential_farms.remove(&FarmId::NetTvl);
        }
        // Check whether some asset can be farmed, but not farming yet.
        let has_non_farmed_assets = potential_farms
            .into_iter()
            .any(|farm_id| self.asset_farms.contains_key(&farm_id));
        AccountAllPositionsDetailedView {
            account_id: account.account_id,
            supplied: account
                .supplied
                .into_iter()
                .map(|(token_id, shares)| self.get_asset_view(token_id, shares, false))
                .collect(),
            positions: account
                .positions
                .into_iter()
                .map(|(position, position_info)| {
                    let position_view = match position_info {
                        Position::RegularPosition(regular_position) => {
                            EPositionView::RegularPosition(
                                PositionView {
                                    collateral: regular_position
                                        .collateral
                                        .into_iter()
                                        .map(|(token_id, shares)| self.get_asset_view(token_id, shares, false))
                                        .collect(),
                                    borrowed: regular_position
                                        .borrowed
                                        .into_iter()
                                        .map(|(token_id, shares)| self.get_asset_view(token_id, shares, true))
                                        .collect()
                                }
                            ) 
                        }
                        Position::LPTokenPosition(lp_token_position) => {
                            EPositionView::LPTokenPosition(
                                PositionView {
                                    collateral: vec![
                                        self.get_asset_view(AccountId::new_unchecked(lp_token_position.lpt_id), lp_token_position.collateral, false)
                                    ],
                                    borrowed: lp_token_position
                                        .borrowed
                                        .into_iter()
                                        .map(|(token_id, shares)| self.get_asset_view(token_id, shares, true))
                                        .collect()
                                }
                            )
                        }
                        Position::MarginTradingPosition(mt_position) => {
                            EPositionView::MarginTradingPosition(
                                MTPositionView {
                                    margin_token_id: mt_position.margin_asset,
                                    margin_shares: mt_position.margin_shares,
                                    debt_token_id: mt_position.debt_asset,
                                    debt_shares: mt_position.debt_shares,
                                    position_token_id: mt_position.position_asset,
                                    position_balance: mt_position.position_amount,
                                    stat: mt_position.stat
                                }
                            )
                        }
                    };
                    (position, position_view)
                })
                .collect(),
            farms,
            has_non_farmed_assets,
            booster_staking: account.booster_staking,
            is_locked: account.is_locked
        }
    }

    fn get_asset_view(&self, token_id: TokenId, shares: Shares, is_borrowing: bool) -> AssetView {
        let asset = self.internal_unwrap_asset(&token_id);
        let apr = if is_borrowing {
            asset.get_borrow_apr()
        } else {
            asset.get_supply_apr()
        };
        let balance = if is_borrowing {
            asset.borrowed.shares_to_amount(shares, true)
        } else {
            asset.supplied.shares_to_amount(shares, false)
        };

        AssetView {
            token_id,
            balance,
            shares,
            apr,
        }
    }
}
