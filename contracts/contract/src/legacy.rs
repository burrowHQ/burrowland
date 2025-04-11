use std::collections::HashSet;

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
        // FIX-AURORA: Account version upgrade is a lazy upgrade, only execute when the account was about to be accessed.
        let mut supplied = supplied_unordered_map
            .iter()
            .map(|(key, value)| {
                let AccountAsset { shares } = value.into();
                (key, shares)
            })
            .collect();
        // FIX-AURORA: replace user supply.
        update_aurora_token_id(&mut supplied);
        let mut collateral = collateral_vec
            .into_iter()
            .map(|c| (c.token_id, c.shares))
            .collect();
        // FIX-AURORA: replace user collateral.
        update_aurora_token_id(&mut collateral);
        let mut borrowed = borrowed_vec
            .into_iter()
            .map(|b| (b.token_id, b.shares))
            .collect();
        // FIX-AURORA: replace user debt.
        update_aurora_token_id(&mut borrowed);
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
            is_locked: false,
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
    pub affected_farms: HashSet<FarmId>,
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
            mut supplied,
            mut collateral,
            mut borrowed,
            farms,
            affected_farms,
            storage_tracker,
            booster_staking,
        } = self;
        // FIX-AURORA: Account version upgrade is a lazy upgrade, only execute when the account was about to be accessed.
        // FIX-AURORA: replace user supply.
        update_aurora_token_id(&mut supplied);
        // FIX-AURORA: replace user collateral.
        update_aurora_token_id(&mut collateral);
        // FIX-AURORA: replace user debt.
        update_aurora_token_id(&mut borrowed);
        Account {
            account_id,
            supplied,
            positions: HashMap::from([(REGULAR_POSITION.to_string(), Position::RegularPosition(RegularPosition { collateral, borrowed }))]),
            farms,
            affected_farms,
            storage_tracker,
            booster_staking,
            is_locked: false,
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct AccountV3 {
    /// A copy of an account ID. Saves one storage_read when iterating on accounts.
    pub account_id: AccountId,
    /// A list of assets that are supplied by the account (but not used a collateral).
    /// It's not returned for account pagination.
    pub supplied: HashMap<TokenId, Shares>,
    pub positions: HashMap<String, Position>,
    /// Keeping track of data required for farms for this account.
    pub farms: HashMap<FarmId, AccountFarm>,
    #[borsh_skip]
    pub affected_farms: HashSet<FarmId>,
    /// Tracks changes in storage usage by persistent collections in this account.
    #[borsh_skip]
    pub storage_tracker: StorageTracker,
    /// Staking of booster token.
    pub booster_staking: Option<BoosterStaking>,
    pub is_locked: bool,
}

impl AccountV3 {
    pub fn into_account(self) -> Account {
        let AccountV3 {
            account_id,
            mut supplied,
            mut positions,
            farms,
            affected_farms,
            storage_tracker,
            booster_staking,
            is_locked,
        } = self;
        // FIX-AURORA: Account version upgrade is a lazy upgrade, only execute when the account was about to be accessed.
        // FIX-AURORA: replace user supply.
        update_aurora_token_id(&mut supplied);
        for position in positions.values_mut() {
            match position {
                Position::RegularPosition(p) => {
                    // FIX-AURORA: replace user debt in regular position.
                    update_aurora_token_id(&mut p.borrowed);
                    // FIX-AURORA: replace user collateral in regular position.
                    update_aurora_token_id(&mut p.collateral);
                },
                Position::LPTokenPosition(p) => {
                    // FIX-AURORA: replace user borrowed in LPTokenPosition.
                    update_aurora_token_id(&mut p.borrowed);
                    // FIX-AURORA: No need to replace collateral as eth won't exist in this type of position.
                } 
            }
        }
        Account {
            account_id,
            supplied,
            positions,
            farms,
            affected_farms,
            storage_tracker,
            booster_staking,
            is_locked,
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct MarginAccountV0 {
    /// A copy of an account ID. Saves one storage_read when iterating on accounts.
    pub account_id: AccountId,
    /// A list of assets that are supplied by the account (but not used a collateral).
    /// It's not returned for account pagination.
    pub supplied: HashMap<TokenId, Shares>,
    // margin trading related
    pub margin_positions: UnorderedMap<PosId, MarginTradingPosition>,
    /// Tracks changes in storage usage by persistent collections in this account.
    #[borsh_skip]
    pub storage_tracker: StorageTracker,
}

impl From<MarginAccountV0> for MarginAccount {
    fn from(a: MarginAccountV0) -> Self {
        let MarginAccountV0 { 
            account_id, 
            supplied, 
            margin_positions,
            storage_tracker,
        } = a;
        Self {
            account_id, 
            supplied, 
            margin_positions,
            position_latest_actions: HashMap::new(),
            storage_tracker,
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
            holding_position_fee_rate: U128(1000000000000000000000000000),
            volatility_ratio,
            extra_decimals,
            can_deposit,
            can_withdraw,
            can_use_as_collateral,
            can_borrow,
            net_tvl_multiplier: DEFAULT_NET_TVL_MULTIPLIER,
            max_change_rate: None,
            supplied_limit: None,
            borrowed_limit: None,
            min_borrowed_amount: None,
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
            margin_debt: Pool::new(),
            margin_pending_debt: 0,
            margin_position: 0,
            reserved,
            prot_fee: 0,
            unit_acc_hp_interest: 0,
            last_update_timestamp,
            config: config.into(),
            lostfound_shares: 0,
            pending_fee_events: None,
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
            holding_position_fee_rate: U128(1000000000000000000000000000),
            volatility_ratio,
            extra_decimals,
            can_deposit,
            can_withdraw,
            can_use_as_collateral,
            can_borrow,
            net_tvl_multiplier,
            max_change_rate: None,
            supplied_limit: None,
            borrowed_limit: None,
            min_borrowed_amount: None,
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
            margin_debt: Pool::new(),
            margin_pending_debt: 0,
            margin_position: 0,
            reserved,
            prot_fee: 0,
            unit_acc_hp_interest: 0,
            last_update_timestamp,
            config: config.into(),
            lostfound_shares: 0,
            pending_fee_events: None,
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
#[serde(crate = "near_sdk::serde")]
pub struct AssetConfigV2 {
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

impl From<AssetConfigV2> for AssetConfig {
    fn from(a: AssetConfigV2) -> Self {
        let AssetConfigV2 {
            reserve_ratio,
            prot_ratio,
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
            prot_ratio,
            target_utilization,
            target_utilization_rate,
            max_utilization_rate,
            holding_position_fee_rate: U128(1000000000000000000000000000),
            volatility_ratio,
            extra_decimals,
            can_deposit,
            can_withdraw,
            can_use_as_collateral,
            can_borrow,
            net_tvl_multiplier,
            max_change_rate: None,
            supplied_limit: None,
            borrowed_limit: None,
            min_borrowed_amount: None,
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct AssetV2 {
    /// Total supplied including collateral, but excluding reserved.
    pub supplied: Pool,
    /// Total borrowed.
    pub borrowed: Pool,
    /// The amount reserved for the stability. This amount can also be borrowed and affects
    /// borrowing rate.
    pub reserved: Balance,
    /// The amount belongs to the protocol. This amount can also be borrowed and affects
    /// borrowing rate.
    pub prot_fee: Balance,
    /// When the asset was last updated. It's always going to be the current block timestamp.
    pub last_update_timestamp: Timestamp,
    /// The asset config.
    pub config: AssetConfigV2,
}

impl From<AssetV2> for Asset {
    fn from(a: AssetV2) -> Self {
        let AssetV2 {
            supplied,
            borrowed,
            reserved,
            prot_fee,
            last_update_timestamp,
            config,
        } = a;
        Self {
            supplied,
            borrowed,
            margin_debt: Pool::new(),
            margin_pending_debt: 0,
            margin_position: 0,
            reserved,
            prot_fee,
            unit_acc_hp_interest: 0,
            last_update_timestamp,
            config: config.into(),
            lostfound_shares: 0,
            pending_fee_events: None,
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
#[serde(crate = "near_sdk::serde")]
pub struct AssetConfigV3 {
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
}

impl From<AssetConfigV3> for AssetConfig {
    fn from(a: AssetConfigV3) -> Self {
        let AssetConfigV3 {
            reserve_ratio,
            prot_ratio,
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
            max_change_rate,
            supplied_limit,
            borrowed_limit,
        } = a;
        Self {
            reserve_ratio,
            prot_ratio,
            target_utilization,
            target_utilization_rate,
            max_utilization_rate,
            holding_position_fee_rate: U128(1000000000000000000000000000),
            volatility_ratio,
            extra_decimals,
            can_deposit,
            can_withdraw,
            can_use_as_collateral,
            can_borrow,
            net_tvl_multiplier,
            max_change_rate,
            supplied_limit,
            borrowed_limit,
            min_borrowed_amount: None,
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct AssetV3 {
    /// Total supplied including collateral, but excluding reserved.
    pub supplied: Pool,
    /// Total borrowed.
    pub borrowed: Pool,
    /// The amount reserved for the stability. This amount can also be borrowed and affects
    /// borrowing rate.
    pub reserved: Balance,
    /// The amount belongs to the protocol. This amount can also be borrowed and affects
    /// borrowing rate.
    pub prot_fee: Balance,
    /// When the asset was last updated. It's always going to be the current block timestamp.
    pub last_update_timestamp: Timestamp,
    /// The asset config.
    pub config: AssetConfigV3,
}

impl From<AssetV3> for Asset {
    fn from(a: AssetV3) -> Self {
        let AssetV3 {
            supplied,
            borrowed,
            reserved,
            prot_fee,
            last_update_timestamp,
            config,
        } = a;
        Self {
            supplied,
            borrowed,
            margin_debt: Pool::new(),
            margin_pending_debt: 0,
            margin_position: 0,
            reserved,
            prot_fee,
            unit_acc_hp_interest: 0,
            last_update_timestamp,
            config: config.into(),
            lostfound_shares: 0,
            pending_fee_events: None,
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct AssetConfigV4 {
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
}

impl From<AssetConfigV4> for AssetConfig {
    fn from(a: AssetConfigV4) -> Self {
        let AssetConfigV4 {
            reserve_ratio,
            prot_ratio,
            target_utilization,
            target_utilization_rate,
            max_utilization_rate,
            holding_position_fee_rate,
            volatility_ratio,
            extra_decimals,
            can_deposit,
            can_withdraw,
            can_use_as_collateral,
            can_borrow,
            net_tvl_multiplier,
            max_change_rate,
            supplied_limit,
            borrowed_limit,
        } = a;
        Self {
            reserve_ratio,
            prot_ratio,
            target_utilization,
            target_utilization_rate,
            max_utilization_rate,
            holding_position_fee_rate,
            volatility_ratio,
            extra_decimals,
            can_deposit,
            can_withdraw,
            can_use_as_collateral,
            can_borrow,
            net_tvl_multiplier,
            max_change_rate,
            supplied_limit,
            borrowed_limit,
            min_borrowed_amount: None,
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct AssetV4 {
    /// Total supplied including collateral, but excluding reserved.
    pub supplied: Pool,
    /// Total borrowed.
    pub borrowed: Pool,
    /// Total margin debt.
    pub margin_debt: Pool,
    /// borrowed by margin position and currently in trading process
    pub margin_pending_debt: Balance,
    /// total position in margin
    pub margin_position: Balance,
    /// The amount reserved for the stability. This amount can also be borrowed and affects
    /// borrowing rate.
    pub reserved: Balance,
    /// The amount belongs to the protocol. This amount can also be borrowed and affects
    /// borrowing rate.
    pub prot_fee: Balance,
    /// The accumulated holding margin position interests till self.last_update_timestamp.
    pub unit_acc_hp_interest: Balance,
    /// When the asset was last updated. It's always going to be the current block timestamp.
    pub last_update_timestamp: Timestamp,
    /// The asset config.
    pub config: AssetConfigV4,
}

impl From<AssetV4> for Asset {
    fn from(a: AssetV4) -> Self {
        let AssetV4 {
            supplied,
            borrowed,
            margin_debt,
            margin_pending_debt,
            margin_position,
            reserved,
            prot_fee,
            unit_acc_hp_interest,
            last_update_timestamp,
            config,
        } = a;
        Self {
            supplied,
            borrowed,
            margin_debt,
            margin_pending_debt,
            margin_position,
            reserved,
            prot_fee,
            unit_acc_hp_interest,
            last_update_timestamp,
            config: config.into(),
            lostfound_shares: 0,
            pending_fee_events: None,
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
            pyth_oracle_account_id: owner_id.clone(),
            ref_exchange_id: owner_id.clone(),
            owner_id, 
            booster_token_id, 
            booster_decimals, 
            max_num_assets, 
            maximum_recency_duration_sec, 
            maximum_staleness_duration_sec, 
            lp_tokens_info_valid_duration_sec: 600,
            pyth_price_valid_duration_sec: 60,
            minimum_staking_duration_sec, 
            maximum_staking_duration_sec, 
            x_booster_multiplier_at_maximum_staking_duration, 
            force_closing_enabled,
            enable_price_oracle: true,
            enable_pyth_oracle: false,
            boost_suppress_factor: 1,
            dcl_id: None,
        }
    }
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
            pyth_oracle_account_id: owner_id.clone(),
            ref_exchange_id,
            owner_id, 
            booster_token_id, 
            booster_decimals, 
            max_num_assets, 
            maximum_recency_duration_sec, 
            maximum_staleness_duration_sec, 
            lp_tokens_info_valid_duration_sec,
            pyth_price_valid_duration_sec: 60,
            minimum_staking_duration_sec, 
            maximum_staking_duration_sec, 
            x_booster_multiplier_at_maximum_staking_duration, 
            force_closing_enabled,
            enable_price_oracle: true,
            enable_pyth_oracle: false,
            boost_suppress_factor: 1,
            dcl_id: None,
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
#[serde(crate = "near_sdk::serde")]
pub struct ConfigV2 {
    /// The account ID of the oracle contract
    pub oracle_account_id: AccountId,

    /// The account ID of the pyth oracle contract
    pub pyth_oracle_account_id: AccountId,

    /// The account ID of the ref_exchange contract
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

    /// The valid duration to lp tokens info in seconds.
    pub lp_tokens_info_valid_duration_sec: DurationSec,

    /// The valid duration to pyth price in seconds.
    pub pyth_price_valid_duration_sec: DurationSec,

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

    /// Whether to use the price of price oracle
    pub enable_price_oracle: bool,
    /// Whether to use the price of pyth oracle
    pub enable_pyth_oracle: bool,
}

impl From<ConfigV2> for Config {
    fn from(a: ConfigV2) -> Self {
        let ConfigV2 { 
            oracle_account_id, 
            pyth_oracle_account_id,
            ref_exchange_id,
            owner_id, 
            booster_token_id, 
            booster_decimals, 
            max_num_assets, 
            maximum_recency_duration_sec, 
            maximum_staleness_duration_sec, 
            lp_tokens_info_valid_duration_sec,
            pyth_price_valid_duration_sec,
            minimum_staking_duration_sec, 
            maximum_staking_duration_sec, 
            x_booster_multiplier_at_maximum_staking_duration,
            force_closing_enabled,
            enable_price_oracle,
            enable_pyth_oracle
        } = a;
        Self {
            oracle_account_id, 
            pyth_oracle_account_id,
            ref_exchange_id,
            owner_id, 
            booster_token_id, 
            booster_decimals, 
            max_num_assets, 
            maximum_recency_duration_sec, 
            maximum_staleness_duration_sec, 
            lp_tokens_info_valid_duration_sec,
            pyth_price_valid_duration_sec,
            minimum_staking_duration_sec, 
            maximum_staking_duration_sec, 
            x_booster_multiplier_at_maximum_staking_duration, 
            force_closing_enabled,
            enable_price_oracle,
            enable_pyth_oracle,
            boost_suppress_factor: 1,
            dcl_id: None,
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
#[serde(crate = "near_sdk::serde")]
pub struct ConfigV3 {
    /// The account ID of the oracle contract
    pub oracle_account_id: AccountId,

    /// The account ID of the pyth oracle contract
    pub pyth_oracle_account_id: AccountId,

    /// The account ID of the ref_exchange contract
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

    /// The valid duration to lp tokens info in seconds.
    pub lp_tokens_info_valid_duration_sec: DurationSec,

    /// The valid duration to pyth price in seconds.
    pub pyth_price_valid_duration_sec: DurationSec,

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

    /// Whether to use the price of price oracle
    pub enable_price_oracle: bool,
    /// Whether to use the price of pyth oracle
    pub enable_pyth_oracle: bool,
    /// The factor that suppresses the effect of boost.
    /// E.g. 1000 means that in the calculation, the actual boost amount will be divided by 1000.
    pub boost_suppress_factor: u128,
}

impl From<ConfigV3> for Config {
    fn from(a: ConfigV3) -> Self {
        let ConfigV3 { 
            oracle_account_id, 
            pyth_oracle_account_id,
            ref_exchange_id,
            owner_id, 
            booster_token_id, 
            booster_decimals, 
            max_num_assets, 
            maximum_recency_duration_sec, 
            maximum_staleness_duration_sec, 
            lp_tokens_info_valid_duration_sec,
            pyth_price_valid_duration_sec,
            minimum_staking_duration_sec, 
            maximum_staking_duration_sec, 
            x_booster_multiplier_at_maximum_staking_duration,
            force_closing_enabled,
            enable_price_oracle,
            enable_pyth_oracle,
            boost_suppress_factor,
        } = a;
        Self {
            oracle_account_id, 
            pyth_oracle_account_id,
            ref_exchange_id,
            owner_id, 
            booster_token_id, 
            booster_decimals, 
            max_num_assets, 
            maximum_recency_duration_sec, 
            maximum_staleness_duration_sec, 
            lp_tokens_info_valid_duration_sec,
            pyth_price_valid_duration_sec,
            minimum_staking_duration_sec, 
            maximum_staking_duration_sec, 
            x_booster_multiplier_at_maximum_staking_duration, 
            force_closing_enabled,
            enable_price_oracle,
            enable_pyth_oracle,
            boost_suppress_factor,
            dcl_id: None,
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct MarginConfigV0 {
    /// When open a position or decrease collateral, the new leverage rate should less than this,
    /// Eg: 5 means 5 times collateral value should more than debt value.
    pub max_leverage_rate: u8, 
    /// Ensure pending debt less than this portion of availabe amount, 
    /// Eg: 1000 means pending debt amount should less than 10% of available amount.
    pub pending_debt_scale: u32,
    /// Ensure the slippage in SwapIndication less than this one,
    /// Eg: 1000 means we allow a max slippage of 10%.
    pub max_slippage_rate: u32,
    /// The position will be liquidated when (margin + position) is less than 
    ///   (debt + hp_fee) * (1 + min_safety_buffer_rate).
    pub min_safety_buffer: u32,
    /// Compare to regular borrowing, margin borrow enjoy a discount.
    /// Eg: 7000 means margin debt equals 70% of regular debt.
    pub margin_debt_discount_rate: u32,
    /// Open fee is on the margin asset.
    pub open_position_fee_rate: u32,
    /// Dex account id and its version (1 - RefV1, 2 - RefV2)
    pub registered_dexes: HashMap<AccountId, u8>,
    /// Token and its party side, such as 1 and 2 are in different parties,
    /// hence they can be a debt and a position. In other word,
    /// Tokens in the same party, can NOT exist in the same position.
    pub registered_tokens: HashMap<AccountId, u8>, 
    /// Maximum amount of margin position allowed for users to hold.
    pub max_active_user_margin_position: u8,
}

impl From<MarginConfigV0> for MarginConfig {
    fn from(a: MarginConfigV0) -> Self {
        let MarginConfigV0 { 
            max_leverage_rate, 
            pending_debt_scale, 
            max_slippage_rate, 
            min_safety_buffer, 
            margin_debt_discount_rate, 
            open_position_fee_rate, 
            registered_dexes, 
            registered_tokens, 
            max_active_user_margin_position,
        } = a;
        Self {
            max_leverage_rate, 
            pending_debt_scale, 
            max_slippage_rate, 
            min_safety_buffer, 
            margin_debt_discount_rate, 
            open_position_fee_rate, 
            registered_dexes, 
            registered_tokens, 
            max_active_user_margin_position,
            liq_benefit_protocol_rate: 5000,
            liq_benefit_liquidator_rate: 5000,
            max_position_action_wait_sec: 3600,
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct MarginConfigV1 {
    /// When open a position or decrease collateral, the new leverage rate should less than this,
    /// Eg: 5 means 5 times collateral value should more than debt value.
    pub max_leverage_rate: u8, 
    /// Ensure pending debt less than this portion of availabe amount, 
    /// Eg: 1000 means pending debt amount should less than 10% of available amount.
    pub pending_debt_scale: u32,
    /// Ensure the slippage in SwapIndication less than this one,
    /// Eg: 1000 means we allow a max slippage of 10%.
    pub max_slippage_rate: u32,
    /// The position will be liquidated when (margin + position) is less than 
    ///   (debt + hp_fee) * (1 + min_safety_buffer_rate).
    pub min_safety_buffer: u32,
    /// Compare to regular borrowing, margin borrow enjoy a discount.
    /// Eg: 7000 means margin debt equals 70% of regular debt.
    pub margin_debt_discount_rate: u32,
    /// Open fee is on the margin asset.
    pub open_position_fee_rate: u32,
    /// Dex account id and its version (1 - RefV1, 2 - RefV2)
    pub registered_dexes: HashMap<AccountId, u8>,
    /// Token and its party side, such as 1 and 2 are in different parties,
    /// hence they can be a debt and a position. In other word,
    /// Tokens in the same party, can NOT exist in the same position.
    pub registered_tokens: HashMap<AccountId, u8>, 
    /// Maximum amount of margin position allowed for users to hold.
    pub max_active_user_margin_position: u8,
    /// base token default value
    /// The rate of liquidation benefits allocated to the protocol.
    pub liq_benefit_protocol_rate: u32,
    /// base token default value
    /// The rate of liquidation benefits allocated to the liquidator.
    pub liq_benefit_liquidator_rate: u32,
}

impl From<MarginConfigV1> for MarginConfig {
    fn from(a: MarginConfigV1) -> Self {
        let MarginConfigV1 { 
            max_leverage_rate, 
            pending_debt_scale, 
            max_slippage_rate, 
            min_safety_buffer, 
            margin_debt_discount_rate, 
            open_position_fee_rate, 
            registered_dexes, 
            registered_tokens, 
            max_active_user_margin_position,
            liq_benefit_protocol_rate,
            liq_benefit_liquidator_rate,
        } = a;
        Self {
            max_leverage_rate, 
            pending_debt_scale, 
            max_slippage_rate, 
            min_safety_buffer, 
            margin_debt_discount_rate, 
            open_position_fee_rate, 
            registered_dexes, 
            registered_tokens, 
            max_active_user_margin_position,
            liq_benefit_protocol_rate,
            liq_benefit_liquidator_rate,
            max_position_action_wait_sec: 3600,
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

#[derive(BorshDeserialize, BorshSerialize)]
pub struct ContractV0100 {
    pub accounts: UnorderedMap<AccountId, VAccount>,
    pub storage: LookupMap<AccountId, VStorage>,
    pub assets: LookupMap<TokenId, VAsset>,
    pub asset_farms: LookupMap<FarmId, VAssetFarm>,
    pub asset_ids: UnorderedSet<TokenId>,
    pub config: LazyOption<ConfigV2>,
    pub guardians: UnorderedSet<AccountId>,
    /// The last recorded price info from the oracle. It's used for Net TVL farm computation.
    pub last_prices: HashMap<TokenId, Price>,
    pub last_lp_token_infos: HashMap<String, UnitShareTokens>,
    pub token_pyth_info: HashMap<TokenId, TokenPythInfo>,
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct ContractV0110 {
    pub accounts: UnorderedMap<AccountId, VAccount>,
    pub storage: LookupMap<AccountId, VStorage>,
    pub assets: LookupMap<TokenId, VAsset>,
    pub asset_farms: LookupMap<FarmId, VAssetFarm>,
    pub asset_ids: UnorderedSet<TokenId>,
    pub config: LazyOption<ConfigV3>,
    pub guardians: UnorderedSet<AccountId>,
    /// The last recorded price info from the oracle. It's used for Net TVL farm computation.
    pub last_prices: HashMap<TokenId, Price>,
    pub last_lp_token_infos: HashMap<String, UnitShareTokens>,
    pub token_pyth_info: HashMap<TokenId, TokenPythInfo>,
    pub blacklist_of_farmers: UnorderedSet<AccountId>,
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct ContractV0120 {
    pub accounts: UnorderedMap<AccountId, VAccount>,
    pub storage: LookupMap<AccountId, VStorage>,
    pub assets: LookupMap<TokenId, VAsset>,
    pub asset_farms: LookupMap<FarmId, VAssetFarm>,
    pub asset_ids: UnorderedSet<TokenId>,
    pub config: LazyOption<ConfigV3>,
    pub guardians: UnorderedSet<AccountId>,
    /// The last recorded price info from the oracle. It's used for Net TVL farm computation.
    pub last_prices: HashMap<TokenId, Price>,
    pub last_lp_token_infos: HashMap<String, UnitShareTokens>,
    pub token_pyth_info: HashMap<TokenId, TokenPythInfo>,
    pub blacklist_of_farmers: UnorderedSet<AccountId>,
    pub last_staking_token_prices: HashMap<TokenId, U128>,
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct ContractV0130 {
    pub accounts: UnorderedMap<AccountId, VAccount>,
    pub storage: LookupMap<AccountId, VStorage>,
    pub assets: LookupMap<TokenId, VAsset>,
    pub asset_farms: LookupMap<FarmId, VAssetFarm>,
    pub asset_ids: UnorderedSet<TokenId>,
    pub config: LazyOption<Config>,
    pub guardians: UnorderedSet<AccountId>,
    /// The last recorded price info from the oracle. It's used for Net TVL farm computation.
    pub last_prices: HashMap<TokenId, Price>,
    pub last_lp_token_infos: HashMap<String, UnitShareTokens>,
    pub token_pyth_info: HashMap<TokenId, TokenPythInfo>,
    pub blacklist_of_farmers: UnorderedSet<AccountId>,
    pub last_staking_token_prices: HashMap<TokenId, U128>,
    pub margin_accounts: UnorderedMap<AccountId, VMarginAccount>,
    pub margin_config: LazyOption<MarginConfigV0>,
    pub accumulated_margin_position_num: u64
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct ContractV0140 {
    pub accounts: UnorderedMap<AccountId, VAccount>,
    pub storage: LookupMap<AccountId, VStorage>,
    pub assets: LookupMap<TokenId, VAsset>,
    pub asset_farms: LookupMap<FarmId, VAssetFarm>,
    pub asset_ids: UnorderedSet<TokenId>,
    pub config: LazyOption<Config>,
    pub guardians: UnorderedSet<AccountId>,
    /// The last recorded price info from the oracle. It's used for Net TVL farm computation.
    pub last_prices: HashMap<TokenId, Price>,
    pub last_lp_token_infos: HashMap<String, UnitShareTokens>,
    pub token_pyth_info: HashMap<TokenId, TokenPythInfo>,
    pub blacklist_of_farmers: UnorderedSet<AccountId>,
    pub last_staking_token_prices: HashMap<TokenId, U128>,
    pub margin_accounts: UnorderedMap<AccountId, VMarginAccount>,
    pub margin_config: LazyOption<MarginConfigV1>,
    pub accumulated_margin_position_num: u64
}