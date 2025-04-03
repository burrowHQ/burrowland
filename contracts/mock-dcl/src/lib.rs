#![allow(hidden_glob_reexports)]
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    serde::{Deserialize, Serialize},
    collections::{LazyOption, LookupMap, UnorderedSet, UnorderedMap, Vector},
    json_types::{U128, U64},
    near_bindgen, require, AccountId, Balance, BorshStorageKey, PanicOnDefault, 
    Promise, PromiseOrValue, PromiseResult, env, assert_one_yocto, log, ext_contract, 
    Gas, Timestamp,
};
use std::collections::{HashSet, HashMap};
use std::cmp::min;

use once_cell::sync::Lazy;
use std::sync::Mutex;

mod event;
mod global_config;
mod legacy;
mod owner;
mod user;
mod user_asset;
mod utils;
mod errors;
mod api;
mod dcl;

pub use crate::event::*;
pub use crate::global_config::*;
pub use crate::legacy::*;
pub use crate::user::*;
pub use crate::utils::*;
pub use crate::errors::*;
pub use crate::api::*;
pub use crate::dcl::*;


#[derive(BorshStorageKey, BorshSerialize)]
pub(crate) enum StorageKeys {
    Operator,
    Pool,

    Frozenlist,

    User,
    UserAsset { account_id: AccountId },

    Liquidity,
    UserLiquidityKey { account_id: AccountId },
    LiquidityApproval,
    LiquidityApprovalId,
    
    UserOrder, 
    UserOrderKey { account_id: AccountId },
    UserOrderHistory { account_id: AccountId },

    PointInfo { pool_id: PoolId },
    PointBitmap { pool_id: PoolId },
    Oracle { pool_id: PoolId },

    MftSupply,
    UserMftAsset { account_id: AccountId },

    VipUser,
    GlobalConfig,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Eq, PartialEq, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
pub enum RunningState {
    Running, Paused
}

static GC: Lazy<Mutex<Option<GlobalConfig>>> = Lazy::new(|| Mutex::new(None));

#[derive(BorshSerialize, BorshDeserialize)]
pub struct ContractData {
    pub state: RunningState,
    pub config: LazyOption<GlobalConfig>,
    pub operators: UnorderedSet<AccountId>,
    pub frozenlist: UnorderedSet<AccountId>,
    
    pub users: LookupMap<AccountId, VUser>,

    pub fee_tier: HashMap<u32, u32>,
    pub protocol_fee_rate: u32,  // ratio of total fee in BPs
    pub vip_users: UnorderedMap<AccountId, HashMap<PoolId, u32>>,
    pub farming_contract_id: AccountId,
    pub farming_contract_id_history: Vec<AccountId>,

    pub pools: UnorderedMap<PoolId, VPool>,
    pub user_liquidities: LookupMap<LptId, VUserLiquidity>,
    pub approvals_by_id: LookupMap<LptId, HashMap<AccountId, u64>>,
    pub next_approval_id_by_id: LookupMap<LptId, u64>,
    pub mft_supply: UnorderedMap<MftId, Balance>,
    pub user_orders: LookupMap<OrderId, VUserOrder>,
    pub latest_liquidity_id: u128,
    pub latest_order_id: u128,
    
    user_count: u64,
    liquidity_count: u64,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub enum VersionedContractData {
    V1000(ContractDataV1000),
    V1001(ContractDataV1001),
    V1002(ContractDataV1002),
    V1003(ContractData),
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Contract {
    data: VersionedContractData,
}

#[near_bindgen]
impl Contract {
    #[init]
    pub fn new(owner_id: AccountId, wnear_id: AccountId, farming_contract_id: AccountId) -> Self {
        require!(!env::state_exists(), E000_ALREADY_INIT);
        let mut ret = Self {
            data: VersionedContractData::V1003(ContractData { 
                state: RunningState::Running, 
                config: LazyOption::new(StorageKeys::GlobalConfig, Some(&GlobalConfig{
                    owner_id: owner_id.clone(),
                    next_owner_id: None,
                    next_owner_accept_deadline: None,
                    wnear_id,
                    storage_price_per_slot: INIT_STORAGE_PRICE_PER_SLOT,
                    storage_for_asset: INIT_STORAGE_FOR_ASSETS,
                })),
                operators: UnorderedSet::new(StorageKeys::Operator), 
                frozenlist: UnorderedSet::new(StorageKeys::Frozenlist),
                vip_users: UnorderedMap::new(StorageKeys::VipUser), 
                farming_contract_id: farming_contract_id.clone(), 
                farming_contract_id_history: Vec::new(),
                pools: UnorderedMap::new(StorageKeys::Pool), 
                user_liquidities: LookupMap::new(StorageKeys::Liquidity), 
                approvals_by_id: LookupMap::new(StorageKeys::LiquidityApproval),
                next_approval_id_by_id: LookupMap::new(StorageKeys::LiquidityApprovalId), 
                mft_supply: UnorderedMap::new(StorageKeys::MftSupply),
                user_orders: LookupMap::new(StorageKeys::UserOrder), 
                latest_liquidity_id: 0, 
                latest_order_id: 0, 
                fee_tier: HashMap::from([(100_u32, 1_u32), (400_u32, 8_u32), (2000_u32, 40_u32), (10000_u32, 200_u32)]), 
                protocol_fee_rate: DEFAULT_PROTOCOL_FEE,

                users: LookupMap::new(StorageKeys::User),
                user_count: 0,
                liquidity_count: 0,
            }),
        };
        // register owner to be an user to accept possible nft transfer revert.
        ret.data_mut()
            .users
            .insert(&owner_id, &User::new(&owner_id, &env::current_account_id(), STORAGE_BALANCE_MIN_BOUND).into());
        ret.data_mut()
            .users
            .insert(&farming_contract_id, &User::new(&farming_contract_id, &env::current_account_id(), STORAGE_BALANCE_MIN_BOUND).into());
        ret.data_mut().user_count += 2;
        ret
    }
}

impl Contract {
    fn data(&self) -> &ContractData {
        match &self.data {
            VersionedContractData::V1003(data) => data,
            _ => unimplemented!(),
        }
    }

    fn data_mut(&mut self) -> &mut ContractData {
        match &mut self.data {
            VersionedContractData::V1003(data) => data,
            _ => unimplemented!(),
        }
    }

    pub fn internal_get_global_config(&self) -> GlobalConfig {
        let mut cache = GC.lock().unwrap();
        cache.clone().unwrap_or_else(|| {
            let gc = 
            self
            .data()
            .config
            .get().unwrap();
            _ = cache.replace(gc.clone());
            gc
        })
    }

    pub fn internal_set_global_config(&mut self, config: GlobalConfig) {
        _ = GC.lock().unwrap().replace(config.clone());
        self.data_mut().config.set(&config);
    }

    fn is_owner_or_operators(&self) -> bool {
        let global_config = self.internal_get_global_config();
        env::predecessor_account_id() == global_config.owner_id
            || self.data()
                .operators
                .contains(&env::predecessor_account_id())
    }

    fn assert_contract_running(&self) {
        require!(self.data().state == RunningState::Running, E004_CONTRACT_PAUSED);
    }

    fn assert_pool_running(&self, pool: &Pool) {
        require!(pool.state == RunningState::Running, E406_POOL_PAUSED);
    }

    fn assert_no_frozen_tokens(&self, tokens: &[AccountId]) {
        let frozens: Vec<&AccountId> = tokens.iter()
        .filter(
            |token| self.data().frozenlist.contains(*token)
        )
        .collect();
        require!(frozens.len() == 0, E010_INCLUDE_FROZEN_TOKEN);
    }
}


