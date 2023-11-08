mod workspace_env;

use crate::workspace_env::*;

pub const MIN_DURATION_SEC: DurationSec = 2678400;
pub const MAX_DURATION_SEC: DurationSec = 31536000;

#[tokio::test]
async fn test_booster_stake_bad_args() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let amount = d(100, 18);
    let booster_token_contract = deploy_mock_ft(&root, "booster", 18).await?;

    let burrowland_contract = deploy_burrowland(&root).await?;
    check!(burrowland_contract.add_asset_handler(&root, &booster_token_contract));
    check!(booster_token_contract.ft_storage_deposit(burrowland_contract.0.id()));

    let alice = create_account(&root, "alice", None).await;
    check!(burrowland_contract.storage_deposit(&alice));
    check!(booster_token_contract.ft_mint(&root, &alice, amount));

    check!(burrowland_contract.deposit(&booster_token_contract, &alice, amount));

    check!(burrowland_contract.account_stake_booster(&alice, Some(0.into()), MAX_DURATION_SEC), "The amount should be greater than zero");
    check!(burrowland_contract.account_stake_booster(&alice, Some((amount + 1).into()), MAX_DURATION_SEC), "Not enough asset balance");
    check!(burrowland_contract.account_stake_booster(&alice, Some(amount.into()), MIN_DURATION_SEC - 1), "Duration is out of range");
    check!(burrowland_contract.account_stake_booster(&alice, Some(amount.into()), MAX_DURATION_SEC + 1), "Duration is out of range");
    Ok(())
}

#[tokio::test]
async fn test_booster_stake_all() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let amount = d(100, 18);
    let booster_token_contract = deploy_mock_ft(&root, "booster", 18).await?;

    let burrowland_contract = deploy_burrowland(&root).await?;
    check!(burrowland_contract.add_asset_handler(&root, &booster_token_contract));
    check!(booster_token_contract.ft_storage_deposit(burrowland_contract.0.id()));

    let alice = create_account(&root, "alice", None).await;
    check!(burrowland_contract.storage_deposit(&alice));
    check!(booster_token_contract.ft_mint(&root, &alice, amount));

    check!(burrowland_contract.deposit(&booster_token_contract, &alice, amount));

    check!(burrowland_contract.account_stake_booster(&alice, None, MAX_DURATION_SEC));
    
    let asset = burrowland_contract.get_asset(&booster_token_contract).await?;
    assert_eq!(asset.supplied.balance, 0);

    let account = burrowland_contract.get_account(&alice).await?.unwrap();
    assert!(account.supplied.is_empty());
    let booster_staking = account.booster_staking.unwrap();
    assert_eq!(booster_staking.staked_booster_amount, amount);
    assert_eq!(booster_staking.x_booster_amount, amount * 4);
    Ok(())
}
