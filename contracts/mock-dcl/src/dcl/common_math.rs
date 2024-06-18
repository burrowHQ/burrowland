#![allow(dead_code)]

use crate::*;
use std::ops::{BitAnd, ShrAssign, MulAssign, BitOrAssign};
use num_bigint::{BigUint, BigInt, ToBigInt};
use std::str::FromStr;

#[inline]
pub fn pow_128() -> U256{
    U256::from(1u128) << 128
}

#[inline]
pub fn pow_96() -> U256{
    U256::from(1u128) << 96
}

#[inline]
pub fn sqrt_rate_96() -> U256 {
    get_sqrt_price(1)
}

impl U256 {
    /// the floor division
    /// @param _numerator
    /// @param _denominator
    /// @return floor(self * _numerator / _denominator)
    pub fn mul_fraction_floor(&self, _numerator: U256, _denominator: U256) -> U256 {
        let sf = U512::from_dec_str(&self.to_string()).unwrap();
        let numerator = U512::from_dec_str(&_numerator.to_string()).unwrap();
        let denominator = U512::from_dec_str(&_denominator.to_string()).unwrap();
        let res = sf * numerator / denominator;
        U256::from_dec_str(&res.to_string()).unwrap()
    }

    /// the ceil division
    /// @param _numerator
    /// @param _denominator
    /// @return ceil(self * _numerator / _denominator)
    pub fn mul_fraction_ceil(&self, _numerator: U256, _denominator: U256) -> U256 {
        let sf = U512::from_dec_str(&self.to_string()).unwrap();
        let numerator = U512::from_dec_str(&_numerator.to_string()).unwrap();
        let denominator = U512::from_dec_str(&_denominator.to_string()).unwrap();
        let res = if (sf * numerator % denominator).is_zero() {
            sf * numerator / denominator
        } else {
            sf * numerator / denominator + 1
        };
        U256::from_dec_str(&res.to_string()).unwrap()
    }
}

// sqrt of 1.0001^(-800000) in 2^96 power
const MIN_PRICE:&str = "337263108622";
// sqrt of 1.0001^(800000) in 2^96 power
const MAX_PRICE:&str = "18611883644907511909590774894315720731532604461";

/// from https://github.com/izumiFinance/izumi-swap-core/blob/main/contracts/libraries/LogPowMath.sol#L16-L44
/// compute the price at a given point
/// @param point: the point
/// @return the price of the point
pub fn get_sqrt_price(point: i32) -> U256 {
    let abs_idx = if point < 0 {U256::from((-point) as u128)} else {U256::from(point as u128)};
    require!(abs_idx <= U256::from(RIGHT_MOST_POINT), E202_ILLEGAL_POINT);

    let mut value = if !abs_idx.bitand(1u128.into()).is_zero() {
        U256::from_str_radix("0xfffcb933bd6fad37aa2d162d1a594001", 16).unwrap()
    } else {
        U256::from_str_radix("0x100000000000000000000000000000000", 16).unwrap()
    };

    let update_value = |value: &mut U256, hex1: &str, hex2: &str|{
        if !abs_idx.bitand(U256::from_str_radix(hex1, 16).unwrap()).is_zero() {
            value.mul_assign(U256::from_str_radix(hex2, 16).unwrap());
            value.shr_assign(128u8);
        }
    };

    update_value(&mut value, "0x2", "0xfff97272373d413259a46990580e213a");
    update_value(&mut value, "0x4", "0xfff2e50f5f656932ef12357cf3c7fdcc");
    update_value(&mut value, "0x8", "0xffe5caca7e10e4e61c3624eaa0941cd0");
    update_value(&mut value, "0x10", "0xffcb9843d60f6159c9db58835c926644");
    update_value(&mut value, "0x20", "0xff973b41fa98c081472e6896dfb254c0");
    update_value(&mut value, "0x40", "0xff2ea16466c96a3843ec78b326b52861");
    update_value(&mut value, "0x80", "0xfe5dee046a99a2a811c461f1969c3053");
    update_value(&mut value, "0x100", "0xfcbe86c7900a88aedcffc83b479aa3a4");
    update_value(&mut value, "0x200", "0xf987a7253ac413176f2b074cf7815e54");
    update_value(&mut value, "0x400", "0xf3392b0822b70005940c7a398e4b70f3");
    update_value(&mut value, "0x800", "0xe7159475a2c29b7443b29c7fa6e889d9");
    update_value(&mut value, "0x1000", "0xd097f3bdfd2022b8845ad8f792aa5825");
    update_value(&mut value, "0x2000", "0xa9f746462d870fdf8a65dc1f90e061e5");
    update_value(&mut value, "0x4000", "0x70d869a156d2a1b890bb3df62baf32f7");
    update_value(&mut value, "0x8000", "0x31be135f97d08fd981231505542fcfa6");
    update_value(&mut value, "0x10000", "0x9aa508b5b7a84e1c677de54f3e99bc9");
    update_value(&mut value, "0x20000", "0x5d6af8dedb81196699c329225ee604");
    update_value(&mut value, "0x40000", "0x2216e584f5fa1ea926041bedfe98");
    update_value(&mut value, "0x80000", "0x48a170391f7dc42444e8fa2");

    if point > 0 {
        value = U256::MAX / value;
    }

    (value >> 32u8) + if (value % (U256::from(1u128 << 32u8))).is_zero() {0} else {1}
}

/// from https://github.com/izumiFinance/izumi-swap-core/blob/main/contracts/libraries/LogPowMath.sol#L47-L190
/// compute the point at a given price
/// @param sqrt_price_96: the price.
/// @return the point of the price
pub fn get_log_sqrt_price_floor(sqrt_price_96: U256) -> i32{
    require!(sqrt_price_96 >= U256::from_dec_str(MIN_PRICE).unwrap() && 
        sqrt_price_96 < U256::from_dec_str(MAX_PRICE).unwrap(), E201_INVALID_SQRT_PRICE);
    let sqrt_price_128  = BigUint::from_str(&sqrt_price_96.to_string()).unwrap() << 32u8;

    let mut x = BigUint::from_str(&sqrt_price_128.to_string()).unwrap();
    let mut m = 0u8;

    let update_x_m = |hex: &[u8], offset: u8, x: &mut BigUint, m: &mut u8|{
        let y = if x > &mut BigUint::parse_bytes(hex, 16).unwrap() {1u8 << offset} else {0u8};
        m.bitor_assign(y);
        x.shr_assign(y);
    };

    update_x_m(b"FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF", 7, &mut x, &mut m);
    update_x_m(b"FFFFFFFFFFFFFFFF", 6, &mut x, &mut m);
    update_x_m(b"FFFFFFFF", 5, &mut x, &mut m);
    update_x_m(b"FFFF", 4, &mut x, &mut m);
    update_x_m(b"FF", 3, &mut x, &mut m);
    update_x_m(b"F", 2, &mut x, &mut m);
    update_x_m(b"3", 1, &mut x, &mut m);

    let y = if x > BigUint::parse_bytes(b"1", 16).unwrap() {1u8} else {0u8};
    m |= y;

    if m >= 128u8 {
        x = sqrt_price_128 >> (m - 127u8);
    } else {
        x = sqrt_price_128 << (127u8 - m);
    }

    let mut l2 = (BigInt::from(m) - BigInt::from(128u8)) << 64u8;

    let update_x_l2 = |offset: u8, x: &mut BigUint, l2: &mut BigInt|{
        x.mul_assign(x.clone());
        x.shr_assign(127u8);
        let y = x.clone() >> 128u8;
        l2.bitor_assign((y.clone() << offset).to_bigint().unwrap());
        x.shr_assign(u128::from_str(&y.to_string()).unwrap());
    };

    update_x_l2(63, &mut x, &mut l2);
    update_x_l2(62, &mut x, &mut l2);
    update_x_l2(61, &mut x, &mut l2);
    update_x_l2(60, &mut x, &mut l2);
    update_x_l2(59, &mut x, &mut l2);
    update_x_l2(58, &mut x, &mut l2);
    update_x_l2(57, &mut x, &mut l2);
    update_x_l2(56, &mut x, &mut l2);
    update_x_l2(55, &mut x, &mut l2);
    update_x_l2(54, &mut x, &mut l2);
    update_x_l2(53, &mut x, &mut l2);
    update_x_l2(52, &mut x, &mut l2);
    update_x_l2(51, &mut x, &mut l2);

    x.mul_assign(x.clone());
    x.shr_assign(127u8);
    let y = x.clone() >> 128u8;
    l2.bitor_assign((y << 50u8).to_bigint().unwrap());

    let ls10001 = l2 * BigInt::from(255738958999603826347141u128);
    let log_floor = i32::from_str(&((ls10001.clone() - BigInt::from_str("3402992956809132418596140100660247210").unwrap()) >> 128u8).to_string()).unwrap();
    let log_upper = i32::from_str(&((ls10001 +  BigInt::from_str("291339464771989622907027621153398088495").unwrap()) >> 128u8).to_string()).unwrap();

    if log_floor == log_upper {
        log_floor
    } else if get_sqrt_price(log_upper) <= sqrt_price_96 {
        log_upper
    } else {
        log_floor
    }
}

/// Get amount of token Y that is needed to add a unit of liquidity in the range [left_pt, right_pt)
/// @param sqrt_price_l_96: sqrt of left point price in 2^96 power
/// @param sqrt_price_r_96: sqrt of right point price in 2^96 power
/// @param sqrt_rate_96: sqrt of 1.0001 in 2^96 power
/// @return amount of token Y
pub fn get_amount_y_unit_liquidity_96(
    sqrt_price_l_96: U256,
    sqrt_price_r_96: U256,
    sqrt_rate_96: U256
) -> U256 {
    let numerator = sqrt_price_r_96 - sqrt_price_l_96;
    let denominator = sqrt_rate_96 - pow_96();
    pow_96().mul_fraction_ceil(numerator, denominator)
}

/// Get amount of token X that is needed to add a unit of liquidity in the range [left_pt, right_pt)
/// @param left_pt: left point of this range
/// @param right_pt: right point of this range
/// @param sqrt_price_r_96: sqrt of right point price in 2^96 power
/// @param sqrt_rate_96: sqrt of 1.0001 in 2^96 power
/// @return amount of token X
pub fn get_amount_x_unit_liquidity_96(
    left_pt: i32,
    right_pt: i32,
    sqrt_price_r_96: U256,
    sqrt_rate_96: U256,
) -> U256 {
    let sqrt_price_pr_pc_96 = get_sqrt_price(right_pt - left_pt + 1);
    let sqrt_price_pr_pd_96 = get_sqrt_price(right_pt + 1);

    let numerator = sqrt_price_pr_pc_96 - sqrt_rate_96;
    let denominator = sqrt_price_pr_pd_96 - sqrt_price_r_96;
    pow_96().mul_fraction_ceil(numerator, denominator)
}

/// Get amount of token Y that can form liquidity in range [l, r)
/// @param liquidity: L = Y/sqrt(p)
/// @param sqrt_price_l_96: sqrt of left point price in 2^96 power
/// @param sqrt_price_r_96: sqrt of right point price in 2^96 power
/// @param sqrt_rate_96: sqrt of 1.0001 in 2^96 power
/// @param upper: flag to indicate rounding up or down
/// @return amount of token Y that can from given liquidity
pub fn get_amount_y(
    liquidity: u128,
    sqrt_price_l_96: U256,
    sqrt_price_r_96: U256,
    sqrt_rate_96: U256,
    upper: bool,
) -> U256 {
    // d = 1.0001, ∵ L = Y / sqrt(P)   ∴ Y(i) = L * sqrt(d ^ i)
    // sqrt(d) ^ r - sqrt(d) ^ l
    // ------------------------- = amount_y_of_unit_liquidity: the amount of token Y equivalent to a unit of liquidity in the range
    // sqrt(d) - 1
    //
    // sqrt(d) ^ l * sqrt(d) ^ (r - l) - sqrt(d) ^ l
    // ----------------------------------------------
    // sqrt(d) - 1
    // 
    // sqrt(d) ^ l * (sqrt(d) ^ (r - l) - 1)
    // ----------------------------------------------
    // sqrt(d) - 1
    //
    // sqrt(d) ^ l * (sqrt(d) - 1) * (sqrt(d) ^ (r - l - 1) + sqrt(d) ^ (r - l - 2) + ...... + sqrt(d) + 1)
    // ----------------------------------------------------------------------------------------------------
    // sqrt(d) - 1
    // 
    // sqrt(d) ^ l + sqrt(d) ^ (l + 1) + ...... + sqrt(d) ^ (r - 1) 
    // 
    // Y(l) + Y(l + 1) + ...... + Y(r - 1) 

    // amount_y = amount_y_of_unit_liquidity * liquidity

    // using sum equation of geomitric series to compute range numbers
    let numerator = sqrt_price_r_96 - sqrt_price_l_96;
    let denominator = sqrt_rate_96 - pow_96();
    if !upper {
        U256::from(liquidity).mul_fraction_floor(numerator, denominator)
    } else {
        U256::from(liquidity).mul_fraction_ceil(numerator, denominator)
    }
}

/// Get amount of token X that can form liquidity in range [l, r)
/// @param liquidity: L = X*sqrt(p)
/// @param left_pt: left point of this range
/// @param right_pt: right point of this range
/// @param sqrt_price_r_96: sqrt of right point price in 2^96 power
/// @param sqrt_rate_96: sqrt of 1.0001 in 2^96 power
/// @param upper: flag to indicate rounding up or down
/// @return amount of token X that can from given liquidity
pub fn get_amount_x(
    liquidity: u128,
    left_pt: i32,
    right_pt: i32,
    sqrt_price_r_96: U256,
    sqrt_rate_96: U256,
    upper: bool
) -> U256 {
    // d = 1.0001,  ∵ L = X * sqrt(P)   ∴ X(i) = L / sqrt(d ^ i)
    // sqrt(d) ^ (r - l) - 1
    // --------------------------------- = amount_x_of_unit_liquidity: the amount of token X equivalent to a unit of  c in the range
    // sqrt(d) ^ r - sqrt(d) ^ (r - 1)
    // 
    // (sqrt(d) - 1) * (sqrt(d) ^ (r - l - 1) + sqrt(d) ^ (r - l - 2) + ...... + 1)
    // ----------------------------------------------------------------------------
    // (sqrt(d) - 1) * sqrt(d) ^ (r - 1))
    //
    //      1                1                             1
    // ------------ + ----------------- + ...... + -----------------
    // sqrt(d) ^ l    sqrt(d) ^ (l + 1)            sqrt(d) ^ (r - 1)
    //
    // X(l) + X(l + 1) + ...... + X(r - 1)

    // amount_x = amount_x_of_unit_liquidity * liquidity

    let sqrt_price_pr_pl_96 = get_sqrt_price(right_pt - left_pt);
    let sqrt_price_pr_m1_96 = sqrt_price_r_96.mul_fraction_floor(pow_96(), sqrt_rate_96);

    // using sum equation of geomitric series to compute range numbers
    let numerator = sqrt_price_pr_pl_96 - pow_96();
    let denominator = sqrt_price_r_96 - sqrt_price_pr_m1_96;
    if !upper {
        U256::from(liquidity).mul_fraction_floor(numerator, denominator)
    } else {
        U256::from(liquidity).mul_fraction_ceil(numerator, denominator)
    }
}

/// compute the most left point so that all liquidities in [most_left_point, right_pt) would be swapped out by amount_x
/// @param liquidity: liquidity in each point
/// @param amount_x: the amount of token X used in swap
/// @param right_pt: right point of this range
/// @param sqrt_price_r_96: sqrt of right point price in 2^96 power
/// @return the most left point in this range swap, if it equals to right_pt, means nothing swapped in this range
pub fn get_most_left_point(
    liquidity: u128,
    amount_x: u128,
    right_pt: i32,
    sqrt_price_r_96: U256,
) -> i32 {
    // d = 1.0001
    // sqrt(d) ^ (r - l) - 1
    // --------------------------------- * liquidity = amount_x
    // sqrt(d) ^ r - sqrt(d) ^ (r - 1)
    //
    // sqrt(d) ^ (r - l) = amount_x * (sqrt(d) ^ r - sqrt(d) ^ (r - 1)) / liquidity + 1

    let sqrt_price_pr_m1_96 = sqrt_price_r_96.mul_fraction_ceil(pow_96(), sqrt_rate_96());
    let sqrt_value_96 = U256::from(amount_x).mul_fraction_floor(sqrt_price_r_96 - sqrt_price_pr_m1_96, U256::from(liquidity)) + pow_96();
    let log_value = get_log_sqrt_price_floor(sqrt_value_96);
    right_pt - log_value
}

/// compute the most right point so that all liquidities in [left_point, most_right_point) would be swapped out by amount_y
/// @param liquidity: liquidity in each point
/// @param amount_y: the amount of token Y used in swap
/// @param sqrt_price_l_96: sqrt of left point price in 2^96 power
/// @return the most right point in this range swap, if it equals to left_pt, means nothing swapped in this range
pub fn get_most_right_point(
    liquidity: u128,
    amount_y: u128,
    sqrt_price_l_96: U256,
) -> i32 {
    // d = 1.0001
    // sqrt(d) ^ r - sqrt(d) ^ l
    // ------------------------- * liquidity = amount_y
    // sqrt(d) - 1
    //
    // sqrt(d) ^ r - sqrt(d) ^ l = amount_y * (sqrt(d) - 1) / liquidity

    let sqrt_loc_96 = U256::from(amount_y).mul_fraction_floor(sqrt_rate_96() - pow_96(), U256::from(liquidity)) + sqrt_price_l_96;
    get_log_sqrt_price_floor(sqrt_loc_96)
}
