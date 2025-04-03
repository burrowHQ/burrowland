mod workspace_env;

use crate::workspace_env::*;


#[tokio::test]
async fn test_deposit_event() -> Result<()> {

    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let amount = d(100, 18);
    let wrap_token_contract = deploy_mock_ft(&root, "wrap", 24).await?;

    let burrowland_contract = deploy_burrowland_with_price_oracle(&root).await?;
    check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));
    check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));

    let alice = create_account(&root, "alice", None).await;
    check!(burrowland_contract.storage_deposit(&alice));
    check!(wrap_token_contract.ft_mint(&root, &alice, amount));

    let outcome = burrowland_contract.deposit(&wrap_token_contract, &alice, amount).await?;
    assert!(outcome.is_success());
    let logs = outcome.logs();
    let event = &logs[3];
    assert!(event.starts_with(EVENT_JSON));

    let value: serde_json::Value =
        serde_json::from_str(&event[EVENT_JSON.len()..]).expect("Failed to parse the event");
    assert_eq!(value["standard"].as_str().unwrap(), "burrow");
    assert_eq!(value["event"].as_str().unwrap(), "deposit");
    assert_eq!(
        value["data"][0]["account_id"].as_str().unwrap(),
        alice.id().as_str()
    );
    assert_eq!(
        value["data"][0]["amount"].as_str().unwrap(),
        amount.to_string()
    );
    assert_eq!(
        value["data"][0]["token_id"].as_str().unwrap(),
        wrap_token_contract.0.id().as_str()
    );
    Ok(())
}
