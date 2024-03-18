mod account;
mod account_asset;
mod account_farm;
mod account_view;
mod actions;
mod asset;
mod asset_config;
mod asset_farm;
mod asset_view;
mod big_decimal;
mod booster_staking;
mod config;
mod events;
mod fungible_token;
mod legacy;
mod pool;
mod price_receiver;
mod prices;
mod storage;
mod storage_tracker;
mod upgrade;
mod utils;
mod shadow_actions;
mod position;
mod pyth;

pub use crate::account::*;
pub use crate::account_asset::*;
pub use crate::account_farm::*;
pub use crate::account_view::*;
pub use crate::actions::*;
pub use crate::asset::*;
pub use crate::asset_config::*;
pub use crate::asset_farm::*;
pub use crate::asset_view::*;
pub use crate::big_decimal::*;
pub use crate::booster_staking::*;
pub use crate::config::*;
pub use crate::fungible_token::*;
pub use crate::legacy::*;
pub use crate::pool::*;
pub use crate::price_receiver::*;
pub use crate::prices::*;
pub use crate::storage::*;
use crate::storage_tracker::*;
pub use crate::utils::*;
pub use crate::shadow_actions::*;
pub use crate::position::*;
pub use crate::pyth::*;

use common::*;

use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LazyOption, LookupMap, UnorderedMap, UnorderedSet};
use near_sdk::json_types::{I64, U64, U128};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{
    assert_one_yocto, env, ext_contract, log, near_bindgen, AccountId, Balance, BorshStorageKey,
    Duration, Gas, PanicOnDefault, Promise, Timestamp,
};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(BorshSerialize, BorshStorageKey)]
#[allow(unused)]
enum StorageKey {
    Accounts,
    AccountAssets { account_id: AccountId },
    AccountFarms { account_id: AccountId },
    Storage,
    Assets,
    AssetFarms,
    InactiveAssetFarmRewards { farm_id: FarmId },
    AssetIds,
    Config,
    Guardian,
    BlacklistOfFarmers,
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Contract {
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
}

#[near_bindgen]
impl Contract {
    /// Initializes the contract with the given config. Needs to be called once.
    #[init]
    pub fn new(config: Config) -> Self {
        config.assert_valid();
        Self {
            accounts: UnorderedMap::new(StorageKey::Accounts),
            storage: LookupMap::new(StorageKey::Storage),
            assets: LookupMap::new(StorageKey::Assets),
            asset_farms: LookupMap::new(StorageKey::AssetFarms),
            asset_ids: UnorderedSet::new(StorageKey::AssetIds),
            config: LazyOption::new(StorageKey::Config, Some(&config)),
            guardians: UnorderedSet::new(StorageKey::Guardian),
            last_prices: HashMap::new(),
            last_lp_token_infos: HashMap::new(),
            token_pyth_info: HashMap::new(),
            blacklist_of_farmers: UnorderedSet::new(StorageKey::BlacklistOfFarmers),
        }
    }

    /// Extend guardians. Only can be called by owner.
    /// - Requires one yoctoNEAR.
    #[payable]
    pub fn extend_guardians(&mut self, guardians: Vec<AccountId>) {
        assert_one_yocto();
        self.assert_owner();
        for guardian in guardians {
            self.guardians.insert(&guardian);
        }
    }

    /// Remove guardians. Only can be called by owner.
    /// - Requires one yoctoNEAR.
    #[payable]
    pub fn remove_guardians(&mut self, guardians: Vec<AccountId>) {
        assert_one_yocto();
        self.assert_owner();
        for guardian in guardians {
            let is_success = self.guardians.remove(&guardian);
            assert!(is_success, "Invalid guardian");
        }
    }

    /// Returns all guardians.s
    pub fn get_guardians(&self) -> Vec<AccountId> {
        self.guardians.to_vec()
    }

    /// Add pyth info for the specified token. Only can be called by owner or guardians.
    /// - Requires one yoctoNEAR.
    #[payable]
    pub fn add_token_pyth_info(&mut self, token_id: TokenId, token_pyth_info: TokenPythInfo) {
        assert_one_yocto();
        self.assert_owner_or_guardians();
        assert!(!self.token_pyth_info.contains_key(&token_id), "Already exist");
        self.token_pyth_info.insert(token_id, token_pyth_info);
    }

    /// Update pyth info for the specified token. Only can be called by owner or guardians.
    /// - Requires one yoctoNEAR.
    #[payable]
    pub fn update_token_pyth_info(&mut self, token_id: TokenId, token_pyth_info: TokenPythInfo) {
        assert_one_yocto();
        self.assert_owner_or_guardians();
        assert!(self.token_pyth_info.contains_key(&token_id), "Invalid token_id");
        self.token_pyth_info.insert(token_id, token_pyth_info);
    }

    /// Returns all pyth info.
    pub fn get_all_token_pyth_infos(&self) -> HashMap<TokenId, TokenPythInfo> {
        self.token_pyth_info.clone()
    }

    /// Return pyth information for the specified token.
    pub fn get_token_pyth_info(&self, token_id: TokenId) -> Option<TokenPythInfo> {
        self.token_pyth_info.get(&token_id).cloned()
    }

    /// Extend farmers to blacklist. Only can be called by owner or guardians.
    /// - Requires one yoctoNEAR.
    #[payable]
    pub fn extend_blacklist_of_farmers(&mut self, farmers: Vec<AccountId>) {
        assert_one_yocto();
        self.assert_owner_or_guardians();
        for farmer in farmers {
            self.blacklist_of_farmers.insert(&farmer);
        }
    }

    /// Remove farmers from blacklist. Only can be called by owner or guardians.
    /// - Requires one yoctoNEAR.
    #[payable]
    pub fn remove_blacklist_of_farmers(&mut self, farmers: Vec<AccountId>) {
        assert_one_yocto();
        self.assert_owner_or_guardians();
        for farmer in farmers {
            let is_success = self.blacklist_of_farmers.remove(&farmer);
            assert!(is_success, "Invalid farmer");
        }
    }

    /// Returns all farmers in the blacklist.
    pub fn get_blacklist_of_farmers(&self) -> Vec<AccountId> {
        self.blacklist_of_farmers.to_vec()
    }
}




#[cfg(test)]
mod unit_env {
    use super::*;
    use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
    use near_contract_standards::storage_management::StorageManagement;
    use near_sdk::json_types::U128;
    use near_sdk::test_utils::VMContextBuilder;
    pub use near_sdk::{testing_env, serde_json, AccountId, Balance};

    pub const MIN_DURATION_SEC: DurationSec = 2678400;
    pub const MAX_DURATION_SEC: DurationSec = 31536000;

    pub struct UnitEnv{
        pub contract: Contract,
        pub context: VMContextBuilder
    }

    impl UnitEnv {
        pub fn init_users(&mut self){
            testing_env!(self.context.predecessor_account_id(owner_id()).attached_deposit(d(1, 23)).build());
            self.contract.storage_deposit(Some(alice()), None);
            testing_env!(self.context.predecessor_account_id(owner_id()).attached_deposit(d(1, 23)).build());
            self.contract.storage_deposit(Some(bob()), None);
            testing_env!(self.context.predecessor_account_id(owner_id()).attached_deposit(d(1, 23)).build());
            self.contract.storage_deposit(Some(charlie()), None);
            testing_env!(self.context.predecessor_account_id(owner_id()).attached_deposit(d(1, 23)).build());
            self.contract.storage_deposit(Some(owner_id()), None);
        }
        
        pub fn init_assets(&mut self){
            testing_env!(self.context.predecessor_account_id(owner_id()).attached_deposit(1).build());
            self.contract.add_asset(
                booster_token_id(),
                AssetConfig {
                    reserve_ratio: 2500,
                    prot_ratio: 0,
                    target_utilization: 8000,
                    target_utilization_rate: U128(1000000000008319516250272147),
                    max_utilization_rate: U128(1000000000039724853136740579),
                    volatility_ratio: 2000,
                    extra_decimals: 0,
                    can_deposit: true,
                    can_withdraw: true,
                    can_use_as_collateral: false,
                    can_borrow: false,
                    net_tvl_multiplier: 10000,
                });
            self.deposit_to_reserve(booster_token_id(), owner_id(), d(10000, 18));
            testing_env!(self.context.predecessor_account_id(owner_id()).attached_deposit(1).build());
            self.contract.add_asset(
                neth_token_id(),
                AssetConfig {
                    reserve_ratio: 2500,
                    prot_ratio: 0,
                    target_utilization: 8000,
                    target_utilization_rate: U128(1000000000001547125956667610),
                    max_utilization_rate: U128(1000000000039724853136740579),
                    volatility_ratio: 6000,
                    extra_decimals: 0,
                    can_deposit: true,
                    can_withdraw: true,
                    can_use_as_collateral: true,
                    can_borrow: true,
                    net_tvl_multiplier: 10000,
                });
            self.deposit_to_reserve(neth_token_id(), owner_id(), d(10000, 18));
            testing_env!(self.context.predecessor_account_id(owner_id()).attached_deposit(1).build());
            self.contract.add_asset(
                ndai_token_id(),
                AssetConfig {
                    reserve_ratio: 2500,
                    prot_ratio: 0,
                    target_utilization: 8000,
                    target_utilization_rate: U128(1000000000002440418605283556),
                    max_utilization_rate: U128(1000000000039724853136740579),
                    volatility_ratio: 9500,
                    extra_decimals: 0,
                    can_deposit: true,
                    can_withdraw: true,
                    can_use_as_collateral: true,
                    can_borrow: true,
                    net_tvl_multiplier: 10000,
                });
            self.deposit_to_reserve(ndai_token_id(), owner_id(), d(10000, 18));
            testing_env!(self.context.predecessor_account_id(owner_id()).attached_deposit(1).build());
            self.contract.add_asset(
                nusdt_token_id(),
                AssetConfig {
                    reserve_ratio: 2500,
                    prot_ratio: 0,
                    target_utilization: 8000,
                    target_utilization_rate: U128(1000000000002440418605283556),
                    max_utilization_rate: U128(1000000000039724853136740579),
                    volatility_ratio: 9500,
                    extra_decimals: 12,
                    can_deposit: true,
                    can_withdraw: true,
                    can_use_as_collateral: true,
                    can_borrow: true,
                    net_tvl_multiplier: 10000,
                });
            self.deposit_to_reserve(nusdt_token_id(), owner_id(), d(10000, 6));
            testing_env!(self.context.predecessor_account_id(owner_id()).attached_deposit(1).build());
            self.contract.add_asset(
                nusdc_token_id(),
                AssetConfig {
                    reserve_ratio: 2500,
                    prot_ratio: 0,
                    target_utilization: 8000,
                    target_utilization_rate: U128(1000000000002440418605283556),
                    max_utilization_rate: U128(1000000000039724853136740579),
                    volatility_ratio: 9500,
                    extra_decimals: 12,
                    can_deposit: true,
                    can_withdraw: true,
                    can_use_as_collateral: true,
                    can_borrow: true,
                    net_tvl_multiplier: 10000,
                });
            self.deposit_to_reserve(nusdc_token_id(), owner_id(), d(10000, 6));
            testing_env!(self.context.predecessor_account_id(owner_id()).attached_deposit(1).build());
            self.contract.add_asset(
                wnear_token_id(),
                AssetConfig {
                    reserve_ratio: 2500,
                    prot_ratio: 0,
                    target_utilization: 8000,
                    target_utilization_rate: U128(1000000000003593629036885046),
                    max_utilization_rate: U128(1000000000039724853136740579),
                    volatility_ratio: 6000,
                    extra_decimals: 0,
                    can_deposit: true,
                    can_withdraw: true,
                    can_use_as_collateral: true,
                    can_borrow: true,
                    net_tvl_multiplier: 10000,
                });
            self.deposit_to_reserve(wnear_token_id(), owner_id(), d(10000, 24));
        }
        
        pub fn contract_ft_transfer_call(
            &mut self,
            token_id: AccountId,
            sender_id: AccountId,
            amount: U128,
            msg: String,
        ) {
            testing_env!(self.context.predecessor_account_id(token_id).build());
            self.contract.ft_on_transfer(sender_id, amount, msg);
        }
        pub fn contract_oracle_call(&mut self, sender_id: AccountId, price_data: PriceData, msg: String) {
            testing_env!(self.context.predecessor_account_id(oracle_id()).build());
            self.contract.oracle_on_call(sender_id, price_data, msg);
        }
        pub fn deposit_to_reserve(
            &mut self,
            token_id: AccountId,
            sender_id: AccountId,
            amount: Balance
        ){
            self.contract_ft_transfer_call(token_id, sender_id, amount.into(), "\"DepositToReserve\"".to_string());
        }

        pub fn deposit(&mut self, 
            token_id: AccountId,
            sender_id: AccountId,
            amount: Balance
        ) {
            self.contract_ft_transfer_call(token_id, sender_id, amount.into(), "".to_string());
        }

        pub fn supply_to_collateral(&mut self, 
            token_id: AccountId,
            sender_id: AccountId,
            amount: Balance
        ) {
            let msg = serde_json::to_string(&TokenReceiverMsg::Execute {
                actions: vec![Action::IncreaseCollateral(AssetAmount {
                    token_id: token_id.clone(),
                    amount: None,
                    max_amount: None,
                })],
            }).unwrap();
            self.contract_ft_transfer_call(token_id, sender_id, amount.into(), msg);
        }

        pub fn borrow(&mut self, sender_id: AccountId, borrow_token_id: AccountId, borrow_amount: Balance, price_data: PriceData) {
            let msg = serde_json::to_string(&PriceReceiverMsg::Execute {
                actions: vec![Action::Borrow(AssetAmount {
                    token_id: borrow_token_id.clone(),
                    amount: Some(borrow_amount.into()),
                    max_amount: None,
                })],
            }).unwrap();
            self.contract_oracle_call(sender_id, price_data, msg);
        }

        pub fn liquidate(&mut self, sender_id: AccountId, liquidation_user: AccountId, price_data: PriceData, in_assets: Vec<AssetAmount>, out_assets: Vec<AssetAmount>){
            let msg = serde_json::to_string(&PriceReceiverMsg::Execute {
                actions: vec![
                    Action::Liquidate{
                    account_id: liquidation_user,
                    in_assets,
                    out_assets,
                    position: None,
                    min_token_amounts: None
                }],
            }).unwrap();
            self.contract_oracle_call(sender_id, price_data, msg);
        }
        
        pub fn force_close(&mut self, sender_id: AccountId, force_close_user: AccountId, price_data: PriceData) {
            let msg = serde_json::to_string(&PriceReceiverMsg::Execute {
                actions: vec![
                    Action::ForceClose { account_id: force_close_user, position: None, min_token_amounts: None }],
            }).unwrap();
            self.contract_oracle_call(sender_id, price_data, msg);
        }

        pub fn borrow_and_withdraw(&mut self, sender_id: AccountId, borrow_token_id: AccountId, borrow_amount: Balance, price_data: PriceData) {
            let msg = serde_json::to_string(&PriceReceiverMsg::Execute {
                actions: vec![Action::Borrow(AssetAmount {
                    token_id: borrow_token_id.clone(),
                    amount: Some(borrow_amount.into()),
                    max_amount: None,
                }),
                Action::Withdraw(AssetAmount {
                    token_id: borrow_token_id.clone(),
                    amount: Some(borrow_amount.into()),
                    max_amount: None,
                })],
            }).unwrap();
            self.contract_oracle_call(sender_id, price_data, msg);
        }

        pub fn add_farm(&mut self,
            farm_id: FarmId,
            reward_token_id: AccountId,
            new_reward_per_day: Balance,
            new_booster_log_base: Balance,
            reward_amount: Balance
        ){
            testing_env!(self.context.predecessor_account_id(owner_id()).attached_deposit(1).build());
            self.contract.add_asset_farm_reward(farm_id, reward_token_id, new_reward_per_day.into(), new_booster_log_base.into(), reward_amount.into());
        }

        pub fn account_farm_claim_all(&mut self, account_id: AccountId){
            clean_assets_cache();
            clean_assets_farm_cache();
            self.contract.account_farm_claim_all(Some(account_id));
        }

        pub fn skip_time_to_by_ms(&mut self, ms: u64){
            testing_env!(self.context.block_timestamp(ms * 10u64.pow(6)).build());
        }

        pub fn skip_time_to_by_sec(&mut self, sec: u32){
            testing_env!(self.context.block_timestamp(sec as u64 * 10u64.pow(9)).build());
        }

        pub fn get_asset(&self, token_id: AccountId) ->  AssetDetailedView{
            clean_assets_cache();
            clean_assets_farm_cache();
            self.contract.get_asset(token_id).unwrap()
        }

        pub fn get_asset_farm(&self, farm_id: FarmId) -> AssetFarm{
            clean_assets_cache();
            clean_assets_farm_cache();
            self.contract.get_asset_farm(farm_id).unwrap()
        }
        
    }

    pub fn unit_price_data(
        block_timestamp: u64,
        wnear_mul: Option<Balance>,
        neth_mul: Option<Balance>,
    ) -> PriceData {
        let mut prices = vec![
            AssetOptionalPrice {
                asset_id: ndai_token_id().to_string(),
                price: Some(Price {
                    multiplier: 10000,
                    decimals: 22,
                }),
            },
            AssetOptionalPrice {
                asset_id: nusdc_token_id().to_string(),
                price: Some(Price {
                    multiplier: 10000,
                    decimals: 10,
                }),
            },
            AssetOptionalPrice {
                asset_id: nusdt_token_id().to_string(),
                price: Some(Price {
                    multiplier: 10000,
                    decimals: 10,
                }),
            },
        ];
        if let Some(wnear_mul) = wnear_mul {
            prices.push(AssetOptionalPrice {
                asset_id: wnear_token_id().to_string(),
                price: Some(Price {
                    multiplier: wnear_mul,
                    decimals: 28,
                }),
            })
        }
        if let Some(neth_mul) = neth_mul {
            prices.push(AssetOptionalPrice {
                asset_id: neth_token_id().to_string(),
                price: Some(Price {
                    multiplier: neth_mul,
                    decimals: 22,
                }),
            })
        }
        PriceData {
            timestamp: block_timestamp,
            recency_duration_sec: 90,
            prices,
        }
    }

    pub fn oracle_id() -> AccountId {
        AccountId::new_unchecked("oracle_id".to_string())
    }
    pub fn pyth_oracle_id() -> AccountId {
        AccountId::new_unchecked("pyth".to_string())
    }
    pub fn ref_exchange_id() -> AccountId {
        AccountId::new_unchecked("ref_exchange_id".to_string())
    }
    pub fn owner_id() -> AccountId {
        AccountId::new_unchecked("owner_id".to_string())
    }
    pub fn booster_token_id() -> AccountId {
        AccountId::new_unchecked("booster_token_id".to_string())
    }
    pub fn neth_token_id() -> AccountId {
        AccountId::new_unchecked("neth_token_id".to_string())
    }
    pub fn ndai_token_id() -> AccountId {
        AccountId::new_unchecked("ndai_token_id".to_string())
    }
    pub fn nusdt_token_id() -> AccountId {
        AccountId::new_unchecked("nusdt_token_id".to_string())
    }
    pub fn nusdc_token_id() -> AccountId {
        AccountId::new_unchecked("nusdc_token_id".to_string())
    }
    pub fn wnear_token_id() -> AccountId {
        AccountId::new_unchecked("wnear_token_id".to_string())
    }
    pub fn alice() -> AccountId {
        AccountId::new_unchecked("alice".to_string())
    }
    pub fn bob() -> AccountId {
        AccountId::new_unchecked("bob".to_string())
    }
    pub fn charlie() -> AccountId {
        AccountId::new_unchecked("charlie".to_string())
    }
    pub fn d(value: Balance, decimals: u8) -> Balance {
        value * 10u128.pow(decimals as _)
    }

    pub fn init_unit_env() -> UnitEnv {
        let mut context = VMContextBuilder::new();
        testing_env!(context.predecessor_account_id(owner_id()).build());
        let contract = Contract::new(Config {
            oracle_account_id: oracle_id(),
            pyth_oracle_account_id: pyth_oracle_id(),
            ref_exchange_id: ref_exchange_id(),
            owner_id: owner_id(),
            booster_token_id: booster_token_id(),
            booster_decimals: 18,
            max_num_assets: 10,
            maximum_recency_duration_sec: 90,
            maximum_staleness_duration_sec: 15,
            lp_tokens_info_valid_duration_sec: 600,
            pyth_price_valid_duration_sec: 60,
            minimum_staking_duration_sec: 2678400,
            maximum_staking_duration_sec: 31536000,
            x_booster_multiplier_at_maximum_staking_duration: 40000,
            force_closing_enabled: true,
            enable_price_oracle: true,
            enable_pyth_oracle: false,
            boost_suppress_factor: 1,
        });
        let mut test_env = UnitEnv{
            contract,
            context
        };
        test_env.init_assets();
        test_env.init_users();
        test_env
    }

    pub fn find_asset<'a>(assets: &'a [AssetView], token_id: &AccountId) -> &'a AssetView {
        assets
            .iter()
            .find(|e| &e.token_id == token_id)
            .expect("Missing asset")
    }

    pub fn assert_balances(actual: &[AssetView], expected: &[AssetView]) {
        assert_eq!(actual.len(), expected.len());
        for asset in actual {
            assert_eq!(asset.balance, find_asset(expected, &asset.token_id).balance);
        }
    }

    pub fn av(token_id: AccountId, balance: Balance) -> AssetView {
        AssetView {
            token_id,
            balance,
            shares: U128(0),
            apr: Default::default(),
        }
    }

    pub fn almost_eq(a: u128, b: u128, prec: u32) {
        let p = 10u128.pow(27 - prec);
        let ap = (a + p / 2) / p;
        let bp = (b + p / 2) / p;
        assert_eq!(
            ap,
            bp,
            "{}",
            format!("Expected {} to eq {}, with precision {}", a, b, prec)
        );
    }

    pub fn asset_amount(token_id: AccountId, amount: Balance) -> AssetAmount {
        AssetAmount {
            token_id,
            amount: Some(amount.into()),
            max_amount: None,
        }
    }
}

#[cfg(test)]
mod basic {
    use super::*;
    use unit_env::*;

    #[test]
    #[ignore]
    fn test_borrow() {
        let mut test_env = init_unit_env();
        // println!("{:?}", test_env.contract.get_assets_paged_detailed(None, None));
        let supply_amount = d(100, 24);
        test_env.supply_to_collateral(wnear_token_id(), alice(), supply_amount.into());
        // println!("{:?}", test_env.contract.get_account(alice()));
        let borrow_amount = d(200, 18);
        test_env.borrow(alice(), ndai_token_id(), borrow_amount, unit_price_data(0, Some(100000), None));
        println!("{:?}", test_env.contract.get_account(alice()));
        let asset = test_env.contract.get_asset(ndai_token_id()).unwrap();
        assert_eq!(asset.borrowed.balance, borrow_amount);
        assert!(asset.borrow_apr > BigDecimal::zero());
        assert_eq!(asset.supplied.balance, borrow_amount);
        assert!(asset.supply_apr > BigDecimal::zero());
        let account = test_env.contract.get_account(alice()).unwrap();
        assert_eq!(account.supplied[0].balance, borrow_amount);
        assert_eq!(account.supplied[0].token_id, ndai_token_id());
        assert!(account.supplied[0].apr > BigDecimal::zero());
        assert_eq!(account.borrowed[0].balance, borrow_amount);
        assert_eq!(account.borrowed[0].token_id, ndai_token_id());
        assert!(account.borrowed[0].apr > BigDecimal::zero());
    }

    #[test]
    #[ignore]
    fn test_borrow_and_withdraw() {
        let mut test_env = init_unit_env();
        let supply_amount = d(100, 24);
        test_env.supply_to_collateral(wnear_token_id(), alice(), supply_amount);
        let borrow_amount = d(200, 18);
        test_env.borrow_and_withdraw(alice(), ndai_token_id(), borrow_amount, unit_price_data(0, Some(100000), None));
        let asset = test_env.contract.get_asset(ndai_token_id()).unwrap();
        assert_eq!(asset.borrowed.balance, borrow_amount);
        assert!(asset.borrow_apr > BigDecimal::zero());
        assert_eq!(asset.supplied.balance, 0);
        assert_eq!(asset.supply_apr, BigDecimal::zero());
        let account = test_env.contract.get_account(alice()).unwrap();
        assert!(account.supplied.is_empty());
        assert_eq!(account.borrowed[0].balance, borrow_amount);
        assert_eq!(account.borrowed[0].token_id, ndai_token_id());
        assert!(account.borrowed[0].apr > BigDecimal::zero());
    }

    #[test]
    #[ignore]
    fn test_interest() {
        let mut test_env = init_unit_env();
        let supply_amount = d(10000, 24);
        test_env.supply_to_collateral(wnear_token_id(), alice(), supply_amount);
        let borrow_amount = d(8000, 18);
        test_env.borrow_and_withdraw(alice(), ndai_token_id(), borrow_amount, unit_price_data(0, Some(100000), None));
        let asset = test_env.contract.get_asset(ndai_token_id()).unwrap();
        assert_eq!(asset.borrowed.balance, borrow_amount);
        approx::assert_relative_eq!(asset.borrow_apr.f64(), 0.08f64);
        test_env.skip_time_to_by_ms(MS_PER_YEAR);
        let expected_borrow_amount = borrow_amount * 108 / 100;
        let asset = test_env.get_asset(ndai_token_id());
        approx::assert_relative_eq!(asset.borrowed.balance as f64, expected_borrow_amount as f64);
        let account = test_env.contract.get_account(alice()).unwrap();
        approx::assert_relative_eq!(
            account.borrowed[0].balance as f64,
            expected_borrow_amount as f64
        );
        assert_eq!(account.borrowed[0].token_id, ndai_token_id());
    }

    #[test]
    #[ignore]
    fn test_withdraw_prot_fee_reserved() {
        let mut test_env = init_unit_env();
        let amount = d(100, 18);
        test_env.deposit(ndai_token_id(), alice(), amount);
        let supply_amount = d(100, 24);
        test_env.supply_to_collateral(wnear_token_id(), alice(), supply_amount);
        let borrow_amount = d(200, 18);
        test_env.borrow(alice(), ndai_token_id(), borrow_amount, unit_price_data(0, Some(100000), None));
        test_env.skip_time_to_by_ms(MS_PER_YEAR);
        let asset_view_old = test_env.get_asset(ndai_token_id());
        assert_eq!(asset_view_old.prot_fee, 0);
        let mut new_config = asset_view_old.config;
        new_config.prot_ratio = 10000;
        testing_env!(test_env.context.predecessor_account_id(owner_id()).attached_deposit(1).build());
        test_env.contract.update_asset(ndai_token_id(), new_config.clone());
        
        test_env.skip_time_to_by_ms(MS_PER_YEAR * 2);
        
        let asset = test_env.get_asset(ndai_token_id());
        assert_eq!(asset_view_old.reserved, asset.reserved);
        testing_env!(test_env.context.predecessor_account_id(owner_id()).attached_deposit(1).build());
        test_env.contract.claim_prot_fee(ndai_token_id(), Some(10000.into()));
        let asset_after_decrease_prot_fee = test_env.get_asset(ndai_token_id());
        assert_eq!(asset.prot_fee - 10000, asset_after_decrease_prot_fee.prot_fee);
        assert_eq!(asset.supplied.balance + 10000, asset_after_decrease_prot_fee.supplied.balance);
        assert_eq!(asset.supplied.shares.0 + asset.supplied.amount_to_shares(10000, false).0, 
            asset_after_decrease_prot_fee.supplied.shares.0);
        testing_env!(test_env.context.predecessor_account_id(owner_id()).attached_deposit(1).build());
        test_env.contract.decrease_reserved(ndai_token_id(), Some(10000.into()));
        let asset_after_decrease_reserved = test_env.get_asset(ndai_token_id());
        assert_eq!(asset_after_decrease_prot_fee.reserved - 10000, asset_after_decrease_reserved.reserved);
        assert_eq!(asset_after_decrease_prot_fee.supplied.balance + 10000, asset_after_decrease_reserved.supplied.balance);
        assert_eq!(asset_after_decrease_prot_fee.supplied.shares.0 + asset.supplied.amount_to_shares(10000, false).0, 
            asset_after_decrease_reserved.supplied.shares.0);
        let account_before_increase_reserved = test_env.contract.get_account(owner_id()).unwrap();
        let asset_before_increase_reserved = test_env.get_asset(ndai_token_id());
        let (shares, amount) =
            asset_amount_to_shares(
                &asset_before_increase_reserved.supplied, 
            account_before_increase_reserved.supplied[0].shares, 
            &AssetAmount{
                token_id: ndai_token_id(),
                amount: Some(500.into()),
                max_amount: None
            }, 
            false);
        test_env.contract.increase_reserved(AssetAmount{
                token_id: ndai_token_id(),
                amount: Some(500.into()),
                max_amount: None
            });
        let asset_after_increase_reserved = test_env.get_asset(ndai_token_id());
        assert_eq!(asset_before_increase_reserved.reserved + amount, asset_after_increase_reserved.reserved);
        assert_eq!(asset_before_increase_reserved.supplied.shares.0 - shares.0, asset_after_increase_reserved.supplied.shares.0);
        assert_eq!(asset_before_increase_reserved.supplied.balance - amount, asset_after_increase_reserved.supplied.balance);
        let account_after_increase_reserved = test_env.contract.get_account(owner_id()).unwrap();
        assert_eq!(account_before_increase_reserved.supplied[0].shares.0 - shares.0, account_after_increase_reserved.supplied[0].shares.0);
    }

}


#[cfg(test)]
mod booster {
    use super::*;
    use unit_env::*;

    #[test]
    #[ignore]
    fn test_booster_stake_unstake() {
        let mut test_env = init_unit_env();

        let amount = d(100, 18);
        test_env.deposit(booster_token_id(), alice(), amount);

        let asset = test_env.get_asset(booster_token_id());
        assert_eq!(asset.supplied.balance, amount);

        let account = test_env.contract.get_account(alice()).unwrap();
        assert_eq!(account.supplied[0].balance, amount);
        assert_eq!(account.supplied[0].token_id, booster_token_id());
        assert!(account.booster_staking.is_none());

        let duration_sec: DurationSec = MAX_DURATION_SEC;

        testing_env!(test_env.context.predecessor_account_id(alice()).attached_deposit(1).build());
        test_env.contract.account_stake_booster(Some(amount.into()), duration_sec);
        
        let asset = test_env.get_asset(booster_token_id());
        assert_eq!(asset.supplied.balance, 0);

        let account = test_env.contract.get_account(alice()).unwrap();
        assert!(account.supplied.is_empty());
        let booster_staking = account.booster_staking.unwrap();
        assert_eq!(booster_staking.staked_booster_amount, amount);
        assert_eq!(booster_staking.x_booster_amount, amount * 4);
        assert_eq!(
            booster_staking.unlock_timestamp,
            sec_to_nano(duration_sec)
        );

        test_env.skip_time_to_by_sec(duration_sec / 2 );

        let account = test_env.contract.get_account(alice()).unwrap();
        assert!(account.supplied.is_empty());
        let booster_staking = account.booster_staking.unwrap();
        assert_eq!(booster_staking.x_booster_amount, amount * 4);

        test_env.skip_time_to_by_sec(duration_sec);

        let account = test_env.contract.get_account(alice()).unwrap();
        assert!(account.supplied.is_empty());
        let booster_staking = account.booster_staking.unwrap();
        assert_eq!(booster_staking.x_booster_amount, amount * 4);

        testing_env!(test_env.context.predecessor_account_id(alice()).attached_deposit(1).build());
        test_env.contract.account_unstake_booster();

        let asset = test_env.get_asset(booster_token_id());
        assert_eq!(asset.supplied.balance, amount);

        let account = test_env.contract.get_account(alice()).unwrap();
        assert_eq!(account.supplied[0].balance, amount);
        assert_eq!(account.supplied[0].token_id, booster_token_id());
        assert!(account.booster_staking.is_none());
    }


    #[test]
    #[ignore]
    fn test_booster_add_stake() {
        let mut test_env = init_unit_env();

        let amount = d(100, 18);
        test_env.deposit(booster_token_id(), alice(), amount);

        let duration_sec: DurationSec = MAX_DURATION_SEC;

        testing_env!(test_env.context.predecessor_account_id(alice()).attached_deposit(1).build());
        test_env.contract.account_stake_booster(Some((amount / 2).into()), duration_sec);

        let asset = test_env.get_asset(booster_token_id());
        assert_eq!(asset.supplied.balance, amount / 2);

        let account = test_env.contract.get_account(alice()).unwrap();
        assert_eq!(account.supplied[0].balance, amount / 2);
        assert_eq!(account.supplied[0].token_id, booster_token_id());
        let booster_staking = account.booster_staking.unwrap();
        assert_eq!(booster_staking.staked_booster_amount, amount / 2);
        assert_eq!(booster_staking.x_booster_amount, amount / 2 * 4);
        assert_eq!(
            booster_staking.unlock_timestamp,
            sec_to_nano(duration_sec)
        );

        test_env.skip_time_to_by_sec(duration_sec / 2 );

        testing_env!(test_env.context.predecessor_account_id(alice()).attached_deposit(1).build());
        test_env.contract.account_stake_booster(Some((amount / 2).into()), duration_sec / 2);

        let asset = test_env.get_asset(booster_token_id());
        assert_eq!(asset.supplied.balance, 0);

        let account = test_env.contract.get_account(alice()).unwrap();
        assert!(account.supplied.is_empty());
        let booster_staking = account.booster_staking.unwrap();
        assert_eq!(booster_staking.staked_booster_amount, amount);
        assert_eq!(
            booster_staking.x_booster_amount,
            amount / 2 * 4
                + amount / 2
                    * u128::from(
                        MAX_DURATION_SEC - MIN_DURATION_SEC + (duration_sec / 2 - MIN_DURATION_SEC) * 3
                    )
                    / u128::from(MAX_DURATION_SEC - MIN_DURATION_SEC)
        );
        assert_eq!(
            booster_staking.unlock_timestamp,
            sec_to_nano(duration_sec)
        );

        test_env.skip_time_to_by_sec(duration_sec);

        testing_env!(test_env.context.predecessor_account_id(alice()).attached_deposit(1).build());
        test_env.contract.account_unstake_booster();

        let asset = test_env.get_asset(booster_token_id());
        assert_eq!(asset.supplied.balance, amount);

        let account = test_env.contract.get_account(alice()).unwrap();
        assert_eq!(account.supplied[0].balance, amount);
        assert_eq!(account.supplied[0].token_id, booster_token_id());
        assert!(account.booster_staking.is_none());
    }

    #[test]
    #[ignore]
    fn test_booster_add_stake_extend_duration() {
        let mut test_env = init_unit_env();

        let amount = d(100, 18);
        test_env.deposit(booster_token_id(), alice(), amount);

        let duration_sec: DurationSec = MAX_DURATION_SEC;

        testing_env!(test_env.context.predecessor_account_id(alice()).attached_deposit(1).build());
        test_env.contract.account_stake_booster(Some((amount / 2).into()), duration_sec);
        
        test_env.skip_time_to_by_sec(duration_sec / 2 );

        testing_env!(test_env.context.predecessor_account_id(alice()).attached_deposit(1).build());
        test_env.contract.account_stake_booster(Some((amount / 2).into()), duration_sec);

        let asset = test_env.get_asset(booster_token_id());
        assert_eq!(asset.supplied.balance, 0);

        let account = test_env.contract.get_account(alice()).unwrap();
        assert!(account.supplied.is_empty());
        let booster_staking = account.booster_staking.unwrap();
        assert_eq!(booster_staking.staked_booster_amount, amount);
        assert_eq!(booster_staking.x_booster_amount, amount * 4);
        assert_eq!(
            booster_staking.unlock_timestamp,
            sec_to_nano(duration_sec * 3 / 2)
        );

        test_env.skip_time_to_by_sec(duration_sec + duration_sec / 2);
        test_env.contract.account_unstake_booster();

        let asset = test_env.get_asset(booster_token_id());
        assert_eq!(asset.supplied.balance, amount);

        let account = test_env.contract.get_account(alice()).unwrap();
        assert_eq!(account.supplied[0].balance, amount);
        assert_eq!(account.supplied[0].token_id, booster_token_id());
        assert!(account.booster_staking.is_none());
    }
}

#[cfg(test)]
mod farms {
    use super::*;
    use unit_env::*;

    pub const ONE_DAY_SEC: DurationSec = 24 * 60 * 60;

    #[test]
    #[ignore]
    fn test_farm_supplied() {
        let mut test_env = init_unit_env();

        // account_farm.block_timestamp init is 0, 
        // If you do not change the current time, 
        // it will be considered that the claim has just been completed.
        // The current timestamp in the real world is definitely not 0
        test_env.skip_time_to_by_sec(10);

        let reward_per_day = d(100, 18);
        let total_reward = d(3000, 18);

        let farm_id = FarmId::Supplied(ndai_token_id());
        test_env.add_farm(farm_id.clone(), booster_token_id(), reward_per_day, d(100, 18), total_reward);

        let asset = test_env.get_asset(ndai_token_id());
        assert_eq!(asset.farms.len(), 1);
        assert_eq!(asset.farms[0].farm_id, farm_id);
        let booster_reward = asset.farms[0]
            .rewards
            .get(&&booster_token_id())
            .cloned()
            .unwrap();
        assert_eq!(booster_reward.remaining_rewards, total_reward);

        let amount = d(100, 18);
        test_env.deposit(ndai_token_id(), alice(), amount);

        let asset = test_env.get_asset(booster_token_id());
        assert_eq!(asset.supplied.balance, 0);

        let asset = test_env.get_asset(ndai_token_id());
        assert_eq!(asset.supplied.balance, amount);
        let booster_reward = asset.farms[0]
            .rewards
            .get(&booster_token_id())
            .cloned()
            .unwrap();
        assert_eq!(booster_reward.remaining_rewards, total_reward);

        let account = test_env.contract.get_account(alice()).unwrap();
        assert_balances(&account.supplied, &[av(ndai_token_id(), amount)]);

        assert_eq!(account.farms[0].farm_id, farm_id);
        assert_eq!(
            account.farms[0].rewards[0].reward_token_id,
            booster_token_id()
        );
        assert_eq!(
            account.farms[0].rewards[0].boosted_shares,
            find_asset(&account.supplied, &ndai_token_id())
                .shares
                .0,
        );
        assert_eq!(account.farms[0].rewards[0].unclaimed_amount, 0);

        test_env.skip_time_to_by_sec(10 + ONE_DAY_SEC * 3);

        let farmed_amount = reward_per_day * 3;

        let asset = test_env.get_asset(booster_token_id());
        assert_eq!(asset.supplied.balance, 0);

        let asset = test_env.get_asset(ndai_token_id());
        assert_eq!(asset.supplied.balance, amount);
        let booster_reward = asset.farms[0]
            .rewards
            .get(&booster_token_id())
            .cloned()
            .unwrap();
        assert_eq!(
            booster_reward.remaining_rewards,
            total_reward - farmed_amount
        );

        let account = test_env.contract.get_account(alice()).unwrap();
        assert_balances(&account.supplied, &[av(ndai_token_id(), amount)]);

        assert_eq!(account.farms[0].farm_id, farm_id);
        assert_eq!(
            account.farms[0].rewards[0].boosted_shares,
            find_asset(&account.supplied, &ndai_token_id())
                .shares
                .0,
        );
        assert_eq!(account.farms[0].rewards[0].unclaimed_amount, farmed_amount);

        test_env.account_farm_claim_all(alice());

        let asset = test_env.get_asset(booster_token_id());
        assert_eq!(asset.supplied.balance, farmed_amount);

        let asset = test_env.get_asset(ndai_token_id());
        assert_eq!(asset.supplied.balance, amount);
        let booster_reward = asset.farms[0]
            .rewards
            .get(&booster_token_id())
            .cloned()
            .unwrap();
        assert_eq!(
            booster_reward.remaining_rewards,
            total_reward - farmed_amount
        );

        let account = test_env.contract.get_account(alice()).unwrap();
        assert_balances(
            &account.supplied,
            &[
                av(ndai_token_id(), amount),
                av(booster_token_id(), farmed_amount),
            ],
        );

        assert_eq!(account.farms[0].farm_id, farm_id);
        assert_eq!(
            account.farms[0].rewards[0].boosted_shares,
            find_asset(&account.supplied, &ndai_token_id())
                .shares
                .0,
        );
        assert_eq!(account.farms[0].rewards[0].unclaimed_amount, 0);

        test_env.skip_time_to_by_sec(10 + ONE_DAY_SEC * 5);

        let asset = test_env.get_asset(booster_token_id());
        assert_eq!(asset.supplied.balance, farmed_amount);

        let asset = test_env.get_asset(ndai_token_id());
        assert_eq!(asset.supplied.balance, amount);
        let booster_reward = asset.farms[0]
            .rewards
            .get(&booster_token_id())
            .cloned()
            .unwrap();
        assert_eq!(
            booster_reward.remaining_rewards,
            total_reward - reward_per_day * 5
        );

        let account = test_env.contract.get_account(alice()).unwrap();
        assert_balances(
            &account.supplied,
            &[
                av(ndai_token_id(), amount),
                av(booster_token_id(), farmed_amount),
            ],
        );

        assert_eq!(account.farms[0].farm_id, farm_id);
        assert_eq!(
            account.farms[0].rewards[0].boosted_shares,
            find_asset(&account.supplied, &ndai_token_id())
                .shares
                .0,
        );
        assert_eq!(
            account.farms[0].rewards[0].unclaimed_amount,
            reward_per_day * 2
        );

        test_env.skip_time_to_by_sec(10 + ONE_DAY_SEC * 35);

        let asset = test_env.get_asset(ndai_token_id());
        assert_eq!(asset.supplied.balance, amount);
        let booster_reward = asset.farms[0]
            .rewards
            .get(&booster_token_id())
            .cloned()
            .unwrap();
        assert_eq!(booster_reward.remaining_rewards, 0);

        let account = test_env.contract.get_account(alice()).unwrap();
        assert_balances(
            &account.supplied,
            &[
                av(ndai_token_id(), amount),
                av(booster_token_id(), farmed_amount),
            ],
        );

        assert_eq!(account.farms[0].farm_id, farm_id);
        assert_eq!(
            account.farms[0].rewards[0].boosted_shares,
            find_asset(&account.supplied, &ndai_token_id())
                .shares
                .0,
        );
        assert_eq!(
            account.farms[0].rewards[0].unclaimed_amount,
            total_reward - farmed_amount
        );

        test_env.account_farm_claim_all(alice());

        let asset = test_env.get_asset(booster_token_id());
        assert_eq!(asset.supplied.balance, total_reward);

        let asset = test_env.get_asset(ndai_token_id());
        assert_eq!(asset.supplied.balance, amount);
        assert!(asset.farms[0]
            .rewards
            .get(&booster_token_id())
            .is_none());

        let account = test_env.contract.get_account(alice()).unwrap();
        assert_balances(
            &account.supplied,
            &[
                av(ndai_token_id(), amount),
                av(booster_token_id(), total_reward),
            ],
        );

        assert_eq!(account.farms[0].farm_id, farm_id);
        assert!(account.farms[0].rewards.is_empty());
        clean_assets_cache();
        clean_assets_farm_cache();
    }

    #[test]
    #[ignore]
    fn test_has_potential_farms() {
        let mut test_env = init_unit_env();

        test_env.skip_time_to_by_sec(10);

        let amount = d(100, 18);
        test_env.deposit(ndai_token_id(), alice(), amount);

        let account = test_env.contract.get_account(alice()).unwrap();
        assert!(!account.has_non_farmed_assets);

        let reward_per_day = d(100, 18);
        let total_reward = d(3000, 18);

        let farm_id = FarmId::Supplied(ndai_token_id());
        test_env.add_farm(farm_id.clone(), booster_token_id(), reward_per_day, d(100, 18), total_reward);
        
        let account = test_env.contract.get_account(alice()).unwrap();
        assert_eq!(account.farms.len(), 0);
        assert!(account.has_non_farmed_assets);

        test_env.contract.extend_blacklist_of_farmers(vec![alice()]);
        let account = test_env.contract.get_account(alice()).unwrap();
        assert_eq!(account.farms.len(), 0);
        assert!(!account.has_non_farmed_assets);

        test_env.contract.remove_blacklist_of_farmers(vec![alice()]);
        let account = test_env.contract.get_account(alice()).unwrap();
        assert_eq!(account.farms.len(), 0);
        assert!(account.has_non_farmed_assets);

        test_env.account_farm_claim_all(alice());

        let account = test_env.contract.get_account(alice()).unwrap();
        assert_eq!(account.farms.len(), 1);
        assert!(!account.has_non_farmed_assets);

        test_env.contract.extend_blacklist_of_farmers(vec![alice()]);
        test_env.account_farm_claim_all(alice());
        let account = test_env.contract.get_account(alice()).unwrap();
        assert_eq!(account.farms.len(), 0);
        assert!(!account.has_non_farmed_assets);
        
        clean_assets_cache();
        clean_assets_farm_cache();
    }

    #[test]
    #[ignore]
    fn test_adjust_boost_staking_policy() {
        let mut test_env = init_unit_env();

        test_env.skip_time_to_by_sec(10);

        let booster_amount = d(5, 18);
        test_env.deposit(booster_token_id(), alice(), booster_amount);
        
        testing_env!(test_env.context.predecessor_account_id(alice()).attached_deposit(1).build());
        test_env.contract.account_stake_booster(Some(booster_amount.into()), MAX_DURATION_SEC);

        let reward_per_day = d(100, 18);
        let total_reward = d(3000, 18);
        let booster_base = d(20, 18);

        let farm_id = FarmId::Supplied(ndai_token_id());
        test_env.add_farm(farm_id.clone(), nusdc_token_id(), reward_per_day, booster_base, total_reward);

        let amount = d(100, 18);
        test_env.deposit(ndai_token_id(), alice(), amount);
        test_env.account_farm_claim_all(alice());

        let account = test_env.contract.get_account(alice()).unwrap();
        let booster_staking = account.booster_staking.unwrap();
        assert_eq!(booster_staking.staked_booster_amount, d(5, 18));
        assert_eq!(booster_staking.x_booster_amount, d(20, 18));
        assert_eq!(booster_staking.unlock_timestamp, to_nano(10 + MAX_DURATION_SEC));
        assert_eq!(account.farms[0].rewards[0].boosted_shares, d(200, 18));
        
        testing_env!(test_env.context.predecessor_account_id(owner_id()).build());
        test_env.contract.adjust_boost_staking_policy(2678400, MAX_DURATION_SEC / 2, 40000);
        test_env.account_farm_claim_all(alice());
        
        let account = test_env.contract.get_account(alice()).unwrap();
        let booster_staking = account.booster_staking.unwrap();
        assert_eq!(booster_staking.staked_booster_amount, d(5, 18));
        assert_eq!(booster_staking.x_booster_amount, d(20, 18));
        assert_eq!(booster_staking.unlock_timestamp, to_nano(10 + MAX_DURATION_SEC / 2));
        assert_eq!(account.farms[0].rewards[0].boosted_shares, d(200, 18));

        testing_env!(test_env.context.predecessor_account_id(owner_id()).build());
        test_env.contract.adjust_boost_staking_policy(2678400, MAX_DURATION_SEC / 2, 20000);
        test_env.account_farm_claim_all(alice());

        let account = test_env.contract.get_account(alice()).unwrap();
        let booster_staking = account.booster_staking.unwrap();
        assert_eq!(booster_staking.staked_booster_amount, d(5, 18));
        assert_eq!(booster_staking.x_booster_amount, d(10, 18));
        assert_eq!(booster_staking.unlock_timestamp, to_nano(10 + MAX_DURATION_SEC / 2));
        assert_eq!(account.farms[0].rewards[0].boosted_shares, d(100, 18) + ((d(100, 18) as f64)
            * ((d(10, 18) as f64) / (10f64.powi(18))).log(20f64)) as u128);
        
        test_env.skip_time_to_by_sec(10 + MAX_DURATION_SEC / 2);

        test_env.account_farm_claim_all(alice());
        let account = test_env.contract.get_account(alice()).unwrap();
        let booster_staking = account.booster_staking.unwrap();
        assert_eq!(booster_staking.staked_booster_amount, d(5, 18));
        assert_eq!(booster_staking.x_booster_amount, 0);
        assert_eq!(booster_staking.unlock_timestamp, to_nano(10 + MAX_DURATION_SEC / 2));
        println!("{:?}", account.farms);
        assert!(account.farms[0].rewards.is_empty());

        test_env.skip_time_to_by_sec(10 + MAX_DURATION_SEC);
        
        let reward_per_day = d(100, 18);
        let total_reward = d(3000, 18);
        let booster_base = d(20, 18);

        let farm_id = FarmId::Supplied(ndai_token_id());
        test_env.add_farm(farm_id.clone(), nusdc_token_id(), reward_per_day, booster_base, total_reward);

        test_env.account_farm_claim_all(alice());
        let account = test_env.contract.get_account(alice()).unwrap();
        let booster_staking = account.booster_staking.unwrap();
        assert_eq!(booster_staking.staked_booster_amount, d(5, 18));
        assert_eq!(booster_staking.x_booster_amount, 0);
        assert_eq!(booster_staking.unlock_timestamp, to_nano(10 + MAX_DURATION_SEC / 2));
        assert_eq!(account.farms[0].rewards[0].boosted_shares, d(100, 18));

        clean_assets_cache();
        clean_assets_farm_cache();
    }

    #[test]
    #[ignore]
    fn test_adjust_boost_suppress_factor() {
        let mut test_env = init_unit_env();

        test_env.skip_time_to_by_sec(10);

        let booster_amount = d(5000, 18);
        test_env.deposit(booster_token_id(), alice(), booster_amount);
        
        testing_env!(test_env.context.predecessor_account_id(alice()).attached_deposit(1).build());
        test_env.contract.account_stake_booster(Some(booster_amount.into()), MAX_DURATION_SEC);

        let reward_per_day = d(100, 18);
        let total_reward = d(3000, 18);
        let booster_base = d(20, 18);

        let farm_id = FarmId::Supplied(ndai_token_id());
        test_env.add_farm(farm_id.clone(), nusdc_token_id(), reward_per_day, booster_base, total_reward);

        let amount = d(100, 18);
        test_env.deposit(ndai_token_id(), alice(), amount);
        test_env.account_farm_claim_all(alice());

        let account = test_env.contract.get_account(alice()).unwrap();
        let booster_staking = account.booster_staking.unwrap();
        assert_eq!(booster_staking.staked_booster_amount, d(5000, 18));
        assert_eq!(booster_staking.x_booster_amount, d(20000, 18));
        assert_eq!(booster_staking.unlock_timestamp, to_nano(10 + MAX_DURATION_SEC));
        assert_eq!(account.farms[0].rewards[0].boosted_shares, d(100, 18) + ((d(100, 18) as f64)
            * ((d(20000, 18) as f64) / (10f64.powi(18))).log(20f64)) as u128);

        testing_env!(test_env.context.predecessor_account_id(owner_id()).build());
        test_env.contract.adjust_boost_suppress_factor(1000);
        test_env.account_farm_claim_all(alice());
        let account = test_env.contract.get_account(alice()).unwrap();
        let booster_staking = account.booster_staking.unwrap();
        assert_eq!(booster_staking.staked_booster_amount, d(5000, 18));
        assert_eq!(booster_staking.x_booster_amount, d(20000, 18));
        assert_eq!(booster_staking.unlock_timestamp, to_nano(10 + MAX_DURATION_SEC));
        assert_eq!(account.farms[0].rewards[0].boosted_shares, d(100, 18) + ((d(100, 18) as f64)
            * ((d(20, 18) as f64) / (10f64.powi(18))).log(20f64)) as u128);

        clean_assets_cache();
        clean_assets_farm_cache();
    }

    #[test]
    #[ignore]
    fn test_adjust_boost_suppress_factor_restake() {
        let mut test_env = init_unit_env();

        testing_env!(test_env.context.predecessor_account_id(owner_id()).attached_deposit(1).build());
        test_env.contract.adjust_boost_staking_policy(MIN_DURATION_SEC, MAX_DURATION_SEC, 120000);

        test_env.skip_time_to_by_sec(10);

        let booster_amount = d(50, 18);
        test_env.deposit(booster_token_id(), alice(), booster_amount);
        
        testing_env!(test_env.context.predecessor_account_id(alice()).attached_deposit(1).build());
        test_env.contract.account_stake_booster(Some(d(5, 18).into()), MAX_DURATION_SEC);

        let account = test_env.contract.get_account(alice()).unwrap();
        let booster_staking = account.booster_staking.unwrap();
        assert_eq!(booster_staking.staked_booster_amount, d(5, 18));
        assert_eq!(booster_staking.x_booster_amount, d(60, 18));
        assert_eq!(booster_staking.unlock_timestamp, to_nano(10 + MAX_DURATION_SEC));

        test_env.skip_time_to_by_sec(10 + MIN_DURATION_SEC * 2);

        testing_env!(test_env.context.predecessor_account_id(alice()).attached_deposit(1).build());
        test_env.contract.account_stake_booster(Some(d(5, 18).into()), MAX_DURATION_SEC);
        let account = test_env.contract.get_account(alice()).unwrap();
        let booster_staking = account.booster_staking.unwrap();
        assert_eq!(booster_staking.staked_booster_amount, d(10, 18));
        assert_eq!(booster_staking.x_booster_amount, d(120, 18));
        assert_eq!(booster_staking.unlock_timestamp, to_nano(10 + MIN_DURATION_SEC * 2 + MAX_DURATION_SEC));

        testing_env!(test_env.context.predecessor_account_id(owner_id()).build());
        test_env.contract.adjust_boost_staking_policy(MIN_DURATION_SEC, MIN_DURATION_SEC * 3, 30000);

        testing_env!(test_env.context.predecessor_account_id(alice()).attached_deposit(1).build());
        test_env.contract.account_stake_booster(Some(d(5, 18).into()), MIN_DURATION_SEC * 3);
        let account = test_env.contract.get_account(alice()).unwrap();
        let booster_staking = account.booster_staking.unwrap();
        assert_eq!(booster_staking.staked_booster_amount, d(15, 18));
        assert_eq!(booster_staking.x_booster_amount, d(45, 18));
        assert_eq!(booster_staking.unlock_timestamp, to_nano(10 + MIN_DURATION_SEC * 5));

        test_env.skip_time_to_by_sec(10 + MIN_DURATION_SEC * 4);

        testing_env!(test_env.context.predecessor_account_id(owner_id()).build());
        test_env.contract.adjust_boost_staking_policy(MIN_DURATION_SEC, MAX_DURATION_SEC, 120000);

        testing_env!(test_env.context.predecessor_account_id(alice()).attached_deposit(1).build());
        test_env.contract.account_stake_booster(Some(d(5, 18).into()), MAX_DURATION_SEC);
        let account = test_env.contract.get_account(alice()).unwrap();
        let booster_staking = account.booster_staking.unwrap();
        assert_eq!(booster_staking.staked_booster_amount, d(20, 18));
        assert_eq!(booster_staking.x_booster_amount, d(240, 18));
        assert_eq!(booster_staking.unlock_timestamp, to_nano(10 + MIN_DURATION_SEC * 4 + MAX_DURATION_SEC));

        clean_assets_cache();
        clean_assets_farm_cache();
    }

    #[test]
    #[ignore]
    fn test_farm_supplied_xbooster() {
        let mut test_env = init_unit_env();

        test_env.skip_time_to_by_sec(10);

        let reward_per_day = d(100, 18);
        let total_reward = d(3000, 18);
        let booster_base = d(20, 18);

        let farm_id = FarmId::Supplied(ndai_token_id());
        test_env.add_farm(farm_id.clone(), nusdc_token_id(), reward_per_day, booster_base, total_reward);

        let booster_amount = d(5, 18);
        test_env.deposit(booster_token_id(), alice(), booster_amount);

        testing_env!(test_env.context.predecessor_account_id(alice()).attached_deposit(1).build());
        test_env.contract.account_stake_booster(Some(booster_amount.into()), MAX_DURATION_SEC);
    
        let amount = d(100, 18);
        test_env.deposit(ndai_token_id(), alice(), amount);

        let asset = test_env.get_asset(nusdc_token_id());
        assert_eq!(asset.supplied.balance, 0);

        let asset = test_env.get_asset(ndai_token_id());
        assert_eq!(asset.supplied.balance, amount);
        let booster_reward = asset.farms[0]
            .rewards
            .get(&nusdc_token_id())
            .cloned()
            .unwrap();
        assert_eq!(booster_reward.remaining_rewards, total_reward);
        assert_eq!(booster_reward.boosted_shares, asset.supplied.shares.0 * 2);

        let account = test_env.contract.get_account(alice()).unwrap();
        assert_balances(&account.supplied, &[av(ndai_token_id(), amount)]);

        let booster_staking = account.booster_staking.unwrap();
        assert_eq!(booster_staking.staked_booster_amount, booster_amount);
        assert_eq!(booster_staking.x_booster_amount, booster_amount * 4);

        // The amount of boosted shares should be 2X due to the log base.
        assert_eq!(
            account.farms[0].rewards[0].boosted_shares,
            find_asset(&account.supplied, &ndai_token_id())
                .shares
                .0
                * 2,
        );
        assert_eq!(account.farms[0].rewards[0].unclaimed_amount, 0);

        test_env.skip_time_to_by_sec(10 + ONE_DAY_SEC * 3);

        let farmed_amount = reward_per_day * 3;
        let asset = test_env.get_asset(ndai_token_id());
        let booster_reward = asset.farms[0]
            .rewards
            .get(&nusdc_token_id())
            .cloned()
            .unwrap();
        assert_eq!(
            booster_reward.remaining_rewards,
            total_reward - farmed_amount
        );

        let account = test_env.contract.get_account(alice()).unwrap();
        assert_eq!(account.farms[0].rewards[0].unclaimed_amount, farmed_amount);

        let booster_amount = d(95, 18);
        test_env.deposit(booster_token_id(), alice(), booster_amount);

        // Increasing booster stake updates all farms.
        testing_env!(test_env.context.predecessor_account_id(alice()).attached_deposit(1).build());
        test_env.contract.account_stake_booster(Some(booster_amount.into()), MAX_DURATION_SEC);

        let asset = test_env.get_asset(nusdc_token_id());
        assert_eq!(asset.supplied.balance, farmed_amount);

        let asset = test_env.get_asset(ndai_token_id());
        let booster_reward = asset.farms[0]
            .rewards
            .get(&nusdc_token_id())
            .cloned()
            .unwrap();
        assert_eq!(
            booster_reward.remaining_rewards,
            total_reward - farmed_amount
        );
        assert_eq!(booster_reward.boosted_shares, asset.supplied.shares.0 * 3);

        let account = test_env.contract.get_account(alice()).unwrap();
        assert_balances(
            &account.supplied,
            &[
                av(ndai_token_id(), amount),
                av(nusdc_token_id(), farmed_amount),
            ],
        );

        // The boosted amount should 3X because the xBooster is 400.
        assert_eq!(
            account.farms[0].rewards[0].boosted_shares,
            find_asset(&account.supplied, &ndai_token_id())
                .shares
                .0
                * 3,
        );
        assert_eq!(account.farms[0].rewards[0].unclaimed_amount, 0);
        let booster_staking = account.booster_staking.unwrap();
        assert_eq!(booster_staking.staked_booster_amount, d(100, 18));
        assert_eq!(booster_staking.x_booster_amount, d(400, 18));
        clean_assets_cache();
        clean_assets_farm_cache();
    }

    #[test]
    #[ignore]
    fn test_farm_supplied_xbooster_unstake() {
        let mut test_env = init_unit_env();

        test_env.skip_time_to_by_sec(10);

        let booster_amount = d(5, 18);
        test_env.deposit(booster_token_id(), alice(), booster_amount);

        testing_env!(test_env.context.predecessor_account_id(alice()).attached_deposit(1).build());
        test_env.contract.account_stake_booster(Some(booster_amount.into()), MAX_DURATION_SEC);

        test_env.skip_time_to_by_sec(10 + MAX_DURATION_SEC - ONE_DAY_SEC * 3);

        let reward_per_day = d(100, 18);
        let total_reward = d(3000, 18);
        let booster_base = d(20, 18);

        let farm_id = FarmId::Supplied(ndai_token_id());
        test_env.add_farm(farm_id.clone(), nusdc_token_id(), reward_per_day, booster_base, total_reward);

        let amount = d(100, 18);
        test_env.deposit(ndai_token_id(), alice(), amount);

        let asset = test_env.get_asset(nusdc_token_id());
        assert_eq!(asset.supplied.balance, 0);

        let asset = test_env.get_asset(ndai_token_id());
        assert_eq!(asset.supplied.balance, amount);
        let booster_reward = asset.farms[0]
            .rewards
            .get(&nusdc_token_id())
            .cloned()
            .unwrap();
        assert_eq!(booster_reward.remaining_rewards, total_reward);
        assert_eq!(booster_reward.boosted_shares, asset.supplied.shares.0 * 2);

        let account = test_env.contract.get_account(alice()).unwrap();

        // The amount of boosted shares should be 2X due to the log base.
        assert_eq!(
            account.farms[0].rewards[0].boosted_shares,
            find_asset(&account.supplied, &ndai_token_id())
                .shares
                .0
                * 2,
        );
        assert_eq!(account.farms[0].rewards[0].unclaimed_amount, 0);

        test_env.skip_time_to_by_sec(10 + MAX_DURATION_SEC);

        let farmed_amount = reward_per_day * 3;
        let asset = test_env.get_asset(ndai_token_id());
        let booster_reward = asset.farms[0]
            .rewards
            .get(&nusdc_token_id())
            .cloned()
            .unwrap();
        assert_eq!(
            booster_reward.remaining_rewards,
            total_reward - farmed_amount
        );

        let account = test_env.contract.get_account(alice()).unwrap();
        assert_eq!(account.farms[0].rewards[0].unclaimed_amount, farmed_amount);

        // Unstaking booster updates all farms.
        testing_env!(test_env.context.predecessor_account_id(alice()).attached_deposit(1).build());
        test_env.contract.account_unstake_booster();

        let asset = test_env.get_asset(nusdc_token_id());
        assert_eq!(asset.supplied.balance, farmed_amount);

        let asset = test_env.get_asset(ndai_token_id());
        let booster_reward = asset.farms[0]
            .rewards
            .get(&nusdc_token_id())
            .cloned()
            .unwrap();
        assert_eq!(
            booster_reward.remaining_rewards,
            total_reward - farmed_amount
        );
        // The boosted amount should 1X because of xBooster unstaking.
        assert_eq!(booster_reward.boosted_shares, asset.supplied.shares.0);

        let account = test_env.contract.get_account(alice()).unwrap();
        assert_balances(
            &account.supplied,
            &[
                av(ndai_token_id(), amount),
                av(booster_token_id(), booster_amount),
                av(nusdc_token_id(), farmed_amount),
            ],
        );

        assert_eq!(
            account.farms[0].rewards[0].boosted_shares,
            find_asset(&account.supplied, &ndai_token_id())
                .shares
                .0,
        );
        assert_eq!(account.farms[0].rewards[0].unclaimed_amount, 0);
        assert!(account.booster_staking.is_none());
        clean_assets_cache();
        clean_assets_farm_cache();
    }

    #[test]
    #[ignore]
    fn test_farm_supplied_two_users() {
        let mut test_env = init_unit_env();

        test_env.skip_time_to_by_sec(10);

        let booster_amount_alice = d(5, 18);
        test_env.deposit(booster_token_id(), alice(), booster_amount_alice);

        testing_env!(test_env.context.predecessor_account_id(alice()).attached_deposit(1).build());
        test_env.contract.account_stake_booster(Some(booster_amount_alice.into()), MAX_DURATION_SEC);

        let booster_amount_bob = d(100, 18);
        test_env.deposit(booster_token_id(), bob(), booster_amount_bob);

        testing_env!(test_env.context.predecessor_account_id(bob()).attached_deposit(1).build());
        test_env.contract.account_stake_booster(Some(booster_amount_bob.into()), MAX_DURATION_SEC);

        let reward_per_day = d(100, 18);
        let total_reward = d(3000, 18);
        let booster_base = d(20, 18);

        let farm_id = FarmId::Supplied(ndai_token_id());
        test_env.add_farm(farm_id.clone(), nusdc_token_id(), reward_per_day, booster_base, total_reward);

        let amount = d(100, 18);
        test_env.deposit(ndai_token_id(), alice(), amount);
        test_env.deposit(ndai_token_id(), bob(), amount);

        let asset = test_env.get_asset(nusdc_token_id());
        assert_eq!(asset.supplied.balance, 0);

        let asset = test_env.get_asset(ndai_token_id());
        assert_eq!(asset.supplied.balance, amount * 2);
        let booster_reward = asset.farms[0]
            .rewards
            .get(&nusdc_token_id())
            .cloned()
            .unwrap();
        assert_eq!(booster_reward.remaining_rewards, total_reward);
        // 2.5X (Alice 2X, Bob 3X)
        assert_eq!(
            booster_reward.boosted_shares,
            asset.supplied.shares.0 * 5 / 2
        );

        let account = test_env.contract.get_account(alice()).unwrap();

        assert_eq!(
            account.farms[0].rewards[0].boosted_shares,
            find_asset(&account.supplied, &ndai_token_id())
                .shares
                .0
                * 2,
        );
        assert_eq!(account.farms[0].rewards[0].unclaimed_amount, 0);

        let account = test_env.contract.get_account(bob()).unwrap();

        assert_eq!(
            account.farms[0].rewards[0].boosted_shares,
            find_asset(&account.supplied, &ndai_token_id())
                .shares
                .0
                * 3,
        );
        assert_eq!(account.farms[0].rewards[0].unclaimed_amount, 0);

        test_env.skip_time_to_by_sec(10 + ONE_DAY_SEC * 3);

        let farmed_amount = reward_per_day * 3;
        let asset = test_env.get_asset(ndai_token_id());
        let booster_reward = asset.farms[0]
            .rewards
            .get(&nusdc_token_id())
            .cloned()
            .unwrap();
        assert_eq!(
            booster_reward.remaining_rewards,
            total_reward - farmed_amount
        );

        let account = test_env.contract.get_account(alice()).unwrap();
        assert_eq!(
            account.farms[0].rewards[0].unclaimed_amount,
            farmed_amount * 2 / 5
        );

        let account = test_env.contract.get_account(bob()).unwrap();
        assert_eq!(
            account.farms[0].rewards[0].unclaimed_amount,
            farmed_amount * 3 / 5
        );

        let extra_booster_amount = d(95, 18);
        test_env.deposit(booster_token_id(), alice(), extra_booster_amount);

        // Increasing booster stake updates all farms.
        testing_env!(test_env.context.predecessor_account_id(alice()).attached_deposit(1).build());
        test_env.contract.account_stake_booster(Some(extra_booster_amount.into()), MAX_DURATION_SEC);

        let asset = test_env.get_asset(nusdc_token_id());
        // The amount of only for Alice, but Bob still unclaimed
        assert_eq!(asset.supplied.balance, farmed_amount * 2 / 5);

        let asset = test_env.get_asset(ndai_token_id());
        let booster_reward = asset.farms[0]
            .rewards
            .get(&nusdc_token_id())
            .cloned()
            .unwrap();
        assert_eq!(
            booster_reward.remaining_rewards,
            total_reward - farmed_amount
        );

        // Both Alice and Bob now have 3X booster
        assert_eq!(booster_reward.boosted_shares, asset.supplied.shares.0 * 3);

        let account = test_env.contract.get_account(alice()).unwrap();
        assert_balances(
            &account.supplied,
            &[
                av(ndai_token_id(), amount),
                av(nusdc_token_id(), farmed_amount * 2 / 5),
            ],
        );

        assert_eq!(
            account.farms[0].rewards[0].boosted_shares,
            find_asset(&account.supplied, &ndai_token_id())
                .shares
                .0
                * 3,
        );
        assert_eq!(account.farms[0].rewards[0].unclaimed_amount, 0);

        let account = test_env.contract.get_account(bob()).unwrap();
        assert_eq!(
            account.farms[0].rewards[0].unclaimed_amount,
            farmed_amount * 3 / 5
        );

        test_env.skip_time_to_by_sec(10 + ONE_DAY_SEC * 5);

        let asset = test_env.get_asset(nusdc_token_id());
        assert_eq!(asset.supplied.balance, farmed_amount * 2 / 5);

        let asset = test_env.get_asset(ndai_token_id());
        let booster_reward = asset.farms[0]
            .rewards
            .get(&nusdc_token_id())
            .cloned()
            .unwrap();
        assert_eq!(
            booster_reward.remaining_rewards,
            total_reward - reward_per_day * 5
        );

        let account = test_env.contract.get_account(alice()).unwrap();
        // Unclaimed half of the rewards for 2 days
        assert_eq!(
            account.farms[0].rewards[0].unclaimed_amount,
            reward_per_day * 2 / 2
        );

        let account = test_env.contract.get_account(bob()).unwrap();
        assert_eq!(
            account.farms[0].rewards[0].unclaimed_amount,
            farmed_amount * 3 / 5 + reward_per_day * 2 / 2
        );
        clean_assets_cache();
        clean_assets_farm_cache();
    }

    #[test]
    #[ignore]
    fn test_farm_net_tvl() {
        let mut test_env = init_unit_env();

        test_env.skip_time_to_by_sec(10);

        let reward_per_day = d(100, 18);
        let total_reward = d(3000, 18);

        let farm_id = FarmId::NetTvl;
        test_env.add_farm(farm_id.clone(), booster_token_id(), reward_per_day, d(100, 18), total_reward);

        let asset_farm = test_env.get_asset_farm(farm_id.clone());
        let booster_reward = asset_farm
            .rewards
            .get(&&booster_token_id())
            .cloned()
            .unwrap();
        assert_eq!(booster_reward.remaining_rewards, total_reward);

        let amount = d(100, 18);
        test_env.supply_to_collateral(ndai_token_id(), alice(), amount.into());
        
        // Borrow 1 NEAR
        let borrow_amount = d(1, 24);
        test_env.borrow_and_withdraw(alice(), wnear_token_id(), borrow_amount, unit_price_data(10, Some(100000), None));


        let asset = test_env.get_asset(booster_token_id());
        assert_eq!(asset.supplied.balance, 0);

        let account = test_env.contract.get_account(alice()).unwrap();
        assert_eq!(account.farms.len(), 1);
        assert_eq!(account.farms[0].farm_id, farm_id);
        assert_eq!(
            account.farms[0].rewards[0].reward_token_id,
            booster_token_id()
        );
        // The account should have 90$ of Net TVL. $100 from dai and 10$ wNEAR borrowed.
        assert_eq!(account.farms[0].rewards[0].boosted_shares, d(90, 18));
        assert_eq!(account.farms[0].rewards[0].unclaimed_amount, 0);

        test_env.skip_time_to_by_sec(10 + ONE_DAY_SEC * 3);

        let farmed_amount = reward_per_day * 3;

        let asset = test_env.get_asset(booster_token_id());
        assert_eq!(asset.supplied.balance, 0);

        let asset_farm = test_env.get_asset_farm(farm_id.clone());
        let booster_reward = asset_farm
            .rewards
            .get(&booster_token_id())
            .cloned()
            .unwrap();
        assert_eq!(
            booster_reward.remaining_rewards,
            total_reward - farmed_amount
        );

        let account = test_env.contract.get_account(alice()).unwrap();
        assert_eq!(account.farms[0].farm_id, farm_id);
        assert_eq!(account.farms[0].rewards[0].unclaimed_amount, farmed_amount);

        test_env.account_farm_claim_all(alice());

        let asset = test_env.get_asset(booster_token_id());
        assert_eq!(asset.supplied.balance, farmed_amount);

        let asset_farm = test_env.get_asset_farm(farm_id.clone());
        let booster_reward = asset_farm
            .rewards
            .get(&booster_token_id())
            .cloned()
            .unwrap();
        assert_eq!(
            booster_reward.remaining_rewards,
            total_reward - farmed_amount
        );

        let account = test_env.contract.get_account(alice()).unwrap();
        assert_balances(
            &account.supplied,
            &[av(booster_token_id(), farmed_amount)],
        );

        assert_eq!(account.farms[0].farm_id, farm_id);
        // Due to borrowing interest
        assert!(
            account.farms[0].rewards[0].boosted_shares >= d(89, 18)
                && account.farms[0].rewards[0].boosted_shares < d(90, 18)
        );
        assert_eq!(account.farms[0].rewards[0].unclaimed_amount, 0);
        clean_assets_cache();
        clean_assets_farm_cache();
    }

    #[test]
    #[ignore]
    fn test_farm_net_tvl_complex() {
        let mut test_env = init_unit_env();

        test_env.skip_time_to_by_sec(10);

        let reward_per_day = d(100, 18);
        let total_reward = d(3000, 18);

        let farm_id = FarmId::NetTvl;
        test_env.add_farm(farm_id.clone(), ndai_token_id(), reward_per_day, d(100, 18), total_reward);


        let asset_farm = test_env.get_asset_farm(farm_id.clone());
        let reward = asset_farm
            .rewards
            .get(&ndai_token_id())
            .cloned()
            .unwrap();
        assert_eq!(reward.remaining_rewards, total_reward);

        let amount = d(100, 18);
        test_env.supply_to_collateral(ndai_token_id(), alice(), amount.into());

        let bob_amount = d(30, 6);
        test_env.supply_to_collateral(nusdc_token_id(), bob(), bob_amount.into());

        let charlie_amount = d(40, 6);
        test_env.supply_to_collateral(nusdc_token_id(), charlie(), charlie_amount.into());

        let bob_borrow_amount = d(1, 24);
        test_env.borrow_and_withdraw(alice(), wnear_token_id(), bob_borrow_amount, unit_price_data(10, Some(100000), None));

        let charlie_borrow_amount = d(10, 18);
        test_env.borrow_and_withdraw(charlie(), nusdc_token_id(), charlie_borrow_amount, unit_price_data(10, Some(100000), None));

        let account = test_env.contract.get_account(alice()).unwrap();
        assert_eq!(account.farms.len(), 1);
        assert_eq!(account.farms[0].farm_id, farm_id);
        assert_eq!(
            account.farms[0].rewards[0].reward_token_id,
            ndai_token_id()
        );
        // The account should have 90$ of Net TVL. $100 from dai and 10$ wNEAR borrowed.
        assert_eq!(account.farms[0].rewards[0].boosted_shares, d(90, 18));
        assert_eq!(account.farms[0].rewards[0].unclaimed_amount, 0);

        // Bob doesn't have a farm, since there were no prices when bob made a deposit.
        let account = test_env.contract.get_account(bob()).unwrap();
        assert!(account.farms.is_empty());

        let asset_farm = test_env.get_asset_farm(farm_id.clone());
        let reward = asset_farm
            .rewards
            .get(&ndai_token_id())
            .cloned()
            .unwrap();
        assert_eq!(reward.boosted_shares, d(120, 18));

        test_env.account_farm_claim_all(bob());

        let account = test_env.contract.get_account(bob()).unwrap();
        assert_eq!(account.farms.len(), 1);
        assert_eq!(account.farms[0].farm_id, farm_id);
        assert_eq!(
            account.farms[0].rewards[0].reward_token_id,
            ndai_token_id()
        );
        // The account should have 30$ of Net TVL. $30 from usdc.
        assert_eq!(account.farms[0].rewards[0].boosted_shares, d(30, 18));
        assert_eq!(account.farms[0].rewards[0].unclaimed_amount, 0);

        test_env.account_farm_claim_all(charlie());


        let account = test_env.contract.get_account(charlie()).unwrap();
        assert_eq!(account.farms.len(), 1);
        assert_eq!(account.farms[0].farm_id, farm_id);
        assert_eq!(
            account.farms[0].rewards[0].reward_token_id,
            ndai_token_id()
        );
        // The account should have 30$ of Net TVL. $40 from usdt deposit - $10 from usdt borrow.
        assert_eq!(account.farms[0].rewards[0].boosted_shares, d(30, 18));
        assert_eq!(account.farms[0].rewards[0].unclaimed_amount, 0);

        let asset_farm = test_env.get_asset_farm(farm_id.clone());
        let reward = asset_farm
            .rewards
            .get(&ndai_token_id())
            .cloned()
            .unwrap();
        assert_eq!(reward.boosted_shares, d(150, 18));

        test_env.skip_time_to_by_sec(10 + ONE_DAY_SEC * 3);

        let farmed_amount = reward_per_day * 3;

        let asset_farm = test_env.get_asset_farm(farm_id.clone());
        let reward = asset_farm
            .rewards
            .get(&ndai_token_id())
            .cloned()
            .unwrap();
        assert_eq!(reward.remaining_rewards, total_reward - farmed_amount);

        let account = test_env.contract.get_account(alice()).unwrap();
        assert_eq!(account.farms[0].farm_id, farm_id);
        almost_eq(
            account.farms[0].rewards[0].unclaimed_amount,
            farmed_amount * 90 / 150,
            18
        );

        let bobs_farmed_amount = farmed_amount * 30 / 150;
        let account = test_env.contract.get_account(bob()).unwrap();
        assert_eq!(account.farms[0].farm_id, farm_id);
        assert_eq!(
            account.farms[0].rewards[0].unclaimed_amount,
            bobs_farmed_amount
        );

        test_env.account_farm_claim_all(bob());

        let account = test_env.contract.get_account(bob()).unwrap();
        assert_balances(
            &account.supplied,
            &[av(ndai_token_id(), bobs_farmed_amount)],
        );
        // 30$ usdc + 60$ ndai from farming rewards.
        almost_eq(account.farms[0].rewards[0].boosted_shares, d(30 + 60 , 18), 13);
        assert_eq!(account.farms[0].rewards[0].unclaimed_amount, 0);

        let asset_farm = test_env.get_asset_farm(farm_id.clone());
        let reward = asset_farm
            .rewards
            .get(&ndai_token_id())
            .cloned()
            .unwrap();
        almost_eq(reward.boosted_shares, d(120 + 90, 18), 13);

        let charlie_farmed_amount = farmed_amount * 30 / 150;
        let account = test_env.contract.get_account(charlie()).unwrap();
        assert_eq!(account.farms[0].farm_id, farm_id);
        assert_eq!(
            account.farms[0].rewards[0].unclaimed_amount,
            charlie_farmed_amount
        );

        test_env.account_farm_claim_all(charlie());

        let account = test_env.contract.get_account(charlie()).unwrap();
        assert_balances(
            &account.supplied,
            &[av(ndai_token_id(), charlie_farmed_amount)],
        );
        // 30$ usdt + 60$ ndai from farming rewards.
        almost_eq(account.farms[0].rewards[0].boosted_shares, d(30 + 60 , 18), 13);
        assert_eq!(account.farms[0].rewards[0].unclaimed_amount, 0);

        let asset_farm = test_env.get_asset_farm(farm_id.clone());
        let reward = asset_farm
            .rewards
            .get(&ndai_token_id())
            .cloned()
            .unwrap();
        almost_eq(reward.boosted_shares, d(120 + 150, 18), 13);
        clean_assets_cache();
        clean_assets_farm_cache();
    }

    #[test]
    #[ignore]
    fn test_farm_net_tvl_price_change() {
        let mut test_env = init_unit_env();

        test_env.skip_time_to_by_sec(10);

        let reward_per_day = d(100, 18);
        let total_reward = d(3000, 18);

        let farm_id = FarmId::NetTvl;
        test_env.add_farm(farm_id.clone(), ndai_token_id(), reward_per_day, d(100, 18), total_reward);

        let asset_farm = test_env.get_asset_farm(farm_id.clone());
        let reward = asset_farm
            .rewards
            .get(&ndai_token_id())
            .cloned()
            .unwrap();
        assert_eq!(reward.remaining_rewards, total_reward);

        let amount = d(100, 18);
        test_env.supply_to_collateral(ndai_token_id(), alice(), amount.into());

        let borrow_amount = d(2, 24);
        test_env.borrow_and_withdraw(alice(), wnear_token_id(), borrow_amount, unit_price_data(10, Some(100000), None));

        let account = test_env.contract.get_account(alice()).unwrap();
        assert_eq!(account.farms.len(), 1);
        assert_eq!(account.farms[0].farm_id, farm_id);
        assert_eq!(
            account.farms[0].rewards[0].reward_token_id,
            ndai_token_id()
        );
        // The account should have 80$ of Net TVL. $100 from dai and 20$ wNEAR borrowed.
        assert_eq!(account.farms[0].rewards[0].boosted_shares, d(80, 18));
        assert_eq!(account.farms[0].rewards[0].unclaimed_amount, 0);

        let amount = d(60, 18);
        test_env.supply_to_collateral(ndai_token_id(), bob(), amount.into());

        let borrow_amount = d(1, 24);
        test_env.borrow_and_withdraw(bob(), wnear_token_id(), borrow_amount, unit_price_data(10, Some(150000), None));

        let account = test_env.contract.get_account(bob()).unwrap();
        assert_eq!(account.farms.len(), 1);
        // The account should have 45$ of Net TVL. $60 from dai and 15$ wNEAR borrowed.
        assert_eq!(account.farms[0].rewards[0].boosted_shares, d(45, 18));

        let account = test_env.contract.get_account(alice()).unwrap();
        // The shares do not change until the account is affected.
        assert_eq!(account.farms[0].rewards[0].boosted_shares, d(80, 18));

        let asset_farm = test_env.get_asset_farm(farm_id.clone());
        let reward = asset_farm
            .rewards
            .get(&ndai_token_id())
            .cloned()
            .unwrap();
        assert_eq!(reward.boosted_shares, d(125, 18));

        test_env.account_farm_claim_all(alice());

        let account = test_env.contract.get_account(alice()).unwrap();
        // The account should have 80$ of Net TVL. $100 from dai and 30$ (2 * 15$) wNEAR borrowed.
        assert_eq!(account.farms[0].rewards[0].boosted_shares, d(70, 18));

        let asset_farm = test_env.get_asset_farm(farm_id.clone());
        let reward = asset_farm
            .rewards
            .get(&ndai_token_id())
            .cloned()
            .unwrap();
        assert_eq!(reward.boosted_shares, d(115, 18));
        clean_assets_cache();
        clean_assets_farm_cache();
    }

    #[test]
    #[ignore]
    fn test_farm_net_tvl_bad_debt() {
        let mut test_env = init_unit_env();

        test_env.skip_time_to_by_sec(10);

        let reward_per_day = d(100, 18);
        let total_reward = d(3000, 18);

        let farm_id = FarmId::NetTvl;
        test_env.add_farm(farm_id.clone(), ndai_token_id(), reward_per_day, d(100, 18), total_reward);

        let asset_farm = test_env.get_asset_farm(farm_id.clone());
        let reward = asset_farm
            .rewards
            .get(&ndai_token_id())
            .cloned()
            .unwrap();
        assert_eq!(reward.remaining_rewards, total_reward);

        let amount = d(100, 18);
        test_env.supply_to_collateral(ndai_token_id(), alice(), amount.into());

        let borrow_amount = d(4, 24);
        test_env.borrow_and_withdraw(alice(), wnear_token_id(), borrow_amount, unit_price_data(10, Some(100000), None));

        let account = test_env.contract.get_account(alice()).unwrap();
        // The account should have 60$ of Net TVL. $100 from dai and 40$ wNEAR borrowed.
        assert_eq!(account.farms[0].rewards[0].boosted_shares, d(60, 18));

        let amount = d(100, 18);
        test_env.supply_to_collateral(ndai_token_id(), bob(), amount.into());

        let borrow_amount = d(1, 24);
        test_env.borrow_and_withdraw(bob(), wnear_token_id(), borrow_amount, unit_price_data(10, Some(300000), None));

        test_env.account_farm_claim_all(alice());

        let account = test_env.contract.get_account(alice()).unwrap();
        // The account has bad debt (more borrowed than collateral), so no net-tvl farm.
        assert!(account.farms.is_empty());

        let asset_farm = test_env.get_asset_farm(farm_id.clone());
        let reward = asset_farm
            .rewards
            .get(&ndai_token_id())
            .cloned()
            .unwrap();
        // Bob only
        assert_eq!(reward.boosted_shares, d(70, 18));

        let borrow_amount = d(1, 24);
        test_env.borrow_and_withdraw(bob(), wnear_token_id(), borrow_amount, unit_price_data(10, Some(120000), None));

        let account = test_env.contract.get_account(bob()).unwrap();
        // 100 - 12 * 2
        assert_eq!(account.farms[0].rewards[0].boosted_shares, d(76, 18));

        test_env.account_farm_claim_all(alice());

        let account = test_env.contract.get_account(alice()).unwrap();
        // 100 - 12 * 4
        assert_eq!(account.farms[0].rewards[0].boosted_shares, d(52, 18));

        let asset_farm = test_env.get_asset_farm(farm_id.clone());
        let reward = asset_farm
            .rewards
            .get(&ndai_token_id())
            .cloned()
            .unwrap();
        // Bob and Alice
        assert_eq!(reward.boosted_shares, d(128, 18));
        clean_assets_cache();
        clean_assets_farm_cache();
    }

    #[test]
    #[ignore]
    fn test_farm_net_tvl_multipliers() {
        let mut test_env = init_unit_env();

        test_env.skip_time_to_by_sec(10);

        let reward_per_day = d(100, 18);
        let total_reward = d(3000, 18);

        let farm_id = FarmId::NetTvl;
        test_env.add_farm(farm_id.clone(), ndai_token_id(), reward_per_day, d(100, 18), total_reward);

        let asset_farm = test_env.get_asset_farm(farm_id.clone());
        let reward = asset_farm
            .rewards
            .get(&ndai_token_id())
            .cloned()
            .unwrap();
        assert_eq!(reward.remaining_rewards, total_reward);

        testing_env!(test_env.context.predecessor_account_id(owner_id()).attached_deposit(1).build());
        test_env.contract.update_asset(wnear_token_id(), AssetConfig {
            reserve_ratio: 2500,
            prot_ratio: 0,
            target_utilization: 8000,
            target_utilization_rate: 1000000000003593629036885046.into(),
            max_utilization_rate: 1000000000039724853136740579.into(),
            volatility_ratio: 6000,
            extra_decimals: 0,
            can_deposit: true,
            can_withdraw: true,
            can_use_as_collateral: true,
            can_borrow: true,
            net_tvl_multiplier: 8000,
        });

        let amount = d(100, 18);
        test_env.supply_to_collateral(ndai_token_id(), alice(), amount.into());

        let borrow_amount = d(4, 24);
        test_env.borrow_and_withdraw(alice(), wnear_token_id(), borrow_amount, unit_price_data(10, Some(100000), None));

        let account = test_env.contract.get_account(alice()).unwrap();
        // 100 - 4 * 10 * 0.8
        assert_eq!(account.farms[0].rewards[0].boosted_shares, d(68, 18));

        // Deposit 10 wNEAR.
        let amount = d(10, 24);
        test_env.deposit(wnear_token_id(), alice(), amount);

        let account = test_env.contract.get_account(alice()).unwrap();
        // 100 - 4 * 10 * 0.8 + 10 * 10 * 0.8
        assert_eq!(account.farms[0].rewards[0].boosted_shares, d(148, 18));
        clean_assets_cache();
        clean_assets_farm_cache();
    }
}

#[cfg(test)]
mod liquidation {
    use super::*;
    use unit_env::*;

    #[test]
    #[ignore]
    fn test_liquidation_alice_by_bob() {
        let mut test_env = init_unit_env();
        test_env.skip_time_to_by_sec(10);

        let extra_decimals_mult = d(1, 12);

        let supply_amount = d(1000, 18);
        test_env.supply_to_collateral(nusdc_token_id(), alice(), (supply_amount / extra_decimals_mult).into());

        let borrow_amount = d(50, 24);
        test_env.borrow_and_withdraw(alice(), wnear_token_id(), borrow_amount, unit_price_data(10, Some(100000), None));

        let account = test_env.contract.get_account(alice()).unwrap();
        assert!(account.supplied.is_empty());
        assert_balances(
            &account.collateral,
            &[av(nusdc_token_id(), supply_amount)],
        );
        assert_balances(
            &account.borrowed,
            &[av(wnear_token_id(), borrow_amount)],
        );
        assert!(find_asset(&account.borrowed, &wnear_token_id()).apr > BigDecimal::zero());

        let bobs_amount = d(100, 24);
        test_env.deposit(wnear_token_id(), bob(), bobs_amount);

        let account = test_env.contract.get_account(bob()).unwrap();
        assert_balances(
            &account.supplied,
            &[av(wnear_token_id(), bobs_amount)],
        );
        assert!(find_asset(&account.supplied, &wnear_token_id()).apr > BigDecimal::zero());

        // Assuming 2% discount for 5 NEAR at 12$.
        let wnear_amount_in = d(49, 23);
        let usdc_amount_out = d(60, 18);
        test_env.liquidate(
            bob(), alice(), unit_price_data(10, Some(120000), None),
            vec![asset_amount(wnear_token_id(), wnear_amount_in)], vec![asset_amount(nusdc_token_id(), usdc_amount_out)],
        );

        let account = test_env.contract.get_account(alice()).unwrap();
        assert!(account.supplied.is_empty());
        assert_balances(
            &account.collateral,
            &[av(
                nusdc_token_id(),
                supply_amount - usdc_amount_out,
            )],
        );
        assert_balances(
            &account.borrowed,
            &[av(
                wnear_token_id(),
                borrow_amount - wnear_amount_in,
            )],
        );
        assert!(find_asset(&account.borrowed, &wnear_token_id()).apr > BigDecimal::zero());

        let account = test_env.contract.get_account(bob()).unwrap();
        assert_balances(
            &account.supplied,
            &[
                av(wnear_token_id(), bobs_amount - wnear_amount_in),
                av(nusdc_token_id(), usdc_amount_out),
            ],
        );
        assert!(find_asset(&account.supplied, &wnear_token_id()).apr > BigDecimal::zero());
        assert_eq!(
            find_asset(&account.supplied, &nusdc_token_id()).apr,
            BigDecimal::zero()
        );
    }

    /// Bob attemps to liquidate Alice which decreases health factor.
    #[test]
    #[ignore]
    fn test_liquidation_decrease_health_factor() {
        let mut test_env = init_unit_env();

        let extra_decimals_mult = d(1, 12);

        let supply_amount = d(1000, 18);
        test_env.supply_to_collateral(nusdc_token_id(), alice(), (supply_amount / extra_decimals_mult).into());

        let wnear_borrow_amount = d(50, 24);
        test_env.borrow_and_withdraw(alice(), wnear_token_id(), wnear_borrow_amount, unit_price_data(0, Some(100000), None));

        let usdt_borrow_amount = d(50, 18);
        test_env.borrow_and_withdraw(alice(), nusdt_token_id(), usdt_borrow_amount, unit_price_data(0, Some(100000), None));

        let account = test_env.contract.get_account(alice()).unwrap();
        assert!(account.supplied.is_empty());
        assert_balances(
            &account.collateral,
            &[av(nusdc_token_id(), supply_amount)],
        );
        assert_balances(
            &account.borrowed,
            &[
                av(wnear_token_id(), wnear_borrow_amount),
                av(nusdt_token_id(), usdt_borrow_amount),
            ],
        );
        assert!(find_asset(&account.borrowed, &wnear_token_id()).apr > BigDecimal::zero());
        assert!(find_asset(&account.borrowed, &nusdt_token_id()).apr > BigDecimal::zero());

        let wnear_bobs_amount = d(100, 24);
        test_env.deposit(wnear_token_id(), bob(), wnear_bobs_amount);

        let usdt_bobs_amount = d(100, 18);
        test_env.deposit(nusdt_token_id(), bob(), usdt_bobs_amount / extra_decimals_mult);

        let account = test_env.contract.get_account(bob()).unwrap();
        assert_balances(
            &account.supplied,
            &[
                av(wnear_token_id(), wnear_bobs_amount),
                av(nusdt_token_id(), usdt_bobs_amount),
            ],
        );
        assert!(find_asset(&account.supplied, &wnear_token_id()).apr > BigDecimal::zero());
        assert!(find_asset(&account.supplied, &nusdt_token_id()).apr > BigDecimal::zero());

        // Assuming ~2% discount for 5 NEAR at 12$. 50 USDT -> ~51 USDC, 4.9 NEAR -> 60 USDC.
        let wnear_amount_in = d(49, 23);
        let usdt_amount_in = d(50, 18);
        let usdc_amount_out = d(111, 18);
        test_env.liquidate(
            bob(), alice(), unit_price_data(0, Some(120000), None),
            vec![asset_amount(wnear_token_id(), wnear_amount_in), asset_amount(nusdt_token_id(), usdt_amount_in)], vec![asset_amount(nusdc_token_id(), usdc_amount_out)],
        );
        
        let account = test_env.contract.get_account(alice()).unwrap();
        assert!(account.supplied.is_empty());
        assert_balances(
            &account.collateral,
            &[av(
                nusdc_token_id(),
                supply_amount - usdc_amount_out,
            )],
        );
        assert_balances(
            &account.borrowed,
            &[av(
                wnear_token_id(),
                wnear_borrow_amount - wnear_amount_in,
            )],
        );
        assert!(find_asset(&account.borrowed, &wnear_token_id()).apr > BigDecimal::zero());

        let account = test_env.contract.get_account(bob()).unwrap();
        assert_balances(
            &account.supplied,
            &[
                av(
                    wnear_token_id(),
                    wnear_bobs_amount - wnear_amount_in,
                ),
                av(nusdt_token_id(), usdt_bobs_amount - usdt_amount_in),
                av(nusdc_token_id(), usdc_amount_out),
            ],
        );
        assert!(find_asset(&account.supplied, &wnear_token_id()).apr > BigDecimal::zero());
        // Now APR should be 0, since Bob has liquidated the entire USDT amount
        assert_eq!(
            find_asset(&account.supplied, &nusdt_token_id()).apr,
            BigDecimal::zero()
        );
        assert_eq!(
            find_asset(&account.supplied, &nusdc_token_id()).apr,
            BigDecimal::zero()
        );
    }

    /// Force closing the account with bad debt.
    #[test]
    #[ignore]
    fn test_force_close() {
        let mut test_env = init_unit_env();

        let extra_decimals_mult = d(1, 12);

        let supply_amount = d(1000, 18);
        test_env.supply_to_collateral(nusdc_token_id(), alice(), (supply_amount / extra_decimals_mult).into());

        let borrow_amount = d(50, 24);
        test_env.borrow_and_withdraw(alice(), wnear_token_id(), borrow_amount, unit_price_data(0, Some(100000), None));

        let asset = test_env.get_asset(nusdc_token_id());
        let usdc_reserve = asset.reserved;

        let asset = test_env.get_asset(wnear_token_id());
        let wnear_reserve = asset.reserved;

        // Force closing account with NEAR at 25$.
        test_env.force_close(bob(), alice(), unit_price_data(0, Some(250000), None));

        let account = test_env.contract.get_account(alice()).unwrap();
        assert!(account.supplied.is_empty());
        assert!(account.collateral.is_empty());
        assert!(account.borrowed.is_empty());

        let asset = test_env.get_asset(nusdc_token_id());
        assert_eq!(asset.reserved, usdc_reserve + supply_amount);

        let asset = test_env.get_asset(wnear_token_id());
        assert_eq!(asset.reserved, wnear_reserve - borrow_amount);
    }

    #[test]
    fn test_get_old_account_without_set() {
        let mut test_env = init_unit_env();
        let mut supplied = UnorderedMap::new(b"a");
        supplied.insert(&AccountId::new_unchecked("token_id".to_string()), &VAccountAsset::Current(AccountAsset{
            shares: U128(100000)
        }));
        let mut farms = UnorderedMap::new(b"b");
        farms.insert(&FarmId::NetTvl, &VAccountFarm::Current(AccountFarm{
            block_timestamp: 12345,
            rewards: HashMap::new(),
        }));
        test_env.contract.accounts.insert(&AccountId::new_unchecked("storage".to_string()), &VAccount::V1(AccountV1{
            account_id: AccountId::new_unchecked("storage".to_string()),
            supplied,
            collateral: vec![],
            borrowed: vec![],
            farms,
            booster_staking: None,
        }));
        test_env.contract.internal_get_account(&AccountId::new_unchecked("storage".to_string()), true).expect("Account is not registered");
    }

    #[test]
    #[should_panic(expected = "Bug, non-tracked storage change")]
    fn test_get_old_account_without_set_failed1() {
        let mut test_env = init_unit_env();
        let mut supplied = UnorderedMap::new(b"a");
        supplied.insert(&AccountId::new_unchecked("token_id".to_string()), &VAccountAsset::Current(AccountAsset{
            shares: U128(100000)
        }));
        let mut farms = UnorderedMap::new(b"b");
        farms.insert(&FarmId::NetTvl, &VAccountFarm::Current(AccountFarm{
            block_timestamp: 12345,
            rewards: HashMap::new(),
        }));
        test_env.contract.accounts.insert(&AccountId::new_unchecked("storage".to_string()), &VAccount::V1(AccountV1{
            account_id: AccountId::new_unchecked("storage".to_string()),
            supplied,
            collateral: vec![],
            borrowed: vec![],
            farms,
            booster_staking: None,
        }));
        test_env.contract.internal_unwrap_account(&AccountId::new_unchecked("storage".to_string()));
    }

    #[test]
    #[should_panic(expected = "Bug, non-tracked storage change")]
    fn test_get_old_account_without_set_failed2() {
        let mut test_env = init_unit_env();
        let mut supplied = UnorderedMap::new(b"a");
        supplied.insert(&AccountId::new_unchecked("token_id".to_string()), &VAccountAsset::Current(AccountAsset{
            shares: U128(100000)
        }));
        let mut farms = UnorderedMap::new(b"b");
        farms.insert(&FarmId::NetTvl, &VAccountFarm::Current(AccountFarm{
            block_timestamp: 12345,
            rewards: HashMap::new(),
        }));
        test_env.contract.accounts.insert(&AccountId::new_unchecked("storage".to_string()), &VAccount::V1(AccountV1{
            account_id: AccountId::new_unchecked("storage".to_string()),
            supplied,
            collateral: vec![],
            borrowed: vec![],
            farms,
            booster_staking: None,
        }));
        let account = test_env.contract.internal_unwrap_account(&AccountId::new_unchecked("storage".to_string()));
        test_env.contract.storage.insert(&AccountId::new_unchecked("storage".to_string()), &VStorage::Current(Storage { storage_balance: 10u128.pow(25), used_bytes: 1000, storage_tracker: Default::default() }));
        let _tmp_account = account.clone();
        test_env.contract.internal_set_account(&AccountId::new_unchecked("storage".to_string()), account);
    }
}