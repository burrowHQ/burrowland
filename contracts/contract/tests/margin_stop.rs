mod workspace_env;

use mock_boost_farming::nano_to_sec;

use crate::workspace_env::*;

/// Test setting margin stop service fee policy by admin
#[tokio::test]
async fn test_margin_stop_set_mssf() -> Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let nusdt_token_contract = deploy_mock_ft(&root, "nusdt", 18).await?;
    let burrowland_contract = deploy_burrowland_with_pyth(&root).await?;
    check!(burrowland_contract.add_asset_handler(&root, &nusdt_token_contract));
    check!(nusdt_token_contract.ft_storage_deposit(burrowland_contract.0.id()));

    // Initially no service fee policy
    let mssf = burrowland_contract.get_mssf().await?;
    assert!(mssf.is_none());

    // Set service fee policy
    check!(burrowland_contract.set_mssf(&root, MarginStopServiceFee {
        token_id: near_sdk::AccountId::new_unchecked(nusdt_token_contract.0.id().to_string()),
        amount: U128(d(1, 18)), // 1 USDT in inner decimals
    }));

    // Verify policy is set
    let mssf = burrowland_contract.get_mssf().await?;
    assert!(mssf.is_some());
    let mssf = mssf.unwrap();
    assert_eq!(mssf.token_id.to_string(), nusdt_token_contract.0.id().to_string());
    assert_eq!(mssf.amount.0, d(1, 18));

    Ok(())
}

/// Test opening position with stop settings
#[tokio::test]
async fn test_margin_stop_open_position_with_stop() -> Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let pyth_contract = deploy_mock_pyth(&root).await?;
    let nusdt_token_contract = deploy_mock_ft(&root, "nusdt", 18).await?;
    let wrap_token_contract = deploy_mock_ft(&root, "wrap", 18).await?;
    let wrap_reserve_amount = d(10000, 24);
    let nusdt_reserve_amount = d(10000, 6);
    check!(wrap_token_contract.ft_mint(&root, &root, wrap_reserve_amount));
    check!(nusdt_token_contract.ft_mint(&root, &root, nusdt_reserve_amount));

    let ref_exchange_contract = deploy_ref_exchange(&root).await?;
    {
        check!(nusdt_token_contract.ft_storage_deposit(ref_exchange_contract.0.id()));
        check!(wrap_token_contract.ft_storage_deposit(ref_exchange_contract.0.id()));
        check!(ref_exchange_contract.storage_deposit(&root));
        check!(ref_exchange_contract.extend_whitelisted_tokens(&root, vec![nusdt_token_contract.0.id(), wrap_token_contract.0.id()]));
    }

    let burrowland_contract = deploy_burrowland_with_pyth(&root).await?;
    check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));
    check!(burrowland_contract.add_asset_handler(&root, &nusdt_token_contract));
    check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(nusdt_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(burrowland_contract.deposit_to_reserve(&wrap_token_contract, &root, wrap_reserve_amount));
    check!(burrowland_contract.deposit_to_reserve(&nusdt_token_contract, &root, nusdt_reserve_amount));

    // Set service fee policy first
    check!(burrowland_contract.set_mssf(&root, MarginStopServiceFee {
        token_id: near_sdk::AccountId::new_unchecked(nusdt_token_contract.0.id().to_string()),
        amount: U128(d(1, 18)),
    }));

    let alice = create_account(&root, "alice", None).await;
    check!(ref_exchange_contract.storage_deposit(&alice));
    check!(burrowland_contract.storage_deposit(&alice));

    assert!(nusdt_token_contract.ft_mint(&root, &alice, d(10000, 6)).await?.is_success());
    assert!(wrap_token_contract.ft_mint(&root, &alice, d(100000, 24)).await?.is_success());

    check!(ref_exchange_contract.deposit(&nusdt_token_contract, &alice, d(10000, 6)));
    check!(ref_exchange_contract.deposit(&wrap_token_contract, &alice, d(10000, 24)));

    check!(ref_exchange_contract.add_simple_swap_pool(&root, vec![nusdt_token_contract.0.id(), wrap_token_contract.0.id()], 5));
    check!(ref_exchange_contract.add_simple_liquidity(&alice, 0, vec![U128(d(10000, 6)), U128(d(1000, 24))], Some(vec![U128(0), U128(0)])));

    let supply_amount = d(1000, 18);
    let extra_decimals_mult = d(1, 12);
    check!(nusdt_token_contract.ft_mint(&root, &alice, supply_amount * 10));

    // Deposit extra for service fee
    check!(burrowland_contract.deposit_to_margin(&nusdt_token_contract, &alice, supply_amount / extra_decimals_mult + d(10, 6)));

    check!(burrowland_contract.register_margin_dex(&root, ref_exchange_contract.0.id(), 1));
    check!(burrowland_contract.register_margin_token(&root, nusdt_token_contract.0.id(), 0));
    check!(burrowland_contract.register_margin_token(&root, wrap_token_contract.0.id(), 1));

    // Set up pyth prices
    let current_timestamp = worker.view_block().await?.timestamp();
    check!(burrowland_contract.add_token_pyth_info(&root, wrap_token_contract.0.id(), 24, 4, "27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", None, None));
    check!(pyth_contract.set_price("27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", PythPrice{
        price: I64(1000000000),
        conf: U64(278100),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));

    let current_timestamp = worker.view_block().await?.timestamp();
    check!(burrowland_contract.add_token_pyth_info(&root, nusdt_token_contract.0.id(), 6, 4, "1fc18861232290221461220bd4e2acd1dcdfbc89c84092c93c18bdc7756c1588", None, None));
    check!(pyth_contract.set_price("1fc18861232290221461220bd4e2acd1dcdfbc89c84092c93c18bdc7756c1588", PythPrice{
        price: I64(100000000),
        conf: U64(103853),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));

    // Open position with stop settings
    check!(logs burrowland_contract.margin_trading_open_position_with_stop_by_pyth(
        &alice,
        nusdt_token_contract.0.id(), d(1000, 18).into(),
        wrap_token_contract.0.id(), d(20, 24).into(),
        nusdt_token_contract.0.id(), d(180, 18).into(),
        SwapIndication {
            dex_id: near_sdk::AccountId::new_unchecked(ref_exchange_contract.0.id().to_string()),
            swap_action_text: serde_json::to_string(&RefV1TokenReceiverMessage::Execute{
                referral_id: None,
                client_echo: None,
                skip_degen_price_sync: None,
                actions: vec![
                    RefV1Action::Swap(RefV1SwapAction{
                        pool_id: 0,
                        token_in: near_sdk::AccountId::new_unchecked(wrap_token_contract.0.id().to_string()),
                        amount_in: Some(U128(d(20, 24))),
                        token_out: near_sdk::AccountId::new_unchecked(nusdt_token_contract.0.id().to_string()),
                        min_amount_out: U128(d(180, 6)),
                    })
                ]
            }).unwrap()
        },
        Some(12000), // stop_profit: 120%
        Some(9000),  // stop_loss: 90%
    ));

    // Verify position and stops are set
    let alice_margin_account = burrowland_contract.get_margin_account(&alice).await?.unwrap();
    assert_eq!(alice_margin_account.margin_positions.len(), 1);
    assert_eq!(alice_margin_account.stops.len(), 1);

    let pos_id = alice_margin_account.margin_positions.keys().collect::<Vec<&String>>()[0].clone();
    let stop = alice_margin_account.stops.get(&pos_id).unwrap();
    assert_eq!(stop.stop_profit, Some(12000));
    assert_eq!(stop.stop_loss, Some(9000));

    Ok(())
}

/// Test setting stop on existing position
#[tokio::test]
async fn test_margin_stop_set_after_open() -> Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let pyth_contract = deploy_mock_pyth(&root).await?;
    let nusdt_token_contract = deploy_mock_ft(&root, "nusdt", 18).await?;
    let wrap_token_contract = deploy_mock_ft(&root, "wrap", 18).await?;
    let wrap_reserve_amount = d(10000, 24);
    let nusdt_reserve_amount = d(10000, 6);
    check!(wrap_token_contract.ft_mint(&root, &root, wrap_reserve_amount));
    check!(nusdt_token_contract.ft_mint(&root, &root, nusdt_reserve_amount));

    let ref_exchange_contract = deploy_ref_exchange(&root).await?;
    {
        check!(nusdt_token_contract.ft_storage_deposit(ref_exchange_contract.0.id()));
        check!(wrap_token_contract.ft_storage_deposit(ref_exchange_contract.0.id()));
        check!(ref_exchange_contract.storage_deposit(&root));
        check!(ref_exchange_contract.extend_whitelisted_tokens(&root, vec![nusdt_token_contract.0.id(), wrap_token_contract.0.id()]));
    }

    let burrowland_contract = deploy_burrowland_with_pyth(&root).await?;
    check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));
    check!(burrowland_contract.add_asset_handler(&root, &nusdt_token_contract));
    check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(nusdt_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(burrowland_contract.deposit_to_reserve(&wrap_token_contract, &root, wrap_reserve_amount));
    check!(burrowland_contract.deposit_to_reserve(&nusdt_token_contract, &root, nusdt_reserve_amount));

    // Set service fee policy
    check!(burrowland_contract.set_mssf(&root, MarginStopServiceFee {
        token_id: near_sdk::AccountId::new_unchecked(nusdt_token_contract.0.id().to_string()),
        amount: U128(d(1, 18)),
    }));

    let alice = create_account(&root, "alice", None).await;
    check!(ref_exchange_contract.storage_deposit(&alice));
    check!(burrowland_contract.storage_deposit(&alice));

    assert!(nusdt_token_contract.ft_mint(&root, &alice, d(10000, 6)).await?.is_success());
    assert!(wrap_token_contract.ft_mint(&root, &alice, d(100000, 24)).await?.is_success());

    check!(ref_exchange_contract.deposit(&nusdt_token_contract, &alice, d(10000, 6)));
    check!(ref_exchange_contract.deposit(&wrap_token_contract, &alice, d(10000, 24)));

    check!(ref_exchange_contract.add_simple_swap_pool(&root, vec![nusdt_token_contract.0.id(), wrap_token_contract.0.id()], 5));
    check!(ref_exchange_contract.add_simple_liquidity(&alice, 0, vec![U128(d(10000, 6)), U128(d(1000, 24))], Some(vec![U128(0), U128(0)])));

    let supply_amount = d(1000, 18);
    let extra_decimals_mult = d(1, 12);
    check!(nusdt_token_contract.ft_mint(&root, &alice, supply_amount * 10));
    check!(burrowland_contract.deposit_to_margin(&nusdt_token_contract, &alice, supply_amount / extra_decimals_mult + d(10, 6)));

    check!(burrowland_contract.register_margin_dex(&root, ref_exchange_contract.0.id(), 1));
    check!(burrowland_contract.register_margin_token(&root, nusdt_token_contract.0.id(), 0));
    check!(burrowland_contract.register_margin_token(&root, wrap_token_contract.0.id(), 1));

    // Set up pyth prices
    let current_timestamp = worker.view_block().await?.timestamp();
    check!(burrowland_contract.add_token_pyth_info(&root, wrap_token_contract.0.id(), 24, 4, "27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", None, None));
    check!(pyth_contract.set_price("27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", PythPrice{
        price: I64(1000000000),
        conf: U64(278100),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));

    let current_timestamp = worker.view_block().await?.timestamp();
    check!(burrowland_contract.add_token_pyth_info(&root, nusdt_token_contract.0.id(), 6, 4, "1fc18861232290221461220bd4e2acd1dcdfbc89c84092c93c18bdc7756c1588", None, None));
    check!(pyth_contract.set_price("1fc18861232290221461220bd4e2acd1dcdfbc89c84092c93c18bdc7756c1588", PythPrice{
        price: I64(100000000),
        conf: U64(103853),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));

    // Open position without stop
    check!(logs burrowland_contract.margin_trading_open_position_by_pyth(
        &alice,
        nusdt_token_contract.0.id(), d(1000, 18).into(),
        wrap_token_contract.0.id(), d(20, 24).into(),
        nusdt_token_contract.0.id(), d(180, 18).into(),
        SwapIndication {
            dex_id: near_sdk::AccountId::new_unchecked(ref_exchange_contract.0.id().to_string()),
            swap_action_text: serde_json::to_string(&RefV1TokenReceiverMessage::Execute{
                referral_id: None,
                client_echo: None,
                skip_degen_price_sync: None,
                actions: vec![
                    RefV1Action::Swap(RefV1SwapAction{
                        pool_id: 0,
                        token_in: near_sdk::AccountId::new_unchecked(wrap_token_contract.0.id().to_string()),
                        amount_in: Some(U128(d(20, 24))),
                        token_out: near_sdk::AccountId::new_unchecked(nusdt_token_contract.0.id().to_string()),
                        min_amount_out: U128(d(180, 6)),
                    })
                ]
            }).unwrap()
        }
    ));

    // Verify no stops initially
    let alice_margin_account = burrowland_contract.get_margin_account(&alice).await?.unwrap();
    assert_eq!(alice_margin_account.margin_positions.len(), 1);
    assert_eq!(alice_margin_account.stops.len(), 0);

    let pos_id = alice_margin_account.margin_positions.keys().collect::<Vec<&String>>()[0].clone();

    // Set stop after position is open
    check!(burrowland_contract.margin_trading_set_stop_by_pyth(
        &alice,
        &pos_id,
        Some(15000), // stop_profit: 150%
        Some(8000),  // stop_loss: 80%
    ));

    // Verify stop is set
    let alice_margin_account = burrowland_contract.get_margin_account(&alice).await?.unwrap();
    assert_eq!(alice_margin_account.stops.len(), 1);
    let stop = alice_margin_account.stops.get(&pos_id).unwrap();
    assert_eq!(stop.stop_profit, Some(15000));
    assert_eq!(stop.stop_loss, Some(8000));

    Ok(())
}

/// Test removing stop (setting both to None refunds service fee)
#[tokio::test]
async fn test_margin_stop_remove() -> Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let pyth_contract = deploy_mock_pyth(&root).await?;
    let nusdt_token_contract = deploy_mock_ft(&root, "nusdt", 18).await?;
    let wrap_token_contract = deploy_mock_ft(&root, "wrap", 18).await?;
    let wrap_reserve_amount = d(10000, 24);
    let nusdt_reserve_amount = d(10000, 6);
    check!(wrap_token_contract.ft_mint(&root, &root, wrap_reserve_amount));
    check!(nusdt_token_contract.ft_mint(&root, &root, nusdt_reserve_amount));

    let ref_exchange_contract = deploy_ref_exchange(&root).await?;
    {
        check!(nusdt_token_contract.ft_storage_deposit(ref_exchange_contract.0.id()));
        check!(wrap_token_contract.ft_storage_deposit(ref_exchange_contract.0.id()));
        check!(ref_exchange_contract.storage_deposit(&root));
        check!(ref_exchange_contract.extend_whitelisted_tokens(&root, vec![nusdt_token_contract.0.id(), wrap_token_contract.0.id()]));
    }

    let burrowland_contract = deploy_burrowland_with_pyth(&root).await?;
    check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));
    check!(burrowland_contract.add_asset_handler(&root, &nusdt_token_contract));
    check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(nusdt_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(burrowland_contract.deposit_to_reserve(&wrap_token_contract, &root, wrap_reserve_amount));
    check!(burrowland_contract.deposit_to_reserve(&nusdt_token_contract, &root, nusdt_reserve_amount));

    // Set service fee policy: 1 USDT
    let service_fee = d(1, 18);
    check!(burrowland_contract.set_mssf(&root, MarginStopServiceFee {
        token_id: near_sdk::AccountId::new_unchecked(nusdt_token_contract.0.id().to_string()),
        amount: U128(service_fee),
    }));

    let alice = create_account(&root, "alice", None).await;
    check!(ref_exchange_contract.storage_deposit(&alice));
    check!(burrowland_contract.storage_deposit(&alice));

    assert!(nusdt_token_contract.ft_mint(&root, &alice, d(10000, 6)).await?.is_success());
    assert!(wrap_token_contract.ft_mint(&root, &alice, d(100000, 24)).await?.is_success());

    check!(ref_exchange_contract.deposit(&nusdt_token_contract, &alice, d(10000, 6)));
    check!(ref_exchange_contract.deposit(&wrap_token_contract, &alice, d(10000, 24)));

    check!(ref_exchange_contract.add_simple_swap_pool(&root, vec![nusdt_token_contract.0.id(), wrap_token_contract.0.id()], 5));
    check!(ref_exchange_contract.add_simple_liquidity(&alice, 0, vec![U128(d(10000, 6)), U128(d(1000, 24))], Some(vec![U128(0), U128(0)])));

    let supply_amount = d(1000, 18);
    let extra_decimals_mult = d(1, 12);
    check!(nusdt_token_contract.ft_mint(&root, &alice, supply_amount * 10));

    let initial_deposit = supply_amount / extra_decimals_mult + d(10, 6);
    check!(burrowland_contract.deposit_to_margin(&nusdt_token_contract, &alice, initial_deposit));

    check!(burrowland_contract.register_margin_dex(&root, ref_exchange_contract.0.id(), 1));
    check!(burrowland_contract.register_margin_token(&root, nusdt_token_contract.0.id(), 0));
    check!(burrowland_contract.register_margin_token(&root, wrap_token_contract.0.id(), 1));

    // Set up pyth prices
    let current_timestamp = worker.view_block().await?.timestamp();
    check!(burrowland_contract.add_token_pyth_info(&root, wrap_token_contract.0.id(), 24, 4, "27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", None, None));
    check!(pyth_contract.set_price("27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", PythPrice{
        price: I64(1000000000),
        conf: U64(278100),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));

    let current_timestamp = worker.view_block().await?.timestamp();
    check!(burrowland_contract.add_token_pyth_info(&root, nusdt_token_contract.0.id(), 6, 4, "1fc18861232290221461220bd4e2acd1dcdfbc89c84092c93c18bdc7756c1588", None, None));
    check!(pyth_contract.set_price("1fc18861232290221461220bd4e2acd1dcdfbc89c84092c93c18bdc7756c1588", PythPrice{
        price: I64(100000000),
        conf: U64(103853),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));

    // Open position without stop
    check!(logs burrowland_contract.margin_trading_open_position_by_pyth(
        &alice,
        nusdt_token_contract.0.id(), d(1000, 18).into(),
        wrap_token_contract.0.id(), d(20, 24).into(),
        nusdt_token_contract.0.id(), d(180, 18).into(),
        SwapIndication {
            dex_id: near_sdk::AccountId::new_unchecked(ref_exchange_contract.0.id().to_string()),
            swap_action_text: serde_json::to_string(&RefV1TokenReceiverMessage::Execute{
                referral_id: None,
                client_echo: None,
                skip_degen_price_sync: None,
                actions: vec![
                    RefV1Action::Swap(RefV1SwapAction{
                        pool_id: 0,
                        token_in: near_sdk::AccountId::new_unchecked(wrap_token_contract.0.id().to_string()),
                        amount_in: Some(U128(d(20, 24))),
                        token_out: near_sdk::AccountId::new_unchecked(nusdt_token_contract.0.id().to_string()),
                        min_amount_out: U128(d(180, 6)),
                    })
                ]
            }).unwrap()
        }
    ));

    let alice_margin_account = burrowland_contract.get_margin_account(&alice).await?.unwrap();
    let pos_id = alice_margin_account.margin_positions.keys().collect::<Vec<&String>>()[0].clone();
    let supply_before_set = alice_margin_account.supplied.iter()
        .find(|s| s.token_id.to_string() == nusdt_token_contract.0.id().to_string())
        .map(|s| s.balance)
        .unwrap_or(0);

    // Set stop (service fee deducted)
    check!(burrowland_contract.margin_trading_set_stop_by_pyth(
        &alice,
        &pos_id,
        Some(12000),
        Some(9000),
    ));

    let alice_margin_account = burrowland_contract.get_margin_account(&alice).await?.unwrap();
    let supply_after_set = alice_margin_account.supplied.iter()
        .find(|s| s.token_id.to_string() == nusdt_token_contract.0.id().to_string())
        .map(|s| s.balance)
        .unwrap_or(0);

    // Service fee should be deducted
    assert!(supply_before_set > supply_after_set);

    // Remove stop (set both to None)
    check!(burrowland_contract.margin_trading_set_stop_by_pyth(
        &alice,
        &pos_id,
        None,
        None,
    ));

    // Verify stop is removed
    let alice_margin_account = burrowland_contract.get_margin_account(&alice).await?.unwrap();
    assert_eq!(alice_margin_account.stops.len(), 0);

    // Verify service fee is refunded
    let supply_after_remove = alice_margin_account.supplied.iter()
        .find(|s| s.token_id.to_string() == nusdt_token_contract.0.id().to_string())
        .map(|s| s.balance)
        .unwrap_or(0);
    assert_eq!(supply_after_remove, supply_before_set);

    Ok(())
}

/// Test keeper executing stop-loss when price moves against position
#[tokio::test]
async fn test_margin_stop_keeper_executes_stop_loss() -> Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let pyth_contract = deploy_mock_pyth(&root).await?;
    let nusdt_token_contract = deploy_mock_ft(&root, "nusdt", 18).await?;
    let wrap_token_contract = deploy_mock_ft(&root, "wrap", 18).await?;
    let wrap_reserve_amount = d(10000, 24);
    let nusdt_reserve_amount = d(10000, 6);
    check!(wrap_token_contract.ft_mint(&root, &root, wrap_reserve_amount));
    check!(nusdt_token_contract.ft_mint(&root, &root, nusdt_reserve_amount));

    let ref_exchange_contract = deploy_ref_exchange(&root).await?;
    {
        check!(nusdt_token_contract.ft_storage_deposit(ref_exchange_contract.0.id()));
        check!(wrap_token_contract.ft_storage_deposit(ref_exchange_contract.0.id()));
        check!(ref_exchange_contract.storage_deposit(&root));
        check!(ref_exchange_contract.extend_whitelisted_tokens(&root, vec![nusdt_token_contract.0.id(), wrap_token_contract.0.id()]));
    }

    let burrowland_contract = deploy_burrowland_with_pyth(&root).await?;
    // reading-notes: doesn't need to storage_deposit root, cause we are gonna use bob to trigger stop.
    check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));
    check!(burrowland_contract.add_asset_handler(&root, &nusdt_token_contract));
    check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(nusdt_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(burrowland_contract.deposit_to_reserve(&wrap_token_contract, &root, wrap_reserve_amount));
    check!(burrowland_contract.deposit_to_reserve(&nusdt_token_contract, &root, nusdt_reserve_amount));

    // Set service fee policy, inner decimals
    let service_fee = d(1, 18);
    check!(burrowland_contract.set_mssf(&root, MarginStopServiceFee {
        token_id: near_sdk::AccountId::new_unchecked(nusdt_token_contract.0.id().to_string()),
        amount: U128(service_fee),
    }));

    // Create alice (position owner) and bob (keeper)
    let alice = create_account(&root, "alice", None).await;
    let bob = create_account(&root, "bob", None).await;
    check!(ref_exchange_contract.storage_deposit(&alice));
    check!(ref_exchange_contract.storage_deposit(&bob));
    check!(burrowland_contract.storage_deposit(&alice));
    check!(burrowland_contract.storage_deposit(&bob));

    assert!(nusdt_token_contract.ft_mint(&root, &alice, d(10000, 6)).await?.is_success());
    assert!(wrap_token_contract.ft_mint(&root, &alice, d(100000, 24)).await?.is_success());

    check!(ref_exchange_contract.deposit(&nusdt_token_contract, &alice, d(10000, 6)));
    check!(ref_exchange_contract.deposit(&wrap_token_contract, &alice, d(10000, 24)));

    // Create pool with larger liquidity for price stability during swaps
    check!(ref_exchange_contract.add_simple_swap_pool(&root, vec![nusdt_token_contract.0.id(), wrap_token_contract.0.id()], 5));
    check!(ref_exchange_contract.add_simple_liquidity(&alice, 0, vec![U128(d(10000, 6)), U128(d(1000, 24))], Some(vec![U128(0), U128(0)])));
    check!(view ref_exchange_contract.get_pool(0));

    let supply_amount = d(1000, 18);
    let extra_decimals_mult = d(1, 12);
    check!(nusdt_token_contract.ft_mint(&root, &alice, supply_amount * 10));
    // supply additional 10 usdt for stop service fee
    check!(burrowland_contract.deposit_to_margin(&nusdt_token_contract, &alice, supply_amount / extra_decimals_mult + d(10, 6)));

    check!(burrowland_contract.register_margin_dex(&root, ref_exchange_contract.0.id(), 1));
    check!(burrowland_contract.register_margin_token(&root, nusdt_token_contract.0.id(), 0));
    check!(burrowland_contract.register_margin_token(&root, wrap_token_contract.0.id(), 1));

    // Set up initial pyth prices: NEAR = $10, USDT = $1
    let current_timestamp = worker.view_block().await?.timestamp();
    check!(burrowland_contract.add_token_pyth_info(&root, wrap_token_contract.0.id(), 24, 4, "27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", None, None));
    check!(pyth_contract.set_price("27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", PythPrice{
        price: I64(1000000000), // $10
        conf: U64(278100),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));

    let current_timestamp = worker.view_block().await?.timestamp();
    check!(burrowland_contract.add_token_pyth_info(&root, nusdt_token_contract.0.id(), 6, 4, "1fc18861232290221461220bd4e2acd1dcdfbc89c84092c93c18bdc7756c1588", None, None));
    check!(pyth_contract.set_price("1fc18861232290221461220bd4e2acd1dcdfbc89c84092c93c18bdc7756c1588", PythPrice{
        price: I64(100000000), // $1
        conf: U64(103853),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));

    // Alice opens a SHORT position: borrows NEAR (will profit if NEAR price drops)
    // Collateral: 1000 USDT, Borrow: 20 NEAR ($200), Position: ~200 USDT
    // With stop_loss at 9000 BPS (90%), position closes if value drops to 90% of collateral
    check!(logs burrowland_contract.margin_trading_open_position_with_stop_by_pyth(
        &alice,
        nusdt_token_contract.0.id(), d(1000, 18).into(),
        wrap_token_contract.0.id(), d(100, 24).into(),
        nusdt_token_contract.0.id(), d(900, 18).into(),
        SwapIndication {
            dex_id: near_sdk::AccountId::new_unchecked(ref_exchange_contract.0.id().to_string()),
            swap_action_text: serde_json::to_string(&RefV1TokenReceiverMessage::Execute{
                referral_id: None,
                client_echo: None,
                skip_degen_price_sync: None,
                actions: vec![
                    RefV1Action::Swap(RefV1SwapAction{
                        pool_id: 0,
                        token_in: near_sdk::AccountId::new_unchecked(wrap_token_contract.0.id().to_string()),
                        amount_in: Some(U128(d(100, 24))),
                        token_out: near_sdk::AccountId::new_unchecked(nusdt_token_contract.0.id().to_string()),
                        min_amount_out: U128(d(900, 6)),
                    })
                ]
            }).unwrap()
        },
        None,           // no stop_profit
        Some(9000),     // stop_loss at 90% - easy to trigger
    ));

    // move pool price for later deal
    check!(ref_exchange_contract.swap(&wrap_token_contract, &alice, d(600, 24), 0, nusdt_token_contract.0.id()));
    check!(view ref_exchange_contract.get_pool(0));

    // Verify position and stop are set
    let alice_margin_account = burrowland_contract.get_margin_account(&alice).await?.unwrap();
    assert_eq!(alice_margin_account.margin_positions.len(), 1);
    assert_eq!(alice_margin_account.stops.len(), 1);

    let pos_id = alice_margin_account.margin_positions.keys().collect::<Vec<&String>>()[0].clone();
    let position = alice_margin_account.margin_positions.get(&pos_id).unwrap();
    let position_amount = position.token_p_amount;
    let debt_balance = position.token_d_info.balance + 10u128.pow(20);

    // Bob (keeper) has no margin account balance initially
    let bob_margin_account = burrowland_contract.get_margin_account(&bob).await?;
    assert!(bob_margin_account.is_none() || bob_margin_account.unwrap().supplied.is_empty());

    // Simulate NEAR price increasing significantly to trigger stop-loss
    // When NEAR price rises, the debt (in NEAR) becomes more expensive in USD terms
    // This makes the position lose value, triggering stop-loss
    // Update both prices with fresh timestamp
    let stop_timestamp = worker.view_block().await?.timestamp();
    check!(pyth_contract.set_price("27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", PythPrice{
        price: I64(1900000000), // $19 (from $10)
        conf: U64(278100),
        expo: -8,
        publish_time: nano_to_sec(stop_timestamp) as i64,
    }));
    // check!(pyth_contract.set_price("1fc18861232290221461220bd4e2acd1dcdfbc89c84092c93c18bdc7756c1588", PythPrice{
    //     price: I64(100000000), // $1
    //     conf: U64(103853),
    //     expo: -8,
    //     publish_time: nano_to_sec(stop_timestamp) as i64,
    // }));

    // Bob (keeper) executes the stop
    // Need to swap position tokens back to debt token to close position
    check!(print burrowland_contract.margin_trading_stop_mtposition_by_pyth(
        &bob,
        alice.id(),
        &pos_id,
        position_amount,
        debt_balance,
        SwapIndication {
            dex_id: near_sdk::AccountId::new_unchecked(ref_exchange_contract.0.id().to_string()),
            swap_action_text: serde_json::to_string(&RefV1TokenReceiverMessage::Execute{
                referral_id: None,
                client_echo: None,
                skip_degen_price_sync: None,
                actions: vec![
                    RefV1Action::Swap(RefV1SwapAction{
                        pool_id: 0,
                        token_in: near_sdk::AccountId::new_unchecked(nusdt_token_contract.0.id().to_string()),
                        amount_in: Some(U128(position_amount / extra_decimals_mult)),
                        token_out: near_sdk::AccountId::new_unchecked(wrap_token_contract.0.id().to_string()),
                        min_amount_out: U128(debt_balance),
                    })
                ]
            }).unwrap()
        }
    ));

    // Verify position is closed
    let alice_margin_account = burrowland_contract.get_margin_account(&alice).await?.unwrap();
    assert_eq!(alice_margin_account.margin_positions.len(), 0);
    assert_eq!(alice_margin_account.stops.len(), 0);

    // Verify keeper (bob) received service fee
    let bob_margin_account = burrowland_contract.get_margin_account(&bob).await?.unwrap();
    let bob_fee_received = bob_margin_account.supplied.iter()
        .find(|s| s.token_id.to_string() == nusdt_token_contract.0.id().to_string())
        .map(|s| s.balance)
        .unwrap_or(0);
    assert!(bob_fee_received > 0, "Keeper should receive service fee");

    Ok(())
}

/// Test storage accounting across multi-step margin stop lifecycle.
/// Verifies the StorageTracker consume() fix correctly accumulates bytes
/// across open position, set stop, update stop, and remove stop operations.
#[tokio::test]
async fn test_margin_stop_storage_accounting() -> Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let pyth_contract = deploy_mock_pyth(&root).await?;
    let nusdt_token_contract = deploy_mock_ft(&root, "nusdt", 18).await?;
    let wrap_token_contract = deploy_mock_ft(&root, "wrap", 18).await?;
    let wrap_reserve_amount = d(10000, 24);
    let nusdt_reserve_amount = d(10000, 6);
    check!(wrap_token_contract.ft_mint(&root, &root, wrap_reserve_amount));
    check!(nusdt_token_contract.ft_mint(&root, &root, nusdt_reserve_amount));

    let ref_exchange_contract = deploy_ref_exchange(&root).await?;
    {
        check!(nusdt_token_contract.ft_storage_deposit(ref_exchange_contract.0.id()));
        check!(wrap_token_contract.ft_storage_deposit(ref_exchange_contract.0.id()));
        check!(ref_exchange_contract.storage_deposit(&root));
        check!(ref_exchange_contract.extend_whitelisted_tokens(&root, vec![nusdt_token_contract.0.id(), wrap_token_contract.0.id()]));
    }

    let burrowland_contract = deploy_burrowland_with_pyth(&root).await?;
    check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));
    check!(burrowland_contract.add_asset_handler(&root, &nusdt_token_contract));
    check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(nusdt_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(burrowland_contract.deposit_to_reserve(&wrap_token_contract, &root, wrap_reserve_amount));
    check!(burrowland_contract.deposit_to_reserve(&nusdt_token_contract, &root, nusdt_reserve_amount));

    // Set service fee policy
    check!(burrowland_contract.set_mssf(&root, MarginStopServiceFee {
        token_id: near_sdk::AccountId::new_unchecked(nusdt_token_contract.0.id().to_string()),
        amount: U128(d(1, 18)),
    }));

    let alice = create_account(&root, "alice", None).await;
    check!(ref_exchange_contract.storage_deposit(&alice));
    check!(burrowland_contract.storage_deposit(&alice));

    assert!(nusdt_token_contract.ft_mint(&root, &alice, d(10000, 6)).await?.is_success());
    assert!(wrap_token_contract.ft_mint(&root, &alice, d(100000, 24)).await?.is_success());

    check!(ref_exchange_contract.deposit(&nusdt_token_contract, &alice, d(10000, 6)));
    check!(ref_exchange_contract.deposit(&wrap_token_contract, &alice, d(10000, 24)));

    check!(ref_exchange_contract.add_simple_swap_pool(&root, vec![nusdt_token_contract.0.id(), wrap_token_contract.0.id()], 5));
    check!(ref_exchange_contract.add_simple_liquidity(&alice, 0, vec![U128(d(10000, 6)), U128(d(1000, 24))], Some(vec![U128(0), U128(0)])));

    let supply_amount = d(1000, 18);
    let extra_decimals_mult = d(1, 12);
    check!(nusdt_token_contract.ft_mint(&root, &alice, supply_amount * 10));
    check!(burrowland_contract.deposit_to_margin(&nusdt_token_contract, &alice, supply_amount / extra_decimals_mult + d(10, 6)));

    check!(burrowland_contract.register_margin_dex(&root, ref_exchange_contract.0.id(), 1));
    check!(burrowland_contract.register_margin_token(&root, nusdt_token_contract.0.id(), 0));
    check!(burrowland_contract.register_margin_token(&root, wrap_token_contract.0.id(), 1));

    // Set up pyth prices
    let current_timestamp = worker.view_block().await?.timestamp();
    check!(burrowland_contract.add_token_pyth_info(&root, wrap_token_contract.0.id(), 24, 4, "27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", None, None));
    check!(pyth_contract.set_price("27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", PythPrice{
        price: I64(1000000000),
        conf: U64(278100),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));

    let current_timestamp = worker.view_block().await?.timestamp();
    check!(burrowland_contract.add_token_pyth_info(&root, nusdt_token_contract.0.id(), 6, 4, "1fc18861232290221461220bd4e2acd1dcdfbc89c84092c93c18bdc7756c1588", None, None));
    check!(pyth_contract.set_price("1fc18861232290221461220bd4e2acd1dcdfbc89c84092c93c18bdc7756c1588", PythPrice{
        price: I64(100000000),
        conf: U64(103853),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));

    // ---- Step 1: Record storage before opening position ----
    let storage_before_open = burrowland_contract.get_storage_balance_of_detail(&alice).await?.unwrap();

    // Open position WITHOUT stop
    check!(logs burrowland_contract.margin_trading_open_position_by_pyth(
        &alice,
        nusdt_token_contract.0.id(), d(1000, 18).into(),
        wrap_token_contract.0.id(), d(20, 24).into(),
        nusdt_token_contract.0.id(), d(180, 18).into(),
        SwapIndication {
            dex_id: near_sdk::AccountId::new_unchecked(ref_exchange_contract.0.id().to_string()),
            swap_action_text: serde_json::to_string(&RefV1TokenReceiverMessage::Execute{
                referral_id: None,
                client_echo: None,
                skip_degen_price_sync: None,
                actions: vec![
                    RefV1Action::Swap(RefV1SwapAction{
                        pool_id: 0,
                        token_in: near_sdk::AccountId::new_unchecked(wrap_token_contract.0.id().to_string()),
                        amount_in: Some(U128(d(20, 24))),
                        token_out: near_sdk::AccountId::new_unchecked(nusdt_token_contract.0.id().to_string()),
                        min_amount_out: U128(d(180, 6)),
                    })
                ]
            }).unwrap()
        }
    ));

    let storage_after_open = burrowland_contract.get_storage_balance_of_detail(&alice).await?.unwrap();
    assert!(
        storage_after_open.used_amount.0 > storage_before_open.used_amount.0,
        "Opening a position should increase storage usage"
    );

    let alice_margin_account = burrowland_contract.get_margin_account(&alice).await?.unwrap();
    let pos_id = alice_margin_account.margin_positions.keys().collect::<Vec<&String>>()[0].clone();

    // ---- Step 2: Set stop → storage should increase ----
    let storage_before_set = burrowland_contract.get_storage_balance_of_detail(&alice).await?.unwrap();

    check!(burrowland_contract.margin_trading_set_stop_by_pyth(
        &alice,
        &pos_id,
        Some(12000),
        Some(9000),
    ));

    let storage_after_set = burrowland_contract.get_storage_balance_of_detail(&alice).await?.unwrap();
    assert!(
        storage_after_set.used_amount.0 > storage_before_set.used_amount.0,
        "Setting a stop should increase storage usage (stop entry added)"
    );

    // ---- Step 3: Update stop → storage should remain similar ----
    let storage_before_update = burrowland_contract.get_storage_balance_of_detail(&alice).await?.unwrap();

    check!(burrowland_contract.margin_trading_set_stop_by_pyth(
        &alice,
        &pos_id,
        Some(15000),
        Some(8000),
    ));

    let storage_after_update = burrowland_contract.get_storage_balance_of_detail(&alice).await?.unwrap();
    // Updating a stop replaces the entry; storage should remain the same
    assert_eq!(
        storage_after_update.used_amount.0, storage_before_update.used_amount.0,
        "Updating a stop should not change storage usage"
    );

    // Verify stop values actually changed
    let alice_margin_account = burrowland_contract.get_margin_account(&alice).await?.unwrap();
    let stop = alice_margin_account.stops.get(&pos_id).unwrap();
    assert_eq!(stop.stop_profit, Some(15000));
    assert_eq!(stop.stop_loss, Some(8000));

    // ---- Step 4: Remove stop → storage should decrease back ----
    check!(burrowland_contract.margin_trading_set_stop_by_pyth(
        &alice,
        &pos_id,
        None,
        None,
    ));

    let storage_after_remove = burrowland_contract.get_storage_balance_of_detail(&alice).await?.unwrap();
    assert_eq!(
        storage_after_remove.used_amount.0, storage_before_set.used_amount.0,
        "Removing a stop should return storage to pre-set level"
    );

    // ---- Step 5: No panics from storage tracker throughout ----
    // The fact that we got here without any "Bug, non-tracked storage change" panic
    // confirms that StorageTracker.consume() correctly accumulated bytes_released
    // across the multiple internal_set_margin_account() calls in each step.

    Ok(())
}
