use crate::*;

#[derive(BorshSerialize, BorshDeserialize, Serialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct MarginAccount {
    /// A copy of an account ID. Saves one storage_read when iterating on accounts.
    pub account_id: AccountId,
    /// A list of assets that are supplied by the account (but not used a collateral).
    /// It's not returned for account pagination.
    pub supplied: HashMap<TokenId, Shares>,
    // margin trading related
    pub margin_positions: HashMap<PosId, MarginTradingPosition>,
    /// Tracks changes in storage usage by persistent collections in this account.
    #[borsh_skip]
    #[serde(skip)]
    pub storage_tracker: StorageTracker,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub enum VMarginAccount {
    Current(MarginAccount),
}

// impl VMarginAccount {
//     pub fn into_margin_account(self) -> MarginAccount {
//         match self {
//             VMarginAccount::Current(c) => c,
//         }
//     }
// }

impl From<MarginAccount> for VMarginAccount {
    fn from(c: MarginAccount) -> Self {
        VMarginAccount::Current(c)
    }
}

impl From<VMarginAccount> for MarginAccount {
    fn from(c: VMarginAccount) -> Self {
        match c {
            VMarginAccount::Current(c) => c,
        }
    }
}

impl MarginAccount {
    pub fn new(account_id: &AccountId) -> Self {
        Self {
            account_id: account_id.clone(),
            supplied: HashMap::new(),
            margin_positions: HashMap::new(),
            storage_tracker: Default::default(),
        }
    }

    pub fn withdraw_supply_shares(&mut self, token_id: &AccountId, shares: &Shares) {
        let supply_shares = self.supplied.get_mut(token_id).unwrap();
        if let Some(new_balance) = supply_shares.0.checked_sub(shares.0) {
            supply_shares.0 = new_balance;
        } else {
            env::panic_str("Not enough asset balance");
        }
    }

    pub fn deposit_supply_shares(&mut self, token_id: &AccountId, shares: &Shares) {
        let supply_shares = self.supplied.get_mut(token_id).unwrap();
        supply_shares.0 += shares.0;
    }
}

impl Contract {
    pub fn internal_get_margin_account(&self, account_id: &AccountId) -> Option<MarginAccount> {
        self.margin_accounts
            .get(account_id)
            .map(|o| o.into())
    }

    pub fn internal_unwrap_margin_account(&self, account_id: &AccountId) -> MarginAccount {
        self.internal_get_margin_account(account_id)
            .expect("Margin account is not registered")
    }

    pub fn internal_set_margin_account(&mut self, account_id: &AccountId, mut account: MarginAccount) {
        let mut storage = self.internal_unwrap_storage(account_id);
        storage
            .storage_tracker
            .consume(&mut account.storage_tracker);
        storage.storage_tracker.start();
        self.margin_accounts.insert(account_id, &account.into());
        storage.storage_tracker.stop();
        self.internal_set_storage(account_id, storage);
    }
}