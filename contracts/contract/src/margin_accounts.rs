use crate::*;
use events::emit::LostfoundSupplyShares;

#[derive(BorshSerialize, BorshDeserialize, Serialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct MarginStop {
    /// profit rate to collateral in BPS
    pub stop_profit: Option<u32>,
    /// loss rate to collateral in BPS
    pub stop_loss: Option<u32>,
    pub service_token_id: TokenId,
    pub service_token_amount: U128,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct MarginAccount {
    /// A copy of an account ID. Saves one storage_read when iterating on accounts.
    pub account_id: AccountId,
    /// A list of assets that are supplied by the account (but not used a collateral).
    /// It's not returned for account pagination.
    pub supplied: HashMap<TokenId, Shares>,
    // margin trading related
    pub margin_positions: UnorderedMap<PosId, MarginTradingPosition>,
    // Record the timestamp of the position initiating the swap action.
    pub position_latest_actions: HashMap<PosId, U64>,
    // margin stops
    pub stops: HashMap<PosId, MarginStop>,

    /// Tracks changes in storage usage by persistent collections in this account.
    #[borsh_skip]
    pub storage_tracker: StorageTracker,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub enum VMarginAccount {
    V0(MarginAccountV0),
    V1(MarginAccountV1),
    Current(MarginAccount),
}

impl From<MarginAccount> for VMarginAccount {
    fn from(c: MarginAccount) -> Self {
        VMarginAccount::Current(c)
    }
}

impl From<VMarginAccount> for MarginAccount {
    fn from(c: VMarginAccount) -> Self {
        match c {
            VMarginAccount::V0(c) => c.into(),
            VMarginAccount::V1(c) => c.into(),
            VMarginAccount::Current(c) => c,
        }
    }
}

impl MarginAccount {
    pub(crate) fn new(account_id: &AccountId) -> Self {
        Self {
            account_id: account_id.clone(),
            supplied: HashMap::new(),
            margin_positions: UnorderedMap::new(StorageKey::MarginPositions { 
                account_id: account_id.clone()
            }),
            position_latest_actions: HashMap::new(),
            stops: HashMap::new(),
            storage_tracker: Default::default(),
        }
    }

    pub(crate) fn withdraw_supply_shares(&mut self, token_id: &AccountId, shares: &Shares) {
        let supply_shares = self.supplied.remove(token_id).unwrap();
        if let Some(new_balance) = supply_shares.0.checked_sub(shares.0) {
            if new_balance > 0 {
                self.supplied.insert(token_id.clone(), new_balance.into());
            }
        } else {
            env::panic_str("Not enough asset balance");
        }
    }

    pub(crate) fn deposit_supply_shares(&mut self, token_id: &AccountId, shares: &Shares) {
        if let Some(supply_shares) = self.supplied.get_mut(token_id) {
            supply_shares.0 += shares.0;
        } else {
            self.supplied.insert(token_id.clone(), shares.clone());
        }
    }
}

impl Contract {
    pub(crate) fn internal_get_margin_account(&self, account_id: &AccountId) -> Option<MarginAccount> {
        self.margin_accounts
            .get(account_id)
            .map(|o| o.into())
    }

    pub(crate) fn internal_unwrap_margin_account(&self, account_id: &AccountId) -> MarginAccount {
        // if inner account exists, would auto create margin account if needed
        if let Some(account) = self.internal_get_margin_account(account_id){
            account
        } else {
            if self.internal_get_account(account_id, true).is_some() {
                MarginAccount::new(account_id)
            } else {
                env::panic_str("Account is not registered")
            }
        }
    }

    pub(crate) fn internal_set_margin_account(&mut self, account_id: &AccountId, mut account: MarginAccount) {
        let mut storage = self.internal_unwrap_storage(account_id);
        storage
            .storage_tracker
            .consume(&mut account.storage_tracker);
        storage.storage_tracker.start();
        self.margin_accounts.insert(account_id, &account.into());
        storage.storage_tracker.stop();
        self.internal_set_storage(account_id, storage);
    }

    // Used for situations where the margin account may exceed storage limits due to the addition of a new supplied type, but cannot panic.
    pub(crate) fn internal_set_margin_account_without_panic(
        &mut self, 
        account_id: &AccountId, 
        mut account: MarginAccount, 
        asset_debt: &mut Asset,
        asset_position: &mut Asset,
        margin_account_updates: MarginAccountUpdates,
    ) {
        // Retrieve the account before the update, used for rolling back the state in case of insufficient storage.
        let old_margin_account = self.internal_unwrap_margin_account(account_id);
        let mut storage = self.internal_unwrap_storage(account_id);
        storage
            .storage_tracker
            .consume(&mut account.storage_tracker);
        storage.storage_tracker.start();
        self.margin_accounts.insert(account_id, &account.into());
        storage.storage_tracker.stop();
        if !self.internal_set_storage_without_panic(account_id, storage) {
            // Rollback the state.
            self.margin_accounts.insert(account_id, &old_margin_account.into());
            // Store the shares in asset.lostfound_shares.
            margin_account_token_shares_to_lostfound(account_id, asset_debt, asset_position, margin_account_updates);
        }
    }

    /// true if set is OK, false if set encountered storage problem and account unchanged
    pub(crate) fn try_to_set_margin_account(&mut self, account_id: &AccountId, mut account: MarginAccount) -> bool {
        let old_margin_account = self.internal_unwrap_margin_account(account_id);
        let mut storage = self.internal_unwrap_storage(account_id);
        storage
            .storage_tracker
            .consume(&mut account.storage_tracker);
        storage.storage_tracker.start();
        self.margin_accounts.insert(account_id, &account.into());
        storage.storage_tracker.stop();
        if !self.internal_set_storage_without_panic(account_id, storage) {
            // Rollback the state.
            self.margin_accounts.insert(account_id, &old_margin_account.into());
            false
        } else {
            true
        }
    }
}

#[derive(Serialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, Deserialize))]
#[serde(crate = "near_sdk::serde")]
pub struct MarginAccountDetailedView {
    pub account_id: AccountId,
    /// A list of assets that are supplied by the account (but not used a collateral).
    pub supplied: Vec<AssetView>,
    pub margin_positions: HashMap<PosId, MarginTradingPositionView>,
    pub position_latest_actions: HashMap<PosId, U64>,
}

#[derive(Serialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, Deserialize))]
#[serde(crate = "near_sdk::serde")]
pub struct MarginTradingPositionView {
    /// Used for convenient view
    pub open_ts: Timestamp,
    /// Record the unit accumulated holding-position interest when open
    #[serde(with = "u128_dec_format")]
    pub uahpi_at_open: Balance,
    /// The capital of debt, used for calculate holding position fee
    #[serde(with = "u128_dec_format")]
    pub debt_cap: Balance,

    pub token_c_info: AssetView,

    pub token_d_info: AssetView,

    pub token_p_id: TokenId,
    #[serde(with = "u128_dec_format")]
    pub token_p_amount: Balance,

    pub is_locking: bool,
}

impl Contract {
    pub fn margin_account_into_detailed_view(&self, account: MarginAccount) -> MarginAccountDetailedView {
        MarginAccountDetailedView {
            account_id: account.account_id.clone(),
            supplied: account
                .supplied
                .into_iter()
                .map(|(token_id, shares)| self.get_asset_view(token_id, shares, false))
                .collect(),
            margin_positions: account
                .margin_positions
                .into_iter()
                .map(|(pos_id, mtp)| (pos_id, self.margin_trading_position_into_view(mtp)))
                .collect(),
            position_latest_actions: account.position_latest_actions.clone(),
        }
    }

    fn margin_trading_position_into_view(&self, mtp: MarginTradingPosition) -> MarginTradingPositionView{
        MarginTradingPositionView {
            open_ts: mtp.open_ts,
            uahpi_at_open: mtp.uahpi_at_open,
            debt_cap: mtp.debt_cap,
            token_c_info: self.get_asset_view(mtp.token_c_id, mtp.token_c_shares, false),
            token_d_info: self.get_margin_debt_asset_view(mtp.token_d_id, mtp.token_d_shares),
            token_p_id: mtp.token_p_id,
            token_p_amount: mtp.token_p_amount,
            is_locking: mtp.is_locking
        }
    }

    fn get_margin_debt_asset_view(&self, token_id: TokenId, shares: Shares) -> AssetView {
        let asset = self.internal_unwrap_asset(&token_id);
        let apr = asset.get_margin_debt_apr(self.internal_margin_config().margin_debt_discount_rate);
        let balance = asset.margin_debt.shares_to_amount(shares, true);
        AssetView {
            token_id,
            balance,
            shares,
            apr,
        }
    }
}

#[near_bindgen]
impl Contract {
    pub fn get_margin_account(&self, account_id: AccountId) -> Option<MarginAccountDetailedView> {
        self.internal_get_margin_account(&account_id)
            .map(|ma| self.margin_account_into_detailed_view(ma))
    }

    pub fn list_margin_accounts(&self, account_ids: Vec<AccountId>) -> Vec<Option<MarginAccountDetailedView>> {
        account_ids.iter().map(|account_id| {
            self.internal_get_margin_account(&account_id)
                .map(|ma| self.margin_account_into_detailed_view(ma))
        }).collect()
    }

    pub fn get_margin_accounts_paged(&self, from_index: Option<u64>, limit: Option<u64>) -> Vec<MarginAccountDetailedView> {
        let values = self.margin_accounts.values_as_vector();
        let from_index = from_index.unwrap_or(0);
        let limit = limit.unwrap_or(values.len());
        (from_index..std::cmp::min(values.len(), from_index + limit))
            .map(|index| self.margin_account_into_detailed_view(values.get(index).unwrap().into()))
            .collect()
    }

    pub fn get_num_margin_accounts(&self) -> u32 {
        self.margin_accounts.len() as _
    }
}

pub fn margin_account_token_shares_to_lostfound(
    account_id: &AccountId, 
    asset_debt: &mut Asset,
    asset_position: &mut Asset,
    margin_account_updates: MarginAccountUpdates,
) {
    asset_debt.lostfound_shares += margin_account_updates.get_token_d_amount();
    asset_position.lostfound_shares += margin_account_updates.get_token_p_amount();
    events::emit::lostfound_supply_shares(LostfoundSupplyShares {
        account_id: account_id.clone(),
        shares: margin_account_updates.to_hash_map()
    });
}
