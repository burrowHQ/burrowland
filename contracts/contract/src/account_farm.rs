use crate::*;

pub const NET_TVL_SHARES_DIVISOR: u128 = 10u128.pow(18);

#[derive(
    BorshSerialize,
    BorshDeserialize,
    Serialize,
    Deserialize,
    Clone,
    Hash,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
#[serde(crate = "near_sdk::serde")]
pub enum FarmId {
    Supplied(TokenId),
    Borrowed(TokenId),
    NetTvl,
    TokenNetBalance(TokenId),
}

/// A data required to keep track of a farm for an account.
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct AccountFarm {
    pub block_timestamp: Timestamp,
    pub rewards: HashMap<TokenId, AccountFarmReward>,
}

impl AccountFarm {
    pub fn new() -> Self {
        Self {
            block_timestamp: 0,
            rewards: HashMap::new(),
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct AccountFarmReward {
    pub boosted_shares: Balance,
    pub last_reward_per_share: BigDecimal,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub enum VAccountFarm {
    Current(AccountFarm),
}

impl From<VAccountFarm> for AccountFarm {
    fn from(v: VAccountFarm) -> Self {
        match v {
            VAccountFarm::Current(c) => c,
        }
    }
}

impl From<AccountFarm> for VAccountFarm {
    fn from(c: AccountFarm) -> Self {
        VAccountFarm::Current(c)
    }
}

impl Contract {
    pub fn get_account_tvl_shares(&self, account: &Account) -> u128 {
        let mut total_supplied: BigDecimal = BigDecimal::zero();
        let mut total_borrowed: BigDecimal = BigDecimal::zero();

        for (token_id, unit_share_tokens) in self.last_lp_token_infos.iter() {
            let token_id = AccountId::new_unchecked(token_id.clone());
            let supplied_shares = account.get_supplied_shares(&token_id);
            if supplied_shares.0 > 0 && 
                unit_share_tokens.tokens.iter().all(|v| self.last_prices.contains_key(&v.token_id)) {
                let asset = self.internal_unwrap_asset(&token_id);
                let unit_share = 10u128.pow(unit_share_tokens.decimals as u32);
                let price = unit_share_tokens.tokens
                    .iter()
                    .fold(BigDecimal::zero(), |sum, unit_share_token_value|{
                        let token_asset = self.internal_unwrap_asset(&unit_share_token_value.token_id);
                        let token_stdd_amount = unit_share_token_value.amount.0 * 10u128.pow(token_asset.config.extra_decimals as u32);
                        let token_balance = u128_ratio(token_stdd_amount, supplied_shares.0, 10u128.pow(asset.config.extra_decimals as u32) * unit_share);
                        sum + BigDecimal::from_balance_price(
                            token_balance,
                            self.last_prices.get(&unit_share_token_value.token_id).unwrap(),
                            token_asset.config.extra_decimals,
                        )
                        .mul_ratio(token_asset.config.net_tvl_multiplier)
                    });
                total_supplied = total_supplied + price.mul_ratio(asset.config.net_tvl_multiplier)
            }
        }

        for (token_id, price) in self.last_prices.iter() {
            let supplied_shares = account.get_supplied_shares(token_id);
            let borrowed_shares = account.get_borrowed_shares(token_id);
            if supplied_shares.0 > 0 || borrowed_shares.0 > 0 {
                let asset = self.internal_unwrap_asset(&token_id);

                if supplied_shares.0 > 0 {
                    let amount =
                        asset.supplied.shares_to_amount(supplied_shares, false);

                    total_supplied = total_supplied
                        + BigDecimal::from_balance_price(
                            amount,
                            price,
                            asset.config.extra_decimals,
                        )
                        .mul_ratio(asset.config.net_tvl_multiplier)
                }

                if borrowed_shares.0 > 0 {
                    let amount = asset.borrowed.shares_to_amount(borrowed_shares, true);

                    total_borrowed = total_borrowed
                        + BigDecimal::from_balance_price(
                            amount,
                            price,
                            asset.config.extra_decimals,
                        )
                        .mul_ratio(asset.config.net_tvl_multiplier)
                }
            }
        }
        if total_supplied > total_borrowed {
            let net_supplied = total_supplied - total_borrowed;
            net_supplied.round_mul_u128(NET_TVL_SHARES_DIVISOR)
        } else {
            0
        }
    }

    pub fn get_account_token_net_balance(&self, account: &Account, token_id: &AccountId) -> u128 {
        let asset = self.internal_unwrap_asset(&token_id);
        let supplied_shares = account.get_supplied_shares(token_id);
        let supplied_amount = asset.supplied.shares_to_amount(supplied_shares, false);
        let borrowed_shares = account.get_borrowed_shares(token_id);
        let borrowed_amount = asset.borrowed.shares_to_amount(borrowed_shares, true);

        supplied_amount.checked_sub(borrowed_amount).unwrap_or(0)
    }

    pub fn internal_account_farm_claim(
        &self,
        account: &Account,
        farm_id: &FarmId,
        asset_farm: &AssetFarm,
    ) -> (
        AccountFarm,
        Vec<(TokenId, Balance)>,
        Vec<(TokenId, Balance)>,
    ) {
        let mut new_rewards = vec![];
        let mut inactive_rewards = vec![];
        let block_timestamp = env::block_timestamp();
        let mut account_farm: AccountFarm = account
            .farms
            .get(farm_id)
            .cloned()
            .unwrap_or_else(AccountFarm::new);
        if account_farm.block_timestamp != block_timestamp {
            account_farm.block_timestamp = block_timestamp;
            let mut old_rewards = std::mem::take(&mut account_farm.rewards);
            for (
                token_id,
                AssetFarmReward {
                    reward_per_share, ..
                },
            ) in &asset_farm.rewards
            {
                let boosted_shares = if let Some(AccountFarmReward {
                    boosted_shares,
                    last_reward_per_share,
                }) = old_rewards.remove(token_id)
                {
                    let diff = reward_per_share.clone() - last_reward_per_share;
                    let amount = diff.round_mul_u128(boosted_shares);
                    if amount > 0 {
                        new_rewards.push((token_id.clone(), amount));
                    }
                    boosted_shares
                } else {
                    0
                };
                account_farm.rewards.insert(
                    token_id.clone(),
                    AccountFarmReward {
                        boosted_shares,
                        last_reward_per_share: reward_per_share.clone(),
                    },
                );
            }
            for (
                token_id,
                AccountFarmReward {
                    boosted_shares,
                    last_reward_per_share,
                },
            ) in old_rewards
            {
                let AssetFarmReward {
                    reward_per_share, ..
                } = asset_farm
                    .internal_get_inactive_asset_farm_reward(&token_id)
                    .unwrap();
                let diff = reward_per_share - last_reward_per_share;
                let amount = diff.round_mul_u128(boosted_shares);
                inactive_rewards.push((token_id.clone(), boosted_shares));
                if amount > 0 {
                    new_rewards.push((token_id, amount));
                }
            }
        }
        (account_farm, new_rewards, inactive_rewards)
    }

    pub fn internal_account_apply_affected_farms(&mut self, account: &mut Account) {
        if account.affected_farms.is_empty() {
            return;
        }
        account.add_affected_farm(FarmId::NetTvl);
        let mut all_rewards: HashMap<TokenId, Balance> = HashMap::new();
        let mut farms = vec![];
        let mut farms_ids: Vec<_> = account.affected_farms.iter().cloned().collect();
        while let Some(farm_id) = farms_ids.pop() {
            if let Some(asset_farm) = self.internal_get_asset_farm(&farm_id, false) {
                let (account_farm, new_rewards, inactive_rewards) =
                    self.internal_account_farm_claim(account, &farm_id, &asset_farm);
                for (token_id, amount) in new_rewards {
                    let new_supplied_farm_id = FarmId::Supplied(token_id.clone());
                    let new_token_net_balance_farm_id = FarmId::TokenNetBalance(token_id.clone());
                    *all_rewards.entry(token_id).or_default() += amount;
                    if account.add_affected_farm(new_supplied_farm_id.clone()) {
                        farms_ids.push(new_supplied_farm_id);
                    }
                    if account.add_affected_farm(new_token_net_balance_farm_id.clone()) {
                        farms_ids.push(new_token_net_balance_farm_id);
                    }
                }
                farms.push((farm_id, account_farm, asset_farm, inactive_rewards));
            }
        }
        for (token_id, &reward) in &all_rewards {
            self.internal_deposit(account, &token_id, reward);
        }
        
        let booster_tokens = read_booster_tokens_from_storage();
        account.sync_booster_policy(&booster_tokens);

        for (farm_id, mut account_farm, mut asset_farm, inactive_rewards) in farms {
            let shares = if self.blacklist_of_farmers.contains(&account.account_id) {
                0
            } else {
                match &farm_id {
                    FarmId::Supplied(token_id) => account.get_supplied_shares(token_id).0,
                    FarmId::Borrowed(token_id) => account.get_borrowed_shares(token_id).0,
                    FarmId::TokenNetBalance(token_id) => self.get_account_token_net_balance(account, token_id),
                    FarmId::NetTvl => self.get_account_tvl_shares(account)
                }
            };
            for (token_id, asset_farm_reward) in asset_farm.rewards.iter_mut() {
                let account_farm_reward = account_farm.rewards.get_mut(token_id).unwrap();
                asset_farm_reward.boosted_shares -= account_farm_reward.boosted_shares;
                if shares > 0 {
                    let extra_shares = get_booster_extra_shares(&account, shares, &asset_farm_reward, &booster_tokens);
                    account_farm_reward.boosted_shares = shares + extra_shares;
                    asset_farm_reward.boosted_shares += account_farm_reward.boosted_shares;
                }
            }
            for (token_id, boosted_shares) in inactive_rewards {
                let mut asset_farm_reward = asset_farm
                    .internal_get_inactive_asset_farm_reward(&token_id)
                    .unwrap();
                asset_farm_reward.boosted_shares -= boosted_shares;
                asset_farm.internal_set_inactive_asset_farm_reward(&token_id, asset_farm_reward);
            }
            self.internal_set_asset_farm(&farm_id, asset_farm);
            if shares > 0 {
                account.farms.insert(farm_id, account_farm);
            } else {
                account.farms.remove(&farm_id);
            }
        }
    }
}

pub fn get_booster_extra_shares(account: &Account, shares: u128, asset_farm_reward: &AssetFarmReward, booster_tokens: &HashMap<TokenId, BoosterTokenInfo>) -> u128 {
    asset_farm_reward.booster_log_bases.iter().filter_map(|(booster_token_id, U128(booster_log_base))| {
        let booster_token_info = booster_tokens.get(booster_token_id)?;
        if !booster_token_info.enable {
            return Some(0);
        }
        
        let booster_staking = account.booster_stakings.get(booster_token_id)?;

        let decimals = booster_token_info.booster_decimals as u32;
        let booster_base = 10u128.pow(decimals) * booster_token_info.boost_suppress_factor;

        if booster_staking.x_booster_amount <= booster_base {
            Some(0)
        } else {
            let log_base = (*booster_log_base as f64) / 10f64.powi(decimals as i32);
            let ratio = booster_staking.x_booster_amount as f64 / booster_base as f64;
            let extra = (shares as f64 * ratio.log(log_base)) as u128;
            Some(extra)
        }
    }).sum::<u128>()
}

#[near_bindgen]
impl Contract {
    /// Claims all unclaimed farm rewards and starts farming new farms.
    /// If the account_id is given, then it claims farms for the given account_id or uses
    /// predecessor_account_id otherwise.
    pub fn account_farm_claim_all(&mut self, account_id: Option<AccountId>) {
        let account_id = account_id.unwrap_or_else(env::predecessor_account_id);
        let mut account = self.internal_unwrap_account(&account_id);
        account
            .affected_farms
            .extend(account.get_all_potential_farms());
        self.internal_account_apply_affected_farms(&mut account);
        self.internal_set_account(&account_id, account);
    }
}
