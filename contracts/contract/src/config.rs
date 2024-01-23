use crate::*;

pub const MIN_BOOSTER_MULTIPLIER: u32 = 10000;

/// Contract config
#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
#[serde(crate = "near_sdk::serde")]
pub struct Config {
    /// The account ID of the oracle contract
    pub oracle_account_id: AccountId,

    pub ref_exchange_id: AccountId,

    /// The account ID of the contract owner that allows to modify config, assets and use reserves.
    pub owner_id: AccountId,

    /// The account ID of the booster token contract.
    pub booster_token_id: TokenId,

    /// The number of decimals of the booster fungible token.
    pub booster_decimals: u8,

    /// The total number of different assets
    pub max_num_assets: u32,

    /// The maximum number of seconds expected from the oracle price call.
    pub maximum_recency_duration_sec: DurationSec,

    /// Maximum staleness duration of the price data timestamp.
    /// Because NEAR protocol doesn't implement the gas auction right now, the only reason to
    /// delay the price updates are due to the shard congestion.
    /// This parameter can be updated in the future by the owner.
    pub maximum_staleness_duration_sec: DurationSec,

    pub lp_tokens_info_valid_duration_sec: DurationSec,

    /// The minimum duration to stake booster token in seconds.
    pub minimum_staking_duration_sec: DurationSec,

    /// The maximum duration to stake booster token in seconds.
    pub maximum_staking_duration_sec: DurationSec,

    /// The rate of xBooster for the amount of Booster given for the maximum staking duration.
    /// Assuming the 100% multiplier at the minimum staking duration. Should be no less than 100%.
    /// E.g. 20000 means 200% multiplier (or 2X).
    pub x_booster_multiplier_at_maximum_staking_duration: u32,

    /// Whether an account with bad debt can be liquidated using reserves.
    /// The account should have borrowed sum larger than the collateral sum.
    pub force_closing_enabled: bool,
}

impl Config {
    pub fn assert_valid(&self) {
        assert!(
            self.minimum_staking_duration_sec < self.maximum_staking_duration_sec,
            "The maximum staking duration must be greater than minimum staking duration"
        );
        assert!(
            self.x_booster_multiplier_at_maximum_staking_duration >= MIN_BOOSTER_MULTIPLIER,
            "xBooster multiplier should be no less than 100%"
        );
    }
}

impl Contract {
    pub fn internal_config(&self) -> Config {
        self.config.get().unwrap()
    }

    pub fn get_oracle_account_id(&self) -> AccountId {
        self.internal_config().oracle_account_id.into()
    }

    pub fn assert_owner(&self) {
        assert_eq!(
            &env::predecessor_account_id(),
            &self.internal_config().owner_id,
            "Not an owner"
        );
    }

    pub fn assert_owner_or_guardians(&self) {
        assert!(env::predecessor_account_id() == self.internal_config().owner_id 
            || self.guardians.contains(&env::predecessor_account_id()), "Not allowed");
    }
}

#[near_bindgen]
impl Contract {
    /// Returns the current config.
    pub fn get_config(&self) -> Config {
        self.internal_config()
    }

    /// Updates the current config.
    /// - Requires one yoctoNEAR.
    /// - Requires to be called by the contract owner.
    #[payable]
    pub fn update_config(&mut self, config: Config) {
        assert_one_yocto();
        self.assert_owner();
        config.assert_valid();
        let current_config = self.internal_config();
        if current_config.booster_token_id != config.booster_token_id || 
            current_config.booster_decimals != config.booster_decimals {
            env::panic_str("Can't change booster_token_id/booster_decimals");
        }
        self.config.set(&config);
    }

    /// Adds an asset with a given token_id and a given asset_config.
    /// - Panics if the asset config is invalid.
    /// - Panics if an asset with the given token_id already exists.
    /// - Requires one yoctoNEAR.
    /// - Requires to be called by the contract owner.
    #[payable]
    pub fn add_asset(&mut self, token_id: AccountId, asset_config: AssetConfig) {
        assert_one_yocto();
        asset_config.assert_valid();
        self.assert_owner();
        assert!(self.asset_ids.insert(&token_id));
        self.internal_set_asset(&token_id, Asset::new(env::block_timestamp(), asset_config))
    }

    /// Updates the asset config for the asset with the a given token_id.
    /// - Panics if the asset config is invalid.
    /// - Panics if an asset with the given token_id doesn't exist.
    /// - Requires one yoctoNEAR.
    /// - Requires to be called by the contract owner.
    #[payable]
    pub fn update_asset(&mut self, token_id: AccountId, asset_config: AssetConfig) {
        assert_one_yocto();
        asset_config.assert_valid();
        self.assert_owner();
        let mut asset = self.internal_unwrap_asset(&token_id);
        if asset.config.extra_decimals != asset_config.extra_decimals {
            assert!(
                asset.borrowed.balance == 0 && asset.supplied.balance == 0 && asset.prot_fee == 0 && asset.reserved == 0,
                "Can't change extra decimals if any of the balances are not 0"
            );
        }
        asset.config = asset_config;
        self.internal_set_asset(&token_id, asset);
    }

    /// Updates the min_reserve_shares for the asset_config with the a given token_id.
    /// - Panics if an asset with the given token_id doesn't exist.
    /// - Requires one yoctoNEAR.
    /// - Requires to be called by the contract owner or guardians.
    #[payable]
    pub fn update_asset_config_min_reserve_shares(&mut self, token_id: AccountId, min_reserve_shares: U128) {
        assert_one_yocto();
        self.assert_owner_or_guardians();
        let mut asset = self.internal_unwrap_asset(&token_id);
        asset.config.min_reserve_shares = min_reserve_shares;
        self.internal_set_asset(&token_id, asset);
    }

    /// Updates the prot_ratio for the asset with the a given token_id.
    /// - Panics if the prot_ratio is invalid.
    /// - Panics if an asset with the given token_id doesn't exist.
    /// - Requires one yoctoNEAR.
    /// - Requires to be called by the contract owner or guardians.
    #[payable]
    pub fn update_asset_prot_ratio(&mut self, token_id: AccountId, prot_ratio: u32) {
        assert_one_yocto();
        self.assert_owner_or_guardians();
        assert!(prot_ratio <= MAX_RATIO);
        let mut asset = self.internal_unwrap_asset(&token_id);
        asset.config.prot_ratio = prot_ratio;
        self.internal_set_asset(&token_id, asset);
    }

    /// Updates the capacity for the asset with the a given token_id.
    /// - Panics if the capacity is invalid.
    /// - Panics if an asset with the given token_id doesn't exist.
    /// - Requires one yoctoNEAR.
    /// - The can_withdraw requires to be called by the contract owner.
    /// - The can_deposit、can_use_as_collateral and can_borrow requires to be called by the contract owner or guardians.
    #[payable]
    pub fn update_asset_capacity(&mut self, token_id: AccountId, can_deposit: Option<bool>, can_withdraw: Option<bool>, can_use_as_collateral: Option<bool>, can_borrow: Option<bool>) {
        assert_one_yocto();
        self.assert_owner_or_guardians();
        let mut asset = self.internal_unwrap_asset(&token_id);
        if let Some(can_deposit) = can_deposit {
            asset.config.can_deposit = can_deposit;
        }
        if let Some(can_withdraw) = can_withdraw {
            self.assert_owner();
            asset.config.can_withdraw = can_withdraw;
        }
        if let Some(can_use_as_collateral) = can_use_as_collateral {
            asset.config.can_use_as_collateral = can_use_as_collateral;
        }
        if let Some(can_borrow) = can_borrow {
            asset.config.can_borrow = can_borrow;
        }
        self.internal_set_asset(&token_id, asset);
    }

    /// Updates the net_tvl_multiplier for the asset with the a given token_id.
    /// - Panics if the net_tvl_multiplier is invalid.
    /// - Panics if an asset with the given token_id doesn't exist.
    /// - Requires one yoctoNEAR.
    /// - Requires to be called by the contract owner or guardians.
    #[payable]
    pub fn update_asset_net_tvl_multiplier(&mut self, token_id: AccountId, net_tvl_multiplier: u32) {
        assert_one_yocto();
        self.assert_owner_or_guardians();
        assert!(net_tvl_multiplier <= MAX_RATIO);
        let mut asset = self.internal_unwrap_asset(&token_id);
        asset.config.net_tvl_multiplier = net_tvl_multiplier;
        self.internal_set_asset(&token_id, asset);
    }

    /// Adds an asset farm reward for the farm with a given farm_id. The reward is of token_id with
    /// the new reward per day amount and a new booster log base. The extra amount of reward is
    /// taken from the asset reserved balance.
    /// - The booster log base should include decimals of the token for better precision of the log
    ///    base. For example, if token decimals is `6` the log base of `10_500_000` will be `10.5`.
    /// - Panics if the farm asset token_id doesn't exists.
    /// - Panics if an asset with the given token_id doesn't exists.
    /// - Panics if an asset with the given token_id doesn't have enough reserved balance.
    /// - Requires one yoctoNEAR.
    /// - Requires to be called by the contract owner.
    #[payable]
    pub fn add_asset_farm_reward(
        &mut self,
        farm_id: FarmId,
        reward_token_id: AccountId,
        new_reward_per_day: U128,
        new_booster_log_base: U128,
        reward_amount: U128,
    ) {
        assert_one_yocto();
        self.assert_owner();
        match &farm_id {
            FarmId::Supplied(token_id) | FarmId::Borrowed(token_id) => {
                assert!(self.assets.contains_key(token_id));
            }
            FarmId::NetTvl => {}
        };
        let reward_token_id: TokenId = reward_token_id.into();
        let mut reward_asset = self.internal_unwrap_asset(&reward_token_id);
        assert!(
            reward_asset.reserved >= reward_amount.0
                && reward_asset.available_amount() >= reward_amount.0,
            "Not enough reserved reward balance"
        );
        reward_asset.reserved -= reward_amount.0;
        self.internal_set_asset(&reward_token_id, reward_asset);
        let mut asset_farm = self
            .internal_get_asset_farm(&farm_id, false)
            .unwrap_or_else(|| AssetFarm {
                block_timestamp: env::block_timestamp(),
                rewards: HashMap::new(),
                inactive_rewards: LookupMap::new(StorageKey::InactiveAssetFarmRewards {
                    farm_id: farm_id.clone(),
                }),
            });

        let mut asset_farm_reward = asset_farm
            .rewards
            .remove(&reward_token_id)
            .or_else(|| asset_farm.internal_remove_inactive_asset_farm_reward(&reward_token_id))
            .unwrap_or_default();
        asset_farm_reward.reward_per_day = new_reward_per_day.into();
        asset_farm_reward.booster_log_base = new_booster_log_base.into();
        asset_farm_reward.remaining_rewards += reward_amount.0;
        asset_farm
            .rewards
            .insert(reward_token_id, asset_farm_reward);
        self.internal_set_asset_farm(&farm_id, asset_farm);
    }

    /// Claim prot_fee from asset with the a given token_id.
    /// - Panics if an asset with the given token_id doesn't exist.
    /// - Requires one yoctoNEAR.
    /// - Requires to be called by the contract owner.
    #[payable]
    pub fn claim_prot_fee(&mut self, token_id: AccountId, stdd_amount: Option<U128>) {
        assert_one_yocto();
        self.assert_owner();
        let mut asset = self.internal_unwrap_asset(&token_id);
        let stdd_amount: u128 = stdd_amount.map(|v| v.into()).unwrap_or(asset.prot_fee);
        
        if stdd_amount > 0 {
            asset.prot_fee = asset.prot_fee.checked_sub(stdd_amount).expect("Asset prot_fee balance not enough!");
            self.internal_set_asset(&token_id, asset);

            self.deposit_to_owner(&token_id, stdd_amount);

            events::emit::claim_prot_fee(&self.internal_config().owner_id, stdd_amount, &token_id);
        }
    }

    /// Decrease reserved from asset with the a given token_id.
    /// - Panics if an asset with the given token_id doesn't exist.
    /// - Requires one yoctoNEAR.
    /// - Requires to be called by the contract owner.
    #[payable]
    pub fn decrease_reserved(&mut self, token_id: AccountId, stdd_amount: Option<U128>) {
        assert_one_yocto();
        self.assert_owner_or_guardians();
        let mut asset = self.internal_unwrap_asset(&token_id);
        let stdd_amount: u128 = stdd_amount.map(|v| v.into()).unwrap_or(asset.reserved);
        
        if stdd_amount > 0 {
            asset.reserved = asset.reserved.checked_sub(stdd_amount).expect("Asset reserved balance not enough!");
            self.internal_set_asset(&token_id, asset);

            self.deposit_to_owner(&token_id, stdd_amount);

            if self.guardians.contains(&env::predecessor_account_id()) {
                let asset = self.internal_unwrap_asset(&token_id);
                let reserve_ratio = BigDecimal::from(asset.reserved).div_u128(asset.supplied.balance + asset.reserved);
                let config_reserve_ratio = BigDecimal::from_ratio(asset.config.reserve_ratio);
                assert!(reserve_ratio >= config_reserve_ratio);
            }
            events::emit::decrease_reserved(&self.internal_config().owner_id, stdd_amount, &token_id);
        }
    }

    /// Increase reserved from asset with the a given token_id.
    /// - Panics if an asset with the given token_id doesn't exist.
    /// - Requires one yoctoNEAR.
    /// - Requires to be called by the contract owner.
    #[payable]
    pub fn increase_reserved(&mut self, asset_amount: AssetAmount) {
        assert_one_yocto();
        self.assert_owner();
        let owner_id = self.internal_config().owner_id;
        let mut account = self.internal_unwrap_account(&owner_id);
        let mut account_asset = account.internal_unwrap_asset(&asset_amount.token_id);
        
        let mut asset = self.internal_unwrap_asset(&asset_amount.token_id);
        let (shares, increase_amount) =
            asset_amount_to_shares(&asset.supplied, account_asset.shares, &asset_amount, false);
        
        account_asset.withdraw_shares(shares);
        account.internal_set_asset(&asset_amount.token_id, account_asset);

        asset.supplied.withdraw(shares, increase_amount);
        asset.reserved += increase_amount;
        self.internal_set_asset(&asset_amount.token_id, asset);
        
        self.internal_account_apply_affected_farms(&mut account);
        self.internal_set_account(&owner_id, account);

        events::emit::increase_reserved(&owner_id, increase_amount, &asset_amount.token_id);
    }
}

impl Contract {
    pub fn deposit_to_owner(&mut self, token_id: &AccountId, stdd_amount: u128) {
        let owner_id = self.internal_config().owner_id;
        let mut account = self.internal_unwrap_account(&owner_id);
        self.internal_deposit(&mut account, &token_id, stdd_amount);
        self.internal_account_apply_affected_farms(&mut account);
        self.internal_set_account(&owner_id, account);
    }
}