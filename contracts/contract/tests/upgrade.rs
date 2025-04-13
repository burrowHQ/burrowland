mod workspace_env;

use mock_boost_farming::nano_to_sec;

use crate::workspace_env::*;

const PREVIOUS_VERSION: &'static str = "0.15.0";
const LATEST_VERSION: &'static str = "0.15.1";

#[tokio::test]
async fn test_upgrade() -> Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let eth_old_account_id: AccountId = get_eth_old_account_id().to_string().parse().unwrap();
    let eth_new_account_id: AccountId = get_eth_new_account_id().to_string().parse().unwrap();
    let (_, sk) = worker.dev_generate().await;
    let eth_old_account = worker
        .create_tla(eth_old_account_id.to_string().parse().unwrap(), sk)
        .await?
        .into_result()?;
    let eth_old_account = FtContract(eth_old_account
        .deploy(&std::fs::read("../../res/mock_ft.wasm").unwrap())
        .await?
        .unwrap());
    assert!(eth_old_account.0
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
            "token_id": eth_old_account_id,
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
        &eth_old_account_id.to_string().parse().unwrap(),
        18,
        4,
        "87a67534df591d2dd5ec577ab3c75668a8e3d35e92e27bf29d9e2e52df8de412",
        None,
        None
    ));

    check!(eth_old_account.ft_mint(&root, &root, parse_near!("50000 N")));

    check!(eth_old_account.ft_storage_deposit(previous_burrowland_contract.0.id()));
    check!(previous_burrowland_contract.deposit_to_reserve(&eth_old_account, &root, parse_near!("10000 N")));
    check!(previous_burrowland_contract.storage_deposit(&root));
    check!(previous_burrowland_contract.supply_to_collateral(&eth_old_account, &root, parse_near!("10000 N")));
    let current_timestamp = worker.view_block().await?.timestamp();

    check!(pyth_contract.set_price("87a67534df591d2dd5ec577ab3c75668a8e3d35e92e27bf29d9e2e52df8de412", PythPrice{
        price: I64(99980647),
        conf: U64(103853),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));
    check!(previous_burrowland_contract.borrow_with_pyth(&root, eth_old_account.0.id(), parse_near!("500 N")));
    check!(previous_burrowland_contract.add_asset_farm_reward(
        &root, 
        FarmId::Supplied(get_eth_old_account_id()), 
        &eth_old_account_id, 
        10000000u128.into(), 
        0u128.into(), 
        1u128.into())
    );
    check!(previous_burrowland_contract.add_asset_farm_reward(
        &root, 
        FarmId::Borrowed(get_eth_old_account_id()), 
        &eth_old_account_id, 
        10000000u128.into(), 
        0u128.into(), 
        1u128.into())
    );
    check!(previous_burrowland_contract.add_asset_farm_reward(
        &root, 
        FarmId::TokenNetBalance(get_eth_old_account_id()), 
        &eth_old_account_id, 
        10000000u128.into(), 
        0u128.into(), 
        1u128.into())
    );

    check!(previous_burrowland_contract.account_farm_claim_all(&root, None));
    check!(previous_burrowland_contract.account_farm_claim_all(&root, None));

    check!(view previous_burrowland_contract.get_asset(&eth_old_account_id));
    check!(view previous_burrowland_contract.get_token_pyth_info(&eth_old_account_id));
    check!(view previous_burrowland_contract.get_account(&root));
    check!(print root
        .call(previous_burrowland_contract.0.id(), "upgrade")
        .args(std::fs::read(BURROWLAND_WASM).unwrap())
        .max_gas()
        .transact());
    let version = previous_burrowland_contract.get_version().await?;
    assert_eq!(version, LATEST_VERSION);
    println!("============");
    check!(view previous_burrowland_contract.get_asset(&eth_new_account_id));
    check!(view previous_burrowland_contract.get_token_pyth_info(&eth_new_account_id));
    check!(view previous_burrowland_contract.get_account(&root));
    Ok(())
}
