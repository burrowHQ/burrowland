use crate::*;
use std::{cmp::{max, min}, collections::HashSet};

#[derive(BorshSerialize, BorshDeserialize, Serialize, Clone)]
#[serde(crate = "near_sdk::serde")]
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

#[derive(BorshSerialize, BorshDeserialize)]
pub enum VAccount {
    V0(AccountV0),
    V1(AccountV1),
    V2(AccountV2),
    Current(Account),
}

impl VAccount {
    pub fn into_account(self, is_view: bool) -> Account {
        match self {
            VAccount::V0(c) => c.into_account(is_view),
            VAccount::V1(c) => c.into_account(is_view),
            VAccount::V2(c) => c.into_account(),
            VAccount::Current(c) => c,
        }
    }
}

impl From<Account> for VAccount {
    fn from(c: Account) -> Self {
        VAccount::Current(c)
    }
}

impl Account {
    pub fn new(account_id: &AccountId) -> Self {
        Self {
            account_id: account_id.clone(),
            supplied: HashMap::new(),
            positions: HashMap::new(),
            farms: HashMap::new(),
            affected_farms: HashSet::new(),
            storage_tracker: Default::default(),
            booster_staking: None,
            is_locked: false
        }
    }

    pub fn increase_collateral(&mut self, position: &String, token_id: &TokenId, shares: Shares) {
        let position_info = self.positions.entry(position.clone())
            .or_insert(Position::new(position));
        position_info.increase_collateral(token_id, shares);
    }

    pub fn decrease_collateral(&mut self, position: &String, token_id: &TokenId, shares: Shares) {
        let position_info = self.positions.get_mut(position).unwrap();
        position_info.decrease_collateral(token_id, shares);
        if position_info.is_empty() {
            self.positions.remove(position);
        }
    }

    pub fn increase_borrowed(&mut self, position: &String, token_id: &TokenId, shares: Shares) {
        let position_info = self.positions.get_mut(position).unwrap();
        position_info.increase_borrowed(token_id, shares);
    }

    pub fn decrease_borrowed(&mut self, position: &String, token_id: &TokenId, shares: Shares) {
        let position_info = self.positions.get_mut(position).unwrap();
        position_info.decrease_borrowed(token_id, shares);
    }

    pub fn internal_unwrap_collateral(&mut self, position: &String, token_id: &TokenId) -> Shares {
        self.positions.get(position)
            .expect("Position not found")
            .internal_unwrap_collateral(token_id)
            
    }

    pub fn internal_unwrap_borrowed(&mut self, position: &String, token_id: &TokenId) -> Shares {
        self.positions.get(position)
            .expect("Position not found")
            .internal_unwrap_borrowed(token_id)
    }

    pub fn add_affected_farm(&mut self, farm_id: FarmId) -> bool {
        self.affected_farms.insert(farm_id)
    }

    /// Returns all assets that can be potentially farmed.
    pub fn get_all_potential_farms(&self) -> HashSet<FarmId> {
        let mut potential_farms = HashSet::new();
        potential_farms.insert(FarmId::NetTvl);
        potential_farms.extend(self.supplied.keys().cloned().map(FarmId::Supplied));
        potential_farms.extend(self.supplied.keys().cloned().map(FarmId::TokenNetBalance));
        self.positions.iter().for_each(|(position, position_info)| {
            match position_info {
                Position::RegularPosition(regular_position) => {
                    potential_farms.extend(regular_position.collateral.keys().cloned().map(FarmId::Supplied));
                    potential_farms.extend(regular_position.collateral.keys().cloned().map(FarmId::TokenNetBalance));
                    potential_farms.extend(regular_position.borrowed.keys().cloned().map(FarmId::Borrowed));
                    potential_farms.extend(regular_position.borrowed.keys().cloned().map(FarmId::TokenNetBalance));
                }
                Position::LPTokenPosition(lp_token_position) => {
                    potential_farms.insert(FarmId::Supplied(AccountId::new_unchecked(position.clone())));
                    potential_farms.insert(FarmId::TokenNetBalance(AccountId::new_unchecked(position.clone())));
                    potential_farms.extend(lp_token_position.borrowed.keys().cloned().map(FarmId::Borrowed));
                    potential_farms.extend(lp_token_position.borrowed.keys().cloned().map(FarmId::TokenNetBalance));
                }
            }
        });
        potential_farms
    }

    pub fn get_supplied_shares(&self, token_id: &TokenId) -> Shares {
        let collateral_shares = self.positions.iter().fold(0u128, |acc, (position, position_info)|{
            match position_info {
                Position::RegularPosition(regular_position) => {
                    acc + regular_position.collateral.get(&token_id).map(|s| s.0).unwrap_or(0)
                }
                Position::LPTokenPosition(lp_token_position) => {
                    if token_id.to_string().eq(position) {
                        acc + lp_token_position.collateral.0
                    } else {
                        acc
                    }
                }
            }
        });
        let supplied_shares = self
            .internal_get_asset(token_id)
            .map(|asset| asset.shares.0)
            .unwrap_or(0);
        (supplied_shares + collateral_shares).into()
    }

    pub fn get_borrowed_shares(&self, token_id: &TokenId) -> Shares {
        self.positions.iter().fold(0u128, |acc, (_, position_info)|{
            match position_info {
                Position::RegularPosition(regular_position) => {
                    acc + regular_position.borrowed.get(&token_id).map(|s| s.0).unwrap_or(0)
                }
                Position::LPTokenPosition(lp_token_position) => {
                    acc + lp_token_position.borrowed.get(&token_id).map(|s| s.0).unwrap_or(0)
                }
            }
        }).into()
    }

    pub fn get_assets_num(&self) -> u32 {
        self.positions.iter().fold(0usize, |acc, (_, position_info)| {
            match position_info {
                Position::RegularPosition(regular_position) => {
                    acc + regular_position.collateral.len() + regular_position.borrowed.len()
                }
                Position::LPTokenPosition(lp_token_position) => {
                    acc + 1 + lp_token_position.borrowed.len()
                }
            }
        }) as u32
    }

    pub fn sync_booster_policy(&mut self, config: &Config) {
        if let Some(booster_staking) = self.booster_staking.as_mut() {
            let timestamp = env::block_timestamp();
            if booster_staking.unlock_timestamp > timestamp {
                let remain_duration_ns = booster_staking.unlock_timestamp - timestamp;
                let maximum_staking_duration_ns = to_nano(config.maximum_staking_duration_sec);
                let max_x_booster_amount = compute_x_booster_amount(
                    config,
                    booster_staking.staked_booster_amount,
                    maximum_staking_duration_ns,
                );
                let recount_duration_ns = max(
                    to_nano(config.minimum_staking_duration_sec),
                    min(remain_duration_ns, maximum_staking_duration_ns)
                );
                let recount_x_booster_amount = compute_x_booster_amount(
                    config,
                    booster_staking.staked_booster_amount,
                    recount_duration_ns,
                );
                booster_staking.x_booster_amount = min(
                    max_x_booster_amount,
                    max(booster_staking.x_booster_amount, recount_x_booster_amount)
                );
                if remain_duration_ns > maximum_staking_duration_ns {
                    booster_staking.unlock_timestamp = timestamp + maximum_staking_duration_ns;
                }
            } else {
                booster_staking.x_booster_amount = 0;
            }
        }
    }
}

impl Contract {
    pub fn internal_get_account(&self, account_id: &AccountId, is_view: bool) -> Option<Account> {
        self.accounts
            .get(account_id)
            .map(|o| o.into_account(is_view))
    }

    pub fn internal_unwrap_account(&self, account_id: &AccountId) -> Account {
        self.internal_get_account(account_id, false)
            .expect("Account is not registered")
    }

    pub fn internal_set_account(&mut self, account_id: &AccountId, mut account: Account) {
        let mut storage = self.internal_unwrap_storage(account_id);
        storage
            .storage_tracker
            .consume(&mut account.storage_tracker);
        storage.storage_tracker.start();
        self.accounts.insert(account_id, &account.into());
        storage.storage_tracker.stop();
        self.internal_set_storage(account_id, storage);
    }
}

#[near_bindgen]
impl Contract {
    /// Returns detailed information about an account for a given account_id.
    /// The information includes regular position supplied assets, collateral and borrowed.
    /// Each asset includes the current balance and the number of shares.
    pub fn get_account(&self, account_id: AccountId) -> Option<AccountDetailedView> {
        self.internal_get_account(&account_id, true)
            .map(|account| self.account_into_regular_position_detailed_view(account))
    }

    /// Returns detailed information about an account for a given account_id.
    /// The information includes all positions supplied assets, collateral and borrowed.
    /// Each asset includes the current balance and the number of shares.
    pub fn get_account_all_positions(&self, account_id: AccountId) -> Option<AccountAllPositionsDetailedView> {
        self.internal_get_account(&account_id, true)
            .map(|account| self.account_into_all_positions_detailed_view(account))
    }

    /// Returns limited account information for accounts from a given index up to a given limit.
    /// The information includes number of shares for collateral and borrowed assets.
    /// This method can be used to iterate on the accounts for liquidation.
    pub fn get_accounts_paged(&self, from_index: Option<u64>, limit: Option<u64>) -> Vec<Account> {
        let values = self.accounts.values_as_vector();
        let from_index = from_index.unwrap_or(0);
        let limit = limit.unwrap_or(values.len());
        (from_index..std::cmp::min(values.len(), from_index + limit))
            .map(|index| values.get(index).unwrap().into_account(true))
            .collect()
    }

    /// Returns the number of accounts
    pub fn get_num_accounts(&self) -> u32 {
        self.accounts.len() as _
    }
}
