use crate::*;

pub(crate) type TokenId = AccountId;
pub(crate) type PosId = String;
pub(crate) const UNIT: u128 = 1_000_000_000_000_000_000_u128;

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
    min_amount_out
        >= amount_out - u128_ratio(amount_out, max_slippage_rate as u128, MAX_RATIO as u128)
}

const ETH_OLD_ACCOUNT_ID: &str = "aurora";
const ETH_NEW_ACCOUNT_ID_MAINNET: &str = "eth.bridge.near";
const ETH_NEW_ACCOUNT_ID_TESTNET: &str = "eth.sepolia.testnet";

pub fn get_eth_old_account_id() -> AccountId {
    ETH_OLD_ACCOUNT_ID.parse().unwrap()
}

/// FIX-ETH: return correct new eth tokenID on both mainnet and testnet env
pub fn get_eth_new_account_id() -> AccountId {
    if env::current_account_id().to_string().ends_with(".near") {
        ETH_NEW_ACCOUNT_ID_MAINNET.parse().unwrap()
    } else {
        ETH_NEW_ACCOUNT_ID_TESTNET.parse().unwrap()
    }
}

/// FIX-ETH: to replace eth tokenID in keys of a given account assets HashMap
pub fn update_account_assets_eth_token_id(assets: &mut HashMap<TokenId, Shares>) {
    if let Some(shares) = assets.remove(&get_eth_old_account_id()) {
        assets.insert(get_eth_new_account_id(), shares);
    }
}

/// FIX-ETH: to replace eth tokenID in keys of a given account farms HashMap
pub fn update_account_farms_eth_token_id(farms: &mut HashMap<FarmId, AccountFarm>, ) {
    let eth_old_account_id = get_eth_old_account_id();
    let eth_new_account_id = get_eth_new_account_id();
    // FarmId::NetTvl has no eth in account.farms.rewards.
    // No update is needed.

    // FarmId::Supplied(eth) has eth in account.farms.rewards.
    // If the user has this farm, the key and account.farms.rewards need to be updated.
    if let Some(mut supplied_farm) = farms.remove(&FarmId::Supplied(eth_old_account_id.clone())) {
        if let Some(supplied_reward) = supplied_farm.rewards.remove(&eth_old_account_id) {
            supplied_farm.rewards.insert(eth_new_account_id.clone(), supplied_reward);
        }
        farms.insert(FarmId::Supplied(eth_new_account_id.clone()), supplied_farm);
    }

    // FarmId::Borrowed(eth) has no eth in account.farms.rewards.
    // If the user has this farm, only the key needs to be updated.
    if let Some(borrowed_farm) = farms.remove(&FarmId::Borrowed(eth_old_account_id.clone())) {
        farms.insert(FarmId::Borrowed(eth_new_account_id.clone()), borrowed_farm);
    }

    // FarmId::TokenNetBalance(eth) has no eth in account.farms.rewards
    // If the user has this farm, only the key needs to be updated.
    if let Some(token_net_balance_farm) = farms.remove(&FarmId::TokenNetBalance(eth_old_account_id.clone())) {
        farms.insert(FarmId::TokenNetBalance(eth_new_account_id.clone()), token_net_balance_farm);
    }
}
