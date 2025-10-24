mod workspace_env;

use crate::workspace_env::*;

#[tokio::test]
async fn test_dev_setup() -> Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;
    
    let amount = d(10000, 24);
    let wrap_token_contract = deploy_mock_ft(&root, "wrap", 24).await?;
    check!(wrap_token_contract.ft_mint(&root, &root, amount));
    
    let burrowland_contract = deploy_burrowland_with_price_oracle(&root).await?;
    check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));
    check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));

    check!(burrowland_contract.deposit_to_reserve(&wrap_token_contract, &root, amount));

    let asset = burrowland_contract.get_asset(wrap_token_contract.0.id()).await?;
    assert_eq!(asset.reserved, d(10000, 24));
    Ok(())
}

#[tokio::test]
async fn test_supply() -> Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let wrap_token_contract = deploy_mock_ft(&root, "wrap", 24).await?;
    
    let burrowland_contract = deploy_burrowland_with_price_oracle(&root).await?;
    check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));
    check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));

    let alice = create_account(&root, "alice", None).await;
    check!(burrowland_contract.storage_deposit(&alice));
    
    let amount = d(100, 24);
    check!(wrap_token_contract.ft_mint(&root, &alice, amount));
    check!(burrowland_contract.deposit(&wrap_token_contract, &alice, amount));

    let asset = burrowland_contract.get_asset(wrap_token_contract.0.id()).await?;
    assert_eq!(asset.supplied.balance, amount);

    let account = burrowland_contract.get_account(&alice).await?.unwrap();
    assert_eq!(account.supplied[0].balance, amount);
    assert_eq!(account.supplied[0].token_id.to_string(), wrap_token_contract.0.id().to_string());
    Ok(())
}

#[tokio::test]
async fn test_supply_to_collateral() -> Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let wrap_token_contract = deploy_mock_ft(&root, "wrap", 24).await?;
    
    let burrowland_contract = deploy_burrowland_with_price_oracle(&root).await?;
    check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));
    check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));

    let amount = d(100, 24);

    let alice = create_account(&root, "alice", None).await;
    check!(burrowland_contract.storage_deposit(&alice));
    check!(wrap_token_contract.ft_mint(&root, &alice, amount));

    check!(burrowland_contract.supply_to_collateral(&wrap_token_contract, &alice, amount));

    let asset = burrowland_contract.get_asset(wrap_token_contract.0.id()).await?;
    assert_eq!(asset.supplied.balance, amount);

    let account = burrowland_contract.get_account(&alice).await?.unwrap();
    assert!(account.supplied.is_empty());
    assert_eq!(account.collateral[0].balance, amount);
    assert_eq!(account.collateral[0].token_id.to_string(), wrap_token_contract.0.id().to_string());
    Ok(())
}

#[tokio::test]
async fn test_withdraw_prot_fee_reserved_failed() -> Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let wrap_token_contract = deploy_mock_ft(&root, "wrap", 24).await?;
    let ndai_token_contract = deploy_mock_ft(&root, "ndai", 18).await?;
    check!(ndai_token_contract.ft_mint(&root, &root, d(500, 18)));
    
    let oracle_contract = deploy_oralce(&root).await?;
    let burrowland_contract = deploy_burrowland_with_price_oracle(&root).await?;
    check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));
    check!(burrowland_contract.add_asset_handler(&root, &ndai_token_contract));
    check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(ndai_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(burrowland_contract.deposit_to_reserve(&ndai_token_contract, &root, d(500, 18)));

    let amount = d(100, 18);
    let supply_amount = d(100, 24);

    let alice = create_account(&root, "alice", None).await;
    check!(burrowland_contract.storage_deposit(&alice));
    check!(burrowland_contract.storage_deposit(&root));
    check!(wrap_token_contract.ft_mint(&root, &alice, supply_amount));
    check!(ndai_token_contract.ft_mint(&root, &alice, amount));

    check!(burrowland_contract.deposit(&ndai_token_contract, &alice, amount));

    check!(burrowland_contract.supply_to_collateral(&wrap_token_contract, &alice, supply_amount));
    
    let borrow_amount = d(200, 18);
    let current_timestamp = worker.view_block().await?.timestamp();
    check!(burrowland_contract.borrow(&alice, &oracle_contract, price_data(current_timestamp, Some(100000)), ndai_token_contract.0.id(), borrow_amount));

    let asset = burrowland_contract.get_asset(ndai_token_contract.0.id()).await?;
    let new_asset_config = asset.config;
    check!(burrowland_contract.update_asset(&root, ndai_token_contract.0.id(), new_asset_config));
    worker.fast_forward(1000).await?;

    let asset = burrowland_contract.get_asset(ndai_token_contract.0.id()).await?;
    check!(burrowland_contract.claim_prot_fee(&root, ndai_token_contract.0.id(), Some(100.into())), "Asset prot_fee balance not enough!");
    check!(burrowland_contract.decrease_reserved(&root, ndai_token_contract.0.id(), Some((asset.reserved * 2).into())), "Asset reserved balance not enough!");

    check!(burrowland_contract.extend_guardians(&root, vec![alice.id()]));
    check!(burrowland_contract.decrease_reserved(&alice, ndai_token_contract.0.id(), Some(asset.reserved.into())), "reserve_ratio >= config_reserve_ratio");
    check!(burrowland_contract.decrease_reserved(&root, ndai_token_contract.0.id(), Some(asset.reserved.into())));

    check!(ndai_token_contract.ft_storage_unregister(&alice));
    check!(burrowland_contract.withdraw(&alice, ndai_token_contract.0.id(), 500), "The account alice.test.near is not registered");
    Ok(())
}

#[tokio::test]
async fn test_modify_config() -> Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;
    let alice = tool_create_account(&root, "alice", None).await;

    let token_id = "s_v1-0".parse::<AccountId>().unwrap();
    let burrowland_contract = deploy_burrowland_with_price_oracle(&root).await?;
    check!(burrowland_contract.add_asset(&root, &token_id, AssetConfig{
        reserve_ratio: 2500,
        beneficiaries: HashMap::new(),
        target_utilization: 8000,
        target_utilization_rate: 1000000000003593629036885046u128.into(),
        max_utilization_rate: 1000000000039724853136740579u128.into(),
        holding_position_fee_rate: U128(1000000000000000000000000000),
        volatility_ratio: 9999,
        extra_decimals: 0,
        can_deposit: true,
        can_withdraw: true,
        can_use_as_collateral: true,
        can_borrow: false,
        net_tvl_multiplier: 10000,
        max_change_rate: None,
        supplied_limit: Some(u128::MAX.into()),
        borrowed_limit: Some(u128::MAX.into()),
        min_borrowed_amount: Some(1u128.into()),
    }));

    let asset = burrowland_contract.get_asset(&token_id).await?;
    assert!(asset.config.beneficiaries.is_empty());

    check!(burrowland_contract.upsert_beneficiary(&alice, &token_id, root.id(), 100), "Not allowed");
    check!(burrowland_contract.enable_asset_capacity(&alice, &token_id, Some(true), None, None, None), "Not an owner");
    check!(burrowland_contract.disable_asset_capacity(&alice, &token_id, Some(false), None, None, None), "Not allowed");
    check!(burrowland_contract.update_asset_net_tvl_multiplier(&alice, &token_id, 200), "Not an owner");

    check!(burrowland_contract.extend_guardians(&alice, vec![alice.id()]), "Not an owner");
    check!(burrowland_contract.remove_guardians(&alice, vec![alice.id()]), "Not an owner");
    check!(burrowland_contract.extend_guardians(&root, vec![alice.id()]));

    check!(burrowland_contract.upsert_beneficiary(&alice, &token_id, root.id(), 100));
    check!(burrowland_contract.update_asset_net_tvl_multiplier(&root, &token_id, 200));
    check!(burrowland_contract.disable_asset_capacity(&alice, &token_id, Some(false), Some(false), Some(false), None));
    check!(burrowland_contract.enable_asset_capacity(&root, &token_id, None, None, None, Some(true)));
    let asset = burrowland_contract.get_asset(&token_id).await?;
    assert_eq!(asset.config.beneficiaries, HashMap::from([("test.near".parse().unwrap(), 100)]));
    assert_eq!(asset.config.net_tvl_multiplier, 200);
    assert_eq!(asset.config.can_deposit, false);
    assert_eq!(asset.config.can_withdraw, false);
    assert_eq!(asset.config.can_use_as_collateral, false);
    assert_eq!(asset.config.can_borrow, true);

    Ok(())
}

#[tokio::test]
async fn test_simple_withdraw_supply() -> Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let wrap_token_contract = deploy_mock_ft(&root, "wrap", 24).await?;

    let burrowland_contract = deploy_burrowland_with_price_oracle(&root).await?;
    check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));
    check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));

    let alice = create_account(&root, "alice", None).await;
    check!(burrowland_contract.storage_deposit(&alice));
    check!(wrap_token_contract.ft_storage_deposit(alice.id()));

    let amount = d(100, 24);
    check!(wrap_token_contract.ft_mint(&root, &alice, amount));
    check!(burrowland_contract.deposit(&wrap_token_contract, &alice, amount));

    let account_before = burrowland_contract.get_account(&alice).await?.unwrap();
    assert_eq!(account_before.supplied[0].balance, amount);

    // Withdraw from supply
    let withdraw_amount = d(30, 24);
    let result = burrowland_contract.simple_withdraw(&alice, wrap_token_contract.0.id(), withdraw_amount, None).await;
    if let Err(e) = &result {
        eprintln!("Error: {:?}", e);
    }
    assert!(result.is_ok(), "simple_withdraw failed");
    let outcome = result?;
    eprintln!("Outcome: {:?}", outcome);
    assert!(outcome.is_success(), "simple_withdraw transaction failed");

    let account_after = burrowland_contract.get_account(&alice).await?.unwrap();
    println!("{:?}", account_after);
    assert_eq!(account_after.supplied[0].balance, d(70, 24)); // Exact: 100 - 30
    assert!(account_after.collateral.is_empty());

    Ok(())
}

#[tokio::test]
async fn test_simple_withdraw_multiple_times() -> Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let wrap_token_contract = deploy_mock_ft(&root, "wrap", 24).await?;

    let burrowland_contract = deploy_burrowland_with_price_oracle(&root).await?;
    check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));
    check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));

    let alice = create_account(&root, "alice", None).await;
    check!(burrowland_contract.storage_deposit(&alice));
    check!(wrap_token_contract.ft_storage_deposit(alice.id()));

    let supply_amount = d(100, 24);

    check!(wrap_token_contract.ft_mint(&root, &alice, supply_amount));

    // Deposit 100 as supply
    check!(burrowland_contract.deposit(&wrap_token_contract, &alice, supply_amount));

    let account_before = burrowland_contract.get_account(&alice).await?.unwrap();
    assert_eq!(account_before.supplied[0].balance, supply_amount);

    // First withdrawal: 30
    let withdraw_amount1 = d(30, 24);
    let result = burrowland_contract.simple_withdraw(&alice, wrap_token_contract.0.id(), withdraw_amount1, None).await;
    assert!(result.is_ok(), "first simple_withdraw failed");
    let outcome = result?;
    assert!(outcome.is_success(), "first simple_withdraw transaction failed");

    let account_mid = burrowland_contract.get_account(&alice).await?.unwrap();
    assert_eq!(account_mid.supplied[0].balance, d(70, 24)); // Exact: 100 - 30

    // Second withdrawal: 20
    let withdraw_amount2 = d(20, 24);
    let result2 = burrowland_contract.simple_withdraw(&alice, wrap_token_contract.0.id(), withdraw_amount2, None).await;
    assert!(result2.is_ok(), "second simple_withdraw failed");
    let outcome2 = result2?;
    assert!(outcome2.is_success(), "second simple_withdraw transaction failed");

    let account_after = burrowland_contract.get_account(&alice).await?.unwrap();
    assert_eq!(account_after.supplied[0].balance, d(50, 24)); // Exact: 100 - 30 - 20

    Ok(())
}

#[tokio::test]
async fn test_simple_withdraw_collateral() -> Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let wrap_token_contract = deploy_mock_ft(&root, "wrap", 24).await?;

    let burrowland_contract = deploy_burrowland_with_price_oracle(&root).await?;
    check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));
    check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));

    let alice = create_account(&root, "alice", None).await;
    check!(burrowland_contract.storage_deposit(&alice));
    check!(wrap_token_contract.ft_storage_deposit(alice.id()));

    let amount = d(100, 24);
    check!(wrap_token_contract.ft_mint(&root, &alice, amount));

    // Supply to collateral (no regular supply)
    check!(burrowland_contract.supply_to_collateral(&wrap_token_contract, &alice, amount));

    let account_before = burrowland_contract.get_account(&alice).await?.unwrap();
    assert!(account_before.supplied.is_empty());
    assert_eq!(account_before.collateral[0].balance, amount);

    // Add Pyth token info (with default price) and enable Pyth oracle for simple_withdraw from collateral
    // Price of 1 NEAR = $1 (multiplier=1, decimals=0)
    check!(burrowland_contract.add_token_pyth_info(&root, wrap_token_contract.0.id(), 24, 4, "27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", None, Some(Price{
        multiplier: 1,
        decimals: 0
    })));
    check!(burrowland_contract.enable_oracle(&root, false, true));

    // Withdraw from collateral
    let withdraw_amount = d(30, 24);
    let result = burrowland_contract.simple_withdraw(&alice, wrap_token_contract.0.id(), withdraw_amount, None).await;
    assert!(result.is_ok(), "simple_withdraw from collateral failed");
    let outcome = result?;
    assert!(outcome.is_success(), "simple_withdraw from collateral transaction failed");

    let account_after = burrowland_contract.get_account(&alice).await?.unwrap();
    assert!(account_after.supplied.is_empty());
    assert_eq!(account_after.collateral[0].balance, d(70, 24)); // Exact: 100 - 30

    Ok(())
}

#[tokio::test]
async fn test_simple_withdraw_supply_and_collateral() -> Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let wrap_token_contract = deploy_mock_ft(&root, "wrap", 24).await?;

    let burrowland_contract = deploy_burrowland_with_price_oracle(&root).await?;
    check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));
    check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));

    let alice = create_account(&root, "alice", None).await;
    check!(burrowland_contract.storage_deposit(&alice));
    check!(wrap_token_contract.ft_storage_deposit(alice.id()));

    let supply_amount = d(10, 24);
    let collateral_amount = d(100, 24);

    check!(wrap_token_contract.ft_mint(&root, &alice, supply_amount + collateral_amount));

    // Supply 100 as collateral first
    check!(burrowland_contract.supply_to_collateral(&wrap_token_contract, &alice, collateral_amount));

    // Then deposit 10 as supply
    check!(burrowland_contract.deposit(&wrap_token_contract, &alice, supply_amount));

    let account_before = burrowland_contract.get_account(&alice).await?.unwrap();
    assert_eq!(account_before.supplied[0].balance, supply_amount);
    assert_eq!(account_before.collateral[0].balance, collateral_amount);

    // Add Pyth token info (with default price) and enable Pyth oracle for simple_withdraw from collateral
    // Price of 1 NEAR = $1 (multiplier=1, decimals=0)
    check!(burrowland_contract.add_token_pyth_info(&root, wrap_token_contract.0.id(), 24, 4, "27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", None, Some(Price{
        multiplier: 1,
        decimals: 0
    })));
    check!(burrowland_contract.enable_oracle(&root, false, true));

    // Withdraw 50: should take all 10 from supply and 40 from collateral
    // Before: 10 supply, 100 collateral
    // After: 0 supply, 60 collateral
    let withdraw_amount = d(50, 24);
    let result = burrowland_contract.simple_withdraw(&alice, wrap_token_contract.0.id(), withdraw_amount, None).await;
    assert!(result.is_ok(), "simple_withdraw failed");
    let outcome = result?;
    assert!(outcome.is_success(), "simple_withdraw transaction failed");

    let account_after = burrowland_contract.get_account(&alice).await?.unwrap();
    assert!(account_after.supplied.is_empty()); // All 10 from supply withdrawn
    assert_eq!(account_after.collateral[0].balance, d(60, 24)); // 100 - 40 = 60

    Ok(())
}