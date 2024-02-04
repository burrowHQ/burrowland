use crate::*;

// group returned values of x2YRange to avoid stake too deep
#[derive(Default)]
pub struct X2YRangeRet {
    // whether user run out of amountX
    pub finished: bool,
    // actual cost of tokenX to buy tokenY
    pub cost_x: u128,
    // amount of acquired tokenY
    pub acquire_y: U256,
    // final point after this swap
    pub final_pt: i32,
    // sqrt price on final point
    pub sqrt_final_price_96: U256,
    // liquidity of tokenX at finalPt
    pub liquidity_x: u128
}

#[derive(Default)]
pub struct X2YRangeRetDesire {
    pub finished: bool,
    pub cost_x: U256,
    pub acquire_y: u128,
    pub final_pt: i32,
    pub sqrt_final_price_96: U256,
    pub liquidity_x: u128
}

#[derive(Default)]
pub struct Y2XRangeRetDesire {
    pub finished: bool,
    pub cost_y: U256,
    pub acquire_x: u128,
    pub final_pt: i32,
    pub sqrt_final_price_96: U256,
    pub liquidity_x: u128
}

#[derive(Default)]
struct X2YRangeCompRet {
    cost_x: u128,
    acquire_y: U256,
    complete_liquidity: bool,
    loc_pt: i32,
    sqrt_loc_96: U256
}

#[derive(Default)]
pub struct Y2XRangeRet {
    // whether user has run out of token_y
    pub finished: bool,
    // actual cost of token_y to buy token_x
    pub cost_y: u128,
    // actual amount of token_x acquired
    pub acquire_x: U256,
    // final point after this swap
    pub final_pt: i32,
    // sqrt price on final point
    pub sqrt_final_price_96: U256,
    // liquidity of token_x at final_pt
    // if final_pt is not right_pt, liquidity_x is meaningless
    pub liquidity_x: u128,
}

#[derive(Default)]
pub struct Y2XRangeCompRet {
    cost_y: u128,
    acquire_x: U256,
    complete_liquidity: bool,
    loc_pt: i32,
    sqrt_loc_96: U256
}

#[derive(Default)]
pub struct X2YRangeCompRetDesire {
    cost_x: U256,
    acquire_y: u128,
    complete_liquidity: bool,
    loc_pt: i32,
    sqrt_loc_96: U256
}

#[derive(Default)]
pub struct Y2XRangeCompRetDesire {
    cost_y: U256,
    acquire_x: u128,
    complete_liquidity: bool,
    loc_pt: i32,
    sqrt_loc_96: U256
}

impl Pool {
    /// @param left_point: the left boundary of range
    /// @param amount_x: the amount of token X to swap-in
    /// @return X2YRangeRet
    /// @remark swap range (from right to left): [left_point, current_point]
    pub fn range_x_swap_y(&mut self, left_point: i32, amount_x: u128) -> X2YRangeRet{
        let mut result = X2YRangeRet::default();
        let mut amount_x = amount_x;

        let current_has_y = self.liquidity_x < self.liquidity;
        if current_has_y && (self.liquidity_x > 0 || left_point == self.current_point) {
            // current point as a special point to swap first
            let (at_price_cost_x, at_price_acquire_y, at_price_liquidity_x) = x_swap_y_at_price_liquidity(amount_x, self.sqrt_price_96, self.liquidity, self.liquidity_x);
            result.cost_x = at_price_cost_x;
            result.acquire_y = at_price_acquire_y;
            result.liquidity_x = at_price_liquidity_x;
            if at_price_liquidity_x < self.liquidity ||  at_price_cost_x >= amount_x {
                result.finished = true;
                result.final_pt = self.current_point;
                result.sqrt_final_price_96 = self.sqrt_price_96;
            } else {
                amount_x -= at_price_cost_x;
            }
        } else if current_has_y {
            // in this branch, current point is same as those in its left, so form it into left range 
            self.current_point += 1;
            self.sqrt_price_96 = self.sqrt_price_96 + self.sqrt_price_96.mul_fraction_floor(sqrt_rate_96() - pow_96(), pow_96());
        } else {
            // only has liquidity_x part
            // TODO: seems this code is useless
            result.liquidity_x = self.liquidity_x;
        }

        if result.finished {
            return result;
        }

        if left_point < self.current_point {
            let sqrt_price_l_96 = get_sqrt_price(left_point);
            let x2y_range_comp_result = 
            x_swap_y_range_complete(self.liquidity, sqrt_price_l_96, left_point, self.sqrt_price_96, self.current_point, amount_x);
            result.cost_x += x2y_range_comp_result.cost_x;
            amount_x -= x2y_range_comp_result.cost_x;
            result.acquire_y += x2y_range_comp_result.acquire_y;
            if x2y_range_comp_result.complete_liquidity {
                result.finished = amount_x == 0;
                result.final_pt = left_point;
                result.sqrt_final_price_96 = sqrt_price_l_96;
                result.liquidity_x = self.liquidity;
            } else {
                let (at_price_cost_x, at_price_acquire_y, at_price_liquidity_x) = x_swap_y_at_price_liquidity(amount_x, x2y_range_comp_result.sqrt_loc_96, self.liquidity, 0);
                result.cost_x += at_price_cost_x;
                result.acquire_y += at_price_acquire_y;
                result.finished = true;
                result.sqrt_final_price_96 = x2y_range_comp_result.sqrt_loc_96;
                result.final_pt = x2y_range_comp_result.loc_pt;
                result.liquidity_x = at_price_liquidity_x;
            }
        } else {
            result.final_pt = self.current_point;
            result.sqrt_final_price_96 = self.sqrt_price_96;
        }

        result
    }

    /// @param right_point: the right boundary of range
    /// @param amount_y: the amount of token Y to swap-in
    /// @return Y2XRangeRet
    pub fn range_y_swap_x(&mut self, right_point: i32, amount_y: u128) -> Y2XRangeRet {
        let mut result = Y2XRangeRet::default();
        let mut amount_y = amount_y;
        // first, if current point is not all x, we can not move right directly
        let start_has_y = self.liquidity_x < self.liquidity;
        if start_has_y {
            (result.cost_y, result.acquire_x, result.liquidity_x) =
            y_swap_x_at_price_liquidity(
                    amount_y,
                    self.sqrt_price_96,
                    self.liquidity_x,
                );
            if result.liquidity_x > 0 || result.cost_y >= amount_y {
                // it means remaining y is not enough to rise current price to price*1.0001
                // but y may remain, so we cannot simply use (cost_y == amount_y)
                result.finished = true;
                result.final_pt = self.current_point;
                result.sqrt_final_price_96 = self.sqrt_price_96;
                return result;
            } else {
                // y not run out
                // not finsihed
                amount_y -= result.cost_y;
                self.current_point += 1;
                if self.current_point == right_point {
                    result.final_pt = self.current_point;
                    // get fixed sqrt price to reduce accumulated error
                    result.sqrt_final_price_96 = get_sqrt_price(right_point);
                    return result;
                }
                // sqrt(price) + sqrt(price) * (1.0001 - 1) == sqrt(price) * 1.0001
                self.sqrt_price_96 = self.sqrt_price_96
                    + self.sqrt_price_96
                        .mul_fraction_floor(sqrt_rate_96() - pow_96(), pow_96());
            }
        }

        let sqrt_price_r_96 = get_sqrt_price(right_point);

        let y2x_range_comp_result = y_swap_x_range_complete(
            self.liquidity,
            self.sqrt_price_96,
            self.current_point,
            sqrt_price_r_96,
            right_point,
            amount_y
        );

        result.cost_y += y2x_range_comp_result.cost_y;
        amount_y -= y2x_range_comp_result.cost_y;
        result.acquire_x += y2x_range_comp_result.acquire_x;
        if y2x_range_comp_result.complete_liquidity {
            result.finished = amount_y == 0;
            result.final_pt = right_point;
            result.sqrt_final_price_96 = sqrt_price_r_96;
        } else {
            // trade at loc_pt
            let (loc_cost_y, loc_acquire_x, loc_liquidity_x) =
            y_swap_x_at_price_liquidity(amount_y, y2x_range_comp_result.sqrt_loc_96, self.liquidity);

            result.liquidity_x = loc_liquidity_x;
            result.cost_y += loc_cost_y;
            result.acquire_x += loc_acquire_x;
            result.finished = true;
            result.sqrt_final_price_96 = y2x_range_comp_result.sqrt_loc_96;
            result.final_pt = y2x_range_comp_result.loc_pt;
        }
        result
    }

    /// @param left_point: the left boundary of range
    /// @param desire_y: the amount of token Y to swap-out
    /// @return X2YRangeRetDesire
    pub fn range_x_swap_y_desire(&mut self, left_point: i32, desire_y: u128) -> X2YRangeRetDesire{
        let mut result = X2YRangeRetDesire::default();
        let current_has_y = self.liquidity_x < self.liquidity;
        let mut desire_y = desire_y;

        if current_has_y && (self.liquidity_x > 0 || left_point == self.current_point) {
            (result.cost_x, result.acquire_y, result.liquidity_x) = x_swap_y_at_price_liquidity_desire(
                desire_y, self.sqrt_price_96, self.liquidity, self.liquidity_x
            );

            if result.liquidity_x < self.liquidity || result.acquire_y >= desire_y {
                result.finished = true;
                result.final_pt = self.current_point;
                result.sqrt_final_price_96 = self.sqrt_price_96;
            } else {
                desire_y -= result.acquire_y;
            }
        } else if current_has_y { // all y
            self.current_point += 1;
            self.sqrt_price_96 = self.sqrt_price_96 + self.sqrt_price_96.mul_fraction_floor(sqrt_rate_96() - pow_96(), pow_96());
        } else {
            result.liquidity_x = self.liquidity_x;
        }
        if result.finished {
            return result;
        }
        if left_point < self.current_point {
            let sqrt_price_l_96 = get_sqrt_price(left_point);
            let x2y_range_comp_desire_result = x_swap_y_range_complete_desire(
                self.liquidity,
                sqrt_price_l_96,
                left_point,
                self.sqrt_price_96,
                self.current_point,
                desire_y
            );            
            result.cost_x += x2y_range_comp_desire_result.cost_x;
            desire_y -= x2y_range_comp_desire_result.acquire_y;
            result.acquire_y += x2y_range_comp_desire_result.acquire_y;
            if x2y_range_comp_desire_result.complete_liquidity {
                result.finished = desire_y == 0;
                result.final_pt = left_point;
                result.sqrt_final_price_96 = sqrt_price_l_96;
                result.liquidity_x = self.liquidity;
            } else {
                let (loc_cost_x, loc_acquire_y, new_liquidity_x) = x_swap_y_at_price_liquidity_desire(
                    desire_y, x2y_range_comp_desire_result.sqrt_loc_96, self.liquidity, 0
                );
                result.liquidity_x = new_liquidity_x;
                result.cost_x += loc_cost_x;
                result.acquire_y += loc_acquire_y;
                result.finished = true;
                result.sqrt_final_price_96 = x2y_range_comp_desire_result.sqrt_loc_96;
                result.final_pt = x2y_range_comp_desire_result.loc_pt;
            }
        } else {
            result.final_pt = self.current_point;
            result.sqrt_final_price_96 = self.sqrt_price_96;
        }
        result
    }

    /// @param right_point: the right boundary of range
    /// @param desire_x: the amount of token X to swap-out
    /// @return X2YRangeRetDesire
    pub fn range_y_swap_x_desire(&mut self, right_point: i32, desire_x: u128) -> Y2XRangeRetDesire{
        let mut result = Y2XRangeRetDesire::default();
        let mut desire_x = desire_x;
        let start_has_y = self.liquidity_x < self.liquidity;
        if start_has_y {
            (result.cost_y, result.acquire_x, result.liquidity_x) = y_swap_x_at_price_liquidity_desire(desire_x, self.sqrt_price_96, self.liquidity_x);
            if result.liquidity_x > 0 || result.acquire_x >= desire_x {
                // currX remain, means desire runout
                result.finished = true;
                result.final_pt = self.current_point;
                result.sqrt_final_price_96 = self.sqrt_price_96;
                return result;
            } else {
                // not finished
                desire_x -= result.acquire_x;
                self.current_point += 1;
                if self.current_point == right_point {
                    result.final_pt = self.current_point;
                    // get fixed sqrt price to reduce accumulated error
                    result.sqrt_final_price_96 = get_sqrt_price(right_point);
                    return result;
                }
                // sqrt(price) + sqrt(price) * (1.0001 - 1) == sqrt(price) * 1.0001
                self.sqrt_price_96 = self.sqrt_price_96 + self.sqrt_price_96.mul_fraction_floor(sqrt_rate_96() - pow_96(), pow_96());
            }
        }
        let sqrt_price_r_96 = get_sqrt_price(right_point);
        let y2x_range_comp_desire_result = y_swap_x_range_complete_desire(
            self.liquidity,
            self.sqrt_price_96,
            self.current_point,
            sqrt_price_r_96,
            right_point,
            desire_x
        );

        result.cost_y += y2x_range_comp_desire_result.cost_y;
        result.acquire_x += y2x_range_comp_desire_result.acquire_x;
        desire_x -= y2x_range_comp_desire_result.acquire_x;

        if y2x_range_comp_desire_result.complete_liquidity {
            result.finished = desire_x == 0;
            result.final_pt = right_point;
            result.sqrt_final_price_96 = sqrt_price_r_96;
        } else {
            let (loc_cost_y, loc_acquire_x, new_liquidity_x) = y_swap_x_at_price_liquidity_desire(desire_x, y2x_range_comp_desire_result.sqrt_loc_96, self.liquidity);
            result.liquidity_x = new_liquidity_x;
            result.cost_y += loc_cost_y;
            result.acquire_x += loc_acquire_x;
            result.finished = true;
            result.final_pt = y2x_range_comp_desire_result.loc_pt;
            result.sqrt_final_price_96 = y2x_range_comp_desire_result.sqrt_loc_96;
        }
        result
    }
}

/// @param amount_x: the amount of swap-in token X
/// @param sqrt_price_96: price of this point
/// @param liquidity: liquidity amount on this point
/// @param liquidity_x: liquidity part from X*sqrt(p)
/// @return tuple (consumed_x, swap_out_y, new_liquidity_x)
pub fn x_swap_y_at_price_liquidity(
    amount_x: u128,
    sqrt_price_96: U256,
    liquidity: u128,
    liquidity_x: u128
) -> (u128, U256, u128) {
    let liquidity_y = U256::from(liquidity - liquidity_x);
    let max_transform_liquidity_x = U256::from(amount_x).mul_fraction_floor(sqrt_price_96, pow_96());
    let transform_liquidity_x = std::cmp::min(max_transform_liquidity_x, liquidity_y);
    
    // rounding up to ensure pool won't be short of X.
    let cost_x = transform_liquidity_x.mul_fraction_ceil(pow_96(), sqrt_price_96).as_u128();
    // TODO: convert to u128
    // rounding down to ensure pool won't be short of Y.
    let acquire_y = transform_liquidity_x.mul_fraction_floor(sqrt_price_96, pow_96());
    let new_liquidity_x = liquidity_x + transform_liquidity_x.as_u128();
    (cost_x, acquire_y, new_liquidity_x)
}

/// @param amount_y: the amount of swap-in token Y
/// @param sqrt_price_96: price of this point
/// @param liquidity_x: liquidity part from X*sqrt(p)
/// @return tuple (consumed_y, swap_out_x, new_liquidity_x)
pub fn y_swap_x_at_price_liquidity(
    amount_y: u128,
    sqrt_price_96: U256,
    liquidity_x: u128,
) -> (u128, U256, u128) {
    let max_transform_liquidity_y = U256::from(amount_y).mul_fraction_floor(pow_96(), sqrt_price_96);
    let transform_liquidity_y = std::cmp::min(max_transform_liquidity_y, U256::from(liquidity_x));
    let cost_y = transform_liquidity_y.mul_fraction_ceil(sqrt_price_96, pow_96()).as_u128();
    let acquire_x = transform_liquidity_y.mul_fraction_floor(pow_96(), sqrt_price_96);
    let new_liquidity_x = U256::from(liquidity_x) - transform_liquidity_y;
    (cost_y, acquire_x, new_liquidity_x.as_u128())
}

/// try to swap from right to left in range [left_point, right_point) with all liquidity used.
/// @param liquidity: liquidity of each point in the range
/// @param sqrt_price_l_96: sqrt of left point price in 2^96 power
/// @param left_point: left point of this range
/// @param sqrt_price_r_96: sqrt of right point price in 2^96 power
/// @param right_point: right point of this range
/// @param amount_x: amount of token X as swap-in
/// @return X2YRangeCompRet
///     .complete_liquidity, true if given range has been fully swapped;
///     .cost_x, used amount of token X;
///     .acquire_y, acquired amount of token Y;
///     .loc_pt, if partial swapped, the right most unswapped point;
///     .sqrt_loc_96, the sqrt_price of loc_pt;
fn x_swap_y_range_complete(
    liquidity: u128,
    sqrt_price_l_96: U256,
    left_point: i32,
    sqrt_price_r_96: U256,
    right_point: i32,
    amount_x: u128,
) -> X2YRangeCompRet {
    let mut result = X2YRangeCompRet::default();

    let max_x = get_amount_x(liquidity, left_point, right_point, sqrt_price_r_96, sqrt_rate_96(), true).as_u128();
        
    if max_x <= amount_x {
        // liquidity in this range has been FULLY swapped out
        result.complete_liquidity = true;
        result.cost_x = max_x;
        result.acquire_y = get_amount_y(liquidity, sqrt_price_l_96, sqrt_price_r_96, sqrt_rate_96(), false);
    } else {
        // liquidity in this range can only be PARTIAL swapped out
        result.complete_liquidity = false;
        result.loc_pt = get_most_left_point(liquidity, amount_x, right_point, sqrt_price_r_96);
        // the distance between left and point must be non-negative
        require!(result.loc_pt <= right_point, E208_INTERNAL_ERR1);
        // it would be fully swap if violated
        require!(result.loc_pt > left_point, E209_INTERNAL_ERR2);
        
        if result.loc_pt == right_point {
            // could not exhaust one point liquidity
            result.cost_x = 0;
            result.acquire_y = 0u128.into();
        } else {
            // exhaust some point liquidity but not all point
            let cost_x_256 = get_amount_x(liquidity, result.loc_pt, right_point, sqrt_price_r_96, sqrt_rate_96(), true);            
            result.cost_x = std::cmp::min(cost_x_256, U256::from(amount_x)).as_u128();
            result.acquire_y = get_amount_y(liquidity, get_sqrt_price(result.loc_pt), sqrt_price_r_96, sqrt_rate_96(), false);
        }
        // put current point to the right_point - 1 to wait for single point process
        result.loc_pt -= 1;
        result.sqrt_loc_96 = get_sqrt_price(result.loc_pt);
    }
    result
}

/// try to swap from left to right in range [left_point, right_point) with all liquidity used.
/// @param liquidity: liquidity of each point in the range
/// @param sqrt_price_l_96: sqrt of left point price in 2^96 power
/// @param left_point: left point of this range
/// @param sqrt_price_r_96: sqrt of right point price in 2^96 power
/// @param right_point: right point of this range
/// @param amount_y: amount of token Y as swap-in
/// @return Y2XRangeCompRet
///     .complete_liquidity, true if given range has been fully swapped;
///     .cost_y, used amount of token Y;
///     .acquire_x, acquired amount of token X;
///     .loc_pt, if partial swapped, the right most unswapped point;
///     .sqrt_loc_96, the sqrt_price of loc_pt;
pub fn y_swap_x_range_complete(
    liquidity: u128,
    sqrt_price_l_96: U256,
    left_point: i32,
    sqrt_price_r_96: U256,
    right_point: i32, 
    amount_y: u128) -> Y2XRangeCompRet {
    let mut result = Y2XRangeCompRet::default();
    let max_y = get_amount_y(
        liquidity,
        sqrt_price_l_96,
        sqrt_price_r_96,
        sqrt_rate_96(),
        true,
    );
    if max_y <= U256::from(amount_y) {
        result.cost_y = max_y.as_u128();
        result.acquire_x = get_amount_x(
            liquidity,
            left_point,
            right_point,
            sqrt_price_r_96,
            sqrt_rate_96(),
            false,
        );
        result.complete_liquidity = true;
    } else {
        result.loc_pt = get_most_right_point(liquidity, amount_y, sqrt_price_l_96);

        // the distance between right and point must be non-negative
        require!(result.loc_pt >= left_point, E210_INTERNAL_ERR3);
        // it would be fully swap if violated
        require!(result.loc_pt < right_point, E211_INTERNAL_ERR4);

        result.complete_liquidity = false;
        result.sqrt_loc_96 = get_sqrt_price(result.loc_pt);
        if result.loc_pt == left_point {
            result.cost_y = 0;
            result.acquire_x = Default::default();
            return result;
        }

        let cost_y_256 = get_amount_y(
            liquidity,
            sqrt_price_l_96,
            result.sqrt_loc_96,
            sqrt_rate_96(),
            true,
        );

        result.cost_y = std::cmp::min(cost_y_256, U256::from(amount_y)).as_u128();

        result.acquire_x = get_amount_x(
            liquidity,
            left_point,
            result.loc_pt,
            result.sqrt_loc_96,
            sqrt_rate_96(),
            false,
        );
    }
    result
}

/// @param amount_x: the amount of swap-in token X
/// @param sqrt_price_96: price of this point
/// @param curr_y: the amount of token Y that can participate in the calc
/// @return tuple (cost_x, acquire_y)
pub fn x_swap_y_at_price(
    amount_x: u128,
    sqrt_price_96: U256,
    curr_y: u128
) -> (u128, u128) {
    let mut l = U256::from(amount_x).mul_fraction_floor(sqrt_price_96, pow_96());

    let mut acquire_y = l.mul_fraction_floor(sqrt_price_96, pow_96()).as_u128();
    if acquire_y > curr_y {
        acquire_y = curr_y;
    }
    l = U256::from(acquire_y).mul_fraction_ceil(pow_96(), sqrt_price_96);
    let cost_x = l.mul_fraction_ceil(pow_96(), sqrt_price_96).as_u128();
    (cost_x, acquire_y)
}

/// @param amount_y: the amount of swap-in token Y
/// @param sqrt_price_96: price of this point
/// @param curr_x: the amount of token X that can participate in the calc
/// @return tuple (cost_y, acquire_x)
pub fn y_swap_x_at_price(
    amount_y: u128, 
    sqrt_price_96: U256, 
    curr_x: u128
) -> (u128, u128) {
    let mut l = U256::from(amount_y).mul_fraction_floor(pow_96(), sqrt_price_96);
    let acquire_x = std::cmp::min(
        l.mul_fraction_floor(pow_96(), sqrt_price_96),
        U256::from(curr_x),
    );
    l = acquire_x.mul_fraction_ceil(sqrt_price_96, pow_96());
    let cost_y = l.mul_fraction_ceil(sqrt_price_96, pow_96());
    (cost_y.as_u128(), acquire_x.as_u128())
}

/// @param desire_y: the amount of swap-out token Y
/// @param sqrt_price_96: price of this point
/// @param liquidity: liquidity of each point in the range
/// @param liquidity_x: liquidity part from X*sqrt(p)
/// @return tuple (cost_x, acquire_y, new_liquidity_x)
fn x_swap_y_at_price_liquidity_desire(
    desire_y: u128,
    sqrt_price_96: U256,
    liquidity: u128,
    liquidity_x: u128
) -> (U256, u128, u128) {
    let liquidity_y = U256::from(liquidity - liquidity_x);
    let max_transform_liquidity_x = U256::from(desire_y).mul_fraction_ceil(pow_96(), sqrt_price_96);
    let transform_liquidity_x = std::cmp::min(max_transform_liquidity_x, liquidity_y);
    let cost_x = transform_liquidity_x.mul_fraction_ceil(pow_96(), sqrt_price_96);
    let acquire_y = transform_liquidity_x.mul_fraction_floor(sqrt_price_96, pow_96()).as_u128();
    let new_liquidity_x = liquidity_x + transform_liquidity_x.as_u128();
    (cost_x, acquire_y, new_liquidity_x)
}

/// @param desire_x: the amount of swap-out token X
/// @param sqrt_price_96: price of this point
/// @param liquidity_x: liquidity part from X*sqrt(p)
/// @return tuple (cost_y, acquire_x, new_liquidity_x)
fn y_swap_x_at_price_liquidity_desire(
    desire_x: u128,
    sqrt_price_96: U256,
    liquidity_x: u128
) -> (U256, u128, u128) {
    let max_transform_liquidity_y = U256::from(desire_x).mul_fraction_ceil(sqrt_price_96, pow_96());
    // transformLiquidityY <= liquidityX <= uint128.max
    let transform_liquidity_y = std::cmp::min(max_transform_liquidity_y, U256::from(liquidity_x));
    let cost_y = transform_liquidity_y.mul_fraction_ceil(sqrt_price_96, pow_96());
    let acquire_x = transform_liquidity_y.mul_fraction_floor(pow_96(), sqrt_price_96).as_u128();
    let new_liquidity_x = liquidity_x - transform_liquidity_y.as_u128();
    (cost_y, acquire_x, new_liquidity_x)
}

/// try to swap from left to right in range [left_point, right_point) with all liquidity used.
/// @param liquidity: liquidity of each point in the range
/// @param sqrt_price_l_96: sqrt of left point price in 2^96 power
/// @param left_point: left point of this range
/// @param sqrt_price_r_96: sqrt of right point price in 2^96 power
/// @param right_point: right point of this range
/// @param desire_y: amount of token Y as swap-out
/// @return X2YRangeCompRetDesire
pub fn x_swap_y_range_complete_desire(
    liquidity: u128,
    sqrt_price_l_96: U256,
    left_point: i32,
    sqrt_price_r_96: U256,
    right_point: i32, 
    desire_y: u128
) -> X2YRangeCompRetDesire {
    let mut result = X2YRangeCompRetDesire::default();
    let max_y = get_amount_y(liquidity, sqrt_price_l_96, sqrt_price_r_96, sqrt_rate_96(), false).as_u128();
    if max_y <= desire_y {
        result.acquire_y = max_y;
        result.cost_x = get_amount_x(liquidity, left_point, right_point, sqrt_price_r_96, sqrt_rate_96(), true);
        result.complete_liquidity = true;
        return result;
    }
    
    let cl = sqrt_price_r_96 - U256::from(desire_y).mul_fraction_floor(sqrt_rate_96() - pow_96(), U256::from(liquidity));
    
    result.loc_pt = get_log_sqrt_price_floor(cl) + 1;
    
    result.loc_pt = std::cmp::min(result.loc_pt, right_point);
    result.loc_pt = std::cmp::max(result.loc_pt, left_point + 1);
    result.complete_liquidity = false;

    if result.loc_pt == right_point {
        result.cost_x = Default::default();
        result.acquire_y = 0;
        result.loc_pt -= 1;
        result.sqrt_loc_96 = get_sqrt_price(result.loc_pt);
    } else {
        let sqrt_price_pr_mloc_96 = get_sqrt_price(right_point - result.loc_pt);
        let sqrt_price_pr_m1_96 = sqrt_price_r_96.mul_fraction_ceil(pow_96(), sqrt_rate_96());
        
        result.cost_x = U256::from(liquidity).mul_fraction_ceil(sqrt_price_pr_mloc_96 - pow_96(), sqrt_price_r_96 - sqrt_price_pr_m1_96);

        result.loc_pt -= 1;
        result.sqrt_loc_96 = get_sqrt_price(result.loc_pt);

        let sqrt_loc_a1_96 = result.sqrt_loc_96 + result.sqrt_loc_96.mul_fraction_floor(sqrt_rate_96() - pow_96(), pow_96());
        
        let acquire_y = get_amount_y(liquidity, sqrt_loc_a1_96, sqrt_price_r_96, sqrt_rate_96(), false).as_u128();
        result.acquire_y = std::cmp::min(acquire_y, desire_y);
    }
    result
}

/// try to swap from right to left in range [left_point, right_point) with all liquidity used.
/// @param liquidity: liquidity of each point in the range
/// @param sqrt_price_l_96: sqrt of left point price in 2^96 power
/// @param left_point: left point of this range
/// @param sqrt_price_r_96: sqrt of right point price in 2^96 power
/// @param right_point: right point of this range
/// @param desire_x: amount of token X as swap-out
/// @return Y2XRangeCompRetDesire
pub fn y_swap_x_range_complete_desire(
    liquidity: u128,
    sqrt_price_l_96: U256,
    left_point: i32,
    sqrt_price_r_96: U256,
    right_point: i32, 
    desire_x: u128
) -> Y2XRangeCompRetDesire {
    let mut result = Y2XRangeCompRetDesire::default();
    let max_x = get_amount_x(liquidity, left_point, right_point, sqrt_price_r_96, sqrt_rate_96(), false).as_u128();
    if max_x <= desire_x {
        // maxX <= desireX <= uint128.max
        result.acquire_x = max_x;
        result.cost_y = get_amount_y(liquidity, sqrt_price_l_96, sqrt_price_r_96, sqrt_rate_96(), true);
        result.complete_liquidity = true;
        return result;
    }

    let sqrt_price_pr_pl_96 = get_sqrt_price(right_point - left_point);
    let sqrt_price_pr_m1_96 = sqrt_price_r_96.mul_fraction_floor(pow_96(), sqrt_rate_96());
    let div = sqrt_price_pr_pl_96 - U256::from(desire_x).mul_fraction_floor(sqrt_price_r_96 - sqrt_price_pr_m1_96, U256::from(liquidity));
   
    let sqrt_price_loc_96 = sqrt_price_r_96.mul_fraction_floor(pow_96(), div);

    result.complete_liquidity = false;
    result.loc_pt = get_log_sqrt_price_floor(sqrt_price_loc_96);

    result.loc_pt = std::cmp::max(left_point, result.loc_pt);
    result.loc_pt = std::cmp::min(right_point - 1, result.loc_pt);
    result.sqrt_loc_96 = get_sqrt_price(result.loc_pt);

    if result.loc_pt == left_point {
        result.acquire_x = 0;
        result.cost_y = Default::default();
        return result;
    }
    result.acquire_x = std::cmp::min(
        get_amount_x(liquidity, left_point, result.loc_pt, result.sqrt_loc_96, sqrt_rate_96(), false).as_u128(), 
        desire_x);

    result.cost_y = get_amount_y(liquidity, sqrt_price_l_96, result.sqrt_loc_96, sqrt_rate_96(), true);
    result
}

/// @param desire_y: the amount of swap-out token Y
/// @param sqrt_price_96: price of this point
/// @param curr_y: the amount of token Y that can participate in the calc
/// @return tuple (cost_x, acquire_y)
pub fn x_swap_y_at_price_desire(
    desire_y: u128,
    sqrt_price_96: U256,
    curr_y: u128
) -> (u128, u128) {
    let mut acquire_y = desire_y;
    if acquire_y > curr_y {
        acquire_y = curr_y;
    }
    let l = U256::from(acquire_y).mul_fraction_ceil(pow_96(), sqrt_price_96);
    let cost_x = l.mul_fraction_ceil(pow_96(), sqrt_price_96).as_u128();
    (cost_x, acquire_y)
}

/// @param desire_x: the amount of swap-out token X
/// @param sqrt_price_96: price of this point
/// @param curr_x: the amount of token X that can participate in the calc
/// @return tuple (cost_y, acquire_x)
pub fn y_swap_x_at_price_desire(
    desire_x: u128,
    sqrt_price_96: U256,
    curr_x: u128
) -> (u128, u128) {
    let acquire_x = std::cmp::min(desire_x, curr_x);
    let l = U256::from(acquire_x).mul_fraction_ceil(sqrt_price_96, pow_96());
    let cost_y = l.mul_fraction_ceil(sqrt_price_96, pow_96()).as_u128();
    (cost_y, acquire_x)
}
