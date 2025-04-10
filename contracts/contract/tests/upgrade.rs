mod workspace_env;

use mock_boost_farming::nano_to_sec;

use crate::workspace_env::*;

const PREVIOUS_VERSION: &'static str = "0.15.0";
const LATEST_VERSION: &'static str = "0.15.1";

#[tokio::test]
async fn test_upgrade() -> Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let aurora_old_account_id: AccountId = get_aurora_old_account_id().to_string().parse().unwrap();
    let aurora_new_account_id: AccountId = get_aurora_new_account_id().to_string().parse().unwrap();
    let (_, sk) = worker.dev_generate().await;
    let aurora_old_account = worker
        .create_tla(aurora_old_account_id.to_string().parse().unwrap(), sk)
        .await?
        .into_result()?;
    let aurora_old_contract = FtContract(aurora_old_account
        .deploy(&std::fs::read("../../res/mock_ft.wasm").unwrap())
        .await?
        .unwrap());
    assert!(aurora_old_contract.0
        .call("new")
        .args_json(json!({
            "name": "aurora",
            "symbol": "aurora",
            "decimals": 18,
        }))
        .max_gas()
        .transact()
        .await?
        .is_success());
    
    let pyth_contract = deploy_mock_pyth(&root).await?;

    let previous_burrowland_contract = deploy_previous_version_burrowland(&root).await?;

    let version = previous_burrowland_contract.get_version().await?;
    assert_eq!(version, PREVIOUS_VERSION);

    check!(root
        .call(previous_burrowland_contract.0.id(), "add_asset")
        .args_json(json!({
            "token_id": aurora_old_account_id,
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
    check!(previous_burrowland_contract.add_token_pyth_info(
        &root,
        &aurora_old_account_id.to_string().parse().unwrap(),
        18,
        4,
        "87a67534df591d2dd5ec577ab3c75668a8e3d35e92e27bf29d9e2e52df8de412",
        None,
        None
    ));

    check!(aurora_old_contract.ft_mint(&root, &root, parse_near!("50000 N")));

    check!(aurora_old_contract.ft_storage_deposit(previous_burrowland_contract.0.id()));
    check!(previous_burrowland_contract.deposit_to_reserve(&aurora_old_contract, &root, parse_near!("10000 N")));
    check!(previous_burrowland_contract.storage_deposit(&root));
    check!(previous_burrowland_contract.supply_to_collateral(&aurora_old_contract, &root, parse_near!("10000 N")));
    let current_timestamp = worker.view_block().await?.timestamp();

    check!(pyth_contract.set_price("87a67534df591d2dd5ec577ab3c75668a8e3d35e92e27bf29d9e2e52df8de412", PythPrice{
        price: I64(99980647),
        conf: U64(103853),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));
    check!(previous_burrowland_contract.borrow_with_pyth(&root, aurora_old_contract.0.id(), parse_near!("500 N")));

    check!(view previous_burrowland_contract.get_asset(&aurora_old_account_id));
    check!(view previous_burrowland_contract.get_token_pyth_info(&aurora_old_account_id));
    check!(view previous_burrowland_contract.get_account(&root));
    check!(root
        .call(previous_burrowland_contract.0.id(), "upgrade")
        .args(std::fs::read(BURROWLAND_WASM).unwrap())
        .max_gas()
        .transact());
    let version = previous_burrowland_contract.get_version().await?;
    assert_eq!(version, LATEST_VERSION);
    println!("============");
    check!(view previous_burrowland_contract.get_asset(&aurora_new_account_id));
    check!(view previous_burrowland_contract.get_token_pyth_info(&aurora_new_account_id));
    check!(view previous_burrowland_contract.get_account(&root));
    Ok(())
}
