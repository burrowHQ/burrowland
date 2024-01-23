mod workspace_env;

use crate::workspace_env::*;

const PREVIOUS_VERSION: &'static str = "0.8.0";
const LATEST_VERSION: &'static str = "0.10.0";

#[tokio::test]
async fn test_upgrade() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let previous_burrowland_contract = deploy_previous_version_burrowland(&root).await?;
    let version = previous_burrowland_contract.get_version().await?;
    assert_eq!(version, PREVIOUS_VERSION);

    check!(root
        .call(previous_burrowland_contract.0.id(), "add_asset")
        .args_json(json!({
            "token_id": "wrap.testnet",
            "asset_config": {
                "reserve_ratio": 2500,
                "prot_ratio": 0,
                "target_utilization": 8000,
                "target_utilization_rate": "1000000000003593629036885046",
                "max_utilization_rate": "1000000000039724853136740579",
                "volatility_ratio": 9999,
                "extra_decimals": 0,
                "can_deposit": true,
                "can_withdraw": true,
                "can_use_as_collateral": true,
                "can_borrow": false,
                "net_tvl_multiplier": 10000,
            },
        }))
        .max_gas()
        .deposit(1)
        .transact());

    check!(view previous_burrowland_contract.get_config_v0());

    assert!(root
        .call(previous_burrowland_contract.0.id(), "upgrade")
        .args(std::fs::read(BURROWLAND_WASM).unwrap())
        .max_gas()
        .transact()
        .await?.is_success());
    let version = previous_burrowland_contract.get_version().await?;
    assert_eq!(version, LATEST_VERSION);
    check!(view previous_burrowland_contract.get_config());
    check!(view previous_burrowland_contract.get_asset(&"wrap.testnet".parse::<AccountId>().unwrap()));
    Ok(())
}
