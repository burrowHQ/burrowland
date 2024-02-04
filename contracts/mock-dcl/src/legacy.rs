use crate::*;

#[derive(BorshSerialize, BorshDeserialize)]
pub struct ContractDataV1000 {
    pub owner_id: AccountId,
    pub next_owner_id: Option<AccountId>,
    pub next_owner_accept_deadline: Option<u64>,
    pub wnear_id: AccountId,
    pub farming_contract_id: AccountId,
    pub state: RunningState,
    pub operators: UnorderedSet<AccountId>,
    pub frozenlist: UnorderedSet<AccountId>,
    pub pools: UnorderedMap<PoolId, VPool>,
    pub users: LookupMap<AccountId, VUser>,
    pub user_liquidities: LookupMap<LptId, VUserLiquidity>,
    pub approvals_by_id: LookupMap<LptId, HashMap<AccountId, u64>>,
    pub next_approval_id_by_id: LookupMap<LptId, u64>,
    pub mft_supply: UnorderedMap<MftId, Balance>,
    pub user_orders: LookupMap<OrderId, VUserOrder>,
    pub latest_liquidity_id: u128,
    pub latest_order_id: u128,
    pub fee_tier: HashMap<u32, u32>,
    pub protocol_fee_rate: u32,
    user_count: u64,
    liquidity_count: u64,
}

impl From<ContractDataV1000> for ContractData {
    fn from(a: ContractDataV1000) -> Self {
        let ContractDataV1000 {
            owner_id,
            next_owner_id,
            next_owner_accept_deadline,
            wnear_id,
            farming_contract_id,
            state,
            operators,
            frozenlist,
            pools,
            users,
            user_liquidities,
            approvals_by_id,
            next_approval_id_by_id,
            mft_supply,
            user_orders,
            latest_liquidity_id,
            latest_order_id,
            fee_tier,
            protocol_fee_rate,
            user_count,
            liquidity_count,
        } = a;
        
        Self {
            state,
            config: LazyOption::new(StorageKeys::GlobalConfig, Some(&GlobalConfig{
                owner_id,
                next_owner_id,
                next_owner_accept_deadline,
                wnear_id,
                storage_price_per_slot: INIT_STORAGE_PRICE_PER_SLOT,
                storage_for_asset: INIT_STORAGE_FOR_ASSETS,
            })),
            operators, 
            frozenlist,
            farming_contract_id,
            farming_contract_id_history: Vec::new(),
            vip_users: UnorderedMap::new(StorageKeys::VipUser), 
            pools,
            users,
            user_liquidities,
            approvals_by_id,
            next_approval_id_by_id,
            mft_supply,
            user_orders,
            latest_liquidity_id,
            latest_order_id,
            fee_tier,
            protocol_fee_rate,
            user_count,
            liquidity_count,
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct ContractDataV1001 {
    pub owner_id: AccountId,
    pub next_owner_id: Option<AccountId>,
    pub next_owner_accept_deadline: Option<u64>,
    pub wnear_id: AccountId,
    pub farming_contract_id: AccountId,
    pub farming_contract_id_history: Vec<AccountId>,
    pub state: RunningState,
    pub operators: UnorderedSet<AccountId>,
    pub frozenlist: UnorderedSet<AccountId>,

    pub pools: UnorderedMap<PoolId, VPool>,
    pub users: LookupMap<AccountId, VUser>,
    pub user_liquidities: LookupMap<LptId, VUserLiquidity>,
    pub approvals_by_id: LookupMap<LptId, HashMap<AccountId, u64>>,
    pub next_approval_id_by_id: LookupMap<LptId, u64>,
    pub mft_supply: UnorderedMap<MftId, Balance>,

    pub user_orders: LookupMap<OrderId, VUserOrder>,
    pub latest_liquidity_id: u128,
    pub latest_order_id: u128,
    
    pub fee_tier: HashMap<u32, u32>,
    pub protocol_fee_rate: u32,

    user_count: u64,
    liquidity_count: u64,
}

impl From<ContractDataV1001> for ContractData {
    fn from(a: ContractDataV1001) -> Self {
        let ContractDataV1001 {
            owner_id,
            next_owner_id,
            next_owner_accept_deadline,
            wnear_id,
            farming_contract_id,
            farming_contract_id_history,
            state,
            operators,
            frozenlist,
            pools,
            users,
            user_liquidities,
            approvals_by_id,
            next_approval_id_by_id,
            mft_supply,
            user_orders,
            latest_liquidity_id,
            latest_order_id,
            fee_tier,
            protocol_fee_rate,
            user_count,
            liquidity_count,
        } = a;
        
        Self {
            state,
            config: LazyOption::new(StorageKeys::GlobalConfig, Some(&GlobalConfig{
                owner_id,
                next_owner_id,
                next_owner_accept_deadline,
                wnear_id,
                storage_price_per_slot: INIT_STORAGE_PRICE_PER_SLOT,
                storage_for_asset: INIT_STORAGE_FOR_ASSETS,
            })),
            operators, 
            frozenlist,
            farming_contract_id,
            farming_contract_id_history,
            vip_users: UnorderedMap::new(StorageKeys::VipUser), 
            pools,
            users,
            user_liquidities,
            approvals_by_id,
            next_approval_id_by_id,
            mft_supply,
            user_orders,
            latest_liquidity_id,
            latest_order_id,
            fee_tier,
            protocol_fee_rate,
            user_count,
            liquidity_count,
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct ContractDataV1002 {
    pub owner_id: AccountId,
    pub next_owner_id: Option<AccountId>,
    pub next_owner_accept_deadline: Option<u64>,
    pub wnear_id: AccountId,
    pub farming_contract_id: AccountId,
    pub farming_contract_id_history: Vec<AccountId>,
    pub state: RunningState,
    pub operators: UnorderedSet<AccountId>,
    pub frozenlist: UnorderedSet<AccountId>,
    pub vip_uesrs: UnorderedMap<AccountId, HashMap<PoolId, u32>>,

    pub pools: UnorderedMap<PoolId, VPool>,
    pub users: LookupMap<AccountId, VUser>,
    pub user_liquidities: LookupMap<LptId, VUserLiquidity>,
    pub approvals_by_id: LookupMap<LptId, HashMap<AccountId, u64>>,
    pub next_approval_id_by_id: LookupMap<LptId, u64>,
    pub mft_supply: UnorderedMap<MftId, Balance>,

    pub user_orders: LookupMap<OrderId, VUserOrder>,
    pub latest_liquidity_id: u128,
    pub latest_order_id: u128,
    
    pub fee_tier: HashMap<u32, u32>,
    pub protocol_fee_rate: u32,

    user_count: u64,
    liquidity_count: u64,
}

impl From<ContractDataV1002> for ContractData {
    fn from(a: ContractDataV1002) -> Self {
        let ContractDataV1002 {
            owner_id,
            next_owner_id,
            next_owner_accept_deadline,
            wnear_id,
            farming_contract_id,
            farming_contract_id_history,
            state,
            operators,
            frozenlist,
            vip_uesrs,
            pools,
            users,
            user_liquidities,
            approvals_by_id,
            next_approval_id_by_id,
            mft_supply,
            user_orders,
            latest_liquidity_id,
            latest_order_id,
            fee_tier,
            protocol_fee_rate,
            user_count,
            liquidity_count,
        } = a;
        
        Self {
            state,
            config: LazyOption::new(StorageKeys::GlobalConfig, Some(&GlobalConfig{
                owner_id,
                next_owner_id,
                next_owner_accept_deadline,
                wnear_id,
                storage_price_per_slot: INIT_STORAGE_PRICE_PER_SLOT,
                storage_for_asset: INIT_STORAGE_FOR_ASSETS,
            })),
            operators, 
            frozenlist,
            farming_contract_id,
            farming_contract_id_history,
            vip_users: vip_uesrs, 
            
            pools,
            users,
            user_liquidities,
            approvals_by_id,
            next_approval_id_by_id,
            mft_supply,
            user_orders,
            latest_liquidity_id,
            latest_order_id,
            fee_tier,
            protocol_fee_rate,
            user_count,
            liquidity_count,
        }
    }
}


#[derive(BorshSerialize, BorshDeserialize)]
pub struct UserV0 {
    pub user_id: AccountId,
    pub sponsor_id: AccountId,
    pub liquidity_keys: UnorderedSet<LptId>,
    pub order_keys: UnorderedMap<UserOrderKey, OrderId>,
    pub history_orders: Vector<UserOrder>,
    pub completed_order_count: u64,
    pub assets: UnorderedMap<AccountId, Balance>,
    pub mft_assets: UnorderedMap<MftId, Balance>,
}

impl From<UserV0> for User {
    fn from(a: UserV0) -> Self {
        let UserV0{
            user_id,
            sponsor_id,
            liquidity_keys,
            order_keys,
            history_orders,
            completed_order_count,
            assets,
            mft_assets,
        } = a;

        Self {
            user_id,
            sponsor_id,
            locked_near_for_storage: STORAGE_BALANCE_MIN_BOUND,
            liquidity_keys,
            order_keys,
            history_orders,
            completed_order_count,
            assets,
            mft_assets
        }
    }
}