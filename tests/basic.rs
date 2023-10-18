mod setup;

use crate::setup::*;

use contract::{BigDecimal, MS_PER_YEAR};

const SEC_PER_YEAR: u32 = (MS_PER_YEAR / 1000) as u32;

#[macro_use]
extern crate approx;

#[test]
fn test_init_env() {
    let e = Env::init();
    let _tokens = Tokens::init(&e);
    let _users = Users::init(&e);
}

#[test]
fn test_mint_tokens() {
    let e = Env::init();
    let tokens = Tokens::init(&e);
    let users = Users::init(&e);
    e.mint_tokens(&tokens, &users.alice);
}

#[test]
fn test_dev_setup() {
    let e = Env::init();
    let tokens = Tokens::init(&e);
    e.setup_assets(&tokens);
    e.deposit_reserves(&tokens);

    let asset = e.get_asset(&tokens.wnear);
    assert_eq!(asset.reserved, d(10000, 24));
}

#[test]
fn test_supply() {
    let (e, tokens, users) = basic_setup();

    let amount = d(100, 24);
    e.contract_ft_transfer_call(&tokens.wnear, &users.alice, amount, "")
        .assert_success();

    let asset = e.get_asset(&tokens.wnear);
    assert_eq!(asset.supplied.balance, amount);

    let account = e.get_account(&users.alice);
    assert_eq!(account.supplied[0].balance, amount);
    assert_eq!(account.supplied[0].token_id, tokens.wnear.account_id());
}

#[test]
fn test_supply_to_collateral() {
    let (e, tokens, users) = basic_setup();

    let amount = d(100, 24);
    e.supply_to_collateral(&users.alice, &tokens.wnear, amount)
        .assert_success();

    let asset = e.get_asset(&tokens.wnear);
    assert_eq!(asset.supplied.balance, amount);

    let account = e.get_account(&users.alice);
    assert!(account.supplied.is_empty());
    assert_eq!(account.collateral[0].balance, amount);
    assert_eq!(account.collateral[0].token_id, tokens.wnear.account_id());
}

#[test]
fn test_borrow() {
    let (e, tokens, users) = basic_setup();

    let supply_amount = d(100, 24);
    e.supply_to_collateral(&users.alice, &tokens.wnear, supply_amount)
        .assert_success();

    let borrow_amount = d(200, 18);
    e.borrow(
        &users.alice,
        &tokens.ndai,
        price_data(&tokens, Some(100000), None),
        borrow_amount,
    )
    .assert_success();

    let asset = e.get_asset(&tokens.ndai);
    assert_eq!(asset.borrowed.balance, borrow_amount);
    assert!(asset.borrow_apr > BigDecimal::zero());
    assert_eq!(asset.supplied.balance, borrow_amount);
    assert!(asset.supply_apr > BigDecimal::zero());

    let account = e.get_account(&users.alice);
    assert_eq!(account.supplied[0].balance, borrow_amount);
    assert_eq!(account.supplied[0].token_id, tokens.ndai.account_id());
    assert!(account.supplied[0].apr > BigDecimal::zero());
    assert_eq!(account.borrowed[0].balance, borrow_amount);
    assert_eq!(account.borrowed[0].token_id, tokens.ndai.account_id());
    assert!(account.borrowed[0].apr > BigDecimal::zero());
}

#[test]
fn test_borrow_and_withdraw() {
    let (e, tokens, users) = basic_setup();

    let supply_amount = d(100, 24);
    e.supply_to_collateral(&users.alice, &tokens.wnear, supply_amount)
        .assert_success();

    let borrow_amount = d(200, 18);
    e.borrow_and_withdraw(
        &users.alice,
        &tokens.ndai,
        price_data(&tokens, Some(100000), None),
        borrow_amount,
    )
    .assert_success();

    let asset = e.get_asset(&tokens.ndai);
    assert_eq!(asset.borrowed.balance, borrow_amount);
    assert!(asset.borrow_apr > BigDecimal::zero());
    assert_eq!(asset.supplied.balance, 0);
    assert_eq!(asset.supply_apr, BigDecimal::zero());

    let account = e.get_account(&users.alice);
    assert!(account.supplied.is_empty());
    assert_eq!(account.borrowed[0].balance, borrow_amount);
    assert_eq!(account.borrowed[0].token_id, tokens.ndai.account_id());
    assert!(account.borrowed[0].apr > BigDecimal::zero());
}

#[test]
fn test_interest() {
    let (e, tokens, users) = basic_setup();

    let supply_amount = d(10000, 24);
    e.supply_to_collateral(&users.alice, &tokens.wnear, supply_amount)
        .assert_success();

    let borrow_amount = d(8000, 18);
    e.borrow_and_withdraw(
        &users.alice,
        &tokens.ndai,
        price_data(&tokens, Some(100000), None),
        borrow_amount,
    )
    .assert_success();

    let asset = e.get_asset(&tokens.ndai);
    assert_eq!(asset.borrowed.balance, borrow_amount);
    assert_relative_eq!(asset.borrow_apr.f64(), 0.08f64);

    e.skip_time(SEC_PER_YEAR);

    let expected_borrow_amount = borrow_amount * 108 / 100;

    let asset = e.get_asset(&tokens.ndai);
    assert_relative_eq!(asset.borrowed.balance as f64, expected_borrow_amount as f64);

    let account = e.get_account(&users.alice);
    assert_relative_eq!(
        account.borrowed[0].balance as f64,
        expected_borrow_amount as f64
    );
    assert_eq!(account.borrowed[0].token_id, tokens.ndai.account_id());
}

#[test]
fn test_withdraw_prot_fee_reserved() {
    let (e, tokens, users) = basic_setup();

    let amount = d(100, 18);
    e.contract_ft_transfer_call(&tokens.ndai, &users.alice, amount, "")
        .assert_success();

    let supply_amount = d(100, 24);
    e.supply_to_collateral(&users.alice, &tokens.wnear, supply_amount)
        .assert_success();

    let borrow_amount = d(200, 18);
    e.borrow(
        &users.alice,
        &tokens.ndai,
        price_data(&tokens, Some(100000), None),
        borrow_amount,
    )
    .assert_success();

    e.skip_time(31536000);

    let asset_view_old = e.get_asset(&tokens.ndai);
    assert_eq!(asset_view_old.prot_fee, 0);

    let mut new_config = asset_view_old.config;
    new_config.prot_ratio = 10000;
    e.update_asset(&tokens.ndai, new_config.clone());
    e.skip_time(31536000);

    let asset = e.get_asset(&tokens.ndai);
    assert_eq!(asset_view_old.reserved, asset.reserved);

    e.claim_prot_fee(&tokens.ndai, Some(10000.into())).assert_success();
    
    let asset_after_decrease_prot_fee = e.get_asset(&tokens.ndai);
    assert_eq!(asset.prot_fee - 10000, asset_after_decrease_prot_fee.prot_fee);
    assert_eq!(asset.supplied.balance + 10000, asset_after_decrease_prot_fee.supplied.balance);
    assert_eq!(asset.supplied.shares.0 + asset.supplied.amount_to_shares(10000, false).0, 
        asset_after_decrease_prot_fee.supplied.shares.0);

    e.decrease_reserved(&tokens.ndai, Some(10000.into())).assert_success();
    let asset_after_decrease_reserved = e.get_asset(&tokens.ndai);
    assert_eq!(asset_after_decrease_prot_fee.reserved - 10000, asset_after_decrease_reserved.reserved);
    assert_eq!(asset_after_decrease_prot_fee.supplied.balance + 10000, asset_after_decrease_reserved.supplied.balance);
    assert_eq!(asset_after_decrease_prot_fee.supplied.shares.0 + asset.supplied.amount_to_shares(10000, false).0, 
        asset_after_decrease_reserved.supplied.shares.0);
    
    let old_balance = e.ft_balance_of(&tokens.ndai, &e.owner).0;
    e.withdraw(&tokens.ndai, 10000).assert_success();
    let current_balance = e.ft_balance_of(&tokens.ndai, &e.owner).0;
    assert_eq!(10000, current_balance - old_balance);

    assert!(format!("{:?}", e.claim_prot_fee(&tokens.ndai, Some((asset_after_decrease_reserved.prot_fee * 2).into()))
        .promise_errors()[0].as_ref().unwrap().status()).contains("Asset prot_fee balance not enough!"));

    assert!(format!("{:?}", e.decrease_reserved(&tokens.ndai, Some((asset_after_decrease_reserved.reserved * 2).into()))
        .promise_errors()[0].as_ref().unwrap().status()).contains("Asset reserved balance not enough!"));

    e.increase_reserved(AssetAmount{
        token_id: tokens.ndai.account_id(),
        amount: Some(500.into()),
        max_amount: None
    }).assert_success();

    let asset_after_increase_reserved = e.get_asset(&tokens.ndai);
    assert_eq!(asset_after_decrease_reserved.reserved + 500, asset_after_increase_reserved.reserved);
    assert_eq!(asset_after_decrease_reserved.supplied.balance - 10500, asset_after_increase_reserved.supplied.balance);

    e.owner.call(
        tokens.ndai.account_id(),
        "storage_unregister",
        &near_sdk::serde_json::json!({
            "force": true
        })
            .to_string()
            .into_bytes(),
        DEFAULT_GAS.0,
        1,
    )
    .assert_success();

    let asset_before_withdraw = e.get_asset(&tokens.ndai);
    assert!(format!("{:?}", e.withdraw(&tokens.ndai, 500)
        .promise_errors()[0].as_ref().unwrap().status()).contains("The account owner.near is not registered"));

    assert_eq!(e.get_asset(&tokens.ndai).supplied.balance, asset_before_withdraw.supplied.balance);
}
