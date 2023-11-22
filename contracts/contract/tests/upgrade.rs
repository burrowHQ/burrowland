mod workspace_env;

use crate::workspace_env::*;

const PREVIOUS_VERSION: &'static str = "0.8.0";
const LATEST_VERSION: &'static str = "0.9.0";

#[tokio::test]
async fn test_upgrade() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let previous_burrowland_contract = deploy_previous_version_burrowland(&root).await?;
    let version = previous_burrowland_contract.get_version().await?;
    assert_eq!(version, PREVIOUS_VERSION);

    check!(view previous_burrowland_contract.get_config_v0());

    assert!(root
        .call(previous_burrowland_contract.0.id(), "upgrade")
        .args(std::fs::read(BURROWLAND_WASM).unwrap())
        .max_gas()
        .transact()
        .await?.is_success());
    let version = previous_burrowland_contract.get_version().await?;
    assert_eq!(version, LATEST_VERSION);
    check!(view previous_burrowland_contract.get_config());
    Ok(())
}
