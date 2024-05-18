# The list of APIs that are provided by the contract

Notes:
- `u128_dec_format`, `WrappedBalance`, `Shares` means the value is passed as a decimal string representation. E.g. `1` serialized as `"1"`
- `BigDecimal` is serialized as floating string representation. E.g. `1.5` serialized as `"1.5"`
- `u64` means the value is passed as an integer.
- `Option<_>` means the value can be omitted, or provided as `null`.
- Rust enums are serialized using JSON objects. E.g. `FarmId::Supplied("token.near")` is serialized as `{"Supplied": "token.near"}`
- `HashMap<_, _>` is serialized using JSON objects.

```rust
trait Contract {
    /// Initializes the contract with the given config. Needs to be called once.
    #[init]
    fn new(config: Config) -> Self;

    /// Extend guardians. Only can be called by owner.
    /// - Requires one yoctoNEAR.
    #[payable]
    fn extend_guardians(&mut self, guardians: Vec<AccountId>);

    /// Remove guardians. Only can be called by owner.
    /// - Requires one yoctoNEAR.
    #[payable]
    fn remove_guardians(&mut self, guardians: Vec<AccountId>);

    /// Returns all guardians.
    fn get_guardians(&self);

    /// Add pyth info for the specified token. Only can be called by owner or guardians.
    /// - Requires one yoctoNEAR.
    #[payable]
    fn add_token_pyth_info(&mut self, token_id: TokenId, token_pyth_info: TokenPythInfo);

    /// Update pyth info for the specified token. Only can be called by owner or guardians.
    /// - Requires one yoctoNEAR.
    #[payable]
    fn update_token_pyth_info(&mut self, token_id: TokenId, token_pyth_info: TokenPythInfo);

    /// Returns all pyth info.
    fn get_all_token_pyth_infos(&self) -> HashMap<TokenId, TokenPythInfo>;

    /// Return pyth information for the specified token.
    fn get_token_pyth_info(&self, token_id: TokenId) -> Option<TokenPythInfo>;

    /// Extend farmers to the blacklist. Only can be called by owner or guardians.
    /// - Requires one yoctoNEAR.
    #[payable]
    fn extend_blacklist_of_farmers(&mut self, farmers: Vec<AccountId>);

    /// Remove farmers from the blacklist. Only can be called by owner or guardians.
    /// - Requires one yoctoNEAR.
    #[payable]
    fn remove_blacklist_of_farmers(&mut self, farmers: Vec<AccountId>);

    /// Returns all farmers in the blacklist.
    fn get_blacklist_of_farmers(&self) -> Vec<AccountId>;

    /// Sync the price of the specified token.
    fn sync_staking_token_price(&mut self, token_id: TokenId);

    /// Returns last_staking_token_prices.
    fn get_last_staking_token_prices(&self) -> HashMap<TokenId, U128>;

    /// Returns detailed information about an account for a given account_id.
    /// The information includes all supplied assets, collateral and borrowed.
    /// Each asset includes the current balance and the number of shares.
    fn get_account(&self, account_id: ValidAccountId) -> Option<AccountDetailedView>;

    /// Returns detailed information about an account for a given account_id.
    /// The information includes all positions supplied assets, collateral and borrowed.
    /// Each asset includes the current balance and the number of shares.
    fn get_account_all_positions(&self, account_id: AccountId) -> Option<AccountAllPositionsDetailedView>;

    /// Returns limited account information for accounts from a given index up to a given limit.
    /// The information includes number of shares for collateral and borrowed assets.
    /// This method can be used to iterate on the accounts for liquidation.
    fn get_accounts_paged(&self, from_index: Option<u64>, limit: Option<u64>) -> Vec<Account>;

    /// Returns the number of accounts
    fn get_num_accounts(&self) -> u32;

    /// Executes a given list actions on behalf of the predecessor account without price.
    /// - Requires one yoctoNEAR.
    #[payable]
    fn execute(&mut self, actions: Vec<Action>);

    /// Executes a given list actions on behalf of the predecessor account with pyth oracle price.
    /// - Requires one yoctoNEAR.
    #[payable]
    pub fn execute_with_pyth(&mut self, actions: Vec<Action>);

    /// Executes a given list margin actions on behalf of the predecessor account.
    /// - Requires one yoctoNEAR.
    #[payable]
    pub fn margin_execute(&mut self, actions: Vec<MarginAction>);

    /// Executes a given list margin actions on behalf of the predecessor account with pyth oracle price.
    /// - Requires one yoctoNEAR.
    #[payable]
    pub fn margin_execute_with_pyth(&mut self, actions: Vec<MarginAction>);

    /// Returns a detailed view asset for a given token_id.
    /// The detailed view includes current APR and corresponding farms.
    fn get_asset(&self, token_id: ValidAccountId) -> Option<AssetDetailedView>;

    /// Returns an list of detailed view assets a given list of token_id.
    /// Only returns existing assets.
    fn get_assets(&self, token_ids: Vec<ValidAccountId>) -> Vec<AssetDetailedView>;

    /// Returns a list of pairs (token_id, asset) for assets from a given index up to a given limit.
    fn get_assets_paged(
        &self,
        from_index: Option<u64>,
        limit: Option<u64>,
    ) -> Vec<(TokenId, Asset)>;

    /// Returns a list of detailed view assets from a given index up to a given limit.
    fn get_assets_paged_detailed(
        &self,
        from_index: Option<u64>,
        limit: Option<u64>,
    ) -> Vec<AssetDetailedView>;

    /// Returns the current config.
    fn get_config(&self) -> Config;

    /// Updates the current config.
    /// - Requires one yoctoNEAR.
    /// - Requires to be called by the contract owner.
    #[payable]
    fn update_config(&mut self, config: Config);

    /// Adjust boost staking policy.
    /// - Panics if minimum_staking_duration_sec >= maximum_staking_duration_sec.
    /// - Panics if x_booster_multiplier_at_maximum_staking_duration < MIN_BOOSTER_MULTIPLIER.
    /// - Requires one yoctoNEAR.
    /// - Requires to be called by the contract owner or guardians.
    #[payable]
    fn adjust_boost_staking_policy(&mut self, minimum_staking_duration_sec: DurationSec, maximum_staking_duration_sec: DurationSec, x_booster_multiplier_at_maximum_staking_duration: u32);

    /// Adjust boost suppress factor.
    /// - Panics if boost_suppress_factor <= 0.
    /// - Requires one yoctoNEAR.
    /// - Requires to be called by the contract owner.
    #[payable]
    fn adjust_boost_suppress_factor(&mut self, boost_suppress_factor: u128);

    /// Adds an asset with a given token_id and a given asset_config.
    /// - Panics if the asset config is invalid.
    /// - Panics if an asset with the given token_id already exists.
    /// - Requires one yoctoNEAR.
    /// - Requires to be called by the contract owner.
    #[payable]
    fn add_asset(&mut self, token_id: ValidAccountId, asset_config: AssetConfig);

    /// Updates the asset config for the asset with the a given token_id.
    /// - Panics if the asset config is invalid.
    /// - Panics if an asset with the given token_id doesn't exist.
    /// - Requires one yoctoNEAR.
    /// - Requires to be called by the contract owner.
    #[payable]
    fn update_asset(&mut self, token_id: ValidAccountId, asset_config: AssetConfig);

    /// Updates the limit for the asset with the a given token_id.
    /// - Panics if an asset with the given token_id doesn't exist.
    /// - Requires one yoctoNEAR.
    /// - Requires to be called by the contract owner.
    #[payable]
    fn update_asset_limit(&mut self, token_id: AccountId, supplied_limit: Option<U128>, borrowed_limit: Option<U128>);

    /// Updates the max_change_rate for the asset with the a given token_id.
    /// - Panics if an asset with the given token_id doesn't exist.
    /// - Requires one yoctoNEAR.
    /// - Requires to be called by the contract owner.
    #[payable]
    fn update_asset_max_change_rate(&mut self, token_id: AccountId, max_change_rate: Option<u32>);

    /// Updates the prot_ratio for the asset with the a given token_id.
    /// - Panics if the prot_ratio is invalid.
    /// - Panics if an asset with the given token_id doesn't exist.
    /// - Requires one yoctoNEAR.
    /// - Requires to be called by the contract owner or guardians.
    #[payable]
    fn update_asset_prot_ratio(&mut self, token_id: AccountId, prot_ratio: u32);

    /// Enable the capacity for the asset with the a given token_id.
    /// - Panics if the capacity is invalid.
    /// - Panics if an asset with the given token_id doesn't exist.
    /// - Requires one yoctoNEAR.
    /// - Requires to be called by the contract owner.
    #[payable]
    fn enable_asset_capacity(&mut self, token_id: AccountId, can_deposit: Option<bool>, can_withdraw: Option<bool>, can_use_as_collateral: Option<bool>, can_borrow: Option<bool>);

    /// Disable the capacity for the asset with the a given token_id.
    /// - Panics if the capacity is invalid.
    /// - Panics if an asset with the given token_id doesn't exist.
    /// - Requires one yoctoNEAR.
    /// - Requires to be called by the contract owner or guardians.
    #[payable]
    fn disable_asset_capacity(&mut self, token_id: AccountId, can_deposit: Option<bool>, can_withdraw: Option<bool>, can_use_as_collateral: Option<bool>, can_borrow: Option<bool>);

    /// Updates the net_tvl_multiplier for the asset with the a given token_id.
    /// - Panics if the net_tvl_multiplier is invalid.
    /// - Panics if an asset with the given token_id doesn't exist.
    /// - Requires one yoctoNEAR.
    /// - Requires to be called by the contract owner or guardians.
    #[payable]
    fn update_asset_net_tvl_multiplier(&mut self, token_id: AccountId, net_tvl_multiplier: u32);

    /// Claim prot_fee from asset with the a given token_id.
    /// - Panics if an asset with the given token_id doesn't exist.
    /// - Requires one yoctoNEAR.
    /// - Requires to be called by the contract owner or guardians.
    #[payable]
    fn claim_prot_fee(&mut self, token_id: AccountId, stdd_amount: Option<U128>);

    /// Decrease reserved from asset with the a given token_id.
    /// - Panics if an asset with the given token_id doesn't exist.
    /// - Requires one yoctoNEAR.
    /// - Requires to be called by the contract owner or guardians.
    #[payable]
    fn decrease_reserved(&mut self, token_id: AccountId, stdd_amount: Option<U128>);

    /// Increase reserved from asset with the a given token_id.
    /// - Panics if an asset with the given token_id doesn't exist.
    /// - Requires one yoctoNEAR.
    /// - Requires to be called by the contract owner or guardians.
    #[payable]
    fn increase_reserved(&mut self, asset_amount: AssetAmount);

    /// Receives the transfer from the fungible token and executes a list of actions given in the
    /// message on behalf of the sender. The actions that can be executed should be limited to a set
    /// that doesn't require pricing.
    /// - Requires to be called by the fungible token account.
    fn ft_on_transfer(
        &mut self,
        sender_id: ValidAccountId,
        amount: U128,
        msg: String,
    ) -> PromiseOrValue<U128>;

    /// The method will execute a given list of actions in the msg using the prices from the `data`
    /// provided by the oracle on behalf of the sender_id.
    /// - Requires to be called by the oracle account ID.
    fn oracle_on_call(&mut self, sender_id: ValidAccountId, data: PriceData, msg: String);

    /// Claims all unclaimed farm rewards.
    fn account_farm_claim_all(&mut self);

    /// Returns an asset farm for a given farm ID.
    fn get_asset_farm(&self, farm_id: FarmId) -> Option<AssetFarm>;

    /// Returns a list of pairs (farm ID, asset farm) for a given list of farm IDs.
    fn get_asset_farms(&self, farm_ids: Vec<FarmId>) -> Vec<(FarmId, AssetFarm)>;

    /// Returns a list of pairs (farm ID, asset farm) from a given index up to a given limit.
    ///
    /// Note, the number of returned elements may be twice larger than the limit, due to the
    /// pagination implementation. To continue to the next page use `from_index + limit`.
    fn get_asset_farms_paged(
        &self,
        from_index: Option<u64>,
        limit: Option<u64>,
    ) -> Vec<(FarmId, AssetFarm)>;

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
    fn add_asset_farm_reward(
        &mut self,
        farm_id: FarmId,
        token_id: ValidAccountId,
        new_reward_per_day: WrappedBalance,
        new_booster_log_base: WrappedBalance,
        extra_amount: WrappedBalance,
    );

    /// Stakes a given amount (or all supplied) booster token for a given duration in seconds.
    /// If the previous stake exists, then the new duration should be longer than the previous
    /// remaining staking duration.
    #[payable]
    fn account_stake_booster(&mut self, amount: Option<U128>, duration: DurationSec);

    /// Unstakes all booster token.
    /// The current timestamp must be greater than the unlock_timestamp.
    #[payable]
    fn account_unstake_booster(&mut self);
}
```

## Structures and types

```rust
pub struct AssetView {
    pub token_id: TokenId,
    #[serde(with = "u128_dec_format")]
    pub balance: Balance,
    /// The number of shares this account holds in the corresponding asset pool
    pub shares: Shares,
    /// The current APR for this asset (either supply or borrow APR).
    pub apr: BigDecimal,
}

pub enum FarmId {
    Supplied(TokenId),
    Borrowed(TokenId),
    NetTvl,
    TokenNetBalance(TokenId),
}

pub struct AccountFarmView {
    pub farm_id: FarmId,
    pub rewards: Vec<AccountFarmRewardView>,
}

pub struct AccountFarmRewardView {
    pub reward_token_id: TokenId,
    pub asset_farm_reward: AssetFarmReward,
    #[serde(with = "u128_dec_format")]
    pub boosted_shares: Balance,
    #[serde(with = "u128_dec_format")]
    pub unclaimed_amount: Balance,
}

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

pub struct AccountAllPositionsDetailedView {
    pub account_id: AccountId,
    /// A list of assets that are supplied by the account (but not used a collateral).
    pub supplied: Vec<AssetView>,
    pub positions: HashMap<String, PositionView>,
    /// Account farms
    pub farms: Vec<AccountFarmView>,
    /// Whether the account has assets, that can be farmed.
    pub has_non_farmed_assets: bool,
    /// Staking of booster token.
    pub booster_staking: Option<BoosterStaking>,
    pub is_locked: bool
}

/// Limited view of the account structure for liquidations
pub struct Account {
    /// A copy of an account ID. Saves one storage_read when iterating on accounts.
    pub account_id: AccountId,
    /// A list of assets that are supplied by the account (but not used a collateral).
    /// It's not returned for account pagination.
    pub supplied: HashMap<TokenId, Shares>,
    pub positions: HashMap<String, Position>,
    /// Keeping track of data required for farms for this account.
    #[serde(skip_serializing)]
    pub farms: HashMap<FarmId, AccountFarm>,
    #[borsh_skip]
    #[serde(skip_serializing)]
    pub affected_farms: HashSet<FarmId>,

    /// Tracks changes in storage usage by persistent collections in this account.
    #[borsh_skip]
    #[serde(skip)]
    pub storage_tracker: StorageTracker,

    /// Staking of booster token.
    pub booster_staking: Option<BoosterStaking>,
    pub is_locked: bool
}

pub struct AssetDetailedView {
    pub token_id: TokenId,
    /// Total supplied including collateral, but excluding reserved.
    pub supplied: Pool,
    /// Total borrowed.
    pub borrowed: Pool,
    /// The amount reserved for the stability. This amount can also be borrowed and affects
    /// borrowing rate.
    #[serde(with = "u128_dec_format")]
    pub reserved: Balance,
    /// When the asset was last updated. It's always going to be the current block timestamp.
    #[serde(with = "u64_dec_format")]
    pub last_update_timestamp: Timestamp,
    /// The asset config.
    pub config: AssetConfig,
    /// Current APR excluding farms for supplying the asset.
    pub supply_apr: BigDecimal,
    /// Current APR excluding farms for borrowing the asset.
    pub borrow_apr: BigDecimal,
    /// Asset farms
    pub farms: Vec<AssetFarmView>,
}

pub struct AssetFarmView {
    pub farm_id: FarmId,
    /// Active rewards for the farm
    pub rewards: HashMap<TokenId, AssetFarmReward>,
}

pub struct AssetFarm {
    #[serde(with = "u64_dec_format")]
    pub block_timestamp: Timestamp,
    /// Active rewards for the farm
    pub rewards: HashMap<TokenId, AssetFarmReward>,
}

pub struct AssetFarmReward {
    /// The amount of reward distributed per day.
    #[serde(with = "u128_dec_format")]
    pub reward_per_day: Balance,
    /// The log base for the booster. Used to compute boosted shares per account.
    /// Including decimals of the booster.
    #[serde(with = "u128_dec_format")]
    pub booster_log_base: Balance,

    /// The amount of rewards remaining to distribute.
    #[serde(with = "u128_dec_format")]
    pub remaining_rewards: Balance,

    /// The total number of boosted shares.
    #[serde(with = "u128_dec_format")]
    pub boosted_shares: Balance,
    #[serde(skip)]
    pub reward_per_share: BigDecimal,
}

pub struct Asset {
    /// Total supplied including collateral, but excluding reserved.
    pub supplied: Pool,
    /// Total borrowed.
    pub borrowed: Pool,
    /// The amount reserved for the stability. This amount can also be borrowed and affects
    /// borrowing rate.
    #[serde(with = "u128_dec_format")]
    pub reserved: Balance,
    /// The amount belongs to the protocol. This amount can also be borrowed and affects
    /// borrowing rate.
    #[serde(with = "u128_dec_format")]
    pub prot_fee: Balance,
    /// When the asset was last updated. It's always going to be the current block timestamp.
    #[serde(with = "u64_dec_format")]
    pub last_update_timestamp: Timestamp,
    /// The asset config.
    pub config: AssetConfig,
}

pub struct Pool {
    pub shares: Shares,
    #[serde(with = "u128_dec_format")]
    pub balance: Balance,
}

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
///   "net_tvl_multiplier": 0
///   "max_change_rate": None,
///   "supplied_limit": "340282366920938463463374607431768211455",
///   "borrowed_limit": "340282366920938463463374607431768211455",
/// }
/// ```
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

pub struct AssetAmount {
    pub token_id: TokenId,
    /// The amount of tokens intended to be used for the action.
    /// If `None`, then the maximum amount will be tried.
    pub amount: Option<WrappedBalance>,
    /// The maximum amount of tokens that can be used for the action.
    /// If `None`, then the maximum `available` amount will be used.
    pub max_amount: Option<WrappedBalance>,
}

/// Contract config
pub struct Config {
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

pub enum Action {
    Withdraw(AssetAmount),
    IncreaseCollateral(AssetAmount),
    PositionIncreaseCollateral{
        position: String,
        asset_amount: AssetAmount
    },
    DecreaseCollateral(AssetAmount),
    PositionDecreaseCollateral{
        position: String,
        asset_amount: AssetAmount
    },
    Borrow(AssetAmount),
    PositionBorrow{
        position: String,
        asset_amount: AssetAmount
    },
    Repay(AssetAmount),
    PositionRepay{
        position: String,
        asset_amount: AssetAmount
    },
    Liquidate {
        account_id: AccountId,
        in_assets: Vec<AssetAmount>,
        out_assets: Vec<AssetAmount>,
        position: Option<String>,
        min_token_amounts: Option<Vec<U128>>
    },
    /// If the sum of burrowed assets exceeds the collateral, the account will be liquidated
    /// using reserves.
    ForceClose {
        account_id: AccountId,
        position: Option<String>,
        min_token_amounts: Option<Vec<U128>>
    },
}

pub enum MarginAction {
    Withdraw {
        token_id: AccountId,
        amount: Option<U128>,
    },
    IncreaseCollateral {
        pos_id: PosId,
        amount: U128,
    },
    DecreaseCollateral {
        pos_id: PosId,
        amount: U128,
    },
    OpenPosition {
        token_c_id: AccountId,
        token_c_amount: U128,
        token_d_id: AccountId,
        token_d_amount: U128,
        token_p_id: AccountId,
        min_token_p_amount: U128,
        swap_indication: SwapIndication,
    },
    DecreaseMTPosition {
        pos_id: PosId,
        token_p_amount: U128,
        min_token_d_amount: U128,
        swap_indication: SwapIndication,
    },
    CloseMTPosition {
        pos_id: PosId,
        token_p_amount: U128,
        min_token_d_amount: U128,
        swap_indication: SwapIndication,
    },
    LiquidateMTPosition {
        pos_owner_id: AccountId,
        pos_id: PosId,
        token_p_amount: U128,
        min_token_d_amount: U128,
        swap_indication: SwapIndication,
    },
    ForceCloseMTPosition {
        pos_owner_id: AccountId,
        pos_id: PosId,
        token_p_amount: U128,
        min_token_d_amount: U128,
        swap_indication: SwapIndication,
    },
}

pub enum TokenReceiverMsg {
    Execute { actions: Vec<Action> },
    ExecuteWithPyth { actions: Vec<Action> },
    DepositToReserve,
    DepositToMargin,
    MarginExecute { actions: Vec<MarginAction> },
    MarginExecuteWithPyth { actions: Vec<MarginAction> },
    SwapReference { swap_ref: SwapReference },
}

enum PriceReceiverMsg {
    Execute { actions: Vec<Action> },
}

pub type TokenId = AccountId;
```

## Also storage management

```rust
pub struct StorageBalance {
    pub total: U128,
    pub available: U128,
}

pub struct StorageBalanceBounds {
    pub min: U128,
    pub max: Option<U128>,
}

pub trait StorageManagement {
    // if `registration_only=true` MUST refund above the minimum balance if the account didn't exist and
    //     refund full deposit if the account exists.
    fn storage_deposit(
        &mut self,
        account_id: Option<ValidAccountId>,
        registration_only: Option<bool>,
    ) -> StorageBalance;

    /// Withdraw specified amount of available Ⓝ for predecessor account.
    ///
    /// This method is safe to call. It MUST NOT remove data.
    ///
    /// `amount` is sent as a string representing an unsigned 128-bit integer. If
    /// omitted, contract MUST refund full `available` balance. If `amount` exceeds
    /// predecessor account's available balance, contract MUST panic.
    ///
    /// If predecessor account not registered, contract MUST panic.
    ///
    /// MUST require exactly 1 yoctoNEAR attached balance to prevent restricted
    /// function-call access-key call (UX wallet security)
    ///
    /// Returns the StorageBalance structure showing updated balances.
    fn storage_withdraw(&mut self, amount: Option<U128>) -> StorageBalance;

    /// Unregestering the account is not allowed to not break the order of accounts.
    fn storage_unregister(&mut self, force: Option<bool>) -> bool;

    fn storage_balance_bounds(&self) -> StorageBalanceBounds;

    fn storage_balance_of(&self, account_id: ValidAccountId) -> Option<StorageBalance>;
}
```
