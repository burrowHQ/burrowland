use crate::*;

#[derive(BorshDeserialize, BorshSerialize, Default)]
#[cfg_attr(test, derive(Debug))]
pub struct PointData{
    pub liquidity_data: Option<LiquidityData>,
    pub order_data: Option<OrderData>,
}

impl PointData {
    /// see if corresponding bit in slot_bitmap should be set
    pub fn has_active_liquidity(&self) -> bool {
        if let Some(liquidity_data) = &self.liquidity_data {
            return liquidity_data.liquidity_sum > 0;
        }
        false
    }

    /// see if there is some x to sell
    pub fn has_active_order_x(&self) -> bool {
        if let Some(order_data) = &self.order_data {
            return order_data.selling_x != 0;
        }
        false
    }

    /// see if there is some y to sell
    pub fn has_active_order_y(&self) -> bool {
        if let Some(order_data) = &self.order_data {
            return order_data.selling_y != 0;
        }
        false
    }

    /// see if corresponding bit in slot_bitmap should be set
    pub fn has_active_order(&self) -> bool {
        self.has_active_order_x() || self.has_active_order_y()
    }

    /// tell self.liquidity_data should be Some or None
    pub fn has_liquidity(&self) -> bool {
        if let Some(liquidity_data) = &self.liquidity_data {
            return liquidity_data.liquidity_sum > 0;
        }
        false
    }
    
    /// tell self.order_data should be Some or None
    pub fn has_order(&self) -> bool {
        if let Some(order_data) = &self.order_data {
            return order_data.user_order_count > 0;
        }
        false
    }
}

#[derive(BorshDeserialize, BorshSerialize, Default)]
#[cfg_attr(test, derive(Debug))]
pub struct LiquidityData{
    pub liquidity_sum: u128,
    pub liquidity_delta: i128,
    pub acc_fee_x_out_128: U256,
    pub acc_fee_y_out_128: U256,
}

impl LiquidityData {
    pub fn pass_endpoint(
        &mut self,
        fee_scale_x_128: U256,
        fee_scale_y_128: U256
    ) {
        self.acc_fee_x_out_128 = fee_scale_x_128 - self.acc_fee_x_out_128;
        self.acc_fee_y_out_128 = fee_scale_y_128 - self.acc_fee_y_out_128;
    }
}

#[derive(BorshDeserialize, BorshSerialize, Serialize,Deserialize,Clone, Copy, Default)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(test, derive(Debug))]
pub struct OrderData{
    #[serde(with = "u128_dec_format")]
    pub selling_x: u128,
    #[serde(with = "u128_dec_format")]
    pub earn_y: u128,
    #[serde(with = "u128_dec_format")]
    pub earn_y_legacy: u128,
    #[serde(with = "u256_dec_format")]
    pub acc_earn_y: U256,
    #[serde(with = "u256_dec_format")]
    pub acc_earn_y_legacy: U256,
 
    #[serde(with = "u128_dec_format")]
    pub selling_y: u128,
    #[serde(with = "u128_dec_format")]
    pub earn_x: u128,
    #[serde(with = "u128_dec_format")]
    pub earn_x_legacy: u128,
    #[serde(with = "u256_dec_format")]
    pub acc_earn_x: U256,
    #[serde(with = "u256_dec_format")]
    pub acc_earn_x_legacy: U256,
    #[serde(with = "u64_dec_format")]
    pub user_order_count: u64,
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct PointInfo(pub LookupMap<i32, PointData>);

impl PointInfo {

    pub fn get_liquidity_data(
        &self,
        point: i32,
    ) -> LiquidityData {
        self.0.get(&point).unwrap().liquidity_data.unwrap()
    }

    pub fn get_order_data(
        &self,
        point: i32,
    ) -> OrderData {
        self.0.get(&point).unwrap().order_data.unwrap()
    }

    pub fn has_active_liquidity(
        &self,
        point: i32,
        point_delta: i32
    ) -> bool {
        if point % point_delta == 0 {
            if let Some(point_data) = self.0.get(&point) {
                return point_data.has_active_liquidity();
            }
        }
        false
    }

    pub fn has_active_order(
        &self,
        point: i32,
        point_delta: i32
    ) -> bool {
        if point % point_delta == 0 {
            if let Some(point_data) = self.0.get(&point) {
                return point_data.has_active_order();
            }
        }
        false
    }

    pub fn get_point_type_value(
        &self,
        point: i32,
        point_delta: i32
    ) -> u8 {
        let mut point_type = 0;
        if point % point_delta == 0 {
            if self.has_active_liquidity(point, point_delta) {
                point_type |= 1;
            }
            if self.has_active_order(point, point_delta) {
                point_type |= 2;
            }
        }
        point_type
    }

    pub fn get_fee_in_range(
        &self,
        left_point: i32,
        right_point: i32,
        current_point: i32,
        fee_scale_x_128: U256,
        fee_scale_y_128: U256
    ) -> (U256, U256) {
        let left_point_data = self.0.get(&left_point).unwrap().liquidity_data.unwrap();
        let right_point_data = self.0.get(&right_point).unwrap().liquidity_data.unwrap();
        let fee_scale_lx_128 = get_fee_scale_l(left_point, current_point, fee_scale_x_128, left_point_data.acc_fee_x_out_128);
        let fee_scale_gex_128 = get_fee_scale_ge(right_point, current_point, fee_scale_x_128, right_point_data.acc_fee_x_out_128);
        let fee_scale_ly_128 = get_fee_scale_l(left_point, current_point, fee_scale_y_128, left_point_data.acc_fee_y_out_128);
        let fee_scale_gey_128 = get_fee_scale_ge(right_point, current_point, fee_scale_y_128, right_point_data.acc_fee_y_out_128);
        
        (fee_scale_x_128.overflowing_sub(fee_scale_lx_128).0
            .overflowing_sub(fee_scale_gex_128).0,
        fee_scale_y_128.overflowing_sub(fee_scale_ly_128).0
            .overflowing_sub(fee_scale_gey_128).0
        )
    }

    pub fn update_endpoint(
        &mut self,
        endpoint: i32,
        is_left: bool,
        current_point: i32,
        liquidity_delta: i128,
        max_liquidity_per_point: u128,
        fee_scale_x_128: U256,
        fee_scale_y_128: U256
    ) -> bool {
        let mut point_data = self.0.remove(&endpoint).unwrap_or_default();
        let mut liquidity_data = point_data.liquidity_data.take().unwrap_or_default();
        let liquid_acc_before = liquidity_data.liquidity_sum;
        let liquid_acc_after = if liquidity_delta > 0 {
            liquid_acc_before + liquidity_delta as u128
        } else {
            liquid_acc_before - (-liquidity_delta) as u128
        };
        require!(liquid_acc_after <= max_liquidity_per_point, E203_LIQUIDITY_OVERFLOW);
        liquidity_data.liquidity_sum = liquid_acc_after;

        if is_left {
            liquidity_data.liquidity_delta += liquidity_delta;
        } else {
            liquidity_data.liquidity_delta -= liquidity_delta;
        }

        let mut new_or_erase = false;
        if liquid_acc_before == 0 {
            new_or_erase = true;
            if endpoint >= current_point {
                liquidity_data.acc_fee_x_out_128 = fee_scale_x_128;
                liquidity_data.acc_fee_y_out_128 = fee_scale_y_128;
            }
        } else if liquid_acc_after == 0 {
            new_or_erase = true;
        }
        point_data.liquidity_data = Some(liquidity_data);
        self.0.insert(&endpoint, &point_data);
        new_or_erase
    }
}

fn get_fee_scale_l(
    endpoint: i32,
    current_point: i32,
    fee_scale_128: U256,
    fee_scale_beyond_128: U256,
) -> U256 {
    if endpoint <= current_point {
        fee_scale_beyond_128
    } else {
        fee_scale_128 - fee_scale_beyond_128
    }
}

fn get_fee_scale_ge(
    endpoint: i32,
    current_point: i32,
    fee_scale_128: U256,
    fee_scale_beyond_128: U256,
) -> U256 {
    if endpoint > current_point {
        fee_scale_beyond_128
    } else {
        fee_scale_128 - fee_scale_beyond_128
    }
}