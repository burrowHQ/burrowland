use near_sdk::collections::Vector;

use crate::*;

#[derive(BorshSerialize, BorshDeserialize, Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Pool {
    pub pool_id: PoolId,
    pub token_x: AccountId,
    pub token_y: AccountId,
    pub fee: u32,
    pub point_delta: i32,

    pub current_point: i32,
    #[serde(skip_serializing)]
    pub sqrt_price_96: U256,
    #[serde(with = "u128_dec_format")]
    pub liquidity: u128,
    #[serde(with = "u128_dec_format")]
    pub liquidity_x: u128,
    #[serde(with = "u128_dec_format")]
    pub max_liquidity_per_point: u128,

    #[serde(skip_serializing)]
    pub fee_scale_x_128: U256, // token X fee per unit of liquidity
    #[serde(skip_serializing)]
    pub fee_scale_y_128: U256, // token Y fee per unit of liquidity

    #[serde(with = "u128_dec_format")]
    pub total_fee_x_charged: u128,
    #[serde(with = "u128_dec_format")]
    pub total_fee_y_charged: u128,

    #[serde(with = "u256_dec_format")]
    pub volume_x_in: U256,
    #[serde(with = "u256_dec_format")]
    pub volume_y_in: U256,
    #[serde(with = "u256_dec_format")]
    pub volume_x_out: U256,
    #[serde(with = "u256_dec_format")]
    pub volume_y_out: U256,

    #[serde(with = "u128_dec_format")]
    pub total_liquidity: u128,
    #[serde(with = "u128_dec_format")]
    pub total_order_x: u128,
    #[serde(with = "u128_dec_format")]
    pub total_order_y: u128,
    #[serde(with = "u128_dec_format")]
    pub total_x: u128,
    #[serde(with = "u128_dec_format")]
    pub total_y: u128,
    
    #[serde(skip_serializing)]
    pub point_info: PointInfo,
    #[serde(skip_serializing)]
    pub slot_bitmap: SlotBitmap,

    #[serde(skip_serializing)]
    pub oracle: Vector<Observation>, // reserve for farming
    #[serde(skip_serializing)]
    pub oracle_current_index: u64, // reserve for farming

    pub state: RunningState,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub enum VPool {
    Current(Pool),
}

impl From<VPool> for Pool {
    fn from(v: VPool) -> Self {
        match v {
            VPool::Current(c) => c,
        }
    }
}

impl From<Pool> for VPool {
    fn from(c: Pool) -> Self {
        VPool::Current(c)
    }
}

impl Pool {
    /// Update the liquidity and fee when passing the endpoint
    /// @param point: id of endpoint
    /// @param is_quote: whether is it called by a quote interface
    /// @param to_the_left: whether does the passing direction head from right to left
    fn pass_endpoint(&mut self, point: i32, is_quote: bool, to_the_left: bool){
        let mut point_data = self.point_info.0.get(&point).unwrap();
        let mut liquidity_data = point_data.liquidity_data.take().unwrap();

        liquidity_data.pass_endpoint(self.fee_scale_x_128, self.fee_scale_y_128);
        let liquidity_delta = if to_the_left { -liquidity_data.liquidity_delta } else { liquidity_data.liquidity_delta };
        self.liquidity = liquidity_add_delta(self.liquidity, liquidity_delta);

        if !is_quote {
            point_data.liquidity_data = Some(liquidity_data);
            self.point_info.0.insert(&point, &point_data);
        }
    }

    /// After swap order, update point_info and slot_bitmap at the current point
    /// @param point_data: point_data at the current point
    /// @param order_data: order_data at the current point
    /// @param is_quote: whether is it called by a quote interface
    fn update_point_order(&mut self, point_data: &mut PointData, order_data: OrderData, is_quote: bool) {
        if !is_quote {
            point_data.order_data = Some(order_data);
            if !point_data.has_active_order() && !point_data.has_active_liquidity() {
                self.slot_bitmap.set_zero(self.current_point, self.point_delta);
            }
            self.point_info.0.insert(&self.current_point, point_data);
        }
    }

    pub fn get_pool_fee_by_user(&self, vip_info: &Option<HashMap<PoolId, u32>>) -> u32{
        if let Some(v) = vip_info.as_ref() {
            if let Some(vip_discount) = v.get(&self.pool_id) {
                (self.fee as u128 * (*vip_discount as u128) / BP_DENOM) as u32
            } else {
                self.fee
            }
        } else {
            self.fee
        }
    }
}

impl Pool {
    /// Process limit_order_y at current point
    /// @param protocol_fee_rate
    /// @param order_data
    /// @param amount_x
    /// @return (is_finished, consumed_x, gained_y)
    fn process_limit_order_y(&mut self, pool_fee: u32, protocol_fee_rate: u32, order_data: &mut OrderData, amount_x: u128) -> (bool, u128, u128, u128, u128) {
        let mut is_finished = false;
        let net_amount = U256::from(amount_x).mul_fraction_floor((10u128.pow(6) - pool_fee as u128).into(), 10u128.pow(6).into()).as_u128();
        if net_amount > 0 {
            let (cost_x, acquire_y) = swap_math::x_swap_y_at_price(net_amount, self.sqrt_price_96, order_data.selling_y);
            if acquire_y < order_data.selling_y || cost_x >= net_amount {
                is_finished = true;
            }

            let fee_amount = if cost_x >= net_amount {
                // all x consumed
                amount_x - cost_x
            } else {
                U256::from(cost_x)
                .mul_fraction_ceil(pool_fee.into(), (10u128.pow(6) - pool_fee as u128).into())
                .as_u128()
            };
            // limit order fee goes to lp and protocol
            let protocol_fee = if self.liquidity != 0 {
                let charged_fee_amount = fee_amount * protocol_fee_rate as u128 / BP_DENOM;
                self.total_fee_x_charged += charged_fee_amount;
                self.fee_scale_x_128 += U256::from(fee_amount - charged_fee_amount).mul_fraction_floor(pow_128(), U256::from(self.liquidity));
                charged_fee_amount
            } else {
                self.total_fee_x_charged += fee_amount;
                fee_amount
            };

            // for statistic
            self.total_order_y -= acquire_y;
            
            order_data.selling_y -= acquire_y;
            order_data.earn_x += cost_x;
            order_data.acc_earn_x += U256::from(cost_x);

            if order_data.selling_y == 0 {
                // point order fulfilled, handle legacy logic
                order_data.earn_x_legacy += order_data.earn_x;
                order_data.acc_earn_x_legacy = order_data.acc_earn_x;
                order_data.earn_x = 0;
            }

            (is_finished, cost_x + fee_amount, acquire_y, fee_amount, protocol_fee)
        } else {
            (true, 0, 0, 0, 0)
        }
        
    }

    /// Process liquidity_x in range [left_pt, self.current_point]
    /// @param protocol_fee_rate
    /// @param amount_x
    /// @param left_pt
    /// @return (is_finished, consumed_x, gained_y)
    fn process_liquidity_y(&mut self, pool_fee: u32, protocol_fee_rate: u32, amount_x: u128, left_pt: i32) -> (bool, u128, u128, u128, u128) {
        let net_amount = U256::from(amount_x).mul_fraction_floor((10u128.pow(6) - pool_fee as u128).into(), 10u128.pow(6).into()).as_u128();
        if net_amount > 0 {
            if self.liquidity > 0 {
                let x2y_range_result = 
                    self.range_x_swap_y(left_pt, net_amount);
                let fee_amount = if x2y_range_result.cost_x >= net_amount {
                    amount_x - x2y_range_result.cost_x
                } else {
                    U256::from(x2y_range_result.cost_x)
                    .mul_fraction_ceil(pool_fee.into(), (10u128.pow(6) - pool_fee as u128).into())
                    .as_u128()
                };
                // distribute fee
                let charged_fee_amount = fee_amount * protocol_fee_rate as u128 / BP_DENOM;
                self.total_fee_x_charged += charged_fee_amount;
                self.fee_scale_x_128 += U256::from(fee_amount - charged_fee_amount).mul_fraction_floor(pow_128(), U256::from(self.liquidity));               
                // update current point liquidity info
                self.current_point = x2y_range_result.final_pt;
                self.sqrt_price_96 = x2y_range_result.sqrt_final_price_96;
                self.liquidity_x = x2y_range_result.liquidity_x;

                (x2y_range_result.finished, x2y_range_result.cost_x + fee_amount, x2y_range_result.acquire_y.as_u128(), fee_amount, charged_fee_amount)
            } else {
                // swap hasn't completed but current range has no liquidity_y 
                if self.current_point != left_pt {
                    self.current_point = left_pt;
                    self.sqrt_price_96 = get_sqrt_price(left_pt);
                    self.liquidity_x = 0;
                }
                (false, 0, 0, 0, 0)
            }
        } else {
            // swap has already completed
            (true, 0, 0, 0, 0)
        }
    }

    /// Process x_swap_y with amount of token X, which is swapping to the left 
    /// @param protocol_fee_rate
    /// @param input_amount: amount of token X
    /// @param low_boundary_point: swap won't pass this point
    /// @param is_quote: whether is it called by a quote interface
    /// @return (consumed_x, gained_y, is_finished)
    pub fn internal_x_swap_y(&mut self, pool_fee: u32, protocol_fee_rate: u32, input_amount: u128, low_boundary_point: i32, is_quote: bool) -> (u128, u128, bool, u128, u128) {
        let boundary_point = std::cmp::max(low_boundary_point, LEFT_MOST_POINT);
        let mut amount = input_amount;
        let mut amount_x = 0;
        let mut amount_y = 0;
        let mut is_finished = false;
        let mut total_fee = 0;
        let mut protocol_fee = 0;

        while boundary_point <= self.current_point && !is_finished {
            let current_order_or_endpt = self.point_info.get_point_type_value(self.current_point, self.point_delta);
            // step1: process possible limit order on current point
            if current_order_or_endpt & 2 > 0 {
                let mut point_data = self.point_info.0.get(&self.current_point).unwrap();
                let mut order_data = point_data.order_data.take().unwrap();
                let process_ret = self.process_limit_order_y(pool_fee, protocol_fee_rate, &mut order_data, amount);
                is_finished = process_ret.0;
                (amount, amount_x, amount_y, total_fee, protocol_fee) = 
                    (amount-process_ret.1, amount_x+process_ret.1, amount_y+process_ret.2, total_fee+process_ret.3, protocol_fee+process_ret.4);
                
                self.update_point_order(&mut point_data, order_data, is_quote);

                if is_finished {
                    break;
                }
            }
            
            // step 2: process possible liquidity on current point
            let search_start = if current_order_or_endpt & 1 > 0 {
                // current point is an liquidity endpoint, process liquidity
                let process_ret = self.process_liquidity_y(pool_fee, protocol_fee_rate, amount, self.current_point);
                is_finished = process_ret.0;
                (amount, amount_x, amount_y, total_fee, protocol_fee) = 
                    (amount-process_ret.1, amount_x+process_ret.1, amount_y+process_ret.2, total_fee+process_ret.3, protocol_fee+process_ret.4);

                if !is_finished {
                    // pass endpoint
                    self.pass_endpoint(self.current_point, is_quote, true);
                    // move one step to the left
                    self.current_point -= 1;
                    self.sqrt_price_96 = get_sqrt_price(self.current_point);
                    self.liquidity_x = 0;
                }
                if is_finished || self.current_point < boundary_point {
                    break;
                }
                // new current point is an endpoint or has order, only exist in point_delta==1
                if self.point_info.get_point_type_value(self.current_point, self.point_delta) & 3 > 0 {
                    continue;
                }
                self.current_point
            } else {
                // the current_point unmoved cause it may be in the middle of some liquidity slot, need to be processed in next step
                self.current_point - 1
            };

            // step 3a: locate the left point for a range swapping headig to the left 
            let mut lack_one_point_to_real_left = false;
            let next_pt= match self.slot_bitmap.get_nearest_left_valued_slot(search_start, self.point_delta, boundary_point / self.point_delta){
                Some(point) => { 
                    if point < boundary_point {
                        boundary_point
                    } else {
                        if self.point_info.get_point_type_value(point, self.point_delta) & 2 > 0 {
                            lack_one_point_to_real_left = true;
                            // case 1: current_point is middle point and found left point is adjacent to it, then we actually need to do a single current point swap using process_liquidity_y;
                            // case 2: otherwise, we increase left point to protect order on left point;
                            point + 1
                        } else {
                            point
                        }
                    }
                },
                None => { boundary_point }
            };

            // step 3b: do range swap according to the left point located in step 3a
            let process_ret = self.process_liquidity_y(pool_fee, protocol_fee_rate, amount, next_pt);
            is_finished = process_ret.0;
            (amount, amount_x, amount_y, total_fee, protocol_fee) = 
                (amount-process_ret.1, amount_x+process_ret.1, amount_y+process_ret.2, total_fee+process_ret.3, protocol_fee+process_ret.4);
            
            // check the swap is completed or not
            if is_finished || self.current_point <= boundary_point {
                break;
            } 

            // Now, is_finished == false && self.current_point > boundary_point
            // adjust current point if necessary 
            if lack_one_point_to_real_left {
                // must move 1 left, otherwise infinite loop
                self.current_point -= 1;
                self.sqrt_price_96 = get_sqrt_price(self.current_point);
                self.liquidity_x = 0;
            }
        }
        
        (amount_x, amount_y, is_finished, total_fee, protocol_fee)
    }


}

impl Pool {
    /// Process limit_order_x at current point
    /// @param protocol_fee_rate
    /// @param order_data
    /// @param amount_y
    /// @return (is_finished, consumed_y, gained_x)
    fn process_limit_order_x(&mut self, pool_fee: u32, protocol_fee_rate: u32, order_data: &mut OrderData, amount_y: u128) -> (bool, u128, u128, u128, u128) {
        let mut is_finished = false;
        let net_amount = U256::from(amount_y).mul_fraction_floor((10u128.pow(6) - pool_fee as u128).into(), 10u128.pow(6).into()).as_u128();
        if net_amount > 0 {
            let (cost_y, acquire_x) = swap_math::y_swap_x_at_price(
                net_amount, self.sqrt_price_96, order_data.selling_x
            );
            if acquire_x < order_data.selling_x || cost_y >= net_amount {
                is_finished = true;
            }

            let fee_amount = if cost_y >= net_amount {
                // all x consumed
                amount_y - cost_y
            } else {
                U256::from(cost_y)
                .mul_fraction_ceil(pool_fee.into(), (10u128.pow(6) - pool_fee as u128).into())
                .as_u128()
            };

            // limit order fee goes to lp and protocol
            let protocol_fee = if self.liquidity != 0 {
                let charged_fee_amount = fee_amount * protocol_fee_rate as u128 / BP_DENOM;
                self.total_fee_y_charged += charged_fee_amount;
                self.fee_scale_y_128 += U256::from(fee_amount - charged_fee_amount).mul_fraction_floor(pow_128(), U256::from(self.liquidity));
                charged_fee_amount
            } else {
                self.total_fee_y_charged += fee_amount;
                fee_amount
            };

            // for statistic
            self.total_order_x -= acquire_x;
            
            order_data.selling_x -= acquire_x;
            order_data.earn_y += cost_y;
            order_data.acc_earn_y += U256::from(cost_y);

            if order_data.selling_x == 0 {
                // point order fulfilled, handle legacy logic
                order_data.earn_y_legacy += order_data.earn_y;
                order_data.acc_earn_y_legacy = order_data.acc_earn_y;
                order_data.earn_y = 0;
            }
            (is_finished, cost_y + fee_amount, acquire_x, fee_amount, protocol_fee)
        } else {
            (true, 0, 0, 0, 0)
        }
    }

    /// Process liquidity_x in range
    /// @param protocol_fee_rate
    /// @param amount_y
    /// @param next_pt
    /// @return (is_finished, consumed_y, gained_x)
    fn process_liquidity_x(&mut self, pool_fee: u32, protocol_fee_rate: u32, amount_y: u128, next_pt: i32) -> (bool, u128, u128, u128, u128) {
        let net_amount = U256::from(amount_y).mul_fraction_floor((10u128.pow(6) - pool_fee as u128).into(), 10u128.pow(6).into()).as_u128();
        if net_amount > 0 {
            let y2x_range_result = 
                self.range_y_swap_x(next_pt, net_amount);
            let fee_amount = if y2x_range_result.cost_y >= net_amount {
                amount_y - y2x_range_result.cost_y
            } else {
                U256::from(y2x_range_result.cost_y)
                    .mul_fraction_ceil(pool_fee.into(), (10u128.pow(6) - pool_fee as u128).into())
                    .as_u128()
            };

            let charged_fee_amount = fee_amount * protocol_fee_rate as u128 / BP_DENOM;
            self.total_fee_y_charged += charged_fee_amount;
            self.fee_scale_y_128 += U256::from(fee_amount - charged_fee_amount).mul_fraction_floor(pow_128(), U256::from(self.liquidity));

            self.current_point = y2x_range_result.final_pt;
            self.sqrt_price_96 = y2x_range_result.sqrt_final_price_96;
            self.liquidity_x = y2x_range_result.liquidity_x;
            (y2x_range_result.finished, y2x_range_result.cost_y + fee_amount, y2x_range_result.acquire_x.as_u128(), fee_amount, charged_fee_amount)
        } else {
            (true, 0, 0, 0, 0)
        }
    }

    /// Process y_swap_x in range
    /// @param protocol_fee_rate
    /// @param input_amount: amount of token Y
    /// @param hight_boundary_point
    /// @param is_quote: whether the quote function is calling
    /// @return (consumed_y, gained_x, is_finished)
    pub fn internal_y_swap_x(&mut self, pool_fee: u32, protocol_fee_rate: u32, input_amount: u128, hight_boundary_point: i32, is_quote: bool) -> (u128, u128, bool, u128, u128) {
        let boundary_point = std::cmp::min(hight_boundary_point, RIGHT_MOST_POINT);
        let mut amount = input_amount;
        let mut amount_x = 0;
        let mut amount_y = 0;
        let mut is_finished = false;
        let mut current_order_or_endpt  = self.point_info.get_point_type_value(self.current_point, self.point_delta);
        let mut total_fee = 0;
        let mut protocol_fee = 0;

        while self.current_point < boundary_point && !is_finished {
            if current_order_or_endpt & 2 > 0 {
                // process limit order
                let mut point_data = self.point_info.0.get(&self.current_point).unwrap();
                let mut order_data = point_data.order_data.take().unwrap();
                let process_ret = self.process_limit_order_x(pool_fee, protocol_fee_rate, &mut order_data, amount);
                is_finished = process_ret.0;
                (amount, amount_x, amount_y, total_fee, protocol_fee) = 
                    (amount-process_ret.1, amount_x+process_ret.2, amount_y+process_ret.1, total_fee+process_ret.3, protocol_fee+process_ret.4);
                
                self.update_point_order(&mut point_data, order_data, is_quote);

                if is_finished {
                    break;
                }
            }
            
            let (next_pt, next_val)= match self.slot_bitmap.get_nearest_right_valued_slot(self.current_point, self.point_delta, boundary_point / self.point_delta) {
                Some(point) => { 
                    if point > boundary_point {
                        (boundary_point, 0)
                    }else {
                        (point, self.point_info.get_point_type_value(point, self.point_delta))
                    }
                },
                None => { (boundary_point, 0) }
            };
            
            if self.liquidity == 0 {
                // no liquidity in the range [self.current_point, next_pt)
                self.current_point = next_pt;
                self.sqrt_price_96 = get_sqrt_price(self.current_point);
                if next_val & 1 > 0 {
                    // pass endpoint
                    self.pass_endpoint(next_pt, is_quote, false);
                    self.liquidity_x = self.liquidity;
                }
                current_order_or_endpt = next_val;
            } else {
                // process range liquidity [self.current_point, next_pt)
                let process_ret = self.process_liquidity_x(pool_fee, protocol_fee_rate, amount, next_pt);
                is_finished = process_ret.0;

                if self.current_point == next_pt {
                    if next_val & 1 > 0 {
                        // pass endpoint
                        self.pass_endpoint(next_pt, is_quote, false);
                    }
                    self.liquidity_x = self.liquidity;
                    current_order_or_endpt = next_val;
                } else {
                    current_order_or_endpt = 0;
                }

                (amount, amount_x, amount_y, total_fee, protocol_fee) = 
                    (amount-process_ret.1, amount_x+process_ret.2, amount_y+process_ret.1, total_fee+process_ret.3, protocol_fee+process_ret.4);
            }
        }

        (amount_y, amount_x, is_finished, total_fee, protocol_fee)
    }
}

impl Pool {
    /// Process limit_order_y by desire_y at current point
    /// @param protocol_fee_rate
    /// @param order_data
    /// @param desire_y
    /// @return (is_finished, consumed_x, gained_y)
    fn process_limit_order_y_desire_y(&mut self, pool_fee: u32, protocol_fee_rate: u32, order_data: &mut OrderData, desire_y: u128) -> (bool, u128, u128, u128, u128) {
        let mut is_finished = false;
        let (cost_x, acquire_y) = swap_math::x_swap_y_at_price_desire(
            desire_y, self.sqrt_price_96, order_data.selling_y
        );
        if acquire_y >= desire_y {
            is_finished = true;
        }

        // limit order fee goes to lp and protocol
        let fee_amount = U256::from(cost_x).mul_fraction_ceil(pool_fee.into(), (10u128.pow(6) - pool_fee as u128).into()).as_u128();
        let protocol_fee = if self.liquidity != 0 {
            let charged_fee_amount = fee_amount * protocol_fee_rate as u128 / BP_DENOM;
            self.total_fee_x_charged += charged_fee_amount;
            self.fee_scale_x_128 += U256::from(fee_amount - charged_fee_amount).mul_fraction_floor(pow_128(), U256::from(self.liquidity));
            charged_fee_amount
        } else {
            self.total_fee_x_charged += fee_amount;
            fee_amount
        };

        // for statistic
        self.total_order_y -= acquire_y;
        
        order_data.selling_y -= acquire_y;
        order_data.earn_x += cost_x;
        order_data.acc_earn_x += U256::from(cost_x);

        if order_data.selling_y == 0 {
            // point order fulfilled, handle legacy logic
            order_data.earn_x_legacy += order_data.earn_x;
            order_data.earn_x = 0;
            order_data.acc_earn_x_legacy = order_data.acc_earn_x;
        }

        (is_finished, cost_x + fee_amount, acquire_y, fee_amount, protocol_fee)
    }

    /// Process liquidity_y by desire_y in range
    /// @param protocol_fee_rate
    /// @param desire_y
    /// @param left_pt
    /// @return (is_finished, consumed_x, gained_y)
    fn process_liquidity_y_desire_y(&mut self, pool_fee: u32, protocol_fee_rate: u32, desire_y: u128, left_pt: i32) -> (bool, u128, u128, u128, u128) {
        if desire_y > 0 {
            if self.liquidity > 0 {
                let x2y_range_desire_result = self.range_x_swap_y_desire(left_pt, desire_y);
                let fee_amount = x2y_range_desire_result.cost_x.mul_fraction_ceil(pool_fee.into(), (10u128.pow(6) - pool_fee as u128).into()).as_u128();
                // distribute fee
                let charged_fee_amount = fee_amount * protocol_fee_rate as u128 / BP_DENOM;
                self.total_fee_x_charged += charged_fee_amount;
                self.fee_scale_x_128 += U256::from(fee_amount - charged_fee_amount).mul_fraction_floor(pow_128(), U256::from(self.liquidity));
                // update current point liquidity info
                self.current_point = x2y_range_desire_result.final_pt;
                self.sqrt_price_96 = x2y_range_desire_result.sqrt_final_price_96;
                self.liquidity_x = x2y_range_desire_result.liquidity_x;

                (x2y_range_desire_result.finished, (x2y_range_desire_result.cost_x + fee_amount).as_u128(), x2y_range_desire_result.acquire_y, fee_amount, charged_fee_amount)
            } else {
                // swap hasn't completed but current range has no liquidity_y 
                if self.current_point != left_pt {
                    self.current_point = left_pt;
                    self.sqrt_price_96 = get_sqrt_price(left_pt);
                    self.liquidity_x = 0;
                }
                (false, 0, 0, 0, 0)
            }
        } else {
            // swap has already completed
            (true, 0, 0, 0, 0)
        }
    }

    /// Process x_swap_y by desire_y in range
    /// @param protocol_fee_rate
    /// @param desire_y
    /// @param low_boundary_point
    /// @param is_quote: whether the quote function is calling
    /// @return (consumed_x, gained_y, is_finished)
    pub fn internal_x_swap_y_desire_y(&mut self, pool_fee: u32, protocol_fee_rate: u32, desire_y: u128, low_boundary_point: i32, is_quote: bool) -> (u128, u128, bool, u128, u128) {
        require!(desire_y > 0, E205_INVALID_DESIRE_AMOUNT);
        let boundary_point = std::cmp::max(low_boundary_point, LEFT_MOST_POINT);
        let mut is_finished = false;
        let mut amount_x = 0;
        let mut amount_y = 0;
        let mut desire_y = desire_y;
        let mut total_fee = 0;
        let mut protocol_fee = 0;

        while boundary_point <= self.current_point && !is_finished {
            let current_order_or_endpt = self.point_info.get_point_type_value(self.current_point, self.point_delta);
            // step1: process possible limit order on current point
            if current_order_or_endpt & 2 > 0 {
                let mut point_data = self.point_info.0.get(&self.current_point).unwrap();
                let mut order_data = point_data.order_data.take().unwrap();
                let process_ret = self.process_limit_order_y_desire_y(pool_fee, protocol_fee_rate, &mut order_data, desire_y);
                is_finished = process_ret.0;
                (desire_y, amount_x, amount_y, total_fee, protocol_fee) = 
                    (if desire_y <= process_ret.2 { 0 } else { desire_y - process_ret.2 }, amount_x + process_ret.1, amount_y + process_ret.2, total_fee + process_ret.3, protocol_fee + process_ret.4);

                self.update_point_order(&mut point_data, order_data, is_quote);

                if is_finished {
                    break;
                }
            }

            // step 2: process possible liquidity on current point
            let search_start = if current_order_or_endpt & 1 > 0 {
                // current point is an liquidity endpoint, process liquidity
                let process_ret = self.process_liquidity_y_desire_y(pool_fee, protocol_fee_rate, desire_y, self.current_point);
                is_finished = process_ret.0;
                (desire_y, amount_x, amount_y, total_fee, protocol_fee) = 
                    (desire_y - std::cmp::min(desire_y, process_ret.2), amount_x+process_ret.1, amount_y+process_ret.2, total_fee + process_ret.3, protocol_fee + process_ret.4);
                
                if !is_finished {
                    // pass endpoint
                    self.pass_endpoint(self.current_point, is_quote, true);
                    // move one step to the left
                    self.current_point -= 1;
                    self.sqrt_price_96 = get_sqrt_price(self.current_point);
                    self.liquidity_x = 0;
                }
                if is_finished || self.current_point < boundary_point {
                    break;
                }
                // new current point is an endpoint or has order, only exist in point_delta==1
                if self.point_info.get_point_type_value(self.current_point, self.point_delta) & 3 > 0 {
                    continue;
                }
                self.current_point
            } else {
                // the current_point unmoved cause it may be in the middle of some liquidity slot, need to be processed in next step
                self.current_point - 1
            };

            // step 3a: locate the left point for a range swapping headig to the left 
            let mut lack_one_point_to_real_left = false;
            let next_pt= match self.slot_bitmap.get_nearest_left_valued_slot(search_start, self.point_delta, boundary_point / self.point_delta){
                Some(point) => { 
                    if point < boundary_point {
                        boundary_point
                    } else {
                        if self.point_info.get_point_type_value(point, self.point_delta) & 2 > 0 {
                            lack_one_point_to_real_left = true;
                            // case 1: current_point is middle point and found left point is adjacent to it, then we actually need to do a single current point swap using process_liquidity_y;
                            // case 2: otherwise, we increase left point to protect order on left point;
                            point + 1
                        } else {
                            point
                        }
                    }
                },
                None => { boundary_point }
            };

            // step 3b: do range swap according to the left point located in step 3a
            let process_ret = self.process_liquidity_y_desire_y(pool_fee, protocol_fee_rate, desire_y, next_pt);
            is_finished = process_ret.0;
            (desire_y, amount_x, amount_y, total_fee, protocol_fee) = 
                (desire_y - std::cmp::min(desire_y, process_ret.2), amount_x+process_ret.1, amount_y+process_ret.2, total_fee + process_ret.3, protocol_fee + process_ret.4);

            // check the swap is completed or not
            if is_finished || self.current_point <= boundary_point {
                break;
            }

            // Now, is_finished == false && self.current_point > boundary_point
            // adjust current point if necessary 
            if lack_one_point_to_real_left {
                // must move 1 left, otherwise infinite loop
                self.current_point -= 1;
                self.sqrt_price_96 = get_sqrt_price(self.current_point);
                self.liquidity_x = 0;
            }
        }
        (amount_x, amount_y, is_finished, total_fee, protocol_fee)
    }
}

impl Pool {
    /// Process limit_order_x by desire_x at current point
    /// @param protocol_fee_rate
    /// @param order_data
    /// @param desire_x
    /// @return (is_finished, consumed_y, gained_x)
    fn process_limit_order_x_desire_x(&mut self, pool_fee: u32, protocol_fee_rate: u32, order_data: &mut OrderData, desire_x: u128) -> (bool, u128, u128, u128, u128) {
        let mut is_finished = false;
        let (cost_y, acquire_x) = swap_math::y_swap_x_at_price_desire(
            desire_x, self.sqrt_price_96, order_data.selling_x
        );
        if acquire_x >= desire_x {
            is_finished = true;
        }

        // limit order fee goes to lp and protocol
        let fee_amount = U256::from(cost_y).mul_fraction_ceil(pool_fee.into(), (10u128.pow(6) - pool_fee as u128).into()).as_u128();
        let protocol_fee = if self.liquidity != 0 {
            let charged_fee_amount = fee_amount * protocol_fee_rate as u128 / BP_DENOM;
            self.total_fee_y_charged += charged_fee_amount;
            self.fee_scale_y_128 += U256::from(fee_amount - charged_fee_amount).mul_fraction_floor(pow_128(), U256::from(self.liquidity));
            charged_fee_amount
        } else {
            self.total_fee_y_charged += fee_amount;
            fee_amount
        };

        // for statistic
        self.total_order_x -= acquire_x;

        order_data.selling_x -= acquire_x;
        order_data.earn_y += cost_y;
        order_data.acc_earn_y += U256::from(cost_y);

        if order_data.selling_x == 0 {
            // point order fulfilled, handle legacy logic
            order_data.earn_y_legacy += order_data.earn_y;
            order_data.earn_y = 0;
            order_data.acc_earn_y_legacy = order_data.acc_earn_y;
        }
        (is_finished, cost_y + fee_amount, acquire_x, fee_amount, protocol_fee)
    }

    /// Process liquidity_x by desire_x in range
    /// @param protocol_fee_rate
    /// @param desire_x
    /// @param next_pt
    /// @return (is_finished, consumed_y, gained_x)
    fn process_liquidity_x_desire_x(&mut self, pool_fee: u32, protocol_fee_rate: u32, desire_x: u128, next_pt: i32) -> (bool, u128, u128, u128, u128) {
        if desire_x > 0 {
            let y2x_range_desire_result = self.range_y_swap_x_desire(next_pt, desire_x);

            let fee_amount = y2x_range_desire_result.cost_y.mul_fraction_ceil(pool_fee.into(), (10u128.pow(6) - pool_fee as u128).into()).as_u128();
            let charged_fee_amount = fee_amount * protocol_fee_rate as u128 / BP_DENOM;
            self.total_fee_y_charged += charged_fee_amount;
            self.fee_scale_y_128 += U256::from(fee_amount - charged_fee_amount).mul_fraction_floor(pow_128(), U256::from(self.liquidity));

            self.current_point = y2x_range_desire_result.final_pt;
            self.sqrt_price_96 = y2x_range_desire_result.sqrt_final_price_96;
            self.liquidity_x = y2x_range_desire_result.liquidity_x;
            (y2x_range_desire_result.finished, (y2x_range_desire_result.cost_y + fee_amount).as_u128(), y2x_range_desire_result.acquire_x, fee_amount, charged_fee_amount)
        } else {
            (true, 0, 0, 0, 0)
        }
    }

    /// Process y_swap_x by desire_x in range
    /// @param protocol_fee_rate
    /// @param desire_x
    /// @param high_boundary_point
    /// @param is_quote: whether the quote function is calling
    /// @return (consumed_y, gained_x, is_finished)
    pub fn internal_y_swap_x_desire_x(&mut self, pool_fee: u32, protocol_fee_rate: u32, desire_x: u128, high_boundary_point: i32, is_quote: bool) -> (u128, u128, bool, u128, u128) {
        require!(desire_x > 0, E205_INVALID_DESIRE_AMOUNT);
        let boundary_point = std::cmp::min(high_boundary_point, RIGHT_MOST_POINT);
        let mut is_finished = false;
        let mut amount_x = 0;
        let mut amount_y = 0;
        let mut desire_x = desire_x;
        let mut total_fee = 0;
        let mut protocol_fee = 0;
        let mut current_order_or_endpt  = self.point_info.get_point_type_value(self.current_point, self.point_delta);
        
        while self.current_point < boundary_point && !is_finished {
            if current_order_or_endpt & 2 > 0 {
                // process limit order
                let mut point_data = self.point_info.0.get(&self.current_point).unwrap();
                let mut order_data = point_data.order_data.take().unwrap();
                let process_ret = self.process_limit_order_x_desire_x(pool_fee, protocol_fee_rate, &mut order_data, desire_x);
                is_finished = process_ret.0;
                (desire_x, amount_x, amount_y, total_fee, protocol_fee) = 
                    (if desire_x <= process_ret.2 { 0 } else { desire_x - process_ret.2 }, amount_x + process_ret.2, amount_y + process_ret.1, total_fee + process_ret.3, protocol_fee + process_ret.4);

                self.update_point_order(&mut point_data, order_data, is_quote);

                if is_finished {
                    break;
                }
            }
            
            let (next_pt, next_val)= match self.slot_bitmap.get_nearest_right_valued_slot(self.current_point, self.point_delta, boundary_point / self.point_delta) {
                Some(point) => { 
                    if point > boundary_point {
                        (boundary_point, 0)
                    }else {
                        (point, self.point_info.get_point_type_value(point, self.point_delta))
                    }
                },
                None => { (boundary_point, 0) }
            };
            
            if self.liquidity == 0 {
                self.current_point = next_pt;
                self.sqrt_price_96 = get_sqrt_price(self.current_point);
                if next_val & 1 > 0 {
                    self.pass_endpoint(next_pt, is_quote, false);
                    self.liquidity_x = self.liquidity;
                }
                current_order_or_endpt = next_val;
            } else {
                let process_ret = self.process_liquidity_x_desire_x(pool_fee, protocol_fee_rate, desire_x, next_pt);
                is_finished = process_ret.0;
                (desire_x, amount_x, amount_y, total_fee, protocol_fee) = 
                    (desire_x - std::cmp::min(desire_x, process_ret.2), amount_x+process_ret.2, amount_y+process_ret.1, total_fee + process_ret.3, protocol_fee + process_ret.4);

                if self.current_point == next_pt {
                    if next_val & 1 > 0 {
                        self.pass_endpoint(next_pt, is_quote, false);
                    }
                    self.liquidity_x = self.liquidity;
                    current_order_or_endpt = next_val;
                } else {
                    current_order_or_endpt = 0;
                }
            }
        }

        (amount_y, amount_x, is_finished, total_fee, protocol_fee)
    }
}

impl Pool {
    /// Create a new pool
    /// @param pool_id: a string like token_a|token_b|fee
    /// @param point_delta: minimum interval between two endpoints
    /// @param initial_point: the current point position when the pool was created
    /// @return Pool object
    pub fn new(pool_id: &PoolId, point_delta: u32, initial_point: i32) -> Self {
        let (token_x, token_y, fee) = pool_id.parse_pool_id();
        let sqrt_price_96 = get_sqrt_price(initial_point);
        let point_num = (RIGHT_MOST_POINT - LEFT_MOST_POINT) as u32 / point_delta + 1;
        Pool {
            pool_id: pool_id.clone(),
            token_x,
            token_y,
            fee,
            point_delta: point_delta as i32,
            current_point: initial_point,
            sqrt_price_96,
            liquidity: 0u128,
            liquidity_x: 0u128,
            max_liquidity_per_point: u128::MAX / point_num as u128,
            fee_scale_x_128: Default::default(),
            fee_scale_y_128: Default::default(),
            total_fee_x_charged: Default::default(),
            total_fee_y_charged: Default::default(),
            total_liquidity: 0,
            total_order_x: 0,
            total_order_y: 0,
            total_x: 0,
            total_y: 0,  
            volume_x_in: U256::zero(),
            volume_y_in: U256::zero(),
            volume_x_out: U256::zero(),
            volume_y_out: U256::zero(),
            point_info: PointInfo(LookupMap::new(StorageKeys::PointInfo { pool_id: pool_id.clone() })),
            slot_bitmap: SlotBitmap(LookupMap::new(StorageKeys::PointBitmap { pool_id: pool_id.clone() })),
            oracle: Vector::new(StorageKeys::Oracle { pool_id: pool_id.clone() }),
            oracle_current_index: 0,
            state: RunningState::Running,
        }
    }

    /// Add liquidity in specified range
    /// @param left_point: left point of this range
    /// @param right_point: right point of this range
    /// @param amount_x: the number of token X users expect to add liquidity to use
    /// @param amount_y: the number of token Y users expect to add liquidity to use
    /// @param min_amount_x: the minimum number of token X users expect to add liquidity to use
    /// @param min_amount_y: the minimum number of token Y users expect to add liquidity to use
    /// @return (liquidity, need_x, need_y, acc_fee_x_in_128, acc_fee_y_in_128)
    pub fn internal_add_liquidity(
        &mut self, 
        left_point: i32,
        right_point: i32,
        amount_x: u128,
        amount_y: u128,
        min_amount_x: u128,
        min_amount_y: u128,
        is_view: bool
    ) -> (u128, u128, u128, U256, U256) {
        let liquidity = self.compute_liquidity(left_point, right_point, amount_x, amount_y);
        require!(liquidity > 0, E214_INVALID_LIQUIDITY);
        let (acc_fee_x_in_128, acc_fee_y_in_128) = if !is_view {
            self.update_pool(left_point, right_point, liquidity as i128)
        } else {
            (Default::default(), Default::default())
        };
        let (need_x, need_y) = self.compute_deposit_x_y(left_point, right_point, liquidity);
        require!(need_x >= min_amount_x, E204_SLIPPAGE_ERR);
        require!(need_y >= min_amount_y, E204_SLIPPAGE_ERR);
        (liquidity, need_x, need_y, acc_fee_x_in_128, acc_fee_y_in_128)
    }

    /// Removes specified number of liquidity in specified range
    /// @param liquidity: the number of liquidity expected to be removed
    /// @param left_point: left point of this range
    /// @param right_point: right point of this range
    /// @param min_amount_x: removing liquidity will at least give you the number of token X
    /// @param min_amount_y: removing liquidity will at least give you the number of token Y
    /// @return (remove_x, remove_y, acc_fee_x_in_128, acc_fee_y_in_128)
    pub fn internal_remove_liquidity(
        &mut self, 
        liquidity: u128,
        left_point: i32,
        right_point: i32,
        min_amount_x: u128,
        min_amount_y: u128,
    ) -> (u128, u128, U256, U256) {
        require!(liquidity <= i128::MAX as u128, E214_INVALID_LIQUIDITY);
        let (acc_fee_x_in_128, acc_fee_y_in_128) = self.update_pool(left_point, right_point, -(liquidity as i128));
        let (remove_x, remove_y) = self.compute_withdraw_x_y(left_point, right_point, liquidity);
        require!(remove_x >= min_amount_x, E204_SLIPPAGE_ERR);
        require!(remove_y >= min_amount_y, E204_SLIPPAGE_ERR);
        (remove_x, remove_y, acc_fee_x_in_128, acc_fee_y_in_128)
    }
}

impl Pool {
    /// Compute the token X and token Y that need to be added to add the specified liquidity in the specified range
    /// @param left_point: left point of this range
    /// @param right_point: right point of this range
    /// @param liquidity: The amount of liquidity expected to be added
    /// @return (amount_x, amount_y)
    fn compute_deposit_x_y(
        &mut self,
        left_point: i32,
        right_point: i32,
        liquidity: u128,
    ) -> (u128, u128) {
        let sqrt_price_r_96 = get_sqrt_price(right_point);
        let mut amount_y = if left_point < self.current_point {
            let sqrt_price_l_96 = get_sqrt_price(left_point);
            if right_point < self.current_point {
                get_amount_y(liquidity, sqrt_price_l_96, sqrt_price_r_96, sqrt_rate_96(), true)
            } else {
                get_amount_y(liquidity, sqrt_price_l_96, self.sqrt_price_96, sqrt_rate_96(), true)
            }
        } else {
            Default::default()
        };

        let amount_x = if right_point > self.current_point {
            let xr_left = if left_point > self.current_point {
                left_point
            } else {
                self.current_point + 1
            };
            get_amount_x(liquidity, xr_left, right_point, sqrt_price_r_96, sqrt_rate_96(), true).as_u128()
        } else {
            0
        };

        if left_point <= self.current_point && right_point > self.current_point {
            amount_y += U256::from(liquidity).mul_fraction_ceil(self.sqrt_price_96, pow_96());
            self.liquidity += liquidity;
        }
        (amount_x, amount_y.as_u128())
    }

    /// Compute the token X and token Y obtained by removing the specified liquidity in the specified range
    /// @param left_point: left point of this range
    /// @param right_point: right point of this range
    /// @param liquidity: The amount of liquidity expected to be removed
    /// @return (amount_x, amount_y)
    fn compute_withdraw_x_y(
        &mut self,
        left_point: i32,
        right_point: i32,
        liquidity: u128,
    ) -> (u128, u128) {
        let sqrt_price_r_96 = get_sqrt_price(right_point);
        let mut amount_y = if left_point < self.current_point {
            let sqrt_price_l_96 = get_sqrt_price(left_point);
            if right_point < self.current_point {
                get_amount_y(liquidity, sqrt_price_l_96, sqrt_price_r_96, sqrt_rate_96(), false)
            } else {
                get_amount_y(liquidity, sqrt_price_l_96, self.sqrt_price_96, sqrt_rate_96(), false)
            }
        } else {
            Default::default()
        };

        let mut amount_x = if right_point > self.current_point {
            let xr_left = if left_point > self.current_point {
                left_point
            } else {
                self.current_point + 1
            };
            get_amount_x(liquidity, xr_left, right_point, sqrt_price_r_96, sqrt_rate_96(), false)
        } else {
            Default::default()
        };
        if left_point <= self.current_point && right_point > self.current_point {
            let origin_liquidity_y = self.liquidity - self.liquidity_x;
            let withdrawed_liquidity_y = if origin_liquidity_y < liquidity { origin_liquidity_y } else { liquidity };
            let withdrawed_liquidity_x = liquidity - withdrawed_liquidity_y;
            amount_y += U256::from(withdrawed_liquidity_y).mul_fraction_floor(self.sqrt_price_96, pow_96());
            amount_x += U256::from(withdrawed_liquidity_x).mul_fraction_floor(pow_96(), self.sqrt_price_96);

            self.liquidity -= liquidity;
            self.liquidity_x -= withdrawed_liquidity_x;
        }
        (amount_x.as_u128(), amount_y.as_u128())
    }

    /// The two boundary points of liquidity are updated according to the amount of liquidity change, and return the fee within range 
    /// @param left_point: left point of this range
    /// @param right_point: right point of this range
    /// @param liquidity_delta: The amount of liquidity change, it could be negative
    /// @return (acc_fee_x_in_128, acc_fee_y_in_128)
    fn update_pool(
        &mut self, 
        left_point: i32,
        right_point: i32,
        liquidity_delta: i128,
    ) -> (U256, U256) {
        let (left_new_or_erase, right_new_or_erase) = if liquidity_delta != 0 {
            (self.point_info.update_endpoint(left_point, true, self.current_point, liquidity_delta, self.max_liquidity_per_point, self.fee_scale_x_128, self.fee_scale_y_128),
            self.point_info.update_endpoint(right_point, false, self.current_point, liquidity_delta, self.max_liquidity_per_point, self.fee_scale_x_128, self.fee_scale_y_128))
        } else {
            (false, false)
        };
        let (acc_fee_x_in_128, acc_fee_y_in_128) = self.point_info.get_fee_in_range(left_point, right_point, self.current_point, self.fee_scale_x_128, self.fee_scale_y_128);


        if left_new_or_erase {
            let mut left_endpoint = self.point_info.0.get(&left_point).unwrap();
            if left_endpoint.has_liquidity() {  // new endpoint for liquidity
                if !left_endpoint.has_active_order() {
                    self.slot_bitmap.set_one(left_point, self.point_delta);
                }
            } else {  // removed endpoint for liquidity
                left_endpoint.liquidity_data = None;
                if !left_endpoint.has_active_order() {
                    self.slot_bitmap.set_zero(left_point, self.point_delta);
                }
                if left_endpoint.has_order() {
                    self.point_info.0.insert(&left_point, &left_endpoint);
                } else {
                    self.point_info.0.remove(&left_point);
                }
            }
        }

        if right_new_or_erase {
            let mut right_endpoint = self.point_info.0.get(&right_point).unwrap();
            if right_endpoint.has_liquidity() {  // new endpoint for liquidity
                if !right_endpoint.has_active_order() {
                    self.slot_bitmap.set_one(right_point, self.point_delta);
                }
            } else {  // removed endpoint for liquidity
                right_endpoint.liquidity_data = None;
                if !right_endpoint.has_active_order() {
                    self.slot_bitmap.set_zero(right_point, self.point_delta);
                } 
                if right_endpoint.has_order() {
                    self.point_info.0.insert(&right_point, &right_endpoint);
                } else {
                    self.point_info.0.remove(&right_point);
                }
            }
        }
        (acc_fee_x_in_128, acc_fee_y_in_128)
    }

    /// compute how much liquidity the specified token X and token Y can add in the specified range
    /// @param left_point: left point of this range
    /// @param right_point: right point of this range
    /// @param amount_x: the number of token X users expect to add liquidity to use
    /// @param amount_y: the number of token Y users expect to add liquidity to use
    /// @return liquidity
    fn compute_liquidity(
        &self, 
        left_point: i32,
        right_point: i32,
        amount_x: u128,
        amount_y: u128,
    ) -> u128 {
        let mut liquidity = u128::MAX / 2;
        let (x, y) = self.compute_deposit_xy_per_unit(left_point, right_point);
        if !x.is_zero() {
            let xl = U256::from(amount_x).mul_fraction_floor(pow_96(), x).as_u128();
            if liquidity > xl {
                liquidity = xl;
            }
        }

        if !y.is_zero() {
            let yl = U256::from(amount_y - 1).mul_fraction_floor(pow_96(), y).as_u128();
            if liquidity > yl {
                liquidity = yl;
            }
        }

        liquidity
    }

    /// compute the amount of token X and token Y required to add a unit of liquidity within a specified range
    /// @param left_point: left point of this range
    /// @param right_point: right point of this range
    /// @return (x, y)
    fn compute_deposit_xy_per_unit(
        &self, 
        left_point: i32,
        right_point: i32,
    ) -> (U256, U256) {
        let sqrt_price_r_96 = get_sqrt_price(right_point);
        let mut y = if left_point < self.current_point {
            let sqrt_price_l_96 = get_sqrt_price(left_point);
            if right_point < self.current_point {
                get_amount_y_unit_liquidity_96(sqrt_price_l_96, sqrt_price_r_96, sqrt_rate_96())
            } else {
                get_amount_y_unit_liquidity_96(sqrt_price_l_96, self.sqrt_price_96, sqrt_rate_96())
            }
        } else {
            U256::from(0u128)
        };
        let x = if right_point > self.current_point {
            let xr_left = if left_point > self.current_point {left_point} else {self.current_point + 1};
            get_amount_x_unit_liquidity_96(xr_left, right_point, sqrt_price_r_96, sqrt_rate_96())
        } else {
            U256::from(0u128)
        };
        if left_point <= self.current_point && right_point > self.current_point {
            y += self.sqrt_price_96;
        }
        (x, y)
    }
    
}

impl Contract {
    /// @param pool_id
    /// @return Option<Pool> 
    pub fn internal_get_pool(&self, pool_id: &PoolId) -> Option<Pool> {
        self.data().pools.get(pool_id).map(|o| o.into())
    }

    /// @param pool_id
    /// @return Pool or expect
    pub fn internal_unwrap_pool(&self, pool_id: &PoolId) -> Pool {
        self.internal_get_pool(pool_id)
            .expect(E403_POOL_NOT_EXIST)
    }

    /// @param pool_id
    /// @param pool
    pub fn internal_set_pool(&mut self, pool_id: &PoolId, pool: Pool) {
        self.data_mut().pools.insert(pool_id, &pool.into());
    }
}

/// Calculate the new liquidity by the current liquidity and the change in liquidity
/// @param liquidity
/// @param delta
/// @return new liquidity
pub fn liquidity_add_delta(liquidity: u128, delta: i128) -> u128 {
    if delta < 0 {
        liquidity - (-delta) as u128
    } else {
        liquidity + delta as u128
    }
}