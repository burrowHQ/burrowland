mod setup;

use crate::setup::*;

#[test]
fn test_upgrade() {
    let (e, tokens, users) = basic_setup_with_contract(burrowland_0_1_1_wasm_bytes());

    let amount = d(100, 24);
    e.contract_ft_transfer_call(&tokens.wnear, &users.alice, amount, "")
        .assert_success();

    let asset = e.get_asset(&tokens.wnear);
    assert_eq!(asset.supplied.balance, amount);

    let account = e.get_account(&users.alice);
    assert_eq!(account.supplied[0].balance, amount);
    assert_eq!(account.supplied[0].token_id, tokens.wnear.account_id());

    e.redeploy_latest();

    let config: Config = e
        .near
        .view_method_call(e.contract.contract.get_config())
        .unwrap_json();
    assert_eq!(config.max_num_assets, 10);
    assert_eq!(config.maximum_recency_duration_sec, 90);
    assert_eq!(config.maximum_staleness_duration_sec, 15);

    let asset = e.get_asset(&tokens.wnear);
    assert_eq!(asset.supplied.balance, amount);

    let account = e.get_account(&users.alice);
    assert_eq!(account.supplied[0].balance, amount);
}
