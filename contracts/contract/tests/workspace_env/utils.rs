use crate::*;

use contract::AssetAmount;

pub const EVENT_JSON: &str = "EVENT_JSON:";

pub fn d(value: Balance, decimals: u8) -> Balance {
    value * 10u128.pow(decimals as _)
}

pub fn asset_amount(token_id: &AccountId, amount: Balance) -> AssetAmount {
    AssetAmount {
        token_id: near_sdk::AccountId::new_unchecked(token_id.to_string()),
        amount: if amount == 0 {None} else {Some(amount.into())},
        max_amount: None,
    }
}

pub async fn create_account(
    master: &Account,
    account_id: &str,
    balance: Option<u128>,
) -> Account {
    let balance = if let Some(balance) = balance {
        balance
    } else {
        parse_near!("50 N")
    };
    master
        .create_subaccount(account_id)
        .initial_balance(balance)
        .transact()
        .await
        .unwrap()
        .unwrap()
}

pub fn tool_err_msg(outcome: Result<ExecutionFinalResult>) -> String {
    match outcome {
        Ok(res) => {
            let mut msg = "".to_string();
            for r in res.receipt_failures(){
                match r.clone().into_result() {
                    Ok(_) => {},
                    Err(err) => {
                        msg += &format!("{:?}", err);
                        msg += "\n";
                    }
                }
            }
            msg
        },
        Err(err) => err.to_string()
    }
}

pub fn price_data(
    timestamp: u64,
    wnear_mul: Option<Balance>,
) -> PriceData {
    let mut prices = vec![
        AssetOptionalPrice {
            asset_id: "ndai.test.near".to_string(),
            price: Some(Price {
                multiplier: 10000,
                decimals: 22,
            }),
        },
        AssetOptionalPrice {
            asset_id: "nusdc.test.near".to_string(),
            price: Some(Price {
                multiplier: 10000,
                decimals: 10,
            }),
        },
        AssetOptionalPrice {
            asset_id: "nusdt.test.near".to_string(),
            price: Some(Price {
                multiplier: 10000,
                decimals: 10,
            }),
        },
        AssetOptionalPrice {
            asset_id: "ref.test.near".to_string(),
            price: Some(Price {
                multiplier: 10000,
                decimals: 22,
            }),
        },
    ];
    if let Some(wnear_mul) = wnear_mul {
        prices.push(AssetOptionalPrice {
            asset_id: "wrap.test.near".to_string(),
            price: Some(Price {
                multiplier: wnear_mul,
                decimals: 28,
            }),
        })
    }
    PriceData {
        timestamp,
        recency_duration_sec: 90,
        prices,
    }
}

pub fn av(token_id: &AccountId, balance: Balance) -> AssetView {
    AssetView {
        token_id: near_sdk::AccountId::new_unchecked(token_id.to_string()),
        balance,
        shares: U128(0),
        apr: Default::default(),
    }
}

pub fn find_asset<'a>(assets: &'a [AssetView], token_id: &AccountId) -> &'a AssetView {
    assets
        .iter()
        .find(|e| e.token_id.to_string() == token_id.to_string())
        .expect("Missing asset")
}

pub fn assert_balances(actual: &[AssetView], expected: &[AssetView]) {
    assert_eq!(actual.len(), expected.len());
    for asset in actual {
        assert_eq!(asset.balance, find_asset(expected, &asset.token_id.to_string().parse().unwrap()).balance);
    }
}

pub async fn tool_create_account(
    master: &Account,
    account_id: &str,
    balance: Option<u128>,
) -> Account {
    let balance = if let Some(balance) = balance {
        balance
    } else {
        parse_near!("50 N")
    };
    master
        .create_subaccount(account_id)
        .initial_balance(balance)
        .transact()
        .await
        .unwrap()
        .unwrap()
}

#[macro_export]
macro_rules! check{
    ($exec_func: expr)=>{
        let outcome = $exec_func.await?;
        assert!(outcome.is_success() && outcome.receipt_failures().is_empty());
    };
    (print $exec_func: expr)=>{
        let outcome = $exec_func.await;
        let err_msg = tool_err_msg(outcome);
        if err_msg.is_empty() {
            println!("success");
        } else {
            println!("{}", err_msg);
        }
    };
    (print $prefix: literal $exec_func: expr)=>{
        let outcome = $exec_func.await;
        let err_msg = tool_err_msg(outcome);
        if err_msg.is_empty() {
            println!("{} success", $prefix);
        } else {
            println!("{} {}", $prefix, err_msg);
        }
    };
    (view $exec_func: expr)=>{
        let query_result = $exec_func.await?;
        println!("{:?}", query_result);
    };
    (view $prefix: literal $exec_func: expr)=>{
        let query_result = $exec_func.await?;
        println!("{} {:?}", $prefix, query_result);
    };
    (logs $exec_func: expr)=>{
        let outcome = $exec_func.await?;
        assert!(outcome.is_success() && outcome.receipt_failures().is_empty());
        println!("{:#?}", outcome.logs());
    };
    ($exec_func: expr, $err_info: expr)=>{
        assert!(tool_err_msg($exec_func.await).contains($err_info));
    };
}