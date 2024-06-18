use crate::*;

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub enum DclFarmingType {
    FixRange{ left_point: i32, right_point: i32},
}

/// dcl_farming related
#[near_bindgen]
impl Contract {
    pub fn mint_v_liquidity(&mut self, lpt_id: LptId, dcl_farming_type: DclFarmingType) {
        self.assert_contract_running();
        let user_id = env::predecessor_account_id();
        let mut user = self.internal_unwrap_user(&user_id);
        self.internal_mint_v_liquidity(&mut user, lpt_id, dcl_farming_type);
        self.internal_set_user(&user_id, user);
    }

    pub fn batch_mint_v_liquidity(&mut self, mint_infos: Vec<(LptId, DclFarmingType)>) {
        self.assert_contract_running();
        let user_id = env::predecessor_account_id();
        let mut user = self.internal_unwrap_user(&user_id);
        for (lpt_id, dcl_farming_type) in mint_infos {
            self.internal_mint_v_liquidity(&mut user, lpt_id, dcl_farming_type);
        }
        self.internal_set_user(&user_id, user);
    }

    pub fn burn_v_liquidity(&mut self, lpt_id: LptId) {
        self.assert_contract_running();
        let user_id = env::predecessor_account_id();
        let mut user = self.internal_unwrap_user(&user_id);
        self.internal_burn_v_liquidity(&mut user, lpt_id);
        self.internal_set_user(&user_id, user);
    }

    pub fn batch_burn_v_liquidity(&mut self, lpt_ids: Vec<LptId>) {
        self.assert_contract_running();
        let user_id = env::predecessor_account_id();
        let mut user = self.internal_unwrap_user(&user_id);
        for lpt_id in lpt_ids {
            self.internal_burn_v_liquidity(&mut user, lpt_id);
        }
        self.internal_set_user(&user_id, user);
    }
}

/// dcl pool related
#[near_bindgen]
impl Contract {
    /// Create a new pool
    /// @param token_a
    /// @param token_b
    /// @param fee: n%, and n in BPs, eg: 10000 means 1% fee rate
    /// @param init_point: the current point position when the pool was created
    /// @return a string like token_a|token_b|fee
    #[payable]
    pub fn create_pool(
        &mut self,
        token_a: AccountId,
        token_b: AccountId,
        fee: u32,
        init_point: i32,
    ) -> PoolId {
        require!(self.is_owner_or_operators(), E002_NOT_ALLOWED);
        self.assert_contract_running();
        self.assert_no_frozen_tokens(&[token_a.clone(), token_b.clone()]);
        let prev_storage = env::storage_usage();
        let pool_id = PoolId::gen_from(&token_a, &token_b, fee);
        require!(
            self.internal_get_pool(&pool_id).is_none(),
            E405_POOL_ALREADY_EXIST
        );
        self.internal_set_pool(
            &pool_id,
            Pool::new(
                &pool_id,
                *self.data().fee_tier.get(&fee).expect(E402_ILLEGAL_FEE),
                init_point,
            ),
        );
        let refund = env::attached_deposit()
            .checked_sub((env::storage_usage() - prev_storage) as u128 * env::storage_byte_cost())
            .expect(E104_INSUFFICIENT_DEPOSIT);
        if refund > 0 {
            Promise::new(env::predecessor_account_id()).transfer(refund);
        }
        pool_id
    }

    /// Get Pool from pool_id
    /// @param pool_id
    /// @return Option<Pool>
    pub fn get_pool(&self, pool_id: PoolId) -> Option<Pool> {
        self.internal_get_pool(&pool_id)
    }

    /// A maximum of limit Pools are obtained starting from from_index
    /// @param from_index
    /// @param limit
    /// @return Vec<Pool>
    pub fn list_pools(&self, from_index: Option<u64>, limit: Option<u64>) -> Vec<Pool> {
        let keys = self.data().pools.keys_as_vector();

        let from_index = from_index.unwrap_or(0);
        let limit = limit.unwrap_or(keys.len());

        (from_index..std::cmp::min(from_index + limit, keys.len()))
            .map(|index| {
                self.data()
                    .pools
                    .get(&keys.get(index).unwrap())
                    .unwrap()
                    .into()
            })
            .collect()
    }
}

#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(test, derive(Clone))]
pub struct QuoteResult {
    pub amount: U128,
    pub tag: Option<String>,
}

/// dcl quote related
#[near_bindgen]
impl Contract {
    /// @param pool_ids: all pools participating in swap
    /// @param input_token: the swap-in token, must be in pool_ids[0].tokens
    /// @param output_token: the swap-out token, must be in pool_ids[-1].tokens
    /// @param input_amount: the amount of swap-in token
    /// @param tag
    /// @return estimated output token amount
    pub fn quote(
        &self,
        vip_info: Option<HashMap<PoolId, u32>>,
        pool_ids: Vec<PoolId>,
        input_token: AccountId,
        output_token: AccountId,
        input_amount: U128,
        tag: Option<String>,
    ) -> QuoteResult {
        let mut pool_cache = HashMap::new();
        self.internal_quote(&mut pool_cache, vip_info, pool_ids, input_token, output_token, input_amount, tag)
    }

    // /// @param pool_ids: all pools participating in swap
    // /// @param input_token: the swap-in token, must be in pool_ids[-1].tokens
    // /// @param output_token: the swap-out token, must be in pool_ids[0].tokens
    // /// @param output_amount: the amount of swap-out token
    // /// @param tag
    // /// @return estimated input token amount
    // pub fn quote_by_output(
    //     &self,
    //     pool_ids: Vec<PoolId>,
    //     input_token: AccountId,
    //     output_token: AccountId,
    //     output_amount: U128,
    //     tag: Option<String>,
    // ) -> QuoteResult {
    //     let quote_failed = QuoteResult {
    //         amount: 0.into(),
    //         tag: tag.clone(),
    //     };
    //     if self.data().state == RunningState::Paused {
    //         return quote_failed;
    //     }
    //     let mut pool_record = HashSet::new();
    //     let protocol_fee_rate = self.data().protocol_fee_rate;
    //     let (actual_input_token, actual_input_amount) = {
    //         let mut next_desire_token = output_token;
    //         let mut next_desire_amount = output_amount.0;
    //         for pool_id in pool_ids {
    //             let mut pool = self.internal_unwrap_pool(&pool_id);
    //             let is_not_exist = pool_record.insert(format!("{}|{}", pool.token_x, pool.token_y));
    //             if !is_not_exist || pool.state == RunningState::Paused || 
    //                 self.data().frozenlist.contains(&pool.token_x) || self.data().frozenlist.contains(&pool.token_y) {
    //                 return quote_failed;
    //             }
    //             let is_finished = if next_desire_token.eq(&pool.token_x) {
    //                 let (need_amount, _, is_finished) = pool.internal_y_swap_x_desire_x(protocol_fee_rate, next_desire_amount, 800001, true);
    //                 next_desire_token = pool.token_y.clone();
    //                 next_desire_amount = need_amount;
    //                 is_finished
    //             } else if next_desire_token.eq(&pool.token_y) {
    //                 let (need_amount, _, is_finished) = pool.internal_x_swap_y_desire_y(protocol_fee_rate, next_desire_amount, -800001, true);
    //                 next_desire_token = pool.token_x.clone();
    //                 next_desire_amount = need_amount;
    //                 is_finished
    //             } else {
    //                 return quote_failed;
    //             };
    //             if !is_finished {
    //                 return quote_failed;
    //             }
    //         }
    //         (next_desire_token, next_desire_amount)
    //     };
    //     if input_token != actual_input_token {
    //         return quote_failed;
    //     }
    //     QuoteResult {
    //         amount: actual_input_amount.into(),
    //         tag,
    //     }
    // }
}

#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(test, derive(Clone, Debug))]
pub struct RangeInfo {
    pub left_point: i32,  // include this point
    pub right_point: i32,  // exclude this point
    pub amount_l: U128,  // liquidity amount of each point in this Range
}

#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(test, derive(Clone, Debug))]
pub struct PointOrderInfo {
    point: i32,
    // x y would be one and only one has none zero value
    amount_x: U128,  
    amount_y: U128, 
}

#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(test, derive(Clone, Debug))]
pub struct MarketDepth {
    pub pool_id: PoolId,
    pub current_point: i32,
    pub amount_l: U128,  // total liquidity at current point
    pub amount_l_x: U128,  // liquidity caused by token X at current point
    pub liquidities: HashMap<i32, RangeInfo>,  // key is start point of this range
    pub orders: HashMap<i32, PointOrderInfo>,  // key is the order located
}

#[near_bindgen]
impl Contract {

    pub fn get_liquidity_range(
        &self,
        pool_id: PoolId,
        left_point: i32,  // N * pointDelta, -800000 min
        right_point: i32, // N * pointDelta, +800000 max
    ) -> HashMap<i32, RangeInfo> {
        require!(left_point <= right_point, E202_ILLEGAL_POINT);
        require!(left_point >= LEFT_MOST_POINT, E202_ILLEGAL_POINT);
        require!(right_point <= RIGHT_MOST_POINT, E202_ILLEGAL_POINT);
        let pool = self.internal_unwrap_pool(&pool_id);
        let mut ret = HashMap::new();
        if left_point >= pool.current_point {
            range_info_to_the_left_of_cp(&pool, left_point, right_point, &mut ret);
        } else if right_point <= pool.current_point {
            range_info_to_the_right_of_cp(&pool, left_point, right_point, &mut ret);
        } else {
            range_info_to_the_right_of_cp(&pool, left_point, pool.current_point, &mut ret);
            range_info_to_the_left_of_cp(&pool, pool.current_point, right_point, &mut ret);
        }
        ret
    }

    pub fn get_pointorder_range(
        &self,
        pool_id: PoolId,
        left_point: i32,
        right_point: i32,
    ) -> HashMap<i32, PointOrderInfo> {
        require!(left_point <= right_point, E202_ILLEGAL_POINT);
        require!(left_point >= LEFT_MOST_POINT, E202_ILLEGAL_POINT);
        require!(right_point <= RIGHT_MOST_POINT, E202_ILLEGAL_POINT);
        let pool = self.internal_unwrap_pool(&pool_id);
        let mut ret: HashMap<i32, PointOrderInfo> = HashMap::new();
        
        let mut current_point = left_point;
        let stop_slot = right_point / pool.point_delta;
        while current_point <= right_point {
            if pool.point_info.has_active_order(current_point, pool.point_delta) {
                let order = pool.point_info.get_order_data(current_point);
                ret.insert(current_point, PointOrderInfo {
                    point: current_point,
                    amount_x: order.selling_x.into(),
                    amount_y: order.selling_y.into(),
                });
            }
            current_point = match pool.slot_bitmap.get_nearest_right_valued_slot(current_point, pool.point_delta, stop_slot) {
                Some(point) => point,
                None => { break; }
            };
        }
        ret
    }

    pub fn get_marketdepth(
        &self,
        pool_id: PoolId,  // pointDelta: 40 (2000 fee)
        depth: u8,  // max elements in liquidities or orders map
    ) -> MarketDepth {
        let pool = self.internal_unwrap_pool(&pool_id);
        let left_slot_boundary = std::cmp::max(LEFT_MOST_POINT / pool.point_delta, pool.current_point / pool.point_delta - MARKET_QUERY_SLOT_LIMIT);
        let right_slot_boundary = std::cmp::min(RIGHT_MOST_POINT / pool.point_delta, pool.current_point / pool.point_delta + MARKET_QUERY_SLOT_LIMIT);
        let mut liquidities = HashMap::new();
        let mut orders = HashMap::new();

        if pool.point_info.has_active_order(pool.current_point, pool.point_delta) {
            let order_data = pool.point_info.get_order_data(pool.current_point);
            orders.insert(pool.current_point, PointOrderInfo{
                point: pool.current_point,
                amount_x: order_data.selling_x.into(),
                amount_y: order_data.selling_y.into(),
            });
        }

        let mut range_info_count = depth;
        let mut order_count = depth;
        let mut range_left_point = pool.current_point;
        let mut current_point = pool.current_point;
        let mut current_liquidity = pool.liquidity;
        while range_info_count != 0 || order_count != 0 {
            if let Some(range_right_point) = pool.slot_bitmap.get_nearest_right_valued_slot(current_point, pool.point_delta, right_slot_boundary){
                if pool.point_info.has_active_liquidity(range_right_point, pool.point_delta) && range_info_count != 0 {
                    liquidities.insert(range_left_point, RangeInfo{
                        left_point: range_left_point,
                        right_point: range_right_point,
                        amount_l: current_liquidity.into(),
                    });
                    range_left_point = range_right_point;
                    range_info_count -= 1;
                    let liquidity_data = pool.point_info.get_liquidity_data(range_right_point);
                    if liquidity_data.liquidity_delta > 0 {
                        current_liquidity += liquidity_data.liquidity_delta as u128;
                    } else {
                        current_liquidity -= (-liquidity_data.liquidity_delta) as u128;
                    }
                }
                if pool.point_info.has_active_order(range_right_point, pool.point_delta) && order_count != 0  {
                    let order_data = pool.point_info.get_order_data(range_right_point);
                    orders.insert(range_right_point, PointOrderInfo{
                        point: range_right_point,
                        amount_x: order_data.selling_x.into(),
                        amount_y: order_data.selling_y.into(),
                    });
                    order_count -= 1;
                }
                current_point = range_right_point;
            } else {
                break;
            }
        } 

        let mut range_info_count = depth;
        let mut order_count = depth;
        let mut range_right_point = pool.current_point;
        let mut current_point = pool.current_point;
        let mut current_liquidity = if pool.point_info.has_active_liquidity(pool.current_point, pool.point_delta) {
            let liquidity_data = pool.point_info.get_liquidity_data(pool.current_point);
            if liquidity_data.liquidity_delta > 0 {
                pool.liquidity - liquidity_data.liquidity_delta as u128
            } else {
                pool.liquidity + (-liquidity_data.liquidity_delta) as u128
            }
        } else {
            pool.liquidity
        };
        while range_info_count != 0 || order_count != 0 {
            if let Some(range_left_point) = pool.slot_bitmap.get_nearest_left_valued_slot(current_point - 1, pool.point_delta, left_slot_boundary){
                if pool.point_info.has_active_liquidity(range_left_point, pool.point_delta) && range_info_count != 0 {
                    liquidities.insert(range_left_point, RangeInfo{
                        left_point: range_left_point,
                        right_point: range_right_point,
                        amount_l: current_liquidity.into(),
                    });
                    range_right_point = range_left_point;
                    range_info_count -= 1;
                    let liquidity_data = pool.point_info.get_liquidity_data(range_left_point);
                    if liquidity_data.liquidity_delta > 0 {
                        current_liquidity -= liquidity_data.liquidity_delta as u128;
                    } else {
                        current_liquidity += (-liquidity_data.liquidity_delta) as u128;
                    }
                }
                if pool.point_info.has_active_order(range_left_point, pool.point_delta) && order_count != 0 {
                    let order_data = pool.point_info.get_order_data(range_left_point);
                    orders.insert(range_left_point, PointOrderInfo{
                        point: range_left_point,
                        amount_x: order_data.selling_x.into(),
                        amount_y: order_data.selling_y.into(),
                    });
                    order_count -= 1;
                }
                current_point = range_left_point;
            } else {
                break;
            }
        }

        MarketDepth {
            pool_id,
            current_point: pool.current_point,
            amount_l: pool.liquidity.into(),
            amount_l_x: pool.liquidity_x.into(),
            liquidities,
            orders,
        }
    }

}