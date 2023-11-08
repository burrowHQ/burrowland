mod workspace_env;

use crate::workspace_env::*;

#[tokio::test]
async fn test_dev_setup() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let root = worker.root_account()?;
    
    let amount = d(10000, 24);
    let wrap_token_contract = deploy_mock_ft(&root, "wrap", 24).await?;
    check!(wrap_token_contract.ft_mint(&root, &root, amount));
    
    let burrowland_contract = deploy_burrowland(&root).await?;
    check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));
    check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));

    check!(burrowland_contract.deposit_to_reserve(&wrap_token_contract, &root, amount));

    let asset = burrowland_contract.get_asset(&wrap_token_contract).await?;
    assert_eq!(asset.reserved, d(10000, 24));
    Ok(())
}

#[tokio::test]
async fn test_supply() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let wrap_token_contract = deploy_mock_ft(&root, "wrap", 24).await?;
    
    let burrowland_contract = deploy_burrowland(&root).await?;
    check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));
    check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));

    let alice = create_account(&root, "alice", None).await;
    check!(burrowland_contract.storage_deposit(&alice));
    
    let amount = d(100, 24);
    check!(wrap_token_contract.ft_mint(&root, &alice, amount));
    check!(burrowland_contract.deposit(&wrap_token_contract, &alice, amount));

    let asset = burrowland_contract.get_asset(&wrap_token_contract).await?;
    assert_eq!(asset.supplied.balance, amount);

    let account = burrowland_contract.get_account(&alice).await?.unwrap();
    assert_eq!(account.supplied[0].balance, amount);
    assert_eq!(account.supplied[0].token_id.to_string(), wrap_token_contract.0.id().to_string());
    Ok(())
}

#[tokio::test]
async fn test_supply_to_collateral() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let wrap_token_contract = deploy_mock_ft(&root, "wrap", 24).await?;
    
    let burrowland_contract = deploy_burrowland(&root).await?;
    check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));
    check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));

    let amount = d(100, 24);

    let alice = create_account(&root, "alice", None).await;
    check!(burrowland_contract.storage_deposit(&alice));
    check!(wrap_token_contract.ft_mint(&root, &alice, amount));

    check!(burrowland_contract.supply_to_collateral(&wrap_token_contract, &alice, amount));

    let asset = burrowland_contract.get_asset(&wrap_token_contract).await?;
    assert_eq!(asset.supplied.balance, amount);

    let account = burrowland_contract.get_account(&alice).await?.unwrap();
    assert!(account.supplied.is_empty());
    assert_eq!(account.collateral[0].balance, amount);
    assert_eq!(account.collateral[0].token_id.to_string(), wrap_token_contract.0.id().to_string());
    Ok(())
}

#[tokio::test]
async fn test_withdraw_prot_fee_reserved_failed() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let wrap_token_contract = deploy_mock_ft(&root, "wrap", 24).await?;
    let ndai_token_contract = deploy_mock_ft(&root, "ndai", 18).await?;
    check!(ndai_token_contract.ft_mint(&root, &root, d(500, 18)));
    
    let oracle_contract = deploy_oralce(&root).await?;
    let burrowland_contract = deploy_burrowland(&root).await?;
    check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));
    check!(burrowland_contract.add_asset_handler(&root, &ndai_token_contract));
    check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(ndai_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(burrowland_contract.deposit_to_reserve(&ndai_token_contract, &root, d(500, 18)));

    let amount = d(100, 18);
    let supply_amount = d(100, 24);

    let alice = create_account(&root, "alice", None).await;
    check!(burrowland_contract.storage_deposit(&alice));
    check!(wrap_token_contract.ft_mint(&root, &alice, supply_amount));
    check!(ndai_token_contract.ft_mint(&root, &alice, amount));

    check!(burrowland_contract.deposit(&ndai_token_contract, &alice, amount));

    check!(burrowland_contract.supply_to_collateral(&wrap_token_contract, &alice, supply_amount));
    
    let borrow_amount = d(200, 18);
    let current_timestamp = worker.view_block().await?.timestamp();
    check!(burrowland_contract.borrow(&alice, &oracle_contract, price_data(current_timestamp, Some(100000)), ndai_token_contract.0.id(), borrow_amount));

    let asset = burrowland_contract.get_asset(&ndai_token_contract).await?;
    let mut new_asset_config = asset.config;
    new_asset_config.prot_ratio = 10000;
    check!(burrowland_contract.update_asset(&root, ndai_token_contract.0.id(), new_asset_config));
    worker.fast_forward(1000).await?;

    let asset = burrowland_contract.get_asset(&ndai_token_contract).await?;
    check!(burrowland_contract.claim_prot_fee(&root, ndai_token_contract.0.id(), Some((asset.prot_fee * 2).into())), "Asset prot_fee balance not enough!");
    check!(burrowland_contract.decrease_reserved(&root, ndai_token_contract.0.id(), Some((asset.reserved * 2).into())), "Asset reserved balance not enough!");

    check!(ndai_token_contract.ft_storage_unregister(&alice));
    check!(burrowland_contract.withdraw(&alice, ndai_token_contract.0.id(), 500), "The account alice.test.near is not registered");
    Ok(())
}


#[tokio::test]
async fn test_modify_booster_token_id_and_decimals() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let burrowland_contract = deploy_burrowland(&root).await?;

    let mut config = burrowland_contract.get_config().await?;
    config.booster_token_id = "new_booster_token_id".parse().unwrap();
    check!(burrowland_contract.update_config(&root, &config), "Can't change booster_token_id/booster_decimals");

    let mut config = burrowland_contract.get_config().await?;
    config.booster_decimals = 0;
    check!(burrowland_contract.update_config(&root, &config), "Can't change booster_token_id/booster_decimals");

    let config = burrowland_contract.get_config().await?;
    check!(burrowland_contract.update_config(&root, &config));

    Ok(())
}