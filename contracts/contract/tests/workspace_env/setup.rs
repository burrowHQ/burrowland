use crate::*;

use contract::Config;

pub const ORACLE_ID: &str = "oracle.test.near";
pub const PYTH_ID: &str = "pyth.test.near";
pub const BOOSTER_TOKEN_ID: &str = "booster.test.near";
pub const BOOSTER_TOKEN_DECIMALS: u8 = 18;

const PREVIOUS_BURROWLAND_WASM: &str = "../../releases/burrowland_0.11.0.wasm";
pub const BURROWLAND_WASM: &str = "../../res/burrowland.wasm";
const REF_EXCHANGE_WASM: &str = "../../res/mock_ref_exchange.wasm";
pub const BOOST_FARMING_WASM: &str = "../../res/mock_boost_farming.wasm";
// const REF_EXCHANGE_WASM: &str = "../../res/ref_exchange.wasm";
// pub const BOOST_FARMING_WASM: &str = "../../res/boost_farming.wasm";
const ORACLE_WASM: &str = "../../res/test_oracle.wasm";
const FT_WASM: &str = "../../res/mock_ft.wasm";
const RATED_TOKEN_WASM: &str = "../../res/mock_rated_token.wasm";
const PYTH_WASM: &str = "../../res/mock_pyth.wasm";

pub async fn deploy_burrowland_with_pyth(
    root: &Account,
) -> Result<Burrowland> {
    let burrowland = root
        .create_subaccount("burrowland")
        .initial_balance(parse_near!("50 N"))
        .transact()
        .await?
        .unwrap();
    let burrowland = burrowland
        .deploy(&std::fs::read(BURROWLAND_WASM).unwrap())
        .await?
        .unwrap();
    assert!(burrowland.call("new")
        .args_json(json!({
            "config": Config {
                oracle_account_id: near_sdk::AccountId::new_unchecked(ORACLE_ID.to_string()),
                pyth_oracle_account_id: near_sdk::AccountId::new_unchecked(PYTH_ID.to_string()),
                ref_exchange_id: near_sdk::AccountId::new_unchecked("ref_exchange.test.near".to_string()),
                owner_id: near_sdk::AccountId::new_unchecked(root.id().to_string()),
                booster_token_id: near_sdk::AccountId::new_unchecked(BOOSTER_TOKEN_ID.to_string()),
                booster_decimals: BOOSTER_TOKEN_DECIMALS,
                max_num_assets: 10,
                maximum_recency_duration_sec: 90,
                maximum_staleness_duration_sec: 15,
                lp_tokens_info_valid_duration_sec: 600,
                pyth_price_valid_duration_sec: 60,
                minimum_staking_duration_sec: 2678400,
                maximum_staking_duration_sec: 31536000,
                x_booster_multiplier_at_maximum_staking_duration: 40000,
                force_closing_enabled: true,
                enable_price_oracle: false,
                enable_pyth_oracle: true,
                boost_suppress_factor: 1,
            },
        }))
        .max_gas()
        .transact()
        .await?
        .is_success());
    Ok(Burrowland(burrowland))
}

pub async fn deploy_burrowland_with_price_oracle(
    root: &Account,
) -> Result<Burrowland> {
    let burrowland = root
        .create_subaccount("burrowland")
        .initial_balance(parse_near!("50 N"))
        .transact()
        .await?
        .unwrap();
    let burrowland = burrowland
        .deploy(&std::fs::read(BURROWLAND_WASM).unwrap())
        .await?
        .unwrap();
    assert!(burrowland.call("new")
        .args_json(json!({
            "config": Config {
                oracle_account_id: near_sdk::AccountId::new_unchecked(ORACLE_ID.to_string()),
                pyth_oracle_account_id: near_sdk::AccountId::new_unchecked(PYTH_ID.to_string()),
                ref_exchange_id: near_sdk::AccountId::new_unchecked("ref_exchange.test.near".to_string()),
                owner_id: near_sdk::AccountId::new_unchecked(root.id().to_string()),
                booster_token_id: near_sdk::AccountId::new_unchecked(BOOSTER_TOKEN_ID.to_string()),
                booster_decimals: BOOSTER_TOKEN_DECIMALS,
                max_num_assets: 10,
                maximum_recency_duration_sec: 90,
                maximum_staleness_duration_sec: 15,
                lp_tokens_info_valid_duration_sec: 600,
                pyth_price_valid_duration_sec: 60,
                minimum_staking_duration_sec: 2678400,
                maximum_staking_duration_sec: 31536000,
                x_booster_multiplier_at_maximum_staking_duration: 40000,
                force_closing_enabled: true,
                enable_price_oracle: true,
                enable_pyth_oracle: false,
                boost_suppress_factor: 1,
            },
        }))
        .max_gas()
        .transact()
        .await?
        .is_success());
    Ok(Burrowland(burrowland))
}

pub async fn deploy_previous_version_burrowland(
    root: &Account,
) -> Result<Burrowland> {
    let burrowland = root
        .create_subaccount("burrowland")
        .initial_balance(parse_near!("50 N"))
        .transact()
        .await?
        .unwrap();
    let burrowland = burrowland
        .deploy(&std::fs::read(PREVIOUS_BURROWLAND_WASM).unwrap())
        .await?
        .unwrap();
    assert!(burrowland.call("new")
        .args_json(json!({
            "config": Config {
                oracle_account_id: near_sdk::AccountId::new_unchecked(ORACLE_ID.to_string()),
                pyth_oracle_account_id: near_sdk::AccountId::new_unchecked(PYTH_ID.to_string()),
                ref_exchange_id: near_sdk::AccountId::new_unchecked("ref_exchange.test.near".to_string()),
                owner_id: near_sdk::AccountId::new_unchecked(root.id().to_string()),
                booster_token_id: near_sdk::AccountId::new_unchecked(BOOSTER_TOKEN_ID.to_string()),
                booster_decimals: BOOSTER_TOKEN_DECIMALS,
                max_num_assets: 10,
                maximum_recency_duration_sec: 90,
                maximum_staleness_duration_sec: 15,
                lp_tokens_info_valid_duration_sec: 600,
                pyth_price_valid_duration_sec: 60,
                minimum_staking_duration_sec: 2678400,
                maximum_staking_duration_sec: 31536000,
                x_booster_multiplier_at_maximum_staking_duration: 40000,
                force_closing_enabled: true,
                enable_price_oracle: true,
                enable_pyth_oracle: true,
                boost_suppress_factor: 1,
            },
        }))
        .max_gas()
        .transact()
        .await?
        .is_success());
    Ok(Burrowland(burrowland))
}

pub async fn deploy_mock_ft(
    root: &Account,
    symbol: &str,
    decimal: u8,
) -> Result<FtContract> {

    let mock_ft = root
        .create_subaccount(symbol)
        .initial_balance(parse_near!("50 N"))
        .transact()
        .await?
        .unwrap();
    let mock_ft = mock_ft
        .deploy(&std::fs::read(FT_WASM).unwrap())
        .await?
        .unwrap();
    assert!(mock_ft
        .call("new")
        .args_json(json!({
            "name": symbol,
            "symbol": symbol,
            "decimals": decimal,
        }))
        .gas(300_000_000_000_000)
        .transact()
        .await?
        .is_success());
    Ok(FtContract(mock_ft))
}

pub async fn deploy_oralce(
    root: &Account
) -> Result<Oralce> {
    let oralce = root
        .create_subaccount("oracle")
        .initial_balance(parse_near!("50 N"))
        .transact()
        .await?
        .unwrap();
    let oralce = oralce
        .deploy(&std::fs::read(ORACLE_WASM).unwrap())
        .await?
        .unwrap();
    Ok(Oralce(oralce))
}

pub async fn deploy_ref_exchange(
    root: &Account,
) -> Result<RefExchange> {
    let ref_exchange = root
        .create_subaccount("ref_exchange")
        .initial_balance(parse_near!("50 N"))
        .transact()
        .await?
        .unwrap();
    let ref_exchange = ref_exchange
        .deploy(&std::fs::read(REF_EXCHANGE_WASM).unwrap())
        .await?
        .unwrap();
    assert!(ref_exchange.call("new")
        .args_json(json!({
            "owner_id": root.id(),
            "exchange_fee": 2000,
            "referral_fee": 0,
            "boost_farm_id": near_sdk::AccountId::new_unchecked("boost_farming.test.near".to_string()),
            "burrowland_id": near_sdk::AccountId::new_unchecked("burrowland.test.near".to_string()),
        }))
        .max_gas()
        .transact()
        .await?
        .is_success());
    Ok(RefExchange(ref_exchange))
}

pub async fn deploy_boost_farming(
    root: &Account
) -> Result<BoostFarmingContract> {
    let boost_farming = root
        .create_subaccount("boost_farming")
        .initial_balance(parse_near!("50 N"))
        .transact()
        .await?
        .unwrap();
    let boost_farming = boost_farming
        .deploy(&std::fs::read(BOOST_FARMING_WASM).unwrap())
        .await?
        .unwrap();
    assert!(boost_farming.call("new")
        .args_json(json!({
            "owner_id": root.id(),
            "ref_exchange_id": near_sdk::AccountId::new_unchecked("ref_exchange.test.near".to_string()),
        }))
        .max_gas()
        .transact()
        .await?
        .is_success());
    Ok(BoostFarmingContract(boost_farming))
}

// pub async fn deploy_previous_version_boost_farming(
//     root: &Account
// ) -> Result<BoostFarmingContract> {
//     let boost_farming = root
//         .create_subaccount("boost_farming")
//         .initial_balance(parse_near!("50 N"))
//         .transact()
//         .await?
//         .unwrap();
//     let boost_farming = boost_farming
//         .deploy(&std::fs::read("../../res/boost_farming_release.wasm").unwrap())
//         .await?
//         .unwrap();
//     assert!(boost_farming.call("new")
//         .args_json(json!({
//             "owner_id": root.id(),
//         }))
//         .max_gas()
//         .transact()
//         .await?
//         .is_success());
//     Ok(BoostFarmingContract(boost_farming))
// }

pub async fn deploy_mock_rated_token(
    root: &Account,
    name: &str,
    symbol: &str, 
    decimals: u8, 
    price: U128
) -> Result<RatedTokenContract> {

    let mock_rated_token = root
        .create_subaccount(symbol)
        .initial_balance(parse_near!("50 N"))
        .transact()
        .await?
        .unwrap();
    let mock_rated_token = mock_rated_token
        .deploy(&std::fs::read(RATED_TOKEN_WASM).unwrap())
        .await?
        .unwrap();
    assert!(mock_rated_token
        .call("new")
        .args_json(json!({
            "name": name,
            "symbol": symbol,
            "decimals": decimals,
            "price": price
        }))
        .gas(300_000_000_000_000)
        .transact()
        .await?
        .is_success());
    Ok(RatedTokenContract(mock_rated_token))
}


pub async fn deploy_mock_pyth(
    root: &Account,
) -> Result<PythContract> {
    let mock_pyth = root
        .create_subaccount("pyth")
        .initial_balance(parse_near!("50 N"))
        .transact()
        .await?
        .unwrap();
    let mock_pyth = mock_pyth
        .deploy(&std::fs::read(PYTH_WASM).unwrap())
        .await?
        .unwrap();
    assert!(mock_pyth
        .call("new")
        .gas(300_000_000_000_000)
        .transact()
        .await?
        .is_success());
    Ok(PythContract(mock_pyth))
}