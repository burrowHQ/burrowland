mod workspace_env;

use mock_boost_farming::nano_to_sec;

use crate::workspace_env::*;

#[tokio::test]
async fn test_margin_trading() -> Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;

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

    let oracle_contract = deploy_oralce(&root).await?;
    let burrowland_contract = deploy_burrowland_with_price_oracle(&root).await?;
    check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));
    check!(burrowland_contract.add_asset_handler(&root, &nusdt_token_contract));
    check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(nusdt_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(burrowland_contract.deposit_to_reserve(&wrap_token_contract, &root, wrap_reserve_amount));
    check!(burrowland_contract.deposit_to_reserve(&nusdt_token_contract, &root, nusdt_reserve_amount));

    let alice = create_account(&root, "alice", None).await;
    check!(ref_exchange_contract.storage_deposit(&alice));
    check!(burrowland_contract.storage_deposit(&alice));

    assert!(nusdt_token_contract.ft_mint(&root, &alice, d(10000, 6)).await?.is_success());
    assert!(wrap_token_contract.ft_mint(&root, &alice, d(100000, 24)).await?.is_success());

    check!(ref_exchange_contract.deposit(&nusdt_token_contract, &alice, d(10000, 6)));
    check!(ref_exchange_contract.deposit(&wrap_token_contract, &alice, d(10000, 24)));

    check!(ref_exchange_contract.add_simple_swap_pool(&root, vec![nusdt_token_contract.0.id(), wrap_token_contract.0.id()], 5));
    check!(ref_exchange_contract.add_simple_liquidity(&alice, 0, vec![U128(d(10000, 6)), U128(d(1000, 24))], Some(vec![U128(0), U128(0)])));

    check!(view ref_exchange_contract.get_pool(0));

    let supply_amount = d(1000, 18);
    let extra_decimals_mult = d(1, 12);
    check!(nusdt_token_contract.ft_mint(&root, &alice, supply_amount * 10));
    check!(wrap_token_contract.ft_storage_deposit(alice.id()));

    check!(view burrowland_contract.get_margin_account(&alice));

    check!(burrowland_contract.deposit_to_margin(&nusdt_token_contract, &alice, supply_amount / extra_decimals_mult));
    
    check!(burrowland_contract.register_margin_dex(&root, ref_exchange_contract.0.id(), 1));
    check!(burrowland_contract.register_margin_token(&root, nusdt_token_contract.0.id(), 0));
    check!(burrowland_contract.register_margin_token(&root, wrap_token_contract.0.id(), 1));

    let current_timestamp = worker.view_block().await?.timestamp();
    check!(logs burrowland_contract.margin_trading_open_position_by_oracle_call(
        &oracle_contract, price_data(current_timestamp, Some(100000)), &alice,
        nusdt_token_contract.0.id(), d(1000, 18).into(), wrap_token_contract.0.id(), d(20, 24).into(), nusdt_token_contract.0.id(), d(180, 18).into(),
        SwapIndication {
            dex_id: near_sdk::AccountId::new_unchecked(ref_exchange_contract.0.id().to_string()),
            swap_action_text: serde_json::to_string(&RefV1TokenReceiverMessage::Execute{
                referral_id: None,
                client_echo: None,
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

    check!(view burrowland_contract.get_margin_account(&alice));

    let mut alice_margin_account = burrowland_contract.get_margin_account(&alice).await?.unwrap();
    let pos_id = alice_margin_account.margin_positions.keys().collect::<Vec<&String>>()[0].clone();

    check!(burrowland_contract.margin_trading_increase_collateral_by_ft_transfer_call(&nusdt_token_contract, &alice, supply_amount / extra_decimals_mult, &pos_id, supply_amount));
    alice_margin_account = burrowland_contract.get_margin_account(&alice).await?.unwrap();
    assert!(alice_margin_account.supplied.is_empty());
    assert_eq!(alice_margin_account.margin_positions.get(&pos_id).unwrap().token_c_info.balance, d(2000, 18));

    check!(burrowland_contract.margin_trading_decrease_collateral_by_oracle_call(&oracle_contract, price_data(current_timestamp, Some(100000)), &alice, &pos_id, supply_amount));
    alice_margin_account = burrowland_contract.get_margin_account(&alice).await?.unwrap();
    println!("{:?}", alice_margin_account);
    assert_eq!(alice_margin_account.supplied[0].balance, d(1000, 18));
    assert_eq!(alice_margin_account.margin_positions.get(&pos_id).unwrap().token_c_info.balance, d(1000, 18));

    let current_timestamp = worker.view_block().await?.timestamp();
    check!(logs burrowland_contract.margin_trading_decrease_mtposition_by_oracle_call(
        &oracle_contract, price_data(current_timestamp, Some(100000)), &alice,
        &pos_id, d(100, 18), d(10, 24), 
        SwapIndication {
            dex_id: near_sdk::AccountId::new_unchecked(ref_exchange_contract.0.id().to_string()),
            swap_action_text: serde_json::to_string(&RefV1TokenReceiverMessage::Execute{
                referral_id: None,
                client_echo: None,
                actions: vec![
                    RefV1Action::Swap(RefV1SwapAction{
                        pool_id: 0,
                        token_in: near_sdk::AccountId::new_unchecked(nusdt_token_contract.0.id().to_string()),
                        amount_in: Some(U128(d(100, 6))),
                        token_out: near_sdk::AccountId::new_unchecked(wrap_token_contract.0.id().to_string()),
                        min_amount_out: U128(d(10, 24)),
                    })
                ]
            }).unwrap()
        }
    ));

    check!(view burrowland_contract.get_margin_account(&alice));

    check!(ref_exchange_contract.swap(&wrap_token_contract, &alice, d(100, 24), 0, nusdt_token_contract.0.id()));

    alice_margin_account = burrowland_contract.get_margin_account(&alice).await?.unwrap();

    let current_timestamp = worker.view_block().await?.timestamp();
    let position_amount = alice_margin_account.margin_positions.get(&pos_id).unwrap().token_p_amount;
    let min_out_amount = alice_margin_account.margin_positions.get(&pos_id).unwrap().token_d_info.balance + 10u128.pow(20);
    check!(print burrowland_contract.margin_trading_close_mtposition_by_oracle_call(
        &oracle_contract, price_data(current_timestamp, Some(100000)), &alice,
        &pos_id, position_amount, min_out_amount, 
        SwapIndication {
            dex_id: near_sdk::AccountId::new_unchecked(ref_exchange_contract.0.id().to_string()),
            swap_action_text: serde_json::to_string(&RefV1TokenReceiverMessage::Execute{
                referral_id: None,
                client_echo: None,
                actions: vec![
                    RefV1Action::Swap(RefV1SwapAction{
                        pool_id: 0,
                        token_in: near_sdk::AccountId::new_unchecked(nusdt_token_contract.0.id().to_string()),
                        amount_in: Some(U128(position_amount / 10u128.pow(12))),
                        token_out: near_sdk::AccountId::new_unchecked(wrap_token_contract.0.id().to_string()),
                        min_amount_out: U128(min_out_amount),
                    })
                ]
            }).unwrap()
        }
    ));

    // check!(view burrowland_contract.get_margin_account(&alice));
    // check!(burrowland_contract.margin_trading_withdraw(&alice, nusdt_token_contract.0.id(), None));
    // check!(view burrowland_contract.get_margin_account(&alice));
    Ok(())
}

#[tokio::test]
async fn test_margin_trading_with_pyth() -> Result<()> {
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

    let alice = create_account(&root, "alice", None).await;
    check!(ref_exchange_contract.storage_deposit(&alice));
    check!(burrowland_contract.storage_deposit(&alice));

    assert!(nusdt_token_contract.ft_mint(&root, &alice, d(10000, 6)).await?.is_success());
    assert!(wrap_token_contract.ft_mint(&root, &alice, d(100000, 24)).await?.is_success());

    check!(ref_exchange_contract.deposit(&nusdt_token_contract, &alice, d(10000, 6)));
    check!(ref_exchange_contract.deposit(&wrap_token_contract, &alice, d(10000, 24)));

    check!(ref_exchange_contract.add_simple_swap_pool(&root, vec![nusdt_token_contract.0.id(), wrap_token_contract.0.id()], 5));
    check!(ref_exchange_contract.add_simple_liquidity(&alice, 0, vec![U128(d(10000, 6)), U128(d(1000, 24))], Some(vec![U128(0), U128(0)])));

    check!(view ref_exchange_contract.get_pool(0));

    let supply_amount = d(1000, 18);
    let extra_decimals_mult = d(1, 12);
    check!(nusdt_token_contract.ft_mint(&root, &alice, supply_amount * 10));
    check!(wrap_token_contract.ft_storage_deposit(alice.id()));

    check!(view burrowland_contract.get_margin_account(&alice));

    check!(burrowland_contract.deposit_to_margin(&nusdt_token_contract, &alice, supply_amount / extra_decimals_mult));
    
    check!(burrowland_contract.register_margin_dex(&root, ref_exchange_contract.0.id(), 1));
    check!(burrowland_contract.register_margin_token(&root, nusdt_token_contract.0.id(), 0));
    check!(burrowland_contract.register_margin_token(&root, wrap_token_contract.0.id(), 1));

    // near
    let current_timestamp = worker.view_block().await?.timestamp();
    check!(burrowland_contract.add_token_pyth_info(&root, wrap_token_contract.0.id(), 24, 4, "27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", None, None));
    check!(pyth_contract.set_price("27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", PythPrice{
        price: I64(1000000000),
        conf: U64(278100),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));
    check!(view pyth_contract.get_price("27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4"));

    // usdt
    let current_timestamp = worker.view_block().await?.timestamp();
    check!(burrowland_contract.add_token_pyth_info(&root, nusdt_token_contract.0.id(), 6, 4, "1fc18861232290221461220bd4e2acd1dcdfbc89c84092c93c18bdc7756c1588", None, None));
    check!(pyth_contract.set_price("1fc18861232290221461220bd4e2acd1dcdfbc89c84092c93c18bdc7756c1588", PythPrice{
        price: I64(100000000),
        conf: U64(103853),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));
    check!(view pyth_contract.get_price("1fc18861232290221461220bd4e2acd1dcdfbc89c84092c93c18bdc7756c1588"));

    check!(logs burrowland_contract.margin_trading_open_position_by_pyth(&alice,
        nusdt_token_contract.0.id(), d(1000, 18).into(), wrap_token_contract.0.id(), d(20, 24).into(), nusdt_token_contract.0.id(), d(180, 18).into(),
        SwapIndication {
            dex_id: near_sdk::AccountId::new_unchecked(ref_exchange_contract.0.id().to_string()),
            swap_action_text: serde_json::to_string(&RefV1TokenReceiverMessage::Execute{
                referral_id: None,
                client_echo: None,
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

    check!(view burrowland_contract.get_margin_account(&alice));

    let mut alice_margin_account = burrowland_contract.get_margin_account(&alice).await?.unwrap();
    let pos_id = alice_margin_account.margin_positions.keys().collect::<Vec<&String>>()[0].clone();

    check!(burrowland_contract.margin_trading_increase_collateral_by_ft_transfer_call(&nusdt_token_contract, &alice, supply_amount / extra_decimals_mult, &pos_id, supply_amount));
    alice_margin_account = burrowland_contract.get_margin_account(&alice).await?.unwrap();
    assert!(alice_margin_account.supplied.is_empty());
    assert_eq!(alice_margin_account.margin_positions.get(&pos_id).unwrap().token_c_info.balance, d(2000, 18));

    check!(burrowland_contract.margin_trading_decrease_collateral_by_pyth(&alice, &pos_id, supply_amount));
    alice_margin_account = burrowland_contract.get_margin_account(&alice).await?.unwrap();
    println!("{:?}", alice_margin_account);
    assert_eq!(alice_margin_account.supplied[0].balance, d(1000, 18));
    assert_eq!(alice_margin_account.margin_positions.get(&pos_id).unwrap().token_c_info.balance, d(1000, 18));

    check!(logs burrowland_contract.margin_trading_decrease_mtposition_by_pyth(&alice,
        &pos_id, d(100, 18), d(10, 24), 
        SwapIndication {
            dex_id: near_sdk::AccountId::new_unchecked(ref_exchange_contract.0.id().to_string()),
            swap_action_text: serde_json::to_string(&RefV1TokenReceiverMessage::Execute{
                referral_id: None,
                client_echo: None,
                actions: vec![
                    RefV1Action::Swap(RefV1SwapAction{
                        pool_id: 0,
                        token_in: near_sdk::AccountId::new_unchecked(nusdt_token_contract.0.id().to_string()),
                        amount_in: Some(U128(d(100, 6))),
                        token_out: near_sdk::AccountId::new_unchecked(wrap_token_contract.0.id().to_string()),
                        min_amount_out: U128(d(10, 24)),
                    })
                ]
            }).unwrap()
        }
    ));

    check!(view burrowland_contract.get_margin_account(&alice));

    check!(ref_exchange_contract.swap(&wrap_token_contract, &alice, d(100, 24), 0, nusdt_token_contract.0.id()));

    alice_margin_account = burrowland_contract.get_margin_account(&alice).await?.unwrap();

    let position_amount = alice_margin_account.margin_positions.get(&pos_id).unwrap().token_p_amount;
    let min_out_amount = alice_margin_account.margin_positions.get(&pos_id).unwrap().token_d_info.balance + 10u128.pow(20);
    check!(logs burrowland_contract.margin_trading_close_mtposition_by_pyth(&alice,
        &pos_id, position_amount, min_out_amount, 
        SwapIndication {
            dex_id: near_sdk::AccountId::new_unchecked(ref_exchange_contract.0.id().to_string()),
            swap_action_text: serde_json::to_string(&RefV1TokenReceiverMessage::Execute{
                referral_id: None,
                client_echo: None,
                actions: vec![
                    RefV1Action::Swap(RefV1SwapAction{
                        pool_id: 0,
                        token_in: near_sdk::AccountId::new_unchecked(nusdt_token_contract.0.id().to_string()),
                        amount_in: Some(U128(position_amount / 10u128.pow(12))),
                        token_out: near_sdk::AccountId::new_unchecked(wrap_token_contract.0.id().to_string()),
                        min_amount_out: U128(min_out_amount),
                    })
                ]
            }).unwrap()
        }
    ));

    check!(view burrowland_contract.get_margin_account(&alice));

    Ok(())
}

#[tokio::test]
async fn test_margin_trading_liquidate_direct_short() -> Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let nusdt_token_contract = deploy_mock_ft(&root, "nusdt", 18).await?;
    let wrap_token_contract = deploy_mock_ft(&root, "wrap", 18).await?;
    let wrap_reserve_amount = d(10000, 24);
    let wrap_collateral_amount = d(10000, 24);
    let nusdt_reserve_amount = d(10000, 6);
    check!(wrap_token_contract.ft_mint(&root, &root, wrap_reserve_amount + wrap_collateral_amount));
    check!(nusdt_token_contract.ft_mint(&root, &root, nusdt_reserve_amount));

    let ref_exchange_contract = deploy_ref_exchange(&root).await?;
    {
        check!(nusdt_token_contract.ft_storage_deposit(ref_exchange_contract.0.id()));
        check!(wrap_token_contract.ft_storage_deposit(ref_exchange_contract.0.id()));
        check!(ref_exchange_contract.storage_deposit(&root));
        check!(ref_exchange_contract.extend_whitelisted_tokens(&root, vec![nusdt_token_contract.0.id(), wrap_token_contract.0.id()]));
    }

    let oracle_contract = deploy_oralce(&root).await?;
    let burrowland_contract = deploy_burrowland_with_price_oracle(&root).await?;
    check!(burrowland_contract.storage_deposit(&root));
    check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));
    check!(burrowland_contract.add_asset_handler(&root, &nusdt_token_contract));
    check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(nusdt_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(burrowland_contract.deposit_to_reserve(&wrap_token_contract, &root, wrap_reserve_amount));
    check!(burrowland_contract.deposit_to_reserve(&nusdt_token_contract, &root, nusdt_reserve_amount));
    check!(burrowland_contract.supply_to_collateral(&wrap_token_contract, &root, wrap_collateral_amount));

    let alice = create_account(&root, "alice", None).await;
    check!(ref_exchange_contract.storage_deposit(&alice));
    check!(burrowland_contract.storage_deposit(&alice));

    assert!(nusdt_token_contract.ft_mint(&root, &alice, d(10000, 6)).await?.is_success());
    assert!(wrap_token_contract.ft_mint(&root, &alice, d(200000, 24)).await?.is_success());

    check!(ref_exchange_contract.deposit(&nusdt_token_contract, &alice, d(10000, 6)));
    check!(ref_exchange_contract.deposit(&wrap_token_contract, &alice, d(10000, 24)));

    check!(ref_exchange_contract.add_simple_swap_pool(&root, vec![nusdt_token_contract.0.id(), wrap_token_contract.0.id()], 5));
    check!(ref_exchange_contract.add_simple_liquidity(&alice, 0, vec![U128(d(10000, 6)), U128(d(1000, 24))], Some(vec![U128(0), U128(0)])));

    check!(view ref_exchange_contract.get_pool(0));

    let supply_amount = d(1000, 18);
    let extra_decimals_mult = d(1, 12);
    check!(nusdt_token_contract.ft_mint(&root, &alice, supply_amount * 10));
    check!(wrap_token_contract.ft_storage_deposit(alice.id()));

    check!(view burrowland_contract.get_margin_account(&alice));

    check!(burrowland_contract.deposit_to_margin(&nusdt_token_contract, &alice, supply_amount / extra_decimals_mult));
    
    check!(burrowland_contract.register_margin_dex(&root, ref_exchange_contract.0.id(), 1));
    check!(burrowland_contract.register_margin_token(&root, nusdt_token_contract.0.id(), 0));
    check!(burrowland_contract.register_margin_token(&root, wrap_token_contract.0.id(), 1));

    let current_timestamp = worker.view_block().await?.timestamp();
    
    check!(print burrowland_contract.margin_trading_open_position_by_oracle_call(
        &oracle_contract, price_data(current_timestamp, Some(100000)), &alice,
        nusdt_token_contract.0.id(), d(1000, 18).into(), wrap_token_contract.0.id(), d(100, 24).into(), nusdt_token_contract.0.id(), d(900, 18).into(),
        SwapIndication {
            dex_id: near_sdk::AccountId::new_unchecked(ref_exchange_contract.0.id().to_string()),
            swap_action_text: serde_json::to_string(&RefV1TokenReceiverMessage::Execute{
                referral_id: None,
                client_echo: None,
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
        }
    ));

    let alice_margin_account = burrowland_contract.get_margin_account(&alice).await?.unwrap();
    let pos_id = alice_margin_account.margin_positions.keys().collect::<Vec<&String>>()[0].clone();
    
    let position_amount = alice_margin_account.margin_positions.get(&pos_id).unwrap().token_p_amount;
    let borrow_amount = alice_margin_account.margin_positions.get(&pos_id).unwrap().token_d_info.balance + 10u128.pow(20);

    let nusdt_token_detail = burrowland_contract.get_asset(nusdt_token_contract.0.id()).await.unwrap();
    assert_eq!(nusdt_token_detail.supplied.shares.0, d(1000, 18));
    assert_eq!(nusdt_token_detail.borrowed.shares.0, 0);
    assert_eq!(nusdt_token_detail.margin_debt.shares.0, 0);
    let wrap_token_detail = burrowland_contract.get_asset(wrap_token_contract.0.id()).await.unwrap();
    assert_eq!(wrap_token_detail.supplied.shares.0, wrap_collateral_amount);
    assert_eq!(wrap_token_detail.borrowed.shares.0, 0);
    assert_eq!(wrap_token_detail.margin_debt.shares.0, d(100, 24));
    let alice_margin_account = burrowland_contract.get_margin_account(&alice).await.unwrap().unwrap();
    assert_eq!(alice_margin_account.margin_positions.len(), 1);
    let root_account = burrowland_contract.get_account(&root).await.unwrap().unwrap();
    assert_eq!(root_account.borrowed.len(), 0);
    check!(logs burrowland_contract.margin_trading_liquidate_mtposition_direct_by_oracle_call(
        &oracle_contract, price_data(current_timestamp, Some(190000)), &root,
        alice.id(), &pos_id, wrap_token_contract.0.id(), borrow_amount,
    ));
    let alice_margin_account = burrowland_contract.get_margin_account(&alice).await.unwrap().unwrap();
    assert_eq!(alice_margin_account.margin_positions.len(), 0);
    let root_account = burrowland_contract.get_account(&root).await.unwrap().unwrap();
    let root_nusdt_supplied_shares = root_account.supplied.iter().find(|v| v.token_id.to_string() == nusdt_token_contract.0.id().to_string()).unwrap().shares.0;
    assert_eq!(root_nusdt_supplied_shares, d(1000, 18) + position_amount);
    assert_eq!(root_account.borrowed.len(), 1);
    let nusdt_token_detail = burrowland_contract.get_asset(nusdt_token_contract.0.id()).await.unwrap();
    assert_eq!(nusdt_token_detail.supplied.shares.0, d(1000, 18) + position_amount);
    assert_eq!(nusdt_token_detail.borrowed.shares.0, 0);
    assert_eq!(nusdt_token_detail.margin_debt.shares.0, 0);
    let wrap_token_detail = burrowland_contract.get_asset(wrap_token_contract.0.id()).await.unwrap();
    let extra_borrow_wrap_shares = root_account.supplied.iter().find(|v| v.token_id.to_string() == wrap_token_contract.0.id().to_string()).unwrap().shares.0;
    let total_borrow_shares = root_account.borrowed.iter().find(|v| v.token_id.to_string() == wrap_token_contract.0.id().to_string()).unwrap().shares.0;
    assert_eq!(wrap_token_detail.supplied.shares.0, wrap_collateral_amount + extra_borrow_wrap_shares);
    assert_eq!(wrap_token_detail.borrowed.shares.0, total_borrow_shares);
    assert_eq!(wrap_token_detail.margin_debt.shares.0, 0);
    Ok(())
}

#[tokio::test]
async fn test_margin_trading_liquidate_direct_long() -> Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let nusdt_token_contract = deploy_mock_ft(&root, "nusdt", 18).await?;
    let wrap_token_contract = deploy_mock_ft(&root, "wrap", 18).await?;
    let wrap_reserve_amount = d(10000, 24);
    let wrap_collateral_amount = d(10000, 24);
    let nusdt_reserve_amount = d(20000, 6);
    check!(wrap_token_contract.ft_mint(&root, &root, wrap_reserve_amount + wrap_collateral_amount));
    check!(nusdt_token_contract.ft_mint(&root, &root, nusdt_reserve_amount));

    let ref_exchange_contract = deploy_ref_exchange(&root).await?;
    {
        check!(nusdt_token_contract.ft_storage_deposit(ref_exchange_contract.0.id()));
        check!(wrap_token_contract.ft_storage_deposit(ref_exchange_contract.0.id()));
        check!(ref_exchange_contract.storage_deposit(&root));
        check!(ref_exchange_contract.extend_whitelisted_tokens(&root, vec![nusdt_token_contract.0.id(), wrap_token_contract.0.id()]));
    }

    let oracle_contract = deploy_oralce(&root).await?;
    let burrowland_contract = deploy_burrowland_with_price_oracle(&root).await?;
    check!(burrowland_contract.storage_deposit(&root));
    check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));
    check!(burrowland_contract.add_asset_handler(&root, &nusdt_token_contract));
    check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(nusdt_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(burrowland_contract.deposit_to_reserve(&wrap_token_contract, &root, wrap_reserve_amount));
    check!(burrowland_contract.deposit_to_reserve(&nusdt_token_contract, &root, nusdt_reserve_amount));
    check!(burrowland_contract.supply_to_collateral(&wrap_token_contract, &root, wrap_collateral_amount));

    let alice = create_account(&root, "alice", None).await;
    check!(ref_exchange_contract.storage_deposit(&alice));
    check!(burrowland_contract.storage_deposit(&alice));

    assert!(nusdt_token_contract.ft_mint(&root, &alice, d(10000, 6)).await?.is_success());
    assert!(wrap_token_contract.ft_mint(&root, &alice, d(200000, 24)).await?.is_success());

    check!(ref_exchange_contract.deposit(&nusdt_token_contract, &alice, d(10000, 6)));
    check!(ref_exchange_contract.deposit(&wrap_token_contract, &alice, d(10000, 24)));

    check!(ref_exchange_contract.add_simple_swap_pool(&root, vec![nusdt_token_contract.0.id(), wrap_token_contract.0.id()], 5));
    check!(ref_exchange_contract.add_simple_liquidity(&alice, 0, vec![U128(d(10000, 6)), U128(d(1000, 24))], Some(vec![U128(0), U128(0)])));

    check!(view ref_exchange_contract.get_pool(0));

    let supply_amount = d(1000, 18);
    let extra_decimals_mult = d(1, 12);
    check!(nusdt_token_contract.ft_mint(&root, &alice, supply_amount * 10));
    check!(wrap_token_contract.ft_storage_deposit(alice.id()));

    check!(view burrowland_contract.get_margin_account(&alice));

    check!(burrowland_contract.deposit_to_margin(&nusdt_token_contract, &alice, supply_amount / extra_decimals_mult));
    
    check!(burrowland_contract.register_margin_dex(&root, ref_exchange_contract.0.id(), 1));
    check!(burrowland_contract.register_margin_token(&root, nusdt_token_contract.0.id(), 0));
    check!(burrowland_contract.register_margin_token(&root, wrap_token_contract.0.id(), 1));

    let current_timestamp = worker.view_block().await?.timestamp();
    
    check!(print burrowland_contract.margin_trading_open_position_by_oracle_call(
        &oracle_contract, price_data(current_timestamp, Some(100000)), &alice,
        nusdt_token_contract.0.id(), d(1000, 18).into(), nusdt_token_contract.0.id(), d(1000, 18).into(), wrap_token_contract.0.id(), d(90, 24).into(),
        SwapIndication {
            dex_id: near_sdk::AccountId::new_unchecked(ref_exchange_contract.0.id().to_string()),
            swap_action_text: serde_json::to_string(&RefV1TokenReceiverMessage::Execute{
                referral_id: None,
                client_echo: None,
                actions: vec![
                    RefV1Action::Swap(RefV1SwapAction{
                        pool_id: 0,
                        token_in: near_sdk::AccountId::new_unchecked(nusdt_token_contract.0.id().to_string()),
                        amount_in: Some(U128(d(1000, 6))),
                        token_out: near_sdk::AccountId::new_unchecked(wrap_token_contract.0.id().to_string()),
                        min_amount_out: U128(d(90, 24)),
                    })
                ]
            }).unwrap()
        }
    ));

    let alice_margin_account = burrowland_contract.get_margin_account(&alice).await?.unwrap();
    let pos_id = alice_margin_account.margin_positions.keys().collect::<Vec<&String>>()[0].clone();
    
    let position_amount = alice_margin_account.margin_positions.get(&pos_id).unwrap().token_p_amount;
    let borrow_amount = alice_margin_account.margin_positions.get(&pos_id).unwrap().token_d_info.balance + 10u128.pow(10);

    let nusdt_token_detail = burrowland_contract.get_asset(nusdt_token_contract.0.id()).await.unwrap();
    assert_eq!(nusdt_token_detail.supplied.shares.0, d(1000, 18));
    assert_eq!(nusdt_token_detail.borrowed.shares.0, 0);
    assert_eq!(nusdt_token_detail.margin_debt.shares.0, d(1000, 18));
    let wrap_token_detail = burrowland_contract.get_asset(wrap_token_contract.0.id()).await.unwrap();
    assert_eq!(wrap_token_detail.supplied.shares.0, wrap_collateral_amount);
    assert_eq!(wrap_token_detail.borrowed.shares.0, 0);
    assert_eq!(wrap_token_detail.margin_debt.shares.0, 0);
    let alice_margin_account = burrowland_contract.get_margin_account(&alice).await.unwrap().unwrap();
    assert_eq!(alice_margin_account.margin_positions.len(), 1);
    let root_account = burrowland_contract.get_account(&root).await.unwrap().unwrap();
    assert_eq!(root_account.borrowed.len(), 0);
    check!(logs burrowland_contract.margin_trading_liquidate_mtposition_direct_by_oracle_call(
        &oracle_contract, price_data(current_timestamp, Some(10000)), &root,
        alice.id(), &pos_id, nusdt_token_contract.0.id(), borrow_amount,
    ));
    let alice_margin_account = burrowland_contract.get_margin_account(&alice).await.unwrap().unwrap();
    assert_eq!(alice_margin_account.margin_positions.len(), 0);
    let root_account = burrowland_contract.get_account(&root).await.unwrap().unwrap();
    let root_nusdt_supplied_shares = root_account.supplied.iter().find(|v| v.token_id.to_string() == nusdt_token_contract.0.id().to_string()).unwrap().shares.0;
    let total_borrow_shares = root_account.borrowed.iter().find(|v| v.token_id.to_string() == nusdt_token_contract.0.id().to_string()).unwrap().shares.0;
    assert_eq!(root_account.borrowed.len(), 1);
    let nusdt_token_detail = burrowland_contract.get_asset(nusdt_token_contract.0.id()).await.unwrap();
    assert_eq!(nusdt_token_detail.supplied.shares.0, root_nusdt_supplied_shares);
    assert_eq!(nusdt_token_detail.borrowed.shares.0, total_borrow_shares);
    assert_eq!(nusdt_token_detail.margin_debt.shares.0, 0);
    let wrap_token_detail = burrowland_contract.get_asset(wrap_token_contract.0.id()).await.unwrap();
    let root_wrap_supplied_shares = root_account.supplied.iter().find(|v| v.token_id.to_string() == wrap_token_contract.0.id().to_string()).unwrap().shares.0;
    assert_eq!(root_wrap_supplied_shares, position_amount);
    assert_eq!(wrap_token_detail.supplied.shares.0, wrap_collateral_amount + root_wrap_supplied_shares);
    assert_eq!(wrap_token_detail.borrowed.shares.0, 0);
    assert_eq!(wrap_token_detail.margin_debt.shares.0, 0);
    Ok(())
}

#[tokio::test]
async fn test_margin_trading_liquidate() -> Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;

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

    let oracle_contract = deploy_oralce(&root).await?;
    let burrowland_contract = deploy_burrowland_with_price_oracle(&root).await?;
    check!(burrowland_contract.storage_deposit(&root));
    check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));
    check!(burrowland_contract.add_asset_handler(&root, &nusdt_token_contract));
    check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(nusdt_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(burrowland_contract.deposit_to_reserve(&wrap_token_contract, &root, wrap_reserve_amount));
    check!(burrowland_contract.deposit_to_reserve(&nusdt_token_contract, &root, nusdt_reserve_amount));

    let alice = create_account(&root, "alice", None).await;
    check!(ref_exchange_contract.storage_deposit(&alice));
    check!(burrowland_contract.storage_deposit(&alice));

    assert!(nusdt_token_contract.ft_mint(&root, &alice, d(10000, 6)).await?.is_success());
    assert!(wrap_token_contract.ft_mint(&root, &alice, d(100000, 24)).await?.is_success());

    check!(ref_exchange_contract.deposit(&nusdt_token_contract, &alice, d(10000, 6)));
    check!(ref_exchange_contract.deposit(&wrap_token_contract, &alice, d(10000, 24)));

    check!(ref_exchange_contract.add_simple_swap_pool(&root, vec![nusdt_token_contract.0.id(), wrap_token_contract.0.id()], 5));
    check!(ref_exchange_contract.add_simple_liquidity(&alice, 0, vec![U128(d(10000, 6)), U128(d(1000, 24))], Some(vec![U128(0), U128(0)])));

    check!(view ref_exchange_contract.get_pool(0));

    let supply_amount = d(1000, 18);
    let extra_decimals_mult = d(1, 12);
    check!(nusdt_token_contract.ft_mint(&root, &alice, supply_amount * 10));
    check!(wrap_token_contract.ft_storage_deposit(alice.id()));

    check!(view burrowland_contract.get_margin_account(&alice));

    check!(burrowland_contract.deposit_to_margin(&nusdt_token_contract, &alice, supply_amount / extra_decimals_mult));
    
    check!(burrowland_contract.register_margin_dex(&root, ref_exchange_contract.0.id(), 1));
    check!(burrowland_contract.register_margin_token(&root, nusdt_token_contract.0.id(), 0));
    check!(burrowland_contract.register_margin_token(&root, wrap_token_contract.0.id(), 1));

    let current_timestamp = worker.view_block().await?.timestamp();
    check!(print burrowland_contract.margin_trading_open_position_by_oracle_call(
        &oracle_contract, price_data(current_timestamp, Some(100000)), &alice,
        nusdt_token_contract.0.id(), d(1000, 18).into(), wrap_token_contract.0.id(), d(100, 24).into(), nusdt_token_contract.0.id(), d(900, 18).into(),
        SwapIndication {
            dex_id: near_sdk::AccountId::new_unchecked(ref_exchange_contract.0.id().to_string()),
            swap_action_text: serde_json::to_string(&RefV1TokenReceiverMessage::Execute{
                referral_id: None,
                client_echo: None,
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
        }
    ));

    check!(view burrowland_contract.get_margin_account(&alice));
    
    check!(ref_exchange_contract.swap(&wrap_token_contract, &alice, d(600, 24), 0, nusdt_token_contract.0.id()));
    check!(view ref_exchange_contract.get_pool(0));

    let alice_margin_account = burrowland_contract.get_margin_account(&alice).await?.unwrap();
    let pos_id = alice_margin_account.margin_positions.keys().collect::<Vec<&String>>()[0].clone();
    
    let position_amount = alice_margin_account.margin_positions.get(&pos_id).unwrap().token_p_amount;
    let min_out_amount = alice_margin_account.margin_positions.get(&pos_id).unwrap().token_d_info.balance + 10u128.pow(20);

    check!(view burrowland_contract.get_asset(nusdt_token_contract.0.id()));

    check!(print burrowland_contract.margin_trading_liquidate_mtposition_by_oracle_call(
        &oracle_contract, price_data(current_timestamp, Some(190000)), &root,
        alice.id(), &pos_id, position_amount, min_out_amount,
        SwapIndication {
            dex_id: near_sdk::AccountId::new_unchecked(ref_exchange_contract.0.id().to_string()),
            swap_action_text: serde_json::to_string(&RefV1TokenReceiverMessage::Execute{
                referral_id: None,
                client_echo: None,
                actions: vec![
                    RefV1Action::Swap(RefV1SwapAction{
                        pool_id: 0,
                        token_in: near_sdk::AccountId::new_unchecked(nusdt_token_contract.0.id().to_string()),
                        amount_in: Some(U128(position_amount / 10u128.pow(12))),
                        token_out: near_sdk::AccountId::new_unchecked(wrap_token_contract.0.id().to_string()),
                        min_amount_out: U128(min_out_amount),
                    })
                ]
            }).unwrap()
        }
    ));
    
    check!(view burrowland_contract.get_margin_account(&alice));

    check!(view burrowland_contract.get_asset(nusdt_token_contract.0.id()));
    Ok(())
}

#[tokio::test]
async fn test_margin_trading_liquidate_with_pyth() -> Result<()> {
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
    check!(burrowland_contract.storage_deposit(&root));
    check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));
    check!(burrowland_contract.add_asset_handler(&root, &nusdt_token_contract));
    check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(nusdt_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(burrowland_contract.deposit_to_reserve(&wrap_token_contract, &root, wrap_reserve_amount));
    check!(burrowland_contract.deposit_to_reserve(&nusdt_token_contract, &root, nusdt_reserve_amount));

    let alice = create_account(&root, "alice", None).await;
    check!(ref_exchange_contract.storage_deposit(&alice));
    check!(burrowland_contract.storage_deposit(&alice));

    assert!(nusdt_token_contract.ft_mint(&root, &alice, d(10000, 6)).await?.is_success());
    assert!(wrap_token_contract.ft_mint(&root, &alice, d(100000, 24)).await?.is_success());

    check!(ref_exchange_contract.deposit(&nusdt_token_contract, &alice, d(10000, 6)));
    check!(ref_exchange_contract.deposit(&wrap_token_contract, &alice, d(10000, 24)));

    check!(ref_exchange_contract.add_simple_swap_pool(&root, vec![nusdt_token_contract.0.id(), wrap_token_contract.0.id()], 5));
    check!(ref_exchange_contract.add_simple_liquidity(&alice, 0, vec![U128(d(10000, 6)), U128(d(1000, 24))], Some(vec![U128(0), U128(0)])));

    check!(view ref_exchange_contract.get_pool(0));

    let supply_amount = d(1000, 18);
    let extra_decimals_mult = d(1, 12);
    check!(nusdt_token_contract.ft_mint(&root, &alice, supply_amount * 10));
    check!(wrap_token_contract.ft_storage_deposit(alice.id()));

    check!(view burrowland_contract.get_margin_account(&alice));

    check!(burrowland_contract.deposit_to_margin(&nusdt_token_contract, &alice, supply_amount / extra_decimals_mult));
    
    check!(burrowland_contract.register_margin_dex(&root, ref_exchange_contract.0.id(), 1));
    check!(burrowland_contract.register_margin_token(&root, nusdt_token_contract.0.id(), 0));
    check!(burrowland_contract.register_margin_token(&root, wrap_token_contract.0.id(), 1));

    // near
    let current_timestamp = worker.view_block().await?.timestamp();
    check!(burrowland_contract.add_token_pyth_info(&root, wrap_token_contract.0.id(), 24, 4, "27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", None, None));
    check!(pyth_contract.set_price("27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", PythPrice{
        price: I64(1000000000),
        conf: U64(278100),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));

    // usdt
    let current_timestamp = worker.view_block().await?.timestamp();
    check!(burrowland_contract.add_token_pyth_info(&root, nusdt_token_contract.0.id(), 6, 4, "1fc18861232290221461220bd4e2acd1dcdfbc89c84092c93c18bdc7756c1588", None, None));
    check!(pyth_contract.set_price("1fc18861232290221461220bd4e2acd1dcdfbc89c84092c93c18bdc7756c1588", PythPrice{
        price: I64(100000000),
        conf: U64(103853),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));
    
    check!(logs burrowland_contract.margin_trading_open_position_by_pyth(&alice,
        nusdt_token_contract.0.id(), d(1000, 18).into(), wrap_token_contract.0.id(), d(100, 24).into(), nusdt_token_contract.0.id(), d(900, 18).into(),
        SwapIndication {
            dex_id: near_sdk::AccountId::new_unchecked(ref_exchange_contract.0.id().to_string()),
            swap_action_text: serde_json::to_string(&RefV1TokenReceiverMessage::Execute{
                referral_id: None,
                client_echo: None,
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
        }
    ));

    check!(view burrowland_contract.get_margin_account(&alice));
    
    check!(ref_exchange_contract.swap(&wrap_token_contract, &alice, d(600, 24), 0, nusdt_token_contract.0.id()));
    check!(view ref_exchange_contract.get_pool(0));

    let alice_margin_account = burrowland_contract.get_margin_account(&alice).await?.unwrap();
    let pos_id = alice_margin_account.margin_positions.keys().collect::<Vec<&String>>()[0].clone();
    
    let position_amount = alice_margin_account.margin_positions.get(&pos_id).unwrap().token_p_amount;
    let min_out_amount = alice_margin_account.margin_positions.get(&pos_id).unwrap().token_d_info.balance + 10u128.pow(20);

    check!(view burrowland_contract.get_asset(nusdt_token_contract.0.id()));

    check!(pyth_contract.set_price("27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", PythPrice{
        price: I64(1900000000),
        conf: U64(278100),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));

    check!(logs burrowland_contract.margin_trading_liquidate_mtposition_by_pyth(&root,
        alice.id(), &pos_id, position_amount, min_out_amount,
        SwapIndication {
            dex_id: near_sdk::AccountId::new_unchecked(ref_exchange_contract.0.id().to_string()),
            swap_action_text: serde_json::to_string(&RefV1TokenReceiverMessage::Execute{
                referral_id: None,
                client_echo: None,
                actions: vec![
                    RefV1Action::Swap(RefV1SwapAction{
                        pool_id: 0,
                        token_in: near_sdk::AccountId::new_unchecked(nusdt_token_contract.0.id().to_string()),
                        amount_in: Some(U128(position_amount / 10u128.pow(12))),
                        token_out: near_sdk::AccountId::new_unchecked(wrap_token_contract.0.id().to_string()),
                        min_amount_out: U128(min_out_amount),
                    })
                ]
            }).unwrap()
        }
    ));
    
    check!(view burrowland_contract.get_margin_account(&alice));

    check!(view burrowland_contract.get_asset(nusdt_token_contract.0.id()));
    Ok(())
}

#[tokio::test]
async fn test_margin_trading_forceclose() -> Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;

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

    let oracle_contract = deploy_oralce(&root).await?;
    let burrowland_contract = deploy_burrowland_with_price_oracle(&root).await?;
    check!(burrowland_contract.storage_deposit(&root));
    check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));
    check!(burrowland_contract.add_asset_handler(&root, &nusdt_token_contract));
    check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(nusdt_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(burrowland_contract.deposit_to_reserve(&wrap_token_contract, &root, wrap_reserve_amount));
    check!(burrowland_contract.deposit_to_reserve(&nusdt_token_contract, &root, nusdt_reserve_amount));

    let alice = create_account(&root, "alice", None).await;
    check!(ref_exchange_contract.storage_deposit(&alice));
    check!(burrowland_contract.storage_deposit(&alice));

    assert!(nusdt_token_contract.ft_mint(&root, &alice, d(10000, 6)).await?.is_success());
    assert!(wrap_token_contract.ft_mint(&root, &alice, d(100000, 24)).await?.is_success());

    check!(ref_exchange_contract.deposit(&nusdt_token_contract, &alice, d(10000, 6)));
    check!(ref_exchange_contract.deposit(&wrap_token_contract, &alice, d(10000, 24)));

    check!(ref_exchange_contract.add_simple_swap_pool(&root, vec![nusdt_token_contract.0.id(), wrap_token_contract.0.id()], 5));
    check!(ref_exchange_contract.add_simple_liquidity(&alice, 0, vec![U128(d(10000, 6)), U128(d(1000, 24))], Some(vec![U128(0), U128(0)])));

    check!(view ref_exchange_contract.get_pool(0));

    let supply_amount = d(1000, 18);
    let extra_decimals_mult = d(1, 12);
    check!(nusdt_token_contract.ft_mint(&root, &alice, supply_amount * 10));
    check!(wrap_token_contract.ft_storage_deposit(alice.id()));

    check!(view burrowland_contract.get_margin_account(&alice));

    check!(burrowland_contract.deposit_to_margin(&nusdt_token_contract, &alice, supply_amount / extra_decimals_mult));
    
    check!(burrowland_contract.register_margin_dex(&root, ref_exchange_contract.0.id(), 1));
    check!(burrowland_contract.register_margin_token(&root, nusdt_token_contract.0.id(), 0));
    check!(burrowland_contract.register_margin_token(&root, wrap_token_contract.0.id(), 1));

    let current_timestamp = worker.view_block().await?.timestamp();
    check!(logs burrowland_contract.margin_trading_open_position_by_oracle_call(
        &oracle_contract, price_data(current_timestamp, Some(100000)), &alice,
        nusdt_token_contract.0.id(), d(1000, 18).into(), wrap_token_contract.0.id(), d(100, 24).into(), nusdt_token_contract.0.id(), d(900, 18).into(),
        SwapIndication {
            dex_id: near_sdk::AccountId::new_unchecked(ref_exchange_contract.0.id().to_string()),
            swap_action_text: serde_json::to_string(&RefV1TokenReceiverMessage::Execute{
                referral_id: None,
                client_echo: None,
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
        }
    ));

    check!(view burrowland_contract.get_margin_account(&alice));
    
    let alice_margin_account = burrowland_contract.get_margin_account(&alice).await?.unwrap();
    let pos_id = alice_margin_account.margin_positions.keys().collect::<Vec<&String>>()[0].clone();
    
    let position_amount = alice_margin_account.margin_positions.get(&pos_id).unwrap().token_p_amount;
    let min_out_amount = alice_margin_account.margin_positions.get(&pos_id).unwrap().token_d_info.balance + 10u128.pow(20);

    check!(view burrowland_contract.get_asset(nusdt_token_contract.0.id()));
    check!(view burrowland_contract.get_asset(wrap_token_contract.0.id()));

    check!(print burrowland_contract.margin_trading_force_close_mtposition_by_oracle_call(
        &oracle_contract, price_data(current_timestamp, Some(200000)), &root,
        alice.id(), &pos_id, position_amount, min_out_amount / 2,
        SwapIndication {
            dex_id: near_sdk::AccountId::new_unchecked(ref_exchange_contract.0.id().to_string()),
            swap_action_text: serde_json::to_string(&RefV1TokenReceiverMessage::Execute{
                referral_id: None,
                client_echo: None,
                actions: vec![
                    RefV1Action::Swap(RefV1SwapAction{
                        pool_id: 0,
                        token_in: near_sdk::AccountId::new_unchecked(nusdt_token_contract.0.id().to_string()),
                        amount_in: Some(U128(position_amount / 10u128.pow(12))),
                        token_out: near_sdk::AccountId::new_unchecked(wrap_token_contract.0.id().to_string()),
                        min_amount_out: U128(min_out_amount / 2),
                    })
                ]
            }).unwrap()
        }
    ));
    
    check!(view burrowland_contract.get_margin_account(&alice));

    check!(view burrowland_contract.get_asset(nusdt_token_contract.0.id()));
    check!(view burrowland_contract.get_asset(wrap_token_contract.0.id()));
    Ok(())
}

#[tokio::test]
async fn test_margin_trading_forceclose_with_pyth() -> Result<()> {
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
    check!(burrowland_contract.storage_deposit(&root));
    check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));
    check!(burrowland_contract.add_asset_handler(&root, &nusdt_token_contract));
    check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(nusdt_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(burrowland_contract.deposit_to_reserve(&wrap_token_contract, &root, wrap_reserve_amount));
    check!(burrowland_contract.deposit_to_reserve(&nusdt_token_contract, &root, nusdt_reserve_amount));

    let alice = create_account(&root, "alice", None).await;
    check!(ref_exchange_contract.storage_deposit(&alice));
    check!(burrowland_contract.storage_deposit(&alice));

    assert!(nusdt_token_contract.ft_mint(&root, &alice, d(10000, 6)).await?.is_success());
    assert!(wrap_token_contract.ft_mint(&root, &alice, d(100000, 24)).await?.is_success());

    check!(ref_exchange_contract.deposit(&nusdt_token_contract, &alice, d(10000, 6)));
    check!(ref_exchange_contract.deposit(&wrap_token_contract, &alice, d(10000, 24)));

    check!(ref_exchange_contract.add_simple_swap_pool(&root, vec![nusdt_token_contract.0.id(), wrap_token_contract.0.id()], 5));
    check!(ref_exchange_contract.add_simple_liquidity(&alice, 0, vec![U128(d(10000, 6)), U128(d(1000, 24))], Some(vec![U128(0), U128(0)])));

    check!(view ref_exchange_contract.get_pool(0));

    let supply_amount = d(1000, 18);
    let extra_decimals_mult = d(1, 12);
    check!(nusdt_token_contract.ft_mint(&root, &alice, supply_amount * 10));
    check!(wrap_token_contract.ft_storage_deposit(alice.id()));

    check!(view burrowland_contract.get_margin_account(&alice));

    check!(burrowland_contract.deposit_to_margin(&nusdt_token_contract, &alice, supply_amount / extra_decimals_mult));
    
    check!(burrowland_contract.register_margin_dex(&root, ref_exchange_contract.0.id(), 1));
    check!(burrowland_contract.register_margin_token(&root, nusdt_token_contract.0.id(), 0));
    check!(burrowland_contract.register_margin_token(&root, wrap_token_contract.0.id(), 1));

    // near
    let current_timestamp = worker.view_block().await?.timestamp();
    check!(burrowland_contract.add_token_pyth_info(&root, wrap_token_contract.0.id(), 24, 4, "27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", None, None));
    check!(pyth_contract.set_price("27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", PythPrice{
        price: I64(1000000000),
        conf: U64(278100),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));

    // usdt
    let current_timestamp = worker.view_block().await?.timestamp();
    check!(burrowland_contract.add_token_pyth_info(&root, nusdt_token_contract.0.id(), 6, 4, "1fc18861232290221461220bd4e2acd1dcdfbc89c84092c93c18bdc7756c1588", None, None));
    check!(pyth_contract.set_price("1fc18861232290221461220bd4e2acd1dcdfbc89c84092c93c18bdc7756c1588", PythPrice{
        price: I64(100000000),
        conf: U64(103853),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));

    let current_timestamp = worker.view_block().await?.timestamp();
    check!(logs burrowland_contract.margin_trading_open_position_by_pyth(&alice,
        nusdt_token_contract.0.id(), d(1000, 18).into(), wrap_token_contract.0.id(), d(100, 24).into(), nusdt_token_contract.0.id(), d(900, 18).into(),
        SwapIndication {
            dex_id: near_sdk::AccountId::new_unchecked(ref_exchange_contract.0.id().to_string()),
            swap_action_text: serde_json::to_string(&RefV1TokenReceiverMessage::Execute{
                referral_id: None,
                client_echo: None,
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
        }
    ));

    check!(view burrowland_contract.get_margin_account(&alice));
    
    let alice_margin_account = burrowland_contract.get_margin_account(&alice).await?.unwrap();
    let pos_id = alice_margin_account.margin_positions.keys().collect::<Vec<&String>>()[0].clone();
    
    let position_amount = alice_margin_account.margin_positions.get(&pos_id).unwrap().token_p_amount;
    let min_out_amount = alice_margin_account.margin_positions.get(&pos_id).unwrap().token_d_info.balance + 10u128.pow(20);

    check!(view burrowland_contract.get_asset(nusdt_token_contract.0.id()));
    check!(view burrowland_contract.get_asset(wrap_token_contract.0.id()));

    check!(pyth_contract.set_price("27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", PythPrice{
        price: I64(2000000000),
        conf: U64(278100),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));

    check!(print burrowland_contract.margin_trading_force_close_mtposition_by_pyth(&root,
        alice.id(), &pos_id, position_amount, min_out_amount / 2,
        SwapIndication {
            dex_id: near_sdk::AccountId::new_unchecked(ref_exchange_contract.0.id().to_string()),
            swap_action_text: serde_json::to_string(&RefV1TokenReceiverMessage::Execute{
                referral_id: None,
                client_echo: None,
                actions: vec![
                    RefV1Action::Swap(RefV1SwapAction{
                        pool_id: 0,
                        token_in: near_sdk::AccountId::new_unchecked(nusdt_token_contract.0.id().to_string()),
                        amount_in: Some(U128(position_amount / 10u128.pow(12))),
                        token_out: near_sdk::AccountId::new_unchecked(wrap_token_contract.0.id().to_string()),
                        min_amount_out: U128(min_out_amount / 2),
                    })
                ]
            }).unwrap()
        }
    ));
    
    check!(view burrowland_contract.get_margin_account(&alice));

    check!(view burrowland_contract.get_asset(nusdt_token_contract.0.id()));
    check!(view burrowland_contract.get_asset(wrap_token_contract.0.id()));
    Ok(())
}

#[tokio::test]
async fn test_margin_trading_dcl() -> Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let pyth_contract = deploy_mock_pyth(&root).await?;
    let nusdt_token_contract = deploy_mock_ft(&root, "nusdt", 18).await?;
    let wrap_token_contract = deploy_mock_ft(&root, "wrap", 18).await?;
    let wrap_reserve_amount = d(10000, 24);
    let nusdt_reserve_amount = d(10000, 6);
    check!(wrap_token_contract.ft_mint(&root, &root, wrap_reserve_amount));
    check!(nusdt_token_contract.ft_mint(&root, &root, nusdt_reserve_amount));

    let dcl_exchange_contract = deploy_mock_dcl(&root).await?;
    {
        check!(nusdt_token_contract.ft_storage_deposit(dcl_exchange_contract.0.id()));
        check!(wrap_token_contract.ft_storage_deposit(dcl_exchange_contract.0.id()));
    }

    let burrowland_contract = deploy_burrowland_with_pyth(&root).await?;
    check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));
    check!(burrowland_contract.add_asset_handler(&root, &nusdt_token_contract));
    check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(nusdt_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(burrowland_contract.deposit_to_reserve(&wrap_token_contract, &root, wrap_reserve_amount));
    check!(burrowland_contract.deposit_to_reserve(&nusdt_token_contract, &root, nusdt_reserve_amount));

    let alice = create_account(&root, "alice", None).await;
    check!(dcl_exchange_contract.storage_deposit(&alice));
    check!(burrowland_contract.storage_deposit(&alice));

    assert!(nusdt_token_contract.ft_mint(&root, &alice, d(10000, 6)).await?.is_success());
    assert!(wrap_token_contract.ft_mint(&root, &alice, d(100000, 24)).await?.is_success());

    let outcome = dcl_exchange_contract.create_pool(&root, nusdt_token_contract.0.id(), wrap_token_contract.0.id(), 100, 392000, parse_near!("0.1 N")).await?;
    let pool_id = outcome.json::<String>().unwrap();

    check!(dcl_exchange_contract.deposit(&nusdt_token_contract, &alice, d(10000, 6)));
    check!(dcl_exchange_contract.deposit(&wrap_token_contract, &alice, d(100000, 24)));
    check!(view dcl_exchange_contract.get_pool(&pool_id));
    check!(view dcl_exchange_contract.list_user_assets(&alice));
    check!(dcl_exchange_contract.add_liquidity(&alice, &pool_id, 390000, 394000, d(10000, 6), d(100000, 24), 1, 1));
    check!(view dcl_exchange_contract.list_user_assets(&alice));

    let supply_amount = d(1000, 18);
    let extra_decimals_mult = d(1, 12);
    check!(nusdt_token_contract.ft_mint(&root, &alice, supply_amount * 10));
    check!(wrap_token_contract.ft_storage_deposit(alice.id()));

    check!(view burrowland_contract.get_margin_account(&alice));

    check!(burrowland_contract.deposit_to_margin(&nusdt_token_contract, &alice, supply_amount / extra_decimals_mult));
    
    check!(burrowland_contract.register_margin_dex(&root, dcl_exchange_contract.0.id(), 2));
    check!(burrowland_contract.register_margin_token(&root, nusdt_token_contract.0.id(), 0));
    check!(burrowland_contract.register_margin_token(&root, wrap_token_contract.0.id(), 1));

    // near
    let current_timestamp = worker.view_block().await?.timestamp();
    check!(burrowland_contract.add_token_pyth_info(&root, wrap_token_contract.0.id(), 24, 4, "27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", None, None));
    check!(pyth_contract.set_price("27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", PythPrice{
        price: I64(1000000000),
        conf: U64(278100),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));

    // usdt
    let current_timestamp = worker.view_block().await?.timestamp();
    check!(burrowland_contract.add_token_pyth_info(&root, nusdt_token_contract.0.id(), 6, 4, "1fc18861232290221461220bd4e2acd1dcdfbc89c84092c93c18bdc7756c1588", None, None));
    check!(pyth_contract.set_price("1fc18861232290221461220bd4e2acd1dcdfbc89c84092c93c18bdc7756c1588", PythPrice{
        price: I64(100000000),
        conf: U64(103853),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));

    check!(view dcl_exchange_contract.quote(vec![&pool_id], wrap_token_contract.0.id(), nusdt_token_contract.0.id(), d(20, 24), None));

    check!(logs burrowland_contract.margin_trading_open_position_by_pyth(&alice,
        nusdt_token_contract.0.id(), d(1000, 18).into(), wrap_token_contract.0.id(), d(20, 24).into(), nusdt_token_contract.0.id(), d(180, 18).into(),
        SwapIndication {
            dex_id: near_sdk::AccountId::new_unchecked(dcl_exchange_contract.0.id().to_string()),
            swap_action_text: serde_json::to_string(&RefV2TokenReceiverMessage::Swap { 
                pool_ids: vec![pool_id.clone()],
                output_token: near_sdk::AccountId::new_unchecked(nusdt_token_contract.0.id().to_string()),
                min_output_amount: U128(d(180, 6)), 
                skip_unwrap_near: Some(true), 
                client_echo: None
            }).unwrap()
        }
    ));

    check!(view burrowland_contract.get_margin_account(&alice));

    let mut alice_margin_account = burrowland_contract.get_margin_account(&alice).await?.unwrap();
    let pos_id = alice_margin_account.margin_positions.keys().collect::<Vec<&String>>()[0].clone();

    check!(burrowland_contract.margin_trading_increase_collateral_by_ft_transfer_call(&nusdt_token_contract, &alice, supply_amount / extra_decimals_mult, &pos_id, supply_amount));
    alice_margin_account = burrowland_contract.get_margin_account(&alice).await?.unwrap();
    assert!(alice_margin_account.supplied.is_empty());
    assert_eq!(alice_margin_account.margin_positions.get(&pos_id).unwrap().token_c_info.balance, d(2000, 18));

    check!(burrowland_contract.margin_trading_decrease_collateral_by_pyth(&alice, &pos_id, supply_amount));
    alice_margin_account = burrowland_contract.get_margin_account(&alice).await?.unwrap();
    println!("{:?}", alice_margin_account);
    assert_eq!(alice_margin_account.supplied[0].balance, d(1000, 18));
    assert_eq!(alice_margin_account.margin_positions.get(&pos_id).unwrap().token_c_info.balance, d(1000, 18));

    check!(logs burrowland_contract.margin_trading_decrease_mtposition_by_pyth(&alice,
        &pos_id, d(100, 18), d(10, 24), 
        SwapIndication {
            dex_id: near_sdk::AccountId::new_unchecked(dcl_exchange_contract.0.id().to_string()),
            swap_action_text: serde_json::to_string(&RefV2TokenReceiverMessage::Swap { 
                pool_ids: vec![pool_id.clone()],
                output_token: near_sdk::AccountId::new_unchecked(wrap_token_contract.0.id().to_string()),
                min_output_amount: U128(d(10, 24)), 
                skip_unwrap_near: Some(true), 
                client_echo: None
            }).unwrap()
        }
    ));

    check!(view burrowland_contract.get_margin_account(&alice));

    check!(dcl_exchange_contract.swap(&wrap_token_contract, &alice, d(100, 24), pool_id.clone(), nusdt_token_contract.0.id()));

    alice_margin_account = burrowland_contract.get_margin_account(&alice).await?.unwrap();

    let position_amount = alice_margin_account.margin_positions.get(&pos_id).unwrap().token_p_amount;
    let min_out_amount = alice_margin_account.margin_positions.get(&pos_id).unwrap().token_d_info.balance + 10u128.pow(20);
    check!(print burrowland_contract.margin_trading_close_mtposition_by_pyth(&alice,
        &pos_id, position_amount, min_out_amount, 
        SwapIndication {
            dex_id: near_sdk::AccountId::new_unchecked(dcl_exchange_contract.0.id().to_string()),
            swap_action_text: serde_json::to_string(&RefV2TokenReceiverMessage::Swap { 
                pool_ids: vec![pool_id.clone()],
                output_token: near_sdk::AccountId::new_unchecked(wrap_token_contract.0.id().to_string()),
                min_output_amount: U128(min_out_amount), 
                skip_unwrap_near: Some(true), 
                client_echo: None
            }).unwrap()
        }
    ));

    check!(view burrowland_contract.get_margin_account(&alice));

    Ok(())
}


#[tokio::test]
async fn test_margin_trading_liquidate_dcl() -> Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let pyth_contract = deploy_mock_pyth(&root).await?;
    let nusdt_token_contract = deploy_mock_ft(&root, "nusdt", 18).await?;
    let wrap_token_contract = deploy_mock_ft(&root, "wrap", 18).await?;
    let wrap_reserve_amount = d(10000, 24);
    let nusdt_reserve_amount = d(10000, 6);
    check!(wrap_token_contract.ft_mint(&root, &root, wrap_reserve_amount));
    check!(nusdt_token_contract.ft_mint(&root, &root, nusdt_reserve_amount));

    let dcl_exchange_contract = deploy_mock_dcl(&root).await?;
    {
        check!(nusdt_token_contract.ft_storage_deposit(dcl_exchange_contract.0.id()));
        check!(wrap_token_contract.ft_storage_deposit(dcl_exchange_contract.0.id()));
    }

    let burrowland_contract = deploy_burrowland_with_pyth(&root).await?;
    check!(burrowland_contract.storage_deposit(&root));
    check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));
    check!(burrowland_contract.add_asset_handler(&root, &nusdt_token_contract));
    check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(nusdt_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(burrowland_contract.deposit_to_reserve(&wrap_token_contract, &root, wrap_reserve_amount));
    check!(burrowland_contract.deposit_to_reserve(&nusdt_token_contract, &root, nusdt_reserve_amount));

    let alice = create_account(&root, "alice", None).await;
    check!(dcl_exchange_contract.storage_deposit(&alice));
    check!(burrowland_contract.storage_deposit(&alice));

    assert!(nusdt_token_contract.ft_mint(&root, &alice, d(100000, 6)).await?.is_success());
    assert!(wrap_token_contract.ft_mint(&root, &alice, d(1000000, 24)).await?.is_success());

    let outcome = dcl_exchange_contract.create_pool(&root, nusdt_token_contract.0.id(), wrap_token_contract.0.id(), 10000, 392000, parse_near!("0.1 N")).await?;
    let pool_id = outcome.json::<String>().unwrap();

    check!(dcl_exchange_contract.deposit(&nusdt_token_contract, &alice, d(100000, 6)));
    check!(dcl_exchange_contract.deposit(&wrap_token_contract, &alice, d(1000000, 24)));
    check!(view dcl_exchange_contract.get_pool(&pool_id));
    check!(view dcl_exchange_contract.list_user_assets(&alice));
    check!(dcl_exchange_contract.add_liquidity(&alice, &pool_id, 390000, 420000, d(100000, 6), d(1000000, 24), 1, 1));
    check!(view dcl_exchange_contract.list_user_assets(&alice));

    let supply_amount = d(1000, 18);
    let extra_decimals_mult = d(1, 12);
    check!(nusdt_token_contract.ft_mint(&root, &alice, supply_amount * 10));
    check!(wrap_token_contract.ft_storage_deposit(alice.id()));

    check!(view burrowland_contract.get_margin_account(&alice));

    check!(burrowland_contract.deposit_to_margin(&nusdt_token_contract, &alice, supply_amount / extra_decimals_mult));
    
    check!(burrowland_contract.register_margin_dex(&root, dcl_exchange_contract.0.id(), 2));
    check!(burrowland_contract.register_margin_token(&root, nusdt_token_contract.0.id(), 0));
    check!(burrowland_contract.register_margin_token(&root, wrap_token_contract.0.id(), 1));

    // near
    let current_timestamp = worker.view_block().await?.timestamp();
    check!(burrowland_contract.add_token_pyth_info(&root, wrap_token_contract.0.id(), 24, 4, "27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", None, None));
    check!(pyth_contract.set_price("27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", PythPrice{
        price: I64(1000000000),
        conf: U64(278100),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));

    // usdt
    let current_timestamp = worker.view_block().await?.timestamp();
    check!(burrowland_contract.add_token_pyth_info(&root, nusdt_token_contract.0.id(), 6, 4, "1fc18861232290221461220bd4e2acd1dcdfbc89c84092c93c18bdc7756c1588", None, None));
    check!(pyth_contract.set_price("1fc18861232290221461220bd4e2acd1dcdfbc89c84092c93c18bdc7756c1588", PythPrice{
        price: I64(100000000),
        conf: U64(103853),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));
    
    check!(logs burrowland_contract.margin_trading_open_position_by_pyth(&alice,
        nusdt_token_contract.0.id(), d(1000, 18).into(), wrap_token_contract.0.id(), d(100, 24).into(), nusdt_token_contract.0.id(), d(900, 18).into(),
        SwapIndication {
            dex_id: near_sdk::AccountId::new_unchecked(dcl_exchange_contract.0.id().to_string()),
            swap_action_text: serde_json::to_string(&RefV2TokenReceiverMessage::Swap { 
                pool_ids: vec![pool_id.clone()],
                output_token: near_sdk::AccountId::new_unchecked(nusdt_token_contract.0.id().to_string()),
                min_output_amount: U128(d(900, 6)), 
                skip_unwrap_near: Some(true), 
                client_echo: None
            }).unwrap()
        }
    ));

    check!(view burrowland_contract.get_margin_account(&alice));
    
    check!(dcl_exchange_contract.swap(&wrap_token_contract, &alice, d(600, 24), pool_id.clone(), nusdt_token_contract.0.id()));

    let alice_margin_account = burrowland_contract.get_margin_account(&alice).await?.unwrap();
    let pos_id = alice_margin_account.margin_positions.keys().collect::<Vec<&String>>()[0].clone();
    
    let position_amount = alice_margin_account.margin_positions.get(&pos_id).unwrap().token_p_amount;
    let min_out_amount = alice_margin_account.margin_positions.get(&pos_id).unwrap().token_d_info.balance + 10u128.pow(20);

    check!(pyth_contract.set_price("27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", PythPrice{
        price: I64(1900000000),
        conf: U64(278100),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));

    check!(view burrowland_contract.get_margin_account(&alice));
    check!(view burrowland_contract.get_margin_account(&root));

    check!(print burrowland_contract.margin_trading_liquidate_mtposition_by_pyth(&root,
        alice.id(), &pos_id, position_amount, min_out_amount,
        SwapIndication {
            dex_id: near_sdk::AccountId::new_unchecked(dcl_exchange_contract.0.id().to_string()),
            swap_action_text: serde_json::to_string(&RefV2TokenReceiverMessage::Swap { 
                pool_ids: vec![pool_id.clone()],
                output_token: near_sdk::AccountId::new_unchecked(wrap_token_contract.0.id().to_string()),
                min_output_amount: U128(min_out_amount), 
                skip_unwrap_near: Some(true), 
                client_echo: None
            }).unwrap()
        }
    ));
    
    check!(view burrowland_contract.get_margin_account(&alice));
    check!(view burrowland_contract.get_margin_account(&root));

    Ok(())
}


#[tokio::test]
async fn test_margin_trading_forceclose_dcl() -> Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let pyth_contract = deploy_mock_pyth(&root).await?;
    let nusdt_token_contract = deploy_mock_ft(&root, "nusdt", 18).await?;
    let wrap_token_contract = deploy_mock_ft(&root, "wrap", 18).await?;
    let wrap_reserve_amount = d(10000, 24);
    let nusdt_reserve_amount = d(10000, 6);
    check!(wrap_token_contract.ft_mint(&root, &root, wrap_reserve_amount));
    check!(nusdt_token_contract.ft_mint(&root, &root, nusdt_reserve_amount));

    let dcl_exchange_contract = deploy_mock_dcl(&root).await?;
    {
        check!(nusdt_token_contract.ft_storage_deposit(dcl_exchange_contract.0.id()));
        check!(wrap_token_contract.ft_storage_deposit(dcl_exchange_contract.0.id()));
    }

    let burrowland_contract = deploy_burrowland_with_pyth(&root).await?;
    check!(burrowland_contract.storage_deposit(&root));
    check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));
    check!(burrowland_contract.add_asset_handler(&root, &nusdt_token_contract));
    check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(nusdt_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(burrowland_contract.deposit_to_reserve(&wrap_token_contract, &root, wrap_reserve_amount));
    check!(burrowland_contract.deposit_to_reserve(&nusdt_token_contract, &root, nusdt_reserve_amount));

    let alice = create_account(&root, "alice", None).await;
    check!(dcl_exchange_contract.storage_deposit(&alice));
    check!(burrowland_contract.storage_deposit(&alice));

    assert!(nusdt_token_contract.ft_mint(&root, &alice, d(10000, 6)).await?.is_success());
    assert!(wrap_token_contract.ft_mint(&root, &alice, d(100000, 24)).await?.is_success());

    let outcome = dcl_exchange_contract.create_pool(&root, nusdt_token_contract.0.id(), wrap_token_contract.0.id(), 100, 392000, parse_near!("0.1 N")).await?;
    let pool_id = outcome.json::<String>().unwrap();

    check!(dcl_exchange_contract.deposit(&nusdt_token_contract, &alice, d(10000, 6)));
    check!(dcl_exchange_contract.deposit(&wrap_token_contract, &alice, d(100000, 24)));
    check!(view dcl_exchange_contract.get_pool(&pool_id));
    check!(view dcl_exchange_contract.list_user_assets(&alice));
    check!(dcl_exchange_contract.add_liquidity(&alice, &pool_id, 390000, 394000, d(10000, 6), d(100000, 24), 1, 1));
    check!(view dcl_exchange_contract.list_user_assets(&alice));

    let supply_amount = d(1000, 18);
    let extra_decimals_mult = d(1, 12);
    check!(nusdt_token_contract.ft_mint(&root, &alice, supply_amount * 10));
    check!(wrap_token_contract.ft_storage_deposit(alice.id()));

    check!(view burrowland_contract.get_margin_account(&alice));

    check!(burrowland_contract.deposit_to_margin(&nusdt_token_contract, &alice, supply_amount / extra_decimals_mult));
    
    check!(burrowland_contract.register_margin_dex(&root, dcl_exchange_contract.0.id(), 2));
    check!(burrowland_contract.register_margin_token(&root, nusdt_token_contract.0.id(), 0));
    check!(burrowland_contract.register_margin_token(&root, wrap_token_contract.0.id(), 1));

    // near
    let current_timestamp = worker.view_block().await?.timestamp();
    check!(burrowland_contract.add_token_pyth_info(&root, wrap_token_contract.0.id(), 24, 4, "27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", None, None));
    check!(pyth_contract.set_price("27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", PythPrice{
        price: I64(1000000000),
        conf: U64(278100),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));

    // usdt
    let current_timestamp = worker.view_block().await?.timestamp();
    check!(burrowland_contract.add_token_pyth_info(&root, nusdt_token_contract.0.id(), 6, 4, "1fc18861232290221461220bd4e2acd1dcdfbc89c84092c93c18bdc7756c1588", None, None));
    check!(pyth_contract.set_price("1fc18861232290221461220bd4e2acd1dcdfbc89c84092c93c18bdc7756c1588", PythPrice{
        price: I64(100000000),
        conf: U64(103853),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));

    check!(logs burrowland_contract.margin_trading_open_position_by_pyth(&alice,
        nusdt_token_contract.0.id(), d(1000, 18).into(), wrap_token_contract.0.id(), d(100, 24).into(), nusdt_token_contract.0.id(), d(900, 18).into(),
        SwapIndication {
            dex_id: near_sdk::AccountId::new_unchecked(dcl_exchange_contract.0.id().to_string()),
            swap_action_text: serde_json::to_string(&RefV2TokenReceiverMessage::Swap { 
                pool_ids: vec![pool_id.clone()],
                output_token: near_sdk::AccountId::new_unchecked(nusdt_token_contract.0.id().to_string()),
                min_output_amount: U128(d(900, 6)), 
                skip_unwrap_near: Some(true), 
                client_echo: None
            }).unwrap()
        }
    ));

    check!(view burrowland_contract.get_margin_account(&alice));
    
    // check!(dcl_exchange_contract.swap(&nusdt_token_contract, &alice, d(1000, 6), pool_id.clone(), wrap_token_contract.0.id()));

    let alice_margin_account = burrowland_contract.get_margin_account(&alice).await?.unwrap();
    let pos_id = alice_margin_account.margin_positions.keys().collect::<Vec<&String>>()[0].clone();
    
    
    check!(pyth_contract.set_price("27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", PythPrice{
        price: I64(2000000000),
        conf: U64(278100),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));

    let position_amount = alice_margin_account.margin_positions.get(&pos_id).unwrap().token_p_amount;
    let min_out_amount = alice_margin_account.margin_positions.get(&pos_id).unwrap().token_d_info.balance + 10u128.pow(20);

    check!(view burrowland_contract.get_asset(nusdt_token_contract.0.id()));
    check!(view burrowland_contract.get_asset(wrap_token_contract.0.id()));

    check!(print burrowland_contract.margin_trading_force_close_mtposition_by_pyth(&root,
        alice.id(), &pos_id, position_amount, min_out_amount / 2,
        SwapIndication {
            dex_id: near_sdk::AccountId::new_unchecked(dcl_exchange_contract.0.id().to_string()),
            swap_action_text: serde_json::to_string(&RefV2TokenReceiverMessage::Swap { 
                pool_ids: vec![pool_id.clone()],
                output_token: near_sdk::AccountId::new_unchecked(wrap_token_contract.0.id().to_string()),
                min_output_amount: U128(min_out_amount / 2), 
                skip_unwrap_near: Some(true), 
                client_echo: None
            }).unwrap()
        }
    ));
    
    check!(view burrowland_contract.get_margin_account(&alice));

    check!(view burrowland_contract.get_asset(nusdt_token_contract.0.id()));
    check!(view burrowland_contract.get_asset(wrap_token_contract.0.id()));
    Ok(())
}

#[tokio::test]
async fn test_margin_trading_forceclose_reserve_not_enough() -> Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let nusdt_token_contract = deploy_mock_ft(&root, "nusdt", 18).await?;
    let wrap_token_contract = deploy_mock_ft(&root, "wrap", 18).await?;
    let wrap_reserve_amount = d(20000, 24);
    check!(wrap_token_contract.ft_mint(&root, &root, wrap_reserve_amount));

    let ref_exchange_contract = deploy_ref_exchange(&root).await?;
    {
        check!(nusdt_token_contract.ft_storage_deposit(ref_exchange_contract.0.id()));
        check!(wrap_token_contract.ft_storage_deposit(ref_exchange_contract.0.id()));
        check!(ref_exchange_contract.storage_deposit(&root));
        check!(ref_exchange_contract.extend_whitelisted_tokens(&root, vec![nusdt_token_contract.0.id(), wrap_token_contract.0.id()]));
    }

    let oracle_contract = deploy_oralce(&root).await?;
    let burrowland_contract = deploy_burrowland_with_price_oracle(&root).await?;
    check!(burrowland_contract.storage_deposit(&root));
    check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));
    check!(burrowland_contract.add_asset_handler(&root, &nusdt_token_contract));
    check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(nusdt_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(burrowland_contract.deposit(&wrap_token_contract, &root, d(10000, 24)));

    let alice = create_account(&root, "alice", None).await;
    check!(ref_exchange_contract.storage_deposit(&alice));
    check!(burrowland_contract.storage_deposit(&alice));

    assert!(nusdt_token_contract.ft_mint(&root, &alice, d(10000, 6)).await?.is_success());
    assert!(wrap_token_contract.ft_mint(&root, &alice, d(100000, 24)).await?.is_success());

    check!(ref_exchange_contract.deposit(&nusdt_token_contract, &alice, d(10000, 6)));
    check!(ref_exchange_contract.deposit(&wrap_token_contract, &alice, d(10000, 24)));

    check!(ref_exchange_contract.add_simple_swap_pool(&root, vec![nusdt_token_contract.0.id(), wrap_token_contract.0.id()], 5));
    check!(ref_exchange_contract.add_simple_liquidity(&alice, 0, vec![U128(d(10000, 6)), U128(d(1000, 24))], Some(vec![U128(0), U128(0)])));

    check!(view ref_exchange_contract.get_pool(0));

    let supply_amount = d(1000, 18);
    let extra_decimals_mult = d(1, 12);
    check!(nusdt_token_contract.ft_mint(&root, &alice, supply_amount * 10));
    check!(wrap_token_contract.ft_storage_deposit(alice.id()));

    check!(view burrowland_contract.get_margin_account(&alice));

    check!(burrowland_contract.deposit_to_margin(&nusdt_token_contract, &alice, supply_amount / extra_decimals_mult));
    
    check!(burrowland_contract.register_margin_dex(&root, ref_exchange_contract.0.id(), 1));
    check!(burrowland_contract.register_margin_token(&root, nusdt_token_contract.0.id(), 0));
    check!(burrowland_contract.register_margin_token(&root, wrap_token_contract.0.id(), 1));

    let current_timestamp = worker.view_block().await?.timestamp();
    check!(logs burrowland_contract.margin_trading_open_position_by_oracle_call(
        &oracle_contract, price_data(current_timestamp, Some(100000)), &alice,
        nusdt_token_contract.0.id(), d(1000, 18).into(), wrap_token_contract.0.id(), d(100, 24).into(), nusdt_token_contract.0.id(), d(900, 18).into(),
        SwapIndication {
            dex_id: near_sdk::AccountId::new_unchecked(ref_exchange_contract.0.id().to_string()),
            swap_action_text: serde_json::to_string(&RefV1TokenReceiverMessage::Execute{
                referral_id: None,
                client_echo: None,
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
        }
    ));

    check!(view burrowland_contract.get_margin_account(&alice));
    
    let alice_margin_account = burrowland_contract.get_margin_account(&alice).await?.unwrap();
    let pos_id = alice_margin_account.margin_positions.keys().collect::<Vec<&String>>()[0].clone();
    
    let position_amount = alice_margin_account.margin_positions.get(&pos_id).unwrap().token_p_amount;
    let min_out_amount = alice_margin_account.margin_positions.get(&pos_id).unwrap().token_d_info.balance + 10u128.pow(20);

    check!(view burrowland_contract.get_asset(nusdt_token_contract.0.id()));
    check!(view burrowland_contract.get_asset(wrap_token_contract.0.id()));

    check!(logs burrowland_contract.margin_trading_force_close_mtposition_by_oracle_call(
        &oracle_contract, price_data(current_timestamp, Some(200000)), &root,
        alice.id(), &pos_id, position_amount, min_out_amount / 2,
        SwapIndication {
            dex_id: near_sdk::AccountId::new_unchecked(ref_exchange_contract.0.id().to_string()),
            swap_action_text: serde_json::to_string(&RefV1TokenReceiverMessage::Execute{
                referral_id: None,
                client_echo: None,
                actions: vec![
                    RefV1Action::Swap(RefV1SwapAction{
                        pool_id: 0,
                        token_in: near_sdk::AccountId::new_unchecked(nusdt_token_contract.0.id().to_string()),
                        amount_in: Some(U128(position_amount / 10u128.pow(12))),
                        token_out: near_sdk::AccountId::new_unchecked(wrap_token_contract.0.id().to_string()),
                        min_amount_out: U128(min_out_amount / 2),
                    })
                ]
            }).unwrap()
        }
    ));
    
    let alice_account = burrowland_contract.get_margin_account(&alice).await.unwrap().unwrap();
    assert!(alice_account.margin_positions.is_empty());

    let protocol_debts = burrowland_contract.get_all_protocol_debts().await.unwrap();
    let wrap_debt = protocol_debts.get(wrap_token_contract.0.id()).unwrap();
    check!(logs burrowland_contract.deposit_to_reserve(&wrap_token_contract, &root, 100));
    let wrap_asset = burrowland_contract.get_asset(wrap_token_contract.0.id()).await.unwrap();
    assert_eq!(wrap_asset.reserved, 0);
    let protocol_debts = burrowland_contract.list_protocol_debts(vec![wrap_token_contract.0.id()]).await.unwrap();
    assert_eq!(protocol_debts.get(wrap_token_contract.0.id()).unwrap().unwrap().0, wrap_debt.0 - 100);

    check!(logs burrowland_contract.deposit_to_reserve(&wrap_token_contract, &root, d(10000, 24) - 100));
    let wrap_asset = burrowland_contract.get_asset(wrap_token_contract.0.id()).await.unwrap();
    assert_eq!(wrap_asset.reserved, d(10000, 24) - wrap_debt.0);
    let protocol_debts = burrowland_contract.list_protocol_debts(vec![wrap_token_contract.0.id()]).await.unwrap();
    assert!(protocol_debts.get(wrap_token_contract.0.id()).unwrap().is_none());
    assert!(burrowland_contract.get_all_protocol_debts().await.unwrap().is_empty());
    Ok(())
}