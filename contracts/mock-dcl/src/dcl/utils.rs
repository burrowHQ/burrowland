use crate::*;
use std::convert::TryInto;

use crate::{E400_INVALID_POOL_ID, DclFarmingType};
use crate::E401_SAME_TOKENS;

pub const DEFAULT_PROTOCOL_FEE: u32 = 5_000;
pub const DEFAULT_USER_ORDER_HISTORY_LEN: u64 = 16;
pub const MAX_USER_ORDER_CLIENT_ID_LEN: usize = 32;
pub const MAX_LIQUIDITY_APPROVAL_COUNT: usize = 16;
pub const NO_DEPOSIT: u128 = 0;
pub const INIT_STORAGE_PRICE_PER_SLOT: u128 = 10_000_000_000_000_000_000_000; // 0.01near
pub const INIT_STORAGE_FOR_ASSETS: u128 = 100_000_000_000_000_000_000_000; // 0.1near
pub const STORAGE_BALANCE_MIN_BOUND: u128 = 500_000_000_000_000_000_000_000; // 0.5near
pub const AVAILABLE_MS_FOR_NEXT_OWNER_ACCEPT: u64 = 72 * 3600 * 1000;
// reserve for farming
pub const ORACLE_RECORD_LIMIT: u64 = 3 * 3600;

pub const LEFT_MOST_POINT: i32 = -800000;
pub const RIGHT_MOST_POINT: i32 = 800000;

pub const MARKET_QUERY_SLOT_LIMIT: i32 = 150000;

pub const BP_DENOM: u128 = 10000;

pub const POOL_ID_BREAK: &str = "|";
pub const LPT_ID_BREAK: &str = "#";
pub const USER_LIQUIDITY_KEY_BREAK: &str = "@";
pub const ORDER_ID_BREAK: &str = "#";
pub const USER_ORDER_KEY_BREAK: &str = "@";
pub const MFT_ID_BREAK: &str = "&";


pub type PoolId = String;

pub trait PoolIdTrait {
    fn gen_from(token_a: &AccountId, token_b: &AccountId, fee_tier: u32) -> Self;
    fn is_earn_y(&self, sell_token: &AccountId) -> bool;
    fn parse_pool_id(&self) -> (AccountId, AccountId, u32);
}

impl PoolIdTrait for PoolId {
    fn gen_from(token_a: &AccountId, token_b: &AccountId, fee_tier: u32) -> Self {
        require!(token_a.to_string() != token_b.to_string(), E401_SAME_TOKENS);
        if token_a.to_string() < token_b.to_string() {
            format!("{}{}{}{}{}", token_a, POOL_ID_BREAK, token_b, POOL_ID_BREAK, fee_tier)
        } else {
            format!("{}{}{}{}{}", token_b, POOL_ID_BREAK, token_a, POOL_ID_BREAK, fee_tier)
        }
    }

    fn is_earn_y(&self, sell_token: &AccountId) -> bool {
        let pos = self.find(POOL_ID_BREAK).expect(E400_INVALID_POOL_ID);
        let (token_x, _) = self.split_at(pos);
        sell_token.to_string() == token_x
    }

    fn parse_pool_id(&self) -> (AccountId, AccountId, u32) {
        let pos = self.find(POOL_ID_BREAK).expect(E400_INVALID_POOL_ID);
        let (token_x, last) = self.split_at(pos);
        
        let tokeny_feetier = last.split_at(1).1;
        let pos = tokeny_feetier.find(POOL_ID_BREAK).expect(E400_INVALID_POOL_ID);
        let (token_y, fee_tier) = tokeny_feetier.split_at(pos);
    
        (
            token_x.to_string().try_into().unwrap(), 
            token_y.to_string().try_into().unwrap(), 
            (fee_tier.split_at(1).1).parse::<u32>().unwrap()
        )
    }
}

pub type LptId = String;
pub fn gen_lpt_id(pool_id: &PoolId, inner_id: &mut u128) -> LptId {
    *inner_id += 1;
    format!("{}{}{}", pool_id, LPT_ID_BREAK, inner_id)
}
pub fn parse_pool_id_from_lpt_id(lpt_id: &LptId) -> PoolId {
    String::from(lpt_id.split(LPT_ID_BREAK).collect::<Vec<_>>()[0])
}

pub type OrderId = String;
pub fn gen_order_id(pool_id: &PoolId, inner_id: &mut u128) -> OrderId {
    *inner_id += 1;
    format!("{}{}{}", pool_id, ORDER_ID_BREAK, inner_id)
}

pub type UserOrderKey = String;
pub fn gen_user_order_key(pool_id: &PoolId, point: i32) -> UserOrderKey {
    format!("{}{}{}", pool_id, USER_ORDER_KEY_BREAK, point)
}

pub type MftId = String;
pub fn gen_mft_id(pool_id: &PoolId, dcl_farming_type: &DclFarmingType) -> MftId {
    match dcl_farming_type{
        DclFarmingType::FixRange { left_point, right_point } => {
            format!(":{}{}{}{}{}{}{}", near_sdk::serde_json::to_string(dcl_farming_type).unwrap(), MFT_ID_BREAK, pool_id, MFT_ID_BREAK, left_point, MFT_ID_BREAK, right_point)
        }
    }
}

pub struct TokenCache(pub HashMap<AccountId, u128>);

impl TokenCache {
    pub fn new() -> Self {
        TokenCache(HashMap::new())
    }

    pub fn add(&mut self, token_id: &AccountId, amount: u128) {
        self.0.entry(token_id.clone()).and_modify(|v| *v += amount).or_insert(amount);
    }

    pub fn sub(&mut self, token_id: &AccountId, amount: u128) {
        if amount != 0 {
            if let Some(prev) = self.0.remove(token_id) {
                require!(amount <= prev, E101_INSUFFICIENT_BALANCE);
                let remain = prev - amount;
                if remain > 0 {
                    self.0.insert(token_id.clone(), remain);
                }
            } else {
                env::panic_str(E101_INSUFFICIENT_BALANCE);
            }
        }
    }
}

impl From<TokenCache> for HashMap<AccountId, U128> {
    fn from(v: TokenCache) -> Self {
        v.0.into_iter().map(|(k, v)| (k, U128(v))).collect()
    }
}