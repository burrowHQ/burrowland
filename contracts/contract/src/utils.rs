use crate::*;

pub(crate) type TokenId = AccountId;
pub(crate) type PosId = String;

pub(crate) fn nano_to_ms(nano: u64) -> u64 {
    nano / 10u64.pow(6)
}

pub(crate) fn ms_to_nano(ms: u64) -> u64 {
    ms * 10u64.pow(6)
}

pub(crate) fn sec_to_nano(sec: u32) -> u64 {
    u64::from(sec) * 10u64.pow(9)
}

pub(crate) fn u128_ratio(a: u128, num: u128, denom: u128) -> Balance {
    (U256::from(a) * U256::from(num) / U256::from(denom)).as_u128()
}

pub(crate) fn ratio(balance: Balance, r: u32) -> Balance {
    assert!(r <= MAX_RATIO);
    u128_ratio(balance, u128::from(r), u128::from(MAX_RATIO))
}

pub const REGULAR_POSITION: &str = "REGULAR";
pub const SHADOW_V1_TOKEN_PREFIX: &str = "shadow_ref_v1-";

pub(crate) fn parse_pool_id(position: &String) -> u64 {
    position.split("-").collect::<Vec<&str>>()[1]
        .parse()
        .expect("Invalid position")
}

pub(crate) fn is_min_amount_out_reasonable(
    amount_in: Balance,
    asset_in: &Asset,
    price_in: &Price,
    asset_out: &Asset,
    price_out: &Price,
    min_amount_out: Balance,
    max_slippage_rate: u32,
) -> bool {
    let value_in =
        BigDecimal::from_balance_price(amount_in, price_in, asset_in.config.extra_decimals);
    let amount_out = value_in.to_balance_in_price(price_out, asset_out.config.extra_decimals);
    min_amount_out >= amount_out * max_slippage_rate as u128 / MAX_RATIO as u128 
}
