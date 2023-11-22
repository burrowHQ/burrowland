mod workspace_env;

use crate::workspace_env::*;

/// Bob attemps to liquidate Alice which decreases health factor.
#[tokio::test]
async fn test_liquidation_decrease_health_factor() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let nusdc_token_contract = deploy_mock_ft(&root, "nusdc", 18).await?;
    let nusdt_token_contract = deploy_mock_ft(&root, "nusdt", 18).await?;
    let wrap_token_contract = deploy_mock_ft(&root, "wrap", 18).await?;
    let wrap_reserve_amount = d(10000, 24);
    let nusdt_reserve_amount = d(10000, 6);
    check!(wrap_token_contract.ft_mint(&root, &root, wrap_reserve_amount));
    check!(nusdt_token_contract.ft_mint(&root, &root, nusdt_reserve_amount));

    let oracle_contract = deploy_oralce(&root).await?;
    let burrowland_contract = deploy_burrowland(&root).await?;
    check!(burrowland_contract.add_asset_handler(&root, &nusdc_token_contract));
    check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));
    check!(burrowland_contract.add_asset_handler(&root, &nusdt_token_contract));
    check!(nusdc_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(nusdt_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(burrowland_contract.deposit_to_reserve(&wrap_token_contract, &root, wrap_reserve_amount));
    check!(burrowland_contract.deposit_to_reserve(&nusdt_token_contract, &root, nusdt_reserve_amount));

    let alice = create_account(&root, "alice", None).await;
    check!(burrowland_contract.storage_deposit(&alice));
    let supply_amount = d(1000, 18);
    let extra_decimals_mult = d(1, 12);
    check!(nusdc_token_contract.ft_mint(&root, &alice, supply_amount));
    check!(wrap_token_contract.ft_storage_deposit(alice.id()));
    check!(nusdt_token_contract.ft_storage_deposit(alice.id()));

    check!(burrowland_contract.supply_to_collateral(&nusdc_token_contract, &alice, (supply_amount / extra_decimals_mult).into()));

    let wnear_borrow_amount = d(50, 24);
    let current_timestamp = worker.view_block().await?.timestamp();
    check!(burrowland_contract.borrow_and_withdraw(&alice, &oracle_contract, burrowland_contract.0.id(), price_data(current_timestamp, Some(100000)), wrap_token_contract.0.id(), wnear_borrow_amount));

    let usdt_borrow_amount = d(50, 18);
    let current_timestamp = worker.view_block().await?.timestamp();
    check!(burrowland_contract.borrow_and_withdraw(&alice, &oracle_contract, burrowland_contract.0.id(), price_data(current_timestamp, Some(100000)), nusdt_token_contract.0.id(), usdt_borrow_amount));

    let account = burrowland_contract.get_account(&alice).await?.unwrap();
    assert!(account.supplied.is_empty());
    let nep_postion = account.borrowed.get(&NEP_POSITION.to_string()).unwrap();
    assert!(find_asset(nep_postion, wrap_token_contract.0.id()).apr > BigDecimal::zero());
    assert!(find_asset(nep_postion, nusdt_token_contract.0.id()).apr > BigDecimal::zero());

    let wnear_bobs_amount = d(100, 24);
    let usdt_bobs_amount = d(100, 18);
    let bob = create_account(&root, "bob", None).await;
    check!(burrowland_contract.storage_deposit(&bob));
    check!(nusdt_token_contract.ft_mint(&root, &bob, usdt_bobs_amount / extra_decimals_mult));
    check!(wrap_token_contract.ft_mint(&root, &bob, wnear_bobs_amount));
    check!(nusdc_token_contract.ft_storage_deposit(bob.id()));
    check!(burrowland_contract.deposit(&wrap_token_contract, &bob, wnear_bobs_amount));
    check!(burrowland_contract.deposit(&nusdt_token_contract, &bob, usdt_bobs_amount / extra_decimals_mult));

    let account = burrowland_contract.get_account(&bob).await?.unwrap();
    assert!(find_asset(&account.supplied, wrap_token_contract.0.id()).apr > BigDecimal::zero());
    assert!(find_asset(&account.supplied, nusdt_token_contract.0.id()).apr > BigDecimal::zero());

    // Assuming 2% discount for NEAR at 12$. Paying 49 USDT for 50 USDC.
    let usdt_amount_in = d(49, 18);
    let usdc_amount_out = d(50, 18);
    let current_timestamp = worker.view_block().await?.timestamp();
    check!(burrowland_contract.liquidate(&bob, &oracle_contract, burrowland_contract.0.id(), alice.id(), price_data(current_timestamp, Some(120000)), 
    vec![asset_amount(nusdt_token_contract.0.id(), usdt_amount_in)], vec![asset_amount(nusdc_token_contract.0.id(), usdc_amount_out)], None), "The health factor of liquidation account can't decrease");

    // Assuming ~2% discount for 5 NEAR at 12$. 50 USDT -> ~51 USDC, 4.9 NEAR -> 60 USDC.
    let wnear_amount_in = d(49, 23);
    let usdt_amount_in = d(50, 18);
    let usdc_amount_out = d(111, 18);
    let current_timestamp = worker.view_block().await?.timestamp();
    let outcome = burrowland_contract.liquidate(&bob, &oracle_contract, burrowland_contract.0.id(), alice.id(), price_data(current_timestamp, Some(120000)), 
    vec![asset_amount(wrap_token_contract.0.id(), wnear_amount_in), asset_amount(nusdt_token_contract.0.id(), usdt_amount_in)], vec![asset_amount(nusdc_token_contract.0.id(), usdc_amount_out)], None).await?;

    let logs = outcome.logs();
    let event = &logs[0];
    assert!(event.starts_with(EVENT_JSON));

    let value: serde_json::Value =
        serde_json::from_str(&event[EVENT_JSON.len()..]).expect("Failed to parse the event");
    assert_eq!(value["standard"].as_str().unwrap(), "burrow");
    assert_eq!(value["event"].as_str().unwrap(), "liquidate");
    assert_eq!(
        value["data"][0]["account_id"].as_str().unwrap(),
        bob.id().as_str()
    );
    assert_eq!(
        value["data"][0]["liquidation_account_id"].as_str().unwrap(),
        alice.id().as_str()
    );
    assert_eq!(
        value["data"][0]["collateral_sum"].as_str().unwrap(),
        "111.0"
    );
    assert_eq!(value["data"][0]["repaid_sum"].as_str().unwrap(), "108.8");

    let account = burrowland_contract.get_account(&alice).await?.unwrap();
    assert!(account.supplied.is_empty());
    let nep_postion = account.borrowed.get(&NEP_POSITION.to_string()).unwrap();
    assert!(find_asset(nep_postion, wrap_token_contract.0.id()).apr > BigDecimal::zero());

    let account = burrowland_contract.get_account(&bob).await?.unwrap();
    assert!(find_asset(&account.supplied, &wrap_token_contract.0.id()).apr > BigDecimal::zero());
    // Now APR should be 0, since Bob has liquidated the entire USDT amount
    assert_eq!(
        find_asset(&account.supplied, &nusdt_token_contract.0.id()).apr,
        BigDecimal::zero()
    );
    assert_eq!(
        find_asset(&account.supplied, &nusdc_token_contract.0.id()).apr,
        BigDecimal::zero()
    );
    Ok(())
}

/// Force closing the account with bad debt.
#[tokio::test]
async fn test_force_close() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let nusdc_token_contract = deploy_mock_ft(&root, "nusdc", 18).await?;
    let wrap_token_contract = deploy_mock_ft(&root, "wrap", 18).await?;
    let wrap_reserve_amount = d(10000, 24);
    check!(wrap_token_contract.ft_mint(&root, &root, wrap_reserve_amount));

    let oracle_contract = deploy_oralce(&root).await?;
    let burrowland_contract = deploy_burrowland(&root).await?;
    check!(burrowland_contract.add_asset_handler(&root, &nusdc_token_contract));
    check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));
    check!(nusdc_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(burrowland_contract.deposit_to_reserve(&wrap_token_contract, &root, wrap_reserve_amount));

    let alice = create_account(&root, "alice", None).await;
    check!(burrowland_contract.storage_deposit(&alice));
    let supply_amount = d(1000, 18);
    let extra_decimals_mult = d(1, 12);
    check!(nusdc_token_contract.ft_mint(&root, &alice, supply_amount));
    check!(wrap_token_contract.ft_storage_deposit(alice.id()));

    check!(burrowland_contract.supply_to_collateral(&nusdc_token_contract, &alice, (supply_amount / extra_decimals_mult).into()));

    let borrow_amount = d(50, 24);
    let current_timestamp = worker.view_block().await?.timestamp();
    check!(burrowland_contract.borrow_and_withdraw(&alice, &oracle_contract, burrowland_contract.0.id(), price_data(current_timestamp, Some(100000)), wrap_token_contract.0.id(), borrow_amount));

   // Attempt to force close the account with NEAR at 12$, the account debt is still not bad.
   let bob = create_account(&root, "bob", None).await;
   check!(burrowland_contract.storage_deposit(&bob));
   let current_timestamp = worker.view_block().await?.timestamp();
   check!(burrowland_contract.force_close(&bob, &oracle_contract, alice.id(), price_data(current_timestamp, Some(120000)), None), "is not greater than total collateral");

    // Force closing account with NEAR at 25$.
   let current_timestamp = worker.view_block().await?.timestamp();
   let outcome = burrowland_contract.force_close(&bob, &oracle_contract, alice.id(), price_data(current_timestamp, Some(250000)), None).await?;

    let logs = outcome.logs();
    let event = &logs[0];
    assert!(event.starts_with(EVENT_JSON));

    let value: serde_json::Value =
        serde_json::from_str(&event[EVENT_JSON.len()..]).expect("Failed to parse the event");
    assert_eq!(value["standard"].as_str().unwrap(), "burrow");
    assert_eq!(value["event"].as_str().unwrap(), "force_close");
    assert_eq!(
        value["data"][0]["liquidation_account_id"].as_str().unwrap(),
        alice.id().as_str()
    );
    assert_eq!(
        value["data"][0]["collateral_sum"].as_str().unwrap(),
        "1000.0"
    );
    assert_eq!(value["data"][0]["repaid_sum"].as_str().unwrap()[..6].as_bytes(), "1250.0".as_bytes());

    let account = burrowland_contract.get_account(&alice).await?.unwrap();

    assert!(account.supplied.is_empty());
    assert!(account.collateral.is_empty());
    assert!(account.borrowed.is_empty());
    Ok(())
}