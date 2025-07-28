mod workspace_env;

use crate::workspace_env::*;

const PREVIOUS_VERSION: &'static str = "0.15.2";
const LATEST_VERSION: &'static str = "0.15.3";

#[tokio::test]
async fn test_upgrade() -> Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let previous_burrowland_contract = deploy_previous_version_burrowland(&root).await?;
    let version = previous_burrowland_contract.get_version().await?;
    assert_eq!(version, PREVIOUS_VERSION);

    let token_id = "shadow_ref_v1-0".parse::<AccountId>().unwrap();
    check!(root.call(previous_burrowland_contract.0.id(), "add_asset")
        .args_json(json!({
            "token_id": token_id,
            "asset_config": AssetConfig {
                reserve_ratio: 2500,
                prot_ratio: 0,
                target_utilization: 8000,
                target_utilization_rate: U128(1000000000003593629036885046),
                max_utilization_rate: U128(1000000000039724853136740579),
                holding_position_fee_rate: U128(1000000000003593629036885046),
                volatility_ratio: 7000,
                extra_decimals: 0,
                can_deposit: true,
                can_withdraw: true,
                can_use_as_collateral: true,
                can_borrow: true,
                net_tvl_multiplier: 2500,
                max_change_rate: None,
                supplied_limit: Some(u128::MAX.into()),
                borrowed_limit: Some(u128::MAX.into()),
                min_borrowed_amount: Some(1u128.into())
            },
        }))
        .max_gas()
        .deposit(NearToken::from_yoctonear(1))
        .transact());
    check!(view previous_burrowland_contract.get_config());

    check!(print root
        .call(previous_burrowland_contract.0.id(), "upgrade")
        .args(std::fs::read(BURROWLAND_WASM).unwrap())
        .max_gas()
        .transact());
    let version = previous_burrowland_contract.get_version().await?;
    assert_eq!(version, LATEST_VERSION);
    check!(view previous_burrowland_contract.get_config());
    check!(view previous_burrowland_contract.get_asset(&token_id));
    Ok(())
}
