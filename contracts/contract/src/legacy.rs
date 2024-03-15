use crate::*;

/// Default multiplier for Net TVL farming. Equals to 1.
const DEFAULT_NET_TVL_MULTIPLIER: u32 = 10000;

#[derive(BorshSerialize, BorshDeserialize)]
pub struct CollateralAsset {
    pub token_id: TokenId,
    pub shares: Shares,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct BorrowedAsset {
    pub token_id: TokenId,
    pub shares: Shares,
}
/// V0 legacy version of Account structure, before staking of the burrow token was introduced.
#[derive(BorshSerialize, BorshDeserialize)]
pub struct AccountV0 {
    /// A copy of an account ID. Saves one storage_read when iterating on accounts.
    pub account_id: AccountId,
    /// A list of assets that are supplied by the account (but not used a collateral).
    /// It's not returned for account pagination.
    pub supplied: UnorderedMap<TokenId, VAccountAsset>,
    /// A list of collateral assets.
    pub collateral: Vec<CollateralAsset>,
    /// A list of borrowed assets.
    pub borrowed: Vec<BorrowedAsset>,
    /// Keeping track of data required for farms for this account.
    pub farms: UnorderedMap<FarmId, VAccountFarm>,
}

impl From<AccountV0> for AccountV1 {
    fn from(a: AccountV0) -> Self {
        let AccountV0 {
            account_id,
            supplied,
            collateral,
            borrowed,
            farms,
        } = a;
        Self {
            account_id,
            supplied,
            collateral,
            borrowed,
            farms,
            booster_staking: None,
        }
    }
}

impl AccountV0 {
    pub fn into_account(self, is_view: bool) -> Account {
        let v1: AccountV1 = self.into();
        v1.into_account(is_view)
    }
}

/// V1 legacy version of Account structure, before staking of the burrow token was introduced.
#[derive(BorshSerialize, BorshDeserialize)]
pub struct AccountV1 {
    /// A copy of an account ID. Saves one storage_read when iterating on accounts.
    pub account_id: AccountId,
    /// A list of assets that are supplied by the account (but not used a collateral).
    /// It's not returned for account pagination.
    pub supplied: UnorderedMap<TokenId, VAccountAsset>,
    /// A list of collateral assets.
    pub collateral: Vec<CollateralAsset>,
    /// A list of borrowed assets.
    pub borrowed: Vec<BorrowedAsset>,
    /// Keeping track of data required for farms for this account.
    pub farms: UnorderedMap<FarmId, VAccountFarm>,
    /// Staking of booster token.
    pub booster_staking: Option<BoosterStaking>,
}

impl AccountV1 {
    pub fn into_account(self, is_view: bool) -> Account {
        let AccountV1 {
            account_id,
            supplied: mut supplied_unordered_map,
            collateral: collateral_vec,
            borrowed: borrowed_vec,
            farms: mut farms_unordered_map,
            booster_staking,
        } = self;
        let affected_farms = Default::default();
        let mut storage_tracker: StorageTracker = Default::default();
        // When is_view we can't touch/clean up storage.
        let supplied = supplied_unordered_map
            .iter()
            .map(|(key, value)| {
                let AccountAsset { shares } = value.into();
                (key, shares)
            })
            .collect();
        let collateral = collateral_vec
            .into_iter()
            .map(|c| (c.token_id, c.shares))
            .collect();
        let borrowed = borrowed_vec
            .into_iter()
            .map(|b| (b.token_id, b.shares))
            .collect();
        let farms = farms_unordered_map
            .iter()
            .map(|(key, value)| (key, value.into()))
            .collect();
        // Clearing persistent storage if this is not a view call.
        if !is_view {
            storage_tracker.start();
            supplied_unordered_map.clear();
            farms_unordered_map.clear();
            storage_tracker.stop();
        }
        Account {
            account_id,
            supplied,
            positions: HashMap::from([(REGULAR_POSITION.to_string(), Position::RegularPosition(RegularPosition { collateral, borrowed }))]),
            farms,
            affected_farms,
            storage_tracker,
            booster_staking,
            is_locked: false
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct AccountV2 {
    /// A copy of an account ID. Saves one storage_read when iterating on accounts.
    pub account_id: AccountId,
    /// A list of assets that are supplied by the account (but not used a collateral).
    /// It's not returned for account pagination.
    pub supplied: HashMap<TokenId, Shares>,
    /// A list of collateral assets.
    pub collateral: HashMap<TokenId, Shares>,
    /// A list of borrowed assets.
    pub borrowed: HashMap<TokenId, Shares>,
    /// Keeping track of data required for farms for this account.
    pub farms: HashMap<FarmId, AccountFarm>,
    #[borsh_skip]
    pub affected_farms: std::collections::HashSet<FarmId>,
    /// Tracks changes in storage usage by persistent collections in this account.
    #[borsh_skip]
    pub storage_tracker: StorageTracker,
    /// Staking of booster token.
    pub booster_staking: Option<BoosterStaking>,
}

impl AccountV2 {
    pub fn into_account(self) -> Account {
        let AccountV2 {
            account_id,
            supplied,
            collateral,
            borrowed,
            farms,
            affected_farms,
            storage_tracker,
            booster_staking,
        } = self;
        Account {
            account_id,
            supplied,
            positions: HashMap::from([(REGULAR_POSITION.to_string(), Position::RegularPosition(RegularPosition { collateral, borrowed }))]),
            farms,
            affected_farms,
            storage_tracker,
            booster_staking,
            is_locked: false
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct AssetConfigV0 {
    /// The ratio of interest that is reserved by the protocol (multiplied by 10000).
    /// E.g. 2500 means 25% from borrowed interests goes to the reserve.
    pub reserve_ratio: u32,
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
}

impl From<AssetConfigV0> for AssetConfig {
    fn from(a: AssetConfigV0) -> Self {
        let AssetConfigV0 {
            reserve_ratio,
            target_utilization,
            target_utilization_rate,
            max_utilization_rate,
            volatility_ratio,
            extra_decimals,
            can_deposit,
            can_withdraw,
            can_use_as_collateral,
            can_borrow,
        } = a;
        Self {
            reserve_ratio,
            prot_ratio: 0,
            target_utilization,
            target_utilization_rate,
            max_utilization_rate,
            volatility_ratio,
            extra_decimals,
            can_deposit,
            can_withdraw,
            can_use_as_collateral,
            can_borrow,
            net_tvl_multiplier: DEFAULT_NET_TVL_MULTIPLIER,
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct AssetV0 {
    /// Total supplied including collateral, but excluding reserved.
    pub supplied: Pool,
    /// Total borrowed.
    pub borrowed: Pool,
    /// The amount reserved for the stability. This amount can also be borrowed and affects
    /// borrowing rate.
    pub reserved: Balance,
    /// When the asset was last updated. It's always going to be the current block timestamp.
    pub last_update_timestamp: Timestamp,
    /// The asset config.
    pub config: AssetConfigV0,
}

impl From<AssetV0> for Asset {
    fn from(a: AssetV0) -> Self {
        let AssetV0 {
            supplied,
            borrowed,
            reserved,
            last_update_timestamp,
            config,
        } = a;
        Self {
            supplied,
            borrowed,
            reserved,
            prot_fee: 0,
            last_update_timestamp,
            config: config.into(),
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct AssetConfigV1 {
    /// The ratio of interest that is reserved by the protocol (multiplied by 10000).
    /// E.g. 2500 means 25% from borrowed interests goes to the reserve.
    pub reserve_ratio: u32,
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
}

impl From<AssetConfigV1> for AssetConfig {
    fn from(a: AssetConfigV1) -> Self {
        let AssetConfigV1 {
            reserve_ratio,
            target_utilization,
            target_utilization_rate,
            max_utilization_rate,
            volatility_ratio,
            extra_decimals,
            can_deposit,
            can_withdraw,
            can_use_as_collateral,
            can_borrow,
            net_tvl_multiplier,
        } = a;
        Self {
            reserve_ratio,
            prot_ratio: 0,
            target_utilization,
            target_utilization_rate,
            max_utilization_rate,
            volatility_ratio,
            extra_decimals,
            can_deposit,
            can_withdraw,
            can_use_as_collateral,
            can_borrow,
            net_tvl_multiplier,
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct AssetV1 {
    /// Total supplied including collateral, but excluding reserved.
    pub supplied: Pool,
    /// Total borrowed.
    pub borrowed: Pool,
    /// The amount reserved for the stability. This amount can also be borrowed and affects
    /// borrowing rate.
    pub reserved: Balance,
    /// When the asset was last updated. It's always going to be the current block timestamp.
    pub last_update_timestamp: Timestamp,
    /// The asset config.
    pub config: AssetConfigV1,
}

impl From<AssetV1> for Asset {
    fn from(a: AssetV1) -> Self {
        let AssetV1 {
            supplied,
            borrowed,
            reserved,
            last_update_timestamp,
            config,
        } = a;
        Self {
            supplied,
            borrowed,
            reserved,
            prot_fee: 0,
            last_update_timestamp,
            config: config.into(),
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
#[serde(crate = "near_sdk::serde")]
pub struct ConfigV0 {
    /// The account ID of the oracle contract
    pub oracle_account_id: AccountId,

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

impl From<ConfigV0> for Config {
    fn from(a: ConfigV0) -> Self {
        let ConfigV0 { 
            oracle_account_id, 
            owner_id, 
            booster_token_id, 
            booster_decimals, 
            max_num_assets, 
            maximum_recency_duration_sec, 
            maximum_staleness_duration_sec, 
            minimum_staking_duration_sec, 
            maximum_staking_duration_sec, 
            x_booster_multiplier_at_maximum_staking_duration, 
            force_closing_enabled 
        } = a;
        Self {
            oracle_account_id, 
            ref_exchange_id: owner_id.clone(),
            owner_id, 
            booster_token_id, 
            booster_decimals, 
            max_num_assets, 
            maximum_recency_duration_sec, 
            maximum_staleness_duration_sec, 
            lp_tokens_info_valid_duration_sec: 600,
            minimum_staking_duration_sec, 
            maximum_staking_duration_sec, 
            x_booster_multiplier_at_maximum_staking_duration, 
            force_closing_enabled,
            boost_suppress_factor: 1,
        }
    }
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct ContractV080 {
    pub accounts: UnorderedMap<AccountId, VAccount>,
    pub storage: LookupMap<AccountId, VStorage>,
    pub assets: LookupMap<TokenId, VAsset>,
    pub asset_farms: LookupMap<FarmId, VAssetFarm>,
    pub asset_ids: UnorderedSet<TokenId>,
    pub config: LazyOption<ConfigV0>,
    /// The last recorded price info from the oracle. It's used for Net TVL farm computation.
    pub last_prices: HashMap<TokenId, Price>,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
#[serde(crate = "near_sdk::serde")]
pub struct ConfigV1 {
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

impl From<ConfigV1> for Config {
    fn from(a: ConfigV1) -> Self {
        let ConfigV1 { 
            oracle_account_id, 
            ref_exchange_id,
            owner_id, 
            booster_token_id, 
            booster_decimals, 
            max_num_assets, 
            maximum_recency_duration_sec, 
            maximum_staleness_duration_sec, 
            lp_tokens_info_valid_duration_sec,
            minimum_staking_duration_sec, 
            maximum_staking_duration_sec, 
            x_booster_multiplier_at_maximum_staking_duration, 
            force_closing_enabled 
        } = a;
        Self {
            oracle_account_id, 
            ref_exchange_id,
            owner_id, 
            booster_token_id, 
            booster_decimals, 
            max_num_assets, 
            maximum_recency_duration_sec, 
            maximum_staleness_duration_sec, 
            lp_tokens_info_valid_duration_sec,
            minimum_staking_duration_sec, 
            maximum_staking_duration_sec, 
            x_booster_multiplier_at_maximum_staking_duration, 
            force_closing_enabled,
            boost_suppress_factor: 1,
        }
    }
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct ContractV090 {
    pub accounts: UnorderedMap<AccountId, VAccount>,
    pub storage: LookupMap<AccountId, VStorage>,
    pub assets: LookupMap<TokenId, VAsset>,
    pub asset_farms: LookupMap<FarmId, VAssetFarm>,
    pub asset_ids: UnorderedSet<TokenId>,
    pub config: LazyOption<ConfigV1>,
    pub guardians: UnorderedSet<AccountId>,
    /// The last recorded price info from the oracle. It's used for Net TVL farm computation.
    pub last_prices: HashMap<TokenId, Price>,
    pub last_lp_token_infos: HashMap<String, UnitShareTokens>,
}