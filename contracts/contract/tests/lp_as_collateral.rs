mod workspace_env;

use crate::workspace_env::*;

#[tokio::test]
async fn test_exchange_boost_farm() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let usdt_token_contract = deploy_mock_ft(&root, "nusdt", 6).await?;
    let usdc_token_contract = deploy_mock_ft(&root, "nusdc", 6).await?;
    let dai_token_contract = deploy_mock_ft(&root, "ndai", 18).await?;
    let wrap_token_contract = deploy_mock_ft(&root, "wrap", 24).await?;
    
    let ref_exchange_contract = deploy_ref_exchange(&root).await?;
    {
        check!(usdt_token_contract.ft_storage_deposit(ref_exchange_contract.0.id()));
        check!(usdc_token_contract.ft_storage_deposit(ref_exchange_contract.0.id()));
        check!(dai_token_contract.ft_storage_deposit(ref_exchange_contract.0.id()));
        check!(ref_exchange_contract.storage_deposit(&root));
        check!(ref_exchange_contract.extend_whitelisted_tokens(&root, vec![usdt_token_contract.0.id(), usdc_token_contract.0.id(), dai_token_contract.0.id()]));
    }
    let boost_farming_contract = deploy_boost_farming(&root).await?;

    let alice = tool_create_account(&root, "alice", None).await;
    check!(ref_exchange_contract.storage_deposit(&alice));
    check!(boost_farming_contract.storage_deposit(&alice));
    let bob = tool_create_account(&root, "bob", None).await;
    check!(ref_exchange_contract.storage_deposit(&bob));
    check!(boost_farming_contract.storage_deposit(&bob));
    
    assert!(usdt_token_contract.ft_mint(&root, &alice, 10000 * 10u128.pow(6)).await?.is_success());
    assert!(usdc_token_contract.ft_mint(&root, &alice, 10000 * 10u128.pow(6)).await?.is_success());
    assert!(dai_token_contract.ft_mint(&root, &alice, 10000 * 10u128.pow(18)).await?.is_success());

    assert!(wrap_token_contract.ft_mint(&root, &alice, 10000 * 10u128.pow(6)).await?.is_success());

    check!(ref_exchange_contract.deposit(&usdt_token_contract, &alice, 10000 * 10u128.pow(6)));
    check!(ref_exchange_contract.deposit(&usdc_token_contract, &alice, 10000 * 10u128.pow(6)));
    check!(ref_exchange_contract.deposit(&dai_token_contract, &alice, 10000 * 10u128.pow(18)));
    
    check!(ref_exchange_contract.add_stable_swap_pool(&root, vec![usdt_token_contract.0.id(), usdc_token_contract.0.id(), dai_token_contract.0.id()], vec![6, 6, 18], 5, 240));
    check!(ref_exchange_contract.add_stable_liquidity(&alice, 0, vec![U128(10000 * 10u128.pow(6)), U128(10000 * 10u128.pow(6)), U128(10000 * 10u128.pow(18))], U128(1)));

    let seed_id = "ref_exchange.test.near@0".to_string();
    check!(boost_farming_contract.create_seed(&root, &seed_id, 18, None, None));
    let seed = boost_farming_contract.get_seed(&seed_id).await?.unwrap();
    assert_eq!(seed.total_seed_amount, 0);
    assert_eq!(seed.total_seed_power, 0);
    assert_eq!(ref_exchange_contract.get_stable_pool(0).await?.shares_total_supply.0, d(30000, 18));
    assert!(ref_exchange_contract.get_shadow_records(&alice).await?.is_empty());
    assert!(boost_farming_contract.get_farmer_seed(&alice, &seed_id).await?.is_none());
    
    check!(print ref_exchange_contract.shadow_farming(&alice, 0, Some(d(30000, 18).into())));
    let seed = boost_farming_contract.get_seed(&seed_id).await?.unwrap();
    assert_eq!(seed.total_seed_amount, d(30000, 18));
    assert_eq!(seed.total_seed_power, d(30000, 18));
    assert_eq!(boost_farming_contract.get_farmer_seed(&alice, &seed_id).await?.unwrap().shadow_amount, d(30000, 18));
    let shadow_record_infos = ref_exchange_contract.get_shadow_records(&alice).await?;
    let shadow_record_info = shadow_record_infos.get(&0).unwrap();
    assert_eq!(shadow_record_info.to_farming_amount.0, d(30000, 18));
    assert_eq!(shadow_record_info.to_burrowland_amount.0, 0);

    check!(logs ref_exchange_contract.shadow_cancel_farming(&alice, 0, Some(d(10000, 18).into())));
    let seed = boost_farming_contract.get_seed(&seed_id).await?.unwrap();
    assert_eq!(seed.total_seed_amount, d(20000, 18));
    assert_eq!(seed.total_seed_power, d(20000, 18));
    assert_eq!(boost_farming_contract.get_farmer_seed(&alice, &seed_id).await?.unwrap().shadow_amount, d(20000, 18));
    let shadow_record_infos = ref_exchange_contract.get_shadow_records(&alice).await?;
    let shadow_record_info = shadow_record_infos.get(&0).unwrap();
    assert_eq!(shadow_record_info.to_farming_amount.0, d(20000, 18));
    assert_eq!(shadow_record_info.to_burrowland_amount.0, 0);
    
    check!(view ref_exchange_contract.get_user_storage_state(&alice));
    check!(logs ref_exchange_contract.shadow_cancel_farming(&alice, 0, None));
    let seed = boost_farming_contract.get_seed(&seed_id).await?.unwrap();
    assert_eq!(seed.total_seed_amount, 0);
    assert_eq!(seed.total_seed_power, 0);
    assert!(boost_farming_contract.get_farmer_seed(&alice, &seed_id).await?.is_none());
    assert!(ref_exchange_contract.get_shadow_records(&alice).await?.is_empty());
    check!(view ref_exchange_contract.get_user_storage_state(&alice));
    
    Ok(())
}

#[tokio::test]
async fn test_exchange_burrowland_boost_farm() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let token_id = "s_v1-0".parse::<AccountId>().unwrap();
    let usdt_token_contract = deploy_mock_ft(&root, "nusdt", 6).await?;
    let usdc_token_contract = deploy_mock_ft(&root, "nusdc", 6).await?;
    let dai_token_contract = deploy_mock_ft(&root, "ndai", 18).await?;
    let wrap_token_contract = deploy_mock_ft(&root, "wrap", 24).await?;
    
    let ref_exchange_contract = deploy_ref_exchange(&root).await?;
    {
        check!(usdt_token_contract.ft_storage_deposit(ref_exchange_contract.0.id()));
        check!(usdc_token_contract.ft_storage_deposit(ref_exchange_contract.0.id()));
        check!(dai_token_contract.ft_storage_deposit(ref_exchange_contract.0.id()));
        check!(ref_exchange_contract.storage_deposit(&root));
        check!(ref_exchange_contract.extend_whitelisted_tokens(&root, vec![usdt_token_contract.0.id(), usdc_token_contract.0.id(), dai_token_contract.0.id()]));
    }
    let burrowland_contract = deploy_burrowland(&root).await?;
    {
        check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
        check!(burrowland_contract.add_asset_handler(&root, &usdt_token_contract));
        check!(burrowland_contract.add_asset_handler(&root, &usdc_token_contract));
        check!(burrowland_contract.add_asset_handler(&root, &dai_token_contract));
        check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));

        check!(burrowland_contract.add_asset(&root, &token_id, AssetConfig{
            reserve_ratio: 2500,
            prot_ratio: 0,
            target_utilization: 8000,
            target_utilization_rate: 1000000000003593629036885046u128.into(),
            max_utilization_rate: 1000000000039724853136740579u128.into(),
            volatility_ratio: 9999,
            extra_decimals: 0,
            can_deposit: false,
            can_withdraw: false,
            can_use_as_collateral: true,
            can_borrow: false,
            net_tvl_multiplier: 10000,
        }));
        check!(wrap_token_contract.ft_mint(&root, &root, parse_near!("10000 N")));
        check!(burrowland_contract.deposit_to_reserve(&wrap_token_contract, &root, parse_near!("10000 N")));
        check!(burrowland_contract.storage_deposit(&root));
    }
    let boost_farming_contract = deploy_boost_farming(&root).await?;

    let alice = tool_create_account(&root, "alice", None).await;
    check!(ref_exchange_contract.storage_deposit(&alice));
    check!(boost_farming_contract.storage_deposit(&alice));
    check!(burrowland_contract.storage_deposit(&alice));
    let bob = tool_create_account(&root, "bob", None).await;
    check!(ref_exchange_contract.storage_deposit(&bob));
    check!(boost_farming_contract.storage_deposit(&bob));
    check!(burrowland_contract.storage_deposit(&bob));
    
    assert!(usdt_token_contract.ft_mint(&root, &alice, 10000 * 10u128.pow(6)).await?.is_success());
    assert!(usdc_token_contract.ft_mint(&root, &alice, 10000 * 10u128.pow(6)).await?.is_success());
    assert!(dai_token_contract.ft_mint(&root, &alice, 10000 * 10u128.pow(18)).await?.is_success());

    assert!(wrap_token_contract.ft_mint(&root, &alice, 10000 * 10u128.pow(6)).await?.is_success());

    check!(ref_exchange_contract.deposit(&usdt_token_contract, &alice, 10000 * 10u128.pow(6)));
    check!(ref_exchange_contract.deposit(&usdc_token_contract, &alice, 10000 * 10u128.pow(6)));
    check!(ref_exchange_contract.deposit(&dai_token_contract, &alice, 10000 * 10u128.pow(18)));
    
    check!(ref_exchange_contract.add_stable_swap_pool(&root, vec![usdt_token_contract.0.id(), usdc_token_contract.0.id(), dai_token_contract.0.id()], vec![6, 6, 18], 5, 240));
    check!(ref_exchange_contract.add_stable_liquidity(&alice, 0, vec![U128(d(10000, 6)), U128(d(10000, 6)), U128(d(10000, 18))], U128(1)));
    
    let seed_id = "ref_exchange.test.near@0".to_string();
    check!(boost_farming_contract.create_seed(&root, &seed_id, 18, None, None));

    check!(ref_exchange_contract.shadow_farming(&alice, 0, None));
    check!(ref_exchange_contract.shadow_burrowland_deposit(&alice, 0, None, None));

    let seed = boost_farming_contract.get_seed(&seed_id).await?.unwrap();
    assert_eq!(seed.total_seed_amount, d(30000, 18));
    assert_eq!(seed.total_seed_power, d(30000, 18));
    assert_eq!(boost_farming_contract.get_farmer_seed(&alice, &seed_id).await?.unwrap().shadow_amount, d(30000, 18));
    let shadow_record_infos = ref_exchange_contract.get_shadow_records(&alice).await?;
    let shadow_record_info = shadow_record_infos.get(&0).unwrap();
    assert_eq!(shadow_record_info.to_farming_amount.0, d(30000, 18));
    assert_eq!(shadow_record_info.to_burrowland_amount.0, d(30000, 18));
    let token_asset = burrowland_contract.get_asset(&token_id).await?;
    assert_eq!(token_asset.supplied.balance, d(30000, 18));
    let alice_burrowland_account = burrowland_contract.get_account(&alice).await?.unwrap();
    assert_eq!(alice_burrowland_account.supplied[0].balance, d(30000, 18));
    assert!(alice_burrowland_account.collateral.is_empty());
    // check!(view "get_user_storage_state" ref_exchange_contract.get_user_storage_state(&alice));
    
    check!(ref_exchange_contract.shadow_cancel_farming(&alice, 0, Some(d(10000, 18).into())));
    check!(ref_exchange_contract.shadow_burrowland_withdraw(&alice, 0, Some(d(10000, 18).into()), None));

    let shadow_record_infos = ref_exchange_contract.get_shadow_records(&alice).await?;
    let shadow_record_info = shadow_record_infos.get(&0).unwrap();
    assert_eq!(shadow_record_info.to_farming_amount.0, d(20000, 18));
    assert_eq!(shadow_record_info.to_burrowland_amount.0, d(20000, 18));
    let token_asset = burrowland_contract.get_asset(&token_id).await?;
    assert_eq!(token_asset.supplied.balance, d(20000, 18));
    let alice_burrowland_account = burrowland_contract.get_account(&alice).await?.unwrap();
    assert_eq!(alice_burrowland_account.supplied[0].balance, d(20000, 18));
    assert!(alice_burrowland_account.collateral.is_empty());

    let msg = serde_json::to_string(&ShadowReceiverMsg::Execute {
        actions: vec![
            Action::IncreaseCollateral(asset_amount(&token_id, d(5000, 18)))
        ]
    }).unwrap();

    check!(ref_exchange_contract.shadow_burrowland_deposit(&alice, 0, Some(d(10000, 18).into()), Some(msg)));
    let token_asset = burrowland_contract.get_asset(&token_id).await?;
    assert_eq!(token_asset.supplied.balance, d(30000, 18));
    let alice_burrowland_account = burrowland_contract.get_account(&alice).await?.unwrap();
    assert_eq!(alice_burrowland_account.supplied[0].balance, d(25000, 18));
    assert_eq!(alice_burrowland_account.collateral[0].balance, d(5000, 18));
    
    let shadow_record_infos = ref_exchange_contract.get_shadow_records(&alice).await?;
    let shadow_record_info = shadow_record_infos.get(&0).unwrap();
    assert_eq!(shadow_record_info.to_farming_amount.0, d(20000, 18));
    assert_eq!(shadow_record_info.to_burrowland_amount.0, d(30000, 18));
    check!(ref_exchange_contract.shadow_burrowland_withdraw(&alice, 0, Some(d(30000, 18).into()), None), "Not enough asset balance");

    let shadow_record_infos = ref_exchange_contract.get_shadow_records(&alice).await?;
    let shadow_record_info = shadow_record_infos.get(&0).unwrap();
    assert_eq!(shadow_record_info.to_farming_amount.0, d(20000, 18));
    assert_eq!(shadow_record_info.to_burrowland_amount.0, d(30000, 18));

    check!(burrowland_contract.sync_ref_exchange_lp_token_infos(&alice, Some(vec![token_id.to_string()])));

    check!(view burrowland_contract.get_last_lp_token_infos());
    Ok(())
}

#[tokio::test]
async fn test_position_liquidate() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let token_id = "s_v1-0".parse::<AccountId>().unwrap();
    let usdt_token_contract = deploy_mock_ft(&root, "nusdt", 6).await?;
    let usdc_token_contract = deploy_mock_ft(&root, "nusdc", 6).await?;
    let dai_token_contract = deploy_mock_ft(&root, "ndai", 18).await?;
    let wrap_token_contract = deploy_mock_ft(&root, "wrap", 24).await?;
    
    let ref_exchange_contract = deploy_ref_exchange(&root).await?;
    {
        check!(usdt_token_contract.ft_storage_deposit(ref_exchange_contract.0.id()));
        check!(usdc_token_contract.ft_storage_deposit(ref_exchange_contract.0.id()));
        check!(dai_token_contract.ft_storage_deposit(ref_exchange_contract.0.id()));
        check!(ref_exchange_contract.storage_deposit(&root));
        check!(ref_exchange_contract.extend_whitelisted_tokens(&root, vec![usdt_token_contract.0.id(), usdc_token_contract.0.id(), dai_token_contract.0.id()]));
    }
    let burrowland_contract = deploy_burrowland(&root).await?;
    {
        check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
        check!(burrowland_contract.add_asset_handler(&root, &usdt_token_contract));
        check!(burrowland_contract.add_asset_handler(&root, &usdc_token_contract));
        check!(burrowland_contract.add_asset_handler(&root, &dai_token_contract));
        check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));

        check!(burrowland_contract.add_asset(&root, &token_id, AssetConfig{
            reserve_ratio: 2500,
            prot_ratio: 0,
            target_utilization: 8000,
            target_utilization_rate: 1000000000003593629036885046u128.into(),
            max_utilization_rate: 1000000000039724853136740579u128.into(),
            volatility_ratio: 9999,
            extra_decimals: 0,
            can_deposit: true,
            can_withdraw: true,
            can_use_as_collateral: true,
            can_borrow: false,
            net_tvl_multiplier: 10000,
        }));
        check!(wrap_token_contract.ft_mint(&root, &root, parse_near!("10000 N")));
        check!(burrowland_contract.deposit_to_reserve(&wrap_token_contract, &root, parse_near!("10000 N")));
        check!(burrowland_contract.storage_deposit(&root));
    }
    let boost_farming_contract = deploy_boost_farming(&root).await?;
    let oralce_contract = deploy_oralce(&root).await?;

    let alice = tool_create_account(&root, "alice", None).await;
    check!(ref_exchange_contract.storage_deposit(&alice));
    check!(boost_farming_contract.storage_deposit(&alice));
    check!(burrowland_contract.storage_deposit(&alice));
    let bob = tool_create_account(&root, "bob", None).await;
    check!(ref_exchange_contract.storage_deposit(&bob));
    check!(boost_farming_contract.storage_deposit(&bob));
    check!(burrowland_contract.storage_deposit(&bob));
    
    assert!(usdt_token_contract.ft_mint(&root, &alice, 10000 * 10u128.pow(6)).await?.is_success());
    assert!(usdc_token_contract.ft_mint(&root, &alice, 10000 * 10u128.pow(6)).await?.is_success());
    assert!(dai_token_contract.ft_mint(&root, &alice, 10000 * 10u128.pow(18)).await?.is_success());

    assert!(wrap_token_contract.ft_mint(&root, &alice, 10000 * 10u128.pow(6)).await?.is_success());

    check!(ref_exchange_contract.deposit(&usdt_token_contract, &alice, 10000 * 10u128.pow(6)));
    check!(ref_exchange_contract.deposit(&usdc_token_contract, &alice, 10000 * 10u128.pow(6)));
    check!(ref_exchange_contract.deposit(&dai_token_contract, &alice, 10000 * 10u128.pow(18)));
    
    check!(ref_exchange_contract.add_stable_swap_pool(&root, vec![usdt_token_contract.0.id(), usdc_token_contract.0.id(), dai_token_contract.0.id()], vec![6, 6, 18], 5, 240));
    check!(ref_exchange_contract.add_stable_liquidity(&alice, 0, vec![U128(10000 * 10u128.pow(6)), U128(10000 * 10u128.pow(6)), U128(10000 * 10u128.pow(18))], U128(1)));
    
    let seed_id = "ref_exchange.test.near@0".to_string();
    check!(boost_farming_contract.create_seed(&root, &seed_id, 18, None, None));

    check!(ref_exchange_contract.shadow_farming(&alice, 0, Some((30000 * 10u128.pow(18)).into())));

    check!(ref_exchange_contract.shadow_burrowland_deposit(&alice, 0, None, None));

    let current_timestamp = worker.view_block().await?.timestamp();
    check!(logs burrowland_contract.sync_ref_exchange_lp_token_infos(&root, Some(vec![token_id.to_string().clone()])));
    check!(burrowland_contract.increase_collateral(&alice, &token_id, 0));
    check!(burrowland_contract.position_borrow_and_withdraw(&alice, &oralce_contract, burrowland_contract.0.id(), 
    price_data(current_timestamp, Some(100000)), Some(token_id.to_string()), wrap_token_contract.0.id(), parse_near!("100 N"), 0));
    
    check!(wrap_token_contract.ft_mint(&root, &bob, parse_near!("10000 N")));
    check!(burrowland_contract.deposit(&wrap_token_contract, &bob, parse_near!("10000 N")));
    check!(ref_exchange_contract.mft_register( &bob, ":0".to_string(), bob.id()));

    let alice_burrowland_account = burrowland_contract.get_account(&alice).await?.unwrap();
    assert_eq!(alice_burrowland_account.collateral[0].balance, d(30000, 18));
    assert_eq!(alice_burrowland_account.collateral[0].token_id.to_string(), token_id.to_string());
    let postion = alice_burrowland_account.borrowed.get(&token_id.to_string()).unwrap();
    assert_eq!(
        (postion[0].balance / d(1, 18)) as f64,
        d(100, 24 - 18) as f64
    );

    let bob_burrowland_account = burrowland_contract.get_account(&bob).await?.unwrap();
    assert_eq!(bob_burrowland_account.supplied[0].token_id.to_string(), wrap_token_contract.0.id().to_string());
    assert_eq!(
        (bob_burrowland_account.supplied[0].balance / d(1, 18)) as f64,
        d(10000, 24 - 18) as f64
    );

    let alice_exchange_account_deposit = ref_exchange_contract.get_deposits(&alice).await?;
    assert_eq!(alice_exchange_account_deposit.get(usdc_token_contract.0.id()).unwrap().0, 0);
    assert_eq!(alice_exchange_account_deposit.get(dai_token_contract.0.id()).unwrap().0, 0);
    assert_eq!(alice_exchange_account_deposit.get(usdt_token_contract.0.id()).unwrap().0, 0);

    assert!(ref_exchange_contract.get_deposits(&bob).await?.is_empty());

    assert_eq!(ref_exchange_contract.get_pool_shares(0, &alice).await?.0, d(30000, 18));
    assert_eq!(ref_exchange_contract.get_pool_shares(0, &bob).await?.0, 0);

    assert_eq!(boost_farming_contract.get_farmer_seed(&alice, &seed_id).await?.unwrap().shadow_amount, d(30000, 18));

    let current_timestamp = worker.view_block().await?.timestamp();
    check!(logs burrowland_contract.liquidate(&bob, &oralce_contract, burrowland_contract.0.id(), alice.id(),
    price_data(current_timestamp, Some(2000000)), vec![asset_amount(wrap_token_contract.0.id(), parse_near!("1 N"))], vec![asset_amount(&token_id, d(13, 18))], Some(token_id.to_string())));

    let alice_burrowland_account = burrowland_contract.get_account(&alice).await?.unwrap();
    assert!(!alice_burrowland_account.is_locked);
    assert_eq!(alice_burrowland_account.collateral[0].balance, d(30000 - 13, 18));
    assert_eq!(alice_burrowland_account.collateral[0].token_id.to_string(), token_id.to_string());
    let postion = alice_burrowland_account.borrowed.get(&token_id.to_string()).unwrap();
    assert_eq!(
        (postion[0].balance / d(1, 18)) as f64,
        d(100 - 1, 24 - 18) as f64
    );

    let bob_burrowland_account = burrowland_contract.get_account(&bob).await?.unwrap();
    assert!(!bob_burrowland_account.is_locked);
    assert_eq!(bob_burrowland_account.supplied[0].token_id.to_string(), wrap_token_contract.0.id().to_string());
    assert_eq!(
        (bob_burrowland_account.supplied[0].balance / d(1, 18)) as f64,
        d(10000 - 1, 24 - 18) as f64
    );

    let alice_exchange_account_deposit = ref_exchange_contract.get_deposits(&alice).await?;
    assert_eq!(alice_exchange_account_deposit.get(usdc_token_contract.0.id()).unwrap().0, 0);
    assert_eq!(alice_exchange_account_deposit.get(dai_token_contract.0.id()).unwrap().0, 0);
    assert_eq!(alice_exchange_account_deposit.get(usdt_token_contract.0.id()).unwrap().0, 0);

    let bob_exchange_account_deposit = ref_exchange_contract.get_deposits(&bob).await?;
    assert_eq!(bob_exchange_account_deposit.get(usdc_token_contract.0.id()).unwrap().0, d(10000, 6) * d(13, 18) / d(30000, 18));//4333333
    assert_eq!(bob_exchange_account_deposit.get(dai_token_contract.0.id()).unwrap().0, (U256::from(d(10000, 18)) * U256::from(d(13, 18)) / U256::from(d(30000, 18))).as_u128());//4333333333333333333
    assert_eq!(bob_exchange_account_deposit.get(usdt_token_contract.0.id()).unwrap().0, d(10000, 6) * d(13, 18) / d(30000, 18));//4333333
    
    assert_eq!(ref_exchange_contract.get_pool_shares(0, &alice).await?.0, d(30000 - 13, 18));
    assert_eq!(ref_exchange_contract.get_pool_shares(0, &bob).await?.0, 0);

    check!(view ref_exchange_contract.get_shadow_records(&alice));
    check!(view ref_exchange_contract.get_shadow_records(&bob));

    assert_eq!(boost_farming_contract.get_farmer_seed(&alice, &seed_id).await?.unwrap().shadow_amount, d(30000 - 13, 18));

    Ok(())
}

#[tokio::test]
async fn test_position_force_close() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let token_id = "s_v1-0".parse::<AccountId>().unwrap();
    let usdt_token_contract = deploy_mock_ft(&root, "nusdt", 6).await?;
    let usdc_token_contract = deploy_mock_ft(&root, "nusdc", 6).await?;
    let dai_token_contract = deploy_mock_ft(&root, "ndai", 18).await?;
    let wrap_token_contract = deploy_mock_ft(&root, "wrap", 24).await?;
    
    let ref_exchange_contract = deploy_ref_exchange(&root).await?;
    {
        check!(usdt_token_contract.ft_storage_deposit(ref_exchange_contract.0.id()));
        check!(usdc_token_contract.ft_storage_deposit(ref_exchange_contract.0.id()));
        check!(dai_token_contract.ft_storage_deposit(ref_exchange_contract.0.id()));
        check!(ref_exchange_contract.storage_deposit(&root));
        check!(ref_exchange_contract.extend_whitelisted_tokens(&root, vec![usdt_token_contract.0.id(), usdc_token_contract.0.id(), dai_token_contract.0.id()]));
    }
    let burrowland_contract = deploy_burrowland(&root).await?;
    {
        check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
        check!(burrowland_contract.add_asset_handler(&root, &usdt_token_contract));
        check!(burrowland_contract.add_asset_handler(&root, &usdc_token_contract));
        check!(burrowland_contract.add_asset_handler(&root, &dai_token_contract));
        check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));

        check!(burrowland_contract.add_asset(&root, &token_id, AssetConfig{
            reserve_ratio: 2500,
            prot_ratio: 0,
            target_utilization: 8000,
            target_utilization_rate: 1000000000003593629036885046u128.into(),
            max_utilization_rate: 1000000000039724853136740579u128.into(),
            volatility_ratio: 9999,
            extra_decimals: 0,
            can_deposit: true,
            can_withdraw: true,
            can_use_as_collateral: true,
            can_borrow: false,
            net_tvl_multiplier: 10000,
        }));
        check!(wrap_token_contract.ft_mint(&root, &root, parse_near!("10000 N")));
        check!(burrowland_contract.deposit_to_reserve(&wrap_token_contract, &root, parse_near!("10000 N")));
        check!(burrowland_contract.storage_deposit(&root));
    }
    let boost_farming_contract = deploy_boost_farming(&root).await?;
    let oralce_contract = deploy_oralce(&root).await?;

    let alice = tool_create_account(&root, "alice", None).await;
    check!(ref_exchange_contract.storage_deposit(&alice));
    check!(boost_farming_contract.storage_deposit(&alice));
    check!(burrowland_contract.storage_deposit(&alice));
    let bob = tool_create_account(&root, "bob", None).await;
    check!(ref_exchange_contract.storage_deposit(&bob));
    check!(boost_farming_contract.storage_deposit(&bob));
    check!(burrowland_contract.storage_deposit(&bob));
    
    assert!(usdt_token_contract.ft_mint(&root, &alice, 10000 * 10u128.pow(6)).await?.is_success());
    assert!(usdc_token_contract.ft_mint(&root, &alice, 10000 * 10u128.pow(6)).await?.is_success());
    assert!(dai_token_contract.ft_mint(&root, &alice, 10000 * 10u128.pow(18)).await?.is_success());
    assert!(usdt_token_contract.ft_mint(&root, &root, 10000 * 10u128.pow(6)).await?.is_success());
    assert!(usdc_token_contract.ft_mint(&root, &root, 10000 * 10u128.pow(6)).await?.is_success());
    assert!(dai_token_contract.ft_mint(&root, &root, 10000 * 10u128.pow(18)).await?.is_success());

    assert!(wrap_token_contract.ft_mint(&root, &alice, 10000 * 10u128.pow(6)).await?.is_success());

    check!(ref_exchange_contract.deposit(&usdt_token_contract, &alice, 10000 * 10u128.pow(6)));
    check!(ref_exchange_contract.deposit(&usdc_token_contract, &alice, 10000 * 10u128.pow(6)));
    check!(ref_exchange_contract.deposit(&dai_token_contract, &alice, 10000 * 10u128.pow(18)));
    check!(ref_exchange_contract.deposit(&usdt_token_contract, &root, 10000 * 10u128.pow(6)));
    check!(ref_exchange_contract.deposit(&usdc_token_contract, &root, 10000 * 10u128.pow(6)));
    check!(ref_exchange_contract.deposit(&dai_token_contract, &root, 10000 * 10u128.pow(18)));
    
    check!(ref_exchange_contract.add_stable_swap_pool(&root, vec![usdt_token_contract.0.id(), usdc_token_contract.0.id(), dai_token_contract.0.id()], vec![6, 6, 18], 5, 240));
    check!(ref_exchange_contract.add_stable_liquidity(&alice, 0, vec![U128(10000 * 10u128.pow(6)), U128(10000 * 10u128.pow(6)), U128(10000 * 10u128.pow(18))], U128(1)));
    check!(ref_exchange_contract.add_stable_liquidity(&root, 0, vec![U128(10000 * 10u128.pow(6)), U128(10000 * 10u128.pow(6)), U128(10000 * 10u128.pow(18))], U128(1)));
    
    let seed_id = "ref_exchange.test.near@0".to_string();
    check!(boost_farming_contract.create_seed(&root, &seed_id, 18, None, None));

    check!(ref_exchange_contract.shadow_farming(&alice, 0, Some((30000 * 10u128.pow(18)).into())));

    check!(ref_exchange_contract.shadow_burrowland_deposit(&alice, 0, None, None));

    let current_timestamp = worker.view_block().await?.timestamp();
    check!(burrowland_contract.sync_ref_exchange_lp_token_infos(&root, Some(vec![token_id.to_string().clone()])));
    check!(burrowland_contract.increase_collateral(&alice, &token_id, 0));
    check!(logs burrowland_contract.position_borrow_and_withdraw(&alice, &oralce_contract, burrowland_contract.0.id(), 
    price_data(current_timestamp, Some(100000)), Some(token_id.to_string()), wrap_token_contract.0.id(), parse_near!("100 N"), 0));
    
    check!(wrap_token_contract.ft_mint(&root, &bob, parse_near!("10000 N")));
    check!(burrowland_contract.deposit(&wrap_token_contract, &bob, parse_near!("10000 N")));
    check!(ref_exchange_contract.mft_register( &bob, ":0".to_string(), bob.id()));

    let alice_burrowland_account = burrowland_contract.get_account(&alice).await?.unwrap();
    assert_eq!(alice_burrowland_account.collateral[0].balance, d(30000, 18));
    assert_eq!(alice_burrowland_account.collateral[0].token_id.to_string(), token_id.to_string());
    let postion = alice_burrowland_account.borrowed.get(&token_id.to_string()).unwrap();
    assert_eq!(
        (postion[0].balance / d(1, 18)) as f64,
        d(100, 24 - 18) as f64
    );

    let bob_burrowland_account = burrowland_contract.get_account(&bob).await?.unwrap();
    assert_eq!(bob_burrowland_account.supplied[0].token_id.to_string(), wrap_token_contract.0.id().to_string());
    assert_eq!(
        (bob_burrowland_account.supplied[0].balance / d(1, 18)) as f64,
        d(10000, 24 - 18) as f64
    );

    let alice_exchange_account_deposit = ref_exchange_contract.get_deposits(&alice).await?;
    assert_eq!(alice_exchange_account_deposit.get(usdc_token_contract.0.id()).unwrap().0, 0);
    assert_eq!(alice_exchange_account_deposit.get(dai_token_contract.0.id()).unwrap().0, 0);
    assert_eq!(alice_exchange_account_deposit.get(usdt_token_contract.0.id()).unwrap().0, 0);

    assert_eq!(ref_exchange_contract.get_pool_shares(0, &alice).await?.0, d(30000, 18));
    assert_eq!(ref_exchange_contract.get_pool_shares(0, &root).await?.0, d(30000, 18));
    assert_eq!(boost_farming_contract.get_farmer_seed(&alice, &seed_id).await?.unwrap().shadow_amount, d(30000, 18));

    let current_timestamp = worker.view_block().await?.timestamp();
    check!(logs burrowland_contract.force_close(&bob, &oralce_contract, alice.id(), price_data(current_timestamp, Some(25000000)), Some(token_id.to_string())));

    let alice_burrowland_account = burrowland_contract.get_account(&alice).await?.unwrap();
    assert!(!alice_burrowland_account.is_locked);
    assert!(alice_burrowland_account.collateral.is_empty());
    assert!(alice_burrowland_account.borrowed.is_empty());

    let alice_exchange_account_deposit = ref_exchange_contract.get_deposits(&alice).await?;
    assert_eq!(alice_exchange_account_deposit.get(usdc_token_contract.0.id()).unwrap().0, 0);
    assert_eq!(alice_exchange_account_deposit.get(dai_token_contract.0.id()).unwrap().0, 0);
    assert_eq!(alice_exchange_account_deposit.get(usdt_token_contract.0.id()).unwrap().0, 0);

    let root_exchange_account_deposit = ref_exchange_contract.get_deposits(&root).await?;
    assert_eq!(root_exchange_account_deposit.get(usdc_token_contract.0.id()).unwrap().0, d(10000, 6));
    assert_eq!(root_exchange_account_deposit.get(dai_token_contract.0.id()).unwrap().0, d(10000, 18));
    assert_eq!(root_exchange_account_deposit.get(usdt_token_contract.0.id()).unwrap().0, d(10000, 6));
    
    assert_eq!(ref_exchange_contract.get_pool_shares(0, &alice).await?.0, 0);
    assert_eq!(ref_exchange_contract.get_pool_shares(0, &root).await?.0, d(30000, 18));

    check!(view ref_exchange_contract.get_shadow_records(&alice));
    check!(view ref_exchange_contract.get_shadow_records(&root));
    assert!(boost_farming_contract.get_farmer_seed(&alice, &seed_id).await?.is_none());

    Ok(())
}

#[tokio::test]
async fn test_position_farming_with_force_close() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let token_id = "s_v1-0".parse::<AccountId>().unwrap();
    let usdt_token_contract = deploy_mock_ft(&root, "nusdt", 6).await?;
    let usdc_token_contract = deploy_mock_ft(&root, "nusdc", 6).await?;
    let dai_token_contract = deploy_mock_ft(&root, "ndai", 18).await?;
    let wrap_token_contract = deploy_mock_ft(&root, "wrap", 24).await?;
    
    let ref_exchange_contract = deploy_ref_exchange(&root).await?;
    {
        check!(usdt_token_contract.ft_storage_deposit(ref_exchange_contract.0.id()));
        check!(usdc_token_contract.ft_storage_deposit(ref_exchange_contract.0.id()));
        check!(dai_token_contract.ft_storage_deposit(ref_exchange_contract.0.id()));
        check!(ref_exchange_contract.storage_deposit(&root));
        check!(ref_exchange_contract.extend_whitelisted_tokens(&root, vec![usdt_token_contract.0.id(), usdc_token_contract.0.id(), dai_token_contract.0.id()]));
    }
    let burrowland_contract = deploy_burrowland(&root).await?;
    {
        check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
        check!(burrowland_contract.add_asset_handler(&root, &usdt_token_contract));
        check!(burrowland_contract.add_asset_handler(&root, &usdc_token_contract));
        check!(burrowland_contract.add_asset_handler(&root, &dai_token_contract));
        check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));

        check!(burrowland_contract.add_asset(&root, &token_id, AssetConfig{
            reserve_ratio: 2500,
            prot_ratio: 0,
            target_utilization: 8000,
            target_utilization_rate: 1000000000003593629036885046u128.into(),
            max_utilization_rate: 1000000000039724853136740579u128.into(),
            volatility_ratio: 9999,
            extra_decimals: 0,
            can_deposit: true,
            can_withdraw: true,
            can_use_as_collateral: true,
            can_borrow: false,
            net_tvl_multiplier: 10000,
        }));
        check!(wrap_token_contract.ft_mint(&root, &root, parse_near!("10000 N")));
        check!(burrowland_contract.deposit_to_reserve(&wrap_token_contract, &root, parse_near!("10000 N")));
        check!(burrowland_contract.storage_deposit(&root));
    }
    let boost_farming_contract = deploy_boost_farming(&root).await?;
    let oralce_contract = deploy_oralce(&root).await?;

    let alice = tool_create_account(&root, "alice", None).await;
    check!(ref_exchange_contract.storage_deposit(&alice));
    check!(boost_farming_contract.storage_deposit(&alice));
    check!(burrowland_contract.storage_deposit(&alice));
    let bob = tool_create_account(&root, "bob", None).await;
    check!(ref_exchange_contract.storage_deposit(&bob));
    check!(boost_farming_contract.storage_deposit(&bob));
    check!(burrowland_contract.storage_deposit(&bob));
    
    assert!(usdt_token_contract.ft_mint(&root, &alice, 10000 * 10u128.pow(6)).await?.is_success());
    assert!(usdc_token_contract.ft_mint(&root, &alice, 10000 * 10u128.pow(6)).await?.is_success());
    assert!(dai_token_contract.ft_mint(&root, &alice, 10000 * 10u128.pow(18)).await?.is_success());
    assert!(usdt_token_contract.ft_mint(&root, &root, 10000 * 10u128.pow(6)).await?.is_success());
    assert!(usdc_token_contract.ft_mint(&root, &root, 10000 * 10u128.pow(6)).await?.is_success());
    assert!(dai_token_contract.ft_mint(&root, &root, 10000 * 10u128.pow(18)).await?.is_success());

    assert!(wrap_token_contract.ft_mint(&root, &alice, 10000 * 10u128.pow(6)).await?.is_success());

    check!(ref_exchange_contract.deposit(&usdt_token_contract, &alice, 10000 * 10u128.pow(6)));
    check!(ref_exchange_contract.deposit(&usdc_token_contract, &alice, 10000 * 10u128.pow(6)));
    check!(ref_exchange_contract.deposit(&dai_token_contract, &alice, 10000 * 10u128.pow(18)));
    check!(ref_exchange_contract.deposit(&usdt_token_contract, &root, 10000 * 10u128.pow(6)));
    check!(ref_exchange_contract.deposit(&usdc_token_contract, &root, 10000 * 10u128.pow(6)));
    check!(ref_exchange_contract.deposit(&dai_token_contract, &root, 10000 * 10u128.pow(18)));
    
    check!(ref_exchange_contract.add_stable_swap_pool(&root, vec![usdt_token_contract.0.id(), usdc_token_contract.0.id(), dai_token_contract.0.id()], vec![6, 6, 18], 5, 240));
    check!(ref_exchange_contract.add_stable_liquidity(&alice, 0, vec![U128(10000 * 10u128.pow(6)), U128(10000 * 10u128.pow(6)), U128(10000 * 10u128.pow(18))], U128(1)));
    check!(ref_exchange_contract.add_stable_liquidity(&root, 0, vec![U128(10000 * 10u128.pow(6)), U128(10000 * 10u128.pow(6)), U128(10000 * 10u128.pow(18))], U128(1)));
    
    check!(ref_exchange_contract.mft_register(&root, ":0".to_string(), boost_farming_contract.0.id()));

    let seed_id = "ref_exchange.test.near@0".to_string();
    check!(boost_farming_contract.create_seed(&root, &seed_id, 18, None, None));

    check!(ref_exchange_contract.shadow_farming(&alice, 0, Some(d(20000,18).into())));
    check!(ref_exchange_contract.shadow_burrowland_deposit(&alice, 0, Some(d(20000,18).into()), None));

    check!(burrowland_contract.sync_ref_exchange_lp_token_infos(&root, Some(vec![token_id.to_string().clone()])));
    check!(burrowland_contract.increase_collateral(&alice, &token_id, 0));
    let current_timestamp = worker.view_block().await?.timestamp();
    check!(burrowland_contract.position_borrow_and_withdraw(&alice, &oralce_contract, burrowland_contract.0.id(), 
    price_data(current_timestamp, Some(100000)), Some(token_id.to_string()), wrap_token_contract.0.id(), parse_near!("100 N"), 0));
    
    check!(boost_farming_contract.stake_free_seed(&alice, &ref_exchange_contract, ":0".to_string(), d(20000, 18)), "Not enough free shares");
    check!(boost_farming_contract.stake_free_seed(&alice, &ref_exchange_contract, ":0".to_string(), d(10000, 18)));

    check!(burrowland_contract.add_asset_farm_reward(&root, FarmId::NetTvl, wrap_token_contract.0.id(), 1000u128.into(), 0u128.into(), 10000000u128.into()));
    check!(burrowland_contract.add_asset_farm_reward(&root, FarmId::Supplied(near_sdk::AccountId::new_unchecked(token_id.to_string())), wrap_token_contract.0.id(), 1000u128.into(), 0u128.into(), 10000000u128.into()));
    check!(burrowland_contract.add_asset_farm_reward(&root, FarmId::Borrowed(near_sdk::AccountId::new_unchecked(wrap_token_contract.0.id().to_string())), wrap_token_contract.0.id(), 1000u128.into(), 0u128.into(), 10000000u128.into()));

    check!(logs burrowland_contract.account_farm_claim_all(&alice, None));

    check!(view burrowland_contract.get_account(&alice));

    let current_timestamp = worker.view_block().await?.timestamp();
    check!(print burrowland_contract.force_close(&bob, &oralce_contract, alice.id(), price_data(current_timestamp, Some(25000000)), Some(token_id.to_string())));

    check!(view burrowland_contract.get_account(&alice));

    Ok(())
}

#[tokio::test]
async fn test_position_farming_liquidate() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let token_id = "s_v1-0".parse::<AccountId>().unwrap();
    let usdt_token_contract = deploy_mock_ft(&root, "nusdt", 6).await?;
    let usdc_token_contract = deploy_mock_ft(&root, "nusdc", 6).await?;
    let dai_token_contract = deploy_mock_ft(&root, "ndai", 18).await?;
    let wrap_token_contract = deploy_mock_ft(&root, "wrap", 24).await?;
    
    let ref_exchange_contract = deploy_ref_exchange(&root).await?;
    {
        check!(usdt_token_contract.ft_storage_deposit(ref_exchange_contract.0.id()));
        check!(usdc_token_contract.ft_storage_deposit(ref_exchange_contract.0.id()));
        check!(dai_token_contract.ft_storage_deposit(ref_exchange_contract.0.id()));
        check!(ref_exchange_contract.storage_deposit(&root));
        check!(ref_exchange_contract.extend_whitelisted_tokens(&root, vec![usdt_token_contract.0.id(), usdc_token_contract.0.id(), dai_token_contract.0.id()]));
    }
    let burrowland_contract = deploy_burrowland(&root).await?;
    {
        check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
        check!(burrowland_contract.add_asset_handler(&root, &usdt_token_contract));
        check!(burrowland_contract.add_asset_handler(&root, &usdc_token_contract));
        check!(burrowland_contract.add_asset_handler(&root, &dai_token_contract));
        check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));

        check!(burrowland_contract.add_asset(&root, &token_id, AssetConfig{
            reserve_ratio: 2500,
            prot_ratio: 0,
            target_utilization: 8000,
            target_utilization_rate: 1000000000003593629036885046u128.into(),
            max_utilization_rate: 1000000000039724853136740579u128.into(),
            volatility_ratio: 9999,
            extra_decimals: 0,
            can_deposit: true,
            can_withdraw: true,
            can_use_as_collateral: true,
            can_borrow: false,
            net_tvl_multiplier: 10000,
        }));
        check!(wrap_token_contract.ft_mint(&root, &root, parse_near!("10000 N")));
        check!(burrowland_contract.deposit_to_reserve(&wrap_token_contract, &root, parse_near!("10000 N")));
        check!(burrowland_contract.storage_deposit(&root));
    }
    let boost_farming_contract = deploy_boost_farming(&root).await?;
    let oralce_contract = deploy_oralce(&root).await?;

    let alice = tool_create_account(&root, "alice", None).await;
    check!(ref_exchange_contract.storage_deposit(&alice));
    check!(boost_farming_contract.storage_deposit(&alice));
    check!(burrowland_contract.storage_deposit(&alice));
    let bob = tool_create_account(&root, "bob", None).await;
    check!(ref_exchange_contract.storage_deposit(&bob));
    check!(boost_farming_contract.storage_deposit(&bob));
    check!(burrowland_contract.storage_deposit(&bob));
    
    assert!(usdt_token_contract.ft_mint(&root, &alice, 10000 * 10u128.pow(6)).await?.is_success());
    assert!(usdc_token_contract.ft_mint(&root, &alice, 10000 * 10u128.pow(6)).await?.is_success());
    assert!(dai_token_contract.ft_mint(&root, &alice, 10000 * 10u128.pow(18)).await?.is_success());
    assert!(usdt_token_contract.ft_mint(&root, &root, 10000 * 10u128.pow(6)).await?.is_success());
    assert!(usdc_token_contract.ft_mint(&root, &root, 10000 * 10u128.pow(6)).await?.is_success());
    assert!(dai_token_contract.ft_mint(&root, &root, 10000 * 10u128.pow(18)).await?.is_success());

    assert!(wrap_token_contract.ft_mint(&root, &alice, 10000 * 10u128.pow(6)).await?.is_success());

    check!(ref_exchange_contract.deposit(&usdt_token_contract, &alice, 10000 * 10u128.pow(6)));
    check!(ref_exchange_contract.deposit(&usdc_token_contract, &alice, 10000 * 10u128.pow(6)));
    check!(ref_exchange_contract.deposit(&dai_token_contract, &alice, 10000 * 10u128.pow(18)));
    check!(ref_exchange_contract.deposit(&usdt_token_contract, &root, 10000 * 10u128.pow(6)));
    check!(ref_exchange_contract.deposit(&usdc_token_contract, &root, 10000 * 10u128.pow(6)));
    check!(ref_exchange_contract.deposit(&dai_token_contract, &root, 10000 * 10u128.pow(18)));
    
    check!(ref_exchange_contract.add_stable_swap_pool(&root, vec![usdt_token_contract.0.id(), usdc_token_contract.0.id(), dai_token_contract.0.id()], vec![6, 6, 18], 5, 240));
    check!(ref_exchange_contract.add_stable_liquidity(&alice, 0, vec![U128(10000 * 10u128.pow(6)), U128(10000 * 10u128.pow(6)), U128(10000 * 10u128.pow(18))], U128(1)));
    check!(ref_exchange_contract.add_stable_liquidity(&root, 0, vec![U128(10000 * 10u128.pow(6)), U128(10000 * 10u128.pow(6)), U128(10000 * 10u128.pow(18))], U128(1)));
    
    check!(ref_exchange_contract.mft_register(&root, ":0".to_string(), boost_farming_contract.0.id()));

    let seed_id = "ref_exchange.test.near@0".to_string();
    check!(boost_farming_contract.create_seed(&root, &seed_id, 18, None, None));

    check!(ref_exchange_contract.shadow_farming(&alice, 0, Some(d(20000,18).into())));
    check!(ref_exchange_contract.shadow_burrowland_deposit(&alice, 0, Some(d(20000,18).into()), None));

    check!(burrowland_contract.sync_ref_exchange_lp_token_infos(&root, Some(vec![token_id.to_string().clone()])));
    check!(burrowland_contract.increase_collateral(&alice, &token_id, 0));
    let current_timestamp = worker.view_block().await?.timestamp();
    check!(burrowland_contract.position_borrow_and_withdraw(&alice, &oralce_contract, burrowland_contract.0.id(), 
    price_data(current_timestamp, Some(100000)), Some(token_id.to_string()), wrap_token_contract.0.id(), parse_near!("100 N"), 0));
    
    check!(boost_farming_contract.stake_free_seed(&alice, &ref_exchange_contract, ":0".to_string(), d(20000, 18)), "Not enough free shares");
    check!(boost_farming_contract.stake_free_seed(&alice, &ref_exchange_contract, ":0".to_string(), d(10000, 18)));

    check!(burrowland_contract.add_asset_farm_reward(&root, FarmId::NetTvl, wrap_token_contract.0.id(), 1000u128.into(), 0u128.into(), 10000000u128.into()));
    check!(burrowland_contract.add_asset_farm_reward(&root, FarmId::Supplied(near_sdk::AccountId::new_unchecked(token_id.to_string())), wrap_token_contract.0.id(), 1000u128.into(), 0u128.into(), 10000000u128.into()));
    check!(burrowland_contract.add_asset_farm_reward(&root, FarmId::Borrowed(near_sdk::AccountId::new_unchecked(wrap_token_contract.0.id().to_string())), wrap_token_contract.0.id(), 1000u128.into(), 0u128.into(), 10000000u128.into()));

    check!(logs burrowland_contract.account_farm_claim_all(&alice, None));

    check!(view burrowland_contract.get_account(&alice));

    check!(wrap_token_contract.ft_mint(&root, &bob, parse_near!("10 N")));
    check!(burrowland_contract.deposit(&wrap_token_contract, &bob, parse_near!("10 N")));
    let current_timestamp = worker.view_block().await?.timestamp();
    check!(print burrowland_contract.liquidate(&bob, &oralce_contract, burrowland_contract.0.id(), alice.id(),
    price_data(current_timestamp, Some(2000000)), vec![asset_amount(wrap_token_contract.0.id(), parse_near!("1 N"))], vec![asset_amount(&token_id, d(13, 18))], Some(token_id.to_string())));

    check!(view burrowland_contract.get_account(&alice));

    Ok(())
}