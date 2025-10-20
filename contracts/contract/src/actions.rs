use std::collections::HashSet;

use crate::*;

#[derive(Deserialize, Serialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
#[serde(crate = "near_sdk::serde")]
pub struct AssetAmount {
    pub token_id: TokenId,
    /// The amount of tokens intended to be used for the action.
    /// If `None`, then the maximum amount will be tried.
    pub amount: Option<U128>,
    /// The maximum amount of tokens that can be used for the action.
    /// If `None`, then the maximum `available` amount will be used.
    pub max_amount: Option<U128>,
}

#[derive(Deserialize, Serialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
#[serde(crate = "near_sdk::serde")]
pub enum Action {
    Withdraw(AssetAmount),
    ClientEchoWithdraw{
        client_echo: String,
        asset_amount: AssetAmount,
    },
    IncreaseCollateral(AssetAmount),
    PositionIncreaseCollateral{
        position: String,
        asset_amount: AssetAmount
    },
    DecreaseCollateral(AssetAmount),
    PositionDecreaseCollateral{
        position: String,
        asset_amount: AssetAmount
    },
    Borrow(AssetAmount),
    PositionBorrow{
        position: String,
        asset_amount: AssetAmount
    },
    Repay(AssetAmount),
    PositionRepay{
        position: String,
        asset_amount: AssetAmount
    },
    Liquidate {
        account_id: AccountId,
        in_assets: Vec<AssetAmount>,
        out_assets: Vec<AssetAmount>,
        position: Option<String>,
        min_token_amounts: Option<Vec<U128>>
    },
    /// If the sum of burrowed assets exceeds the collateral, the account will be liquidated
    /// using reserves.
    ForceClose {
        account_id: AccountId,
        position: Option<String>,
        min_token_amounts: Option<Vec<U128>>
    },
    LiquidateMTPositionDirect {
        pos_owner_id: AccountId,
        pos_id: PosId,
    },
}

impl Contract {
    pub fn internal_execute(
        &mut self,
        account_id: &AccountId,
        account: &mut Account,
        actions: Vec<Action>,
        prices: Prices,
    ) {
        // Set reliable liquidator context if signer is in whitelist
        self.is_reliable_liquidator_context = in_reliable_liquidator_whitelist(&env::signer_account_id().to_string());

        self.internal_set_prices(&prices);
        let mut need_number_check = false;
        let mut risk_check_positions = HashSet::new();
        for action in actions {
            assert!(!account.is_locked, "Account is locked!");
            match action {
                Action::Withdraw(asset_amount) => {
                    assert!(!asset_amount.token_id.to_string().starts_with(SHADOW_V1_TOKEN_PREFIX));
                    if account.supplied.get(&asset_amount.token_id).is_some() {
                        let (amount, ft_amount) = self.internal_withdraw(account, &asset_amount);
                        if ft_amount > 0 {
                            self.internal_ft_transfer(account_id, &asset_amount.token_id, amount, ft_amount, false, account_id);
                            events::emit::withdraw_started(&account_id, amount, &asset_amount.token_id);
                        } else {
                            events::emit::withdraw_succeeded(&account_id, amount, &asset_amount.token_id);
                        }
                    }
                }
                Action::ClientEchoWithdraw { client_echo, asset_amount } => {
                    assert!(in_client_echo_sender_whitelist(account_id.as_str()), "Unauthorized client echo sender: {}", account_id);
                    assert!(!asset_amount.token_id.to_string().starts_with(SHADOW_V1_TOKEN_PREFIX));
                    if account.supplied.get(&asset_amount.token_id).is_some() {
                        let (amount, ft_amount) = self.internal_withdraw(account, &asset_amount);
                        if ft_amount > 0 {
                            self.internal_ft_transfer_call(account_id, &asset_amount.token_id, amount, ft_amount, client_echo);
                            events::emit::withdraw_started(&account_id, amount, &asset_amount.token_id);
                        } else {
                            events::emit::withdraw_succeeded(&account_id, amount, &asset_amount.token_id);
                        }
                    }
                }
                Action::IncreaseCollateral(asset_amount) => {
                    need_number_check = true;
                    assert!(!asset_amount.token_id.to_string().starts_with(SHADOW_V1_TOKEN_PREFIX));
                    let position = REGULAR_POSITION.to_string();
                    let amount = self.internal_increase_collateral(&position, account, &asset_amount);
                    events::emit::increase_collateral(&account_id, amount, &asset_amount.token_id, &position);
                }
                Action::PositionIncreaseCollateral { position, asset_amount } => {
                    need_number_check = true;
                    if position == REGULAR_POSITION {
                        assert!(!asset_amount.token_id.to_string().starts_with(SHADOW_V1_TOKEN_PREFIX));
                    } else {
                        assert!(asset_amount.token_id.to_string() == position);
                    }
                    let amount = self.internal_increase_collateral(&position, account, &asset_amount);
                    events::emit::increase_collateral(&account_id, amount, &asset_amount.token_id, &position);
                }
                Action::DecreaseCollateral(asset_amount) => {
                    let position = REGULAR_POSITION.to_string();
                    assert!(!asset_amount.token_id.to_string().starts_with(SHADOW_V1_TOKEN_PREFIX));
                    risk_check_positions.insert(position.clone());
                    let mut account_asset =
                        account.internal_get_asset_or_default(&asset_amount.token_id);
                    let amount = self.internal_decrease_collateral(
                        &position,
                        &mut account_asset,
                        account,
                        &asset_amount,
                    );
                    account.internal_set_asset(&asset_amount.token_id, account_asset);
                    events::emit::decrease_collateral(&account_id, amount, &asset_amount.token_id, &position);
                }
                Action::PositionDecreaseCollateral { position, asset_amount } => {
                    if position == REGULAR_POSITION {
                        assert!(!asset_amount.token_id.to_string().starts_with(SHADOW_V1_TOKEN_PREFIX));
                    } else {
                        assert!(asset_amount.token_id.to_string() == position);
                    }
                    risk_check_positions.insert(position.clone());
                    let mut account_asset =
                        account.internal_get_asset_or_default(&asset_amount.token_id);
                    let amount = self.internal_decrease_collateral(
                        &position,
                        &mut account_asset,
                        account,
                        &asset_amount,
                    );
                    account.internal_set_asset(&asset_amount.token_id, account_asset);
                    events::emit::decrease_collateral(&account_id, amount, &asset_amount.token_id, &position);
                }
                Action::Borrow(asset_amount) => {
                    need_number_check = true;
                    let position = REGULAR_POSITION.to_string();
                    risk_check_positions.insert(position.clone());
                    account.add_affected_farm(FarmId::Borrowed(asset_amount.token_id.clone()));
                    let amount = self.internal_borrow(&position, account, &asset_amount);
                    events::emit::borrow(&account_id, amount, &asset_amount.token_id, &position);
                }
                Action::PositionBorrow{ position, asset_amount } => {
                    need_number_check = true;
                    risk_check_positions.insert(position.clone());
                    account.add_affected_farm(FarmId::Borrowed(asset_amount.token_id.clone()));
                    let amount = self.internal_borrow(&position, account, &asset_amount);
                    events::emit::borrow(&account_id, amount, &asset_amount.token_id, &position);
                }
                Action::Repay(asset_amount) => {
                    let position = REGULAR_POSITION.to_string();
                    let amount = self.internal_owner_repay(&position, account, &asset_amount);
                    account.add_affected_farm(FarmId::Borrowed(asset_amount.token_id.clone()));
                    events::emit::repay(&account_id, amount, &asset_amount.token_id, &position);
                }
                Action::PositionRepay{ position, asset_amount} => {
                    let amount = self.internal_owner_repay(&position, account, &asset_amount);
                    account.add_affected_farm(FarmId::Borrowed(asset_amount.token_id.clone()));
                    events::emit::repay(&account_id, amount, &asset_amount.token_id, &position);
                }
                Action::Liquidate {
                    account_id: liquidation_account_id,
                    in_assets,
                    out_assets,
                    position,
                    min_token_amounts
                } => {
                    assert_ne!(
                        account_id, &liquidation_account_id,
                        "Can't liquidate yourself"
                    );
                    assert!(!self.internal_get_account(&liquidation_account_id, true).expect("Account is not registered").is_locked, "Liquidation account is locked!");
                    let position = position.unwrap_or(REGULAR_POSITION.to_string());
                    if position == REGULAR_POSITION {
                        assert!(!in_assets.is_empty() && !out_assets.is_empty());
                        assert!(min_token_amounts.is_none());
                        self.internal_liquidate(
                            account_id,
                            account,
                            &prices,
                            &liquidation_account_id,
                            in_assets,
                            out_assets,
                        );
                    } else {
                        assert!(!in_assets.is_empty()
                            && out_assets.len() == 1 && out_assets[0].token_id.to_string() == position);
                        let min_token_amounts = min_token_amounts.expect("Missing min_token_amounts");
                        assert!(min_token_amounts.len() == self.last_lp_token_infos.get(&position).unwrap().tokens.len(), "Invalid min_token_amounts");
                        let mut in_asset_tokens = HashSet::new();
                        in_assets.iter().for_each(|v| assert!(in_asset_tokens.insert(&v.token_id), "Duplicate assets!"));
                        let mut temp_account = account.clone();
                        temp_account.storage_tracker.clean();
                        self.internal_shadow_liquidate(
                            &position,
                            account_id,
                            &mut temp_account,
                            &prices,
                            &liquidation_account_id,
                            in_assets,
                            out_assets,
                            min_token_amounts
                        );
                        let mut liquidation_account = self.internal_unwrap_account(&liquidation_account_id);
                        liquidation_account.is_locked = true;
                        account.is_locked = true;
                        self.internal_set_account(&liquidation_account_id, liquidation_account);
                    }
                }
                Action::ForceClose {
                    account_id: liquidation_account_id,
                    position,
                    min_token_amounts
                } => {
                    assert_ne!(
                        account_id, &liquidation_account_id,
                        "Can't liquidate yourself"
                    );
                    assert!(!self.internal_get_account(&liquidation_account_id, true).expect("Account is not registered").is_locked, "Liquidation account is locked!");
                    let position = position.unwrap_or(REGULAR_POSITION.to_string());
                    if position == REGULAR_POSITION {
                        assert!(min_token_amounts.is_none());
                        self.internal_force_close(&prices, &liquidation_account_id);
                    } else {
                        let min_token_amounts = min_token_amounts.expect("Missing min_token_amounts");
                        assert!(min_token_amounts.len() == self.last_lp_token_infos.get(&position).unwrap().tokens.len(), "Invalid min_token_amounts");
                        self.internal_shadow_force_close(&position, &prices, &liquidation_account_id, min_token_amounts);
                        let mut liquidation_account = self.internal_unwrap_account(&liquidation_account_id);
                        liquidation_account.is_locked = true;
                        self.internal_set_account(&liquidation_account_id, liquidation_account);
                    }
                }
                Action::LiquidateMTPositionDirect {
                    pos_owner_id,
                    pos_id,
                } => {
                    assert_ne!(
                        account_id, &pos_owner_id,
                        "Can't liquidate yourself"
                    );
                    self.process_margin_liquidate_direct(
                        &pos_owner_id, 
                        &pos_id,
                        &prices, 
                        account
                    );
                }
            }
        }
        if need_number_check {
            assert!(
                account.get_assets_num() <= self.internal_config().max_num_assets
            );
        }
        for position in risk_check_positions {
            assert!(self.compute_max_discount(&position, account, &prices) == BigDecimal::zero());
        }

        self.internal_account_apply_affected_farms(account);
    }

    pub fn internal_deposit(
        &mut self,
        account: &mut Account,
        token_id: &TokenId,
        amount: Balance,
    ) -> Shares {
        let mut asset = self.internal_unwrap_asset(token_id);
        let mut account_asset = account.internal_get_asset_or_default(token_id);

        let shares: Shares = asset.supplied.amount_to_shares(amount, false);

        account_asset.deposit_shares(shares);
        account.internal_set_asset(&token_id, account_asset);

        asset.supplied.deposit(shares, amount);
        self.internal_set_asset(token_id, asset);

        shares
    }

    pub fn internal_deposit_without_asset_basic_check(
        &mut self,
        account: &mut Account,
        token_id: &TokenId,
        amount: Balance,
    ) -> Shares {
        let mut asset = self.internal_unwrap_asset(token_id);
        let mut account_asset = account.internal_get_asset_or_default(token_id);

        let shares: Shares = asset.supplied.amount_to_shares(amount, false);

        account_asset.deposit_shares(shares);
        account.internal_set_asset(&token_id, account_asset);

        asset.supplied.deposit(shares, amount);
        self.internal_set_asset_without_asset_basic_check(token_id, asset);

        shares
    }

    pub fn internal_withdraw(
        &mut self,
        account: &mut Account,
        asset_amount: &AssetAmount,
    ) -> (Balance, Balance) {
        let mut asset = self.internal_unwrap_asset(&asset_amount.token_id);
        assert!(
            asset.config.can_withdraw,
            "Withdrawals for this asset are not enabled"
        );

        let mut account_asset = account.internal_unwrap_asset(&asset_amount.token_id);

        let (shares, amount) =
            asset_amount_to_shares(&asset.supplied, account_asset.shares, &asset_amount, false);

        let available_amount = asset.available_amount();

        assert!(
            amount <= available_amount,
            "Withdraw error: Exceeded available amount {} of {}",
            available_amount,
            &asset_amount.token_id
        );

        let ft_amount = amount / 10u128.pow(asset.config.extra_decimals as u32);
        if ft_amount > 0 {
            account_asset.withdraw_shares(shares);
            account.internal_set_asset(&asset_amount.token_id, account_asset);

            asset.supplied.withdraw(shares, amount);
            self.internal_set_asset(&asset_amount.token_id, asset);
            (amount, ft_amount)
        } else {
            (0, 0)
        }
    }

    pub fn internal_increase_collateral(
        &mut self,
        position: &String,
        account: &mut Account,
        asset_amount: &AssetAmount,
    ) -> Balance {
        let asset = self.internal_unwrap_asset(&asset_amount.token_id);
        assert!(
            asset.config.can_use_as_collateral,
            "This asset can't be used as a collateral"
        );
        // check if supply limit has hit, then need panic here
        if !self.is_reliable_liquidator_context {
            if let Some(supplied_limit) = asset.config.supplied_limit {
                assert!(
                    asset.supplied.balance <= supplied_limit.0, 
                    "Asset {} has hit supply limit, increasing collateral is not allowed", &asset_amount.token_id
                );
            }
        }
        
        let mut account_asset = account.internal_unwrap_asset(&asset_amount.token_id);

        let (shares, amount) =
            asset_amount_to_shares(&asset.supplied, account_asset.shares, &asset_amount, false);

        account_asset.withdraw_shares(shares);
        account.internal_set_asset(&asset_amount.token_id, account_asset);

        account.increase_collateral(position, &asset_amount.token_id, shares);

        amount
    }

    pub fn internal_decrease_collateral(
        &mut self,
        position: &String,
        account_asset: &mut AccountAsset,
        account: &mut Account,
        asset_amount: &AssetAmount,
    ) -> Balance {
        let asset = self.internal_unwrap_asset(&asset_amount.token_id);
        let collateral_shares = account.internal_unwrap_collateral(position, &asset_amount.token_id);

        let (shares, amount) =
            asset_amount_to_shares(&asset.supplied, collateral_shares, &asset_amount, false);

        account.decrease_collateral(position, &asset_amount.token_id, shares);

        account_asset.deposit_shares(shares);

        amount
    }

    pub fn internal_borrow(
        &mut self,
        position: &String,
        account: &mut Account,
        asset_amount: &AssetAmount,
    ) -> Balance {
        let mut asset = self.internal_unwrap_asset(&asset_amount.token_id);
        assert!(asset.config.can_borrow, "Thi asset can't be used borrowed");

        let mut account_asset = account.internal_get_asset_or_default(&asset_amount.token_id);

        let available_amount = asset.available_amount();
        let max_borrow_shares = asset.borrowed.amount_to_shares(available_amount, false);

        let (borrowed_shares, amount) =
            asset_amount_to_shares(&asset.borrowed, max_borrow_shares, &asset_amount, false);

        assert!(
            amount <= available_amount,
            "Borrow error: Exceeded available amount {} of {}",
            available_amount,
            &asset_amount.token_id
        );

        // check if borrow limit has hit, then need panic here
        if !self.is_reliable_liquidator_context {
            if let Some(borrowed_limit) = asset.config.borrowed_limit {
                assert!(
                    asset.borrowed.balance + asset.margin_debt.balance + asset.margin_pending_debt + amount <= borrowed_limit.0, 
                    "Asset {} has hit borrow limit, new borrow is not allowed", &asset_amount.token_id
                );
            }
        }

        let supplied_shares: Shares = asset.supplied.amount_to_shares(amount, false);

        asset.borrowed.deposit(borrowed_shares, amount);
        asset.supplied.deposit(supplied_shares, amount);
        self.internal_set_asset(&asset_amount.token_id, asset);

        account.increase_borrowed(position, &asset_amount.token_id, borrowed_shares);

        account_asset.deposit_shares(supplied_shares);
        account.internal_set_asset(&asset_amount.token_id, account_asset);

        amount
    }

    pub fn internal_owner_repay(
        &mut self,
        position: &String,
        account: &mut Account,
        asset_amount: &AssetAmount,
    ) -> Balance {
        if asset_amount.token_id == *ETH_OLD_ACCOUNT_ID {
            let mut account_asset = account.internal_unwrap_asset(&ETH_NEW_ACCOUNT_ID);
            // FIX-ETH: repay the old eth debt using the supplied new eth.
            let amount = self.internal_repay_old_eth(&position, &mut account_asset, account, &asset_amount);
            account.internal_set_asset(&ETH_NEW_ACCOUNT_ID, account_asset);
            amount
        } else {
            let mut account_asset = account.internal_unwrap_asset(&asset_amount.token_id);
            let amount = self.internal_repay(&position, &mut account_asset, account, &asset_amount);
            account.internal_set_asset(&asset_amount.token_id, account_asset);
            amount
        }
    }

    pub fn internal_liquidate_repay(
        &mut self,
        position: &String,
        account: &mut Account,
        liquidation_account: &mut Account,
        asset_amount: &AssetAmount,
    ) -> Balance {
        if asset_amount.token_id == *ETH_OLD_ACCOUNT_ID {
            let mut account_asset = account.internal_unwrap_asset(&ETH_NEW_ACCOUNT_ID);
            // FIX-ETH: repay the old eth debt using the supplied new eth.
            let amount = self.internal_repay_old_eth(&position, &mut account_asset, liquidation_account, &asset_amount);
            account.internal_set_asset(&ETH_NEW_ACCOUNT_ID, account_asset);
            amount
        } else {
            let mut account_asset = account.internal_unwrap_asset(&asset_amount.token_id);
            let amount = self.internal_repay(&position, &mut account_asset, liquidation_account, &asset_amount);
            account.internal_set_asset(&asset_amount.token_id, account_asset);
            amount
        }
    }

    pub fn internal_repay(
        &mut self,
        position: &String,
        account_asset: &mut AccountAsset,
        account: &mut Account,
        asset_amount: &AssetAmount,
    ) -> Balance {
        let mut asset = self.internal_unwrap_asset(&asset_amount.token_id);
        let available_borrowed_shares = account.internal_unwrap_borrowed(position, &asset_amount.token_id);

        let (mut borrowed_shares, mut amount) = asset_amount_to_shares(
            &asset.borrowed,
            available_borrowed_shares,
            &asset_amount,
            true,
        );

        let mut supplied_shares = asset.supplied.amount_to_shares(amount, true);
        if supplied_shares.0 > account_asset.shares.0 {
            supplied_shares = account_asset.shares;
            amount = asset.supplied.shares_to_amount(supplied_shares, false);
            if let Some(min_amount) = &asset_amount.amount {
                assert!(amount >= min_amount.0, "Not enough supplied balance");
            }
            assert!(amount > 0, "Repayment amount can't be 0");

            borrowed_shares = asset.borrowed.amount_to_shares(amount, false);
            assert!(borrowed_shares.0 > 0, "Shares can't be 0");
            assert!(borrowed_shares.0 <= available_borrowed_shares.0, "Repaying shares exceeds available borrowed shares");
        }

        asset.supplied.withdraw(supplied_shares, amount);
        asset.borrowed.withdraw(borrowed_shares, amount);
        self.internal_set_asset(&asset_amount.token_id, asset);

        account.decrease_borrowed(position, &asset_amount.token_id, borrowed_shares);

        account_asset.withdraw_shares(supplied_shares);

        amount
    }

    pub fn internal_repay_old_eth(
        &mut self,
        position: &String,
        account_asset: &mut AccountAsset,
        account: &mut Account,
        asset_amount: &AssetAmount,
    ) -> Balance {
        let mut borrowed_asset = self.internal_unwrap_asset(&ETH_OLD_ACCOUNT_ID);
        let mut supplied_asset = self.internal_unwrap_asset(&ETH_NEW_ACCOUNT_ID);

        let available_borrowed_shares = account.internal_unwrap_borrowed(position, &ETH_OLD_ACCOUNT_ID);

        let (mut borrowed_shares, mut amount) = asset_amount_to_shares(
            &borrowed_asset.borrowed,
            available_borrowed_shares,
            &asset_amount,
            true,
        );

        let mut supplied_shares = supplied_asset.supplied.amount_to_shares(amount, true);
        if supplied_shares.0 > account_asset.shares.0 {
            supplied_shares = account_asset.shares;
            amount = supplied_asset.supplied.shares_to_amount(supplied_shares, false);
            if let Some(min_amount) = &asset_amount.amount {
                assert!(amount >= min_amount.0, "Not enough supplied balance");
            }
            assert!(amount > 0, "Repayment amount can't be 0");

            borrowed_shares = borrowed_asset.borrowed.amount_to_shares(amount, false);
            assert!(borrowed_shares.0 > 0, "Shares can't be 0");
            assert!(borrowed_shares.0 <= available_borrowed_shares.0, "Repaying shares exceeds available borrowed shares");
        }

        supplied_asset.supplied.withdraw(supplied_shares, amount);
        borrowed_asset.borrowed.withdraw(borrowed_shares, amount);
        self.internal_set_asset(&ETH_OLD_ACCOUNT_ID, borrowed_asset);
        self.internal_set_asset(&ETH_NEW_ACCOUNT_ID, supplied_asset);

        account.decrease_borrowed(position, &ETH_OLD_ACCOUNT_ID, borrowed_shares);

        account_asset.withdraw_shares(supplied_shares);

        amount
    }

    pub fn internal_liquidate(
        &mut self,
        account_id: &AccountId,
        account: &mut Account,
        prices: &Prices,
        liquidation_account_id: &AccountId,
        in_assets: Vec<AssetAmount>,
        out_assets: Vec<AssetAmount>,
    ) {
        let position = REGULAR_POSITION.to_string();
        let mut liquidation_account = self.internal_unwrap_account(liquidation_account_id);

        let max_discount = self.compute_max_discount(&position, &liquidation_account, &prices);
        assert!(
            max_discount > BigDecimal::zero(),
            "The liquidation account is not at risk"
        );

        let mut borrowed_repaid_sum = BigDecimal::zero();
        let mut collateral_taken_sum = BigDecimal::zero();

        for asset_amount in in_assets {
            liquidation_account.add_affected_farm(FarmId::Borrowed(asset_amount.token_id.clone()));
            liquidation_account.add_affected_farm(FarmId::TokenNetBalance(asset_amount.token_id.clone()));
            let amount = self.internal_liquidate_repay(&position, account, &mut liquidation_account, &asset_amount);
            let asset = self.internal_unwrap_asset(&asset_amount.token_id);

            borrowed_repaid_sum = borrowed_repaid_sum
                + BigDecimal::from_balance_price(
                    amount,
                    prices.get_unwrap(&asset_amount.token_id),
                    asset.config.extra_decimals,
                );
        }

        for asset_amount in out_assets {
            let asset = self.internal_unwrap_asset(&asset_amount.token_id);
            liquidation_account.add_affected_farm(FarmId::Supplied(asset_amount.token_id.clone()));
            liquidation_account.add_affected_farm(FarmId::TokenNetBalance(asset_amount.token_id.clone()));
            let mut account_asset = account.internal_get_asset_or_default(&asset_amount.token_id);
            let amount = self.internal_decrease_collateral(
                &position,
                &mut account_asset,
                &mut liquidation_account,
                &asset_amount,
            );
            account.internal_set_asset(&asset_amount.token_id, account_asset);

            collateral_taken_sum = collateral_taken_sum
                + BigDecimal::from_balance_price(
                    amount,
                    prices.get_unwrap(&asset_amount.token_id),
                    asset.config.extra_decimals,
                );
        }

        let discounted_collateral_taken = collateral_taken_sum * (BigDecimal::one() - max_discount);
        assert!(
            discounted_collateral_taken <= borrowed_repaid_sum,
            "Not enough balances repaid: discounted collateral {} > borrowed repaid sum {}",
            discounted_collateral_taken,
            borrowed_repaid_sum
        );

        let new_max_discount = self.compute_max_discount(&position, &liquidation_account, &prices);
        assert!(
            new_max_discount > BigDecimal::zero(),
            "The liquidation amount is too large. The liquidation account should stay in risk"
        );
        assert!(
            new_max_discount < max_discount,
            "The health factor of liquidation account can't decrease. New discount {} < old discount {}",
            new_max_discount, max_discount
        );

        self.internal_account_apply_affected_farms(&mut liquidation_account);
        self.internal_set_account(liquidation_account_id, liquidation_account);

        events::emit::liquidate(
            &account_id,
            &liquidation_account_id,
            &collateral_taken_sum,
            &borrowed_repaid_sum,
            &max_discount,
            &new_max_discount,
            &position
        );
    }

    pub fn internal_force_close(&mut self, prices: &Prices, liquidation_account_id: &AccountId) {
        let position = REGULAR_POSITION.to_string();
        let config = self.internal_config();
        assert!(
            config.force_closing_enabled,
            "The force closing is not enabled"
        );

        let mut liquidation_account = self.internal_unwrap_account(liquidation_account_id);
        let discount = self.compute_max_discount(&position, &liquidation_account, &prices);

        let mut borrowed_sum = BigDecimal::zero();
        let mut collateral_sum = BigDecimal::zero();
        let mut collateral_assets = HashMap::new();
        let mut borrowed_assets = HashMap::new();

        let mut affected_farms = vec![];

        if let Position::RegularPosition(mut regular_position) = liquidation_account.positions.remove(&position).expect("Position not found") {
            for (token_id, shares) in regular_position.collateral.drain() {
                let mut asset = self.internal_unwrap_asset(&token_id);
                let amount = asset.supplied.shares_to_amount(shares, false);
                asset.reserved += amount;
                asset.supplied.withdraw(shares, amount);
    
                collateral_assets.insert(token_id.clone(), amount.into());
                collateral_sum = collateral_sum
                    + BigDecimal::from_balance_price(
                        amount,
                        prices.get_unwrap(&token_id),
                        asset.config.extra_decimals,
                    );
                self.internal_set_asset(&token_id, asset);
                affected_farms.push(FarmId::Supplied(token_id.clone()));
                affected_farms.push(FarmId::TokenNetBalance(token_id));
            }
    
            for (token_id, shares) in regular_position.borrowed.drain() {
                let mut asset = self.internal_unwrap_asset(&token_id);
                let amount = asset.borrowed.shares_to_amount(shares, true);
                assert!(
                    asset.reserved >= amount,
                    "Not enough {} in reserve",
                    token_id
                );
                asset.reserved -= amount;
                asset.borrowed.withdraw(shares, amount);
    
                borrowed_assets.insert(token_id.clone(), amount.into());
                borrowed_sum = borrowed_sum
                    + BigDecimal::from_balance_price(
                        amount,
                        prices.get_unwrap(&token_id),
                        asset.config.extra_decimals,
                    );
                self.internal_set_asset(&token_id, asset);
                affected_farms.push(FarmId::Borrowed(token_id.clone()));
                affected_farms.push(FarmId::TokenNetBalance(token_id));
            }
    
            assert!(
                borrowed_sum > collateral_sum,
                "Total borrowed sum {} is not greater than total collateral sum {}",
                borrowed_sum,
                collateral_sum
            );
            liquidation_account.affected_farms.extend(affected_farms);
    
            self.internal_account_apply_affected_farms(&mut liquidation_account);
            self.internal_set_account(liquidation_account_id, liquidation_account);
    
            events::emit::force_close(&liquidation_account_id, &collateral_sum, &borrowed_sum, collateral_assets, borrowed_assets, &discount, &position);
        } else {
            env::panic_str("Internal error");
        }
    }

    pub fn compute_max_discount(&self, position: &String, account: &Account, prices: &Prices) -> BigDecimal {
        if let Some(position_info) = account.positions.get(position) {
            if position_info.is_no_borrowed() {
                return BigDecimal::zero();
            }
    
            let collateral_sum = self.get_collateral_sum_with_volatility_ratio(position_info, prices);
    
            let borrowed_sum = self.get_borrowed_sum_with_volatility_ratio(position_info, prices);
            
            if borrowed_sum <= collateral_sum {
                BigDecimal::zero()
            } else {
                (borrowed_sum - collateral_sum) / borrowed_sum / BigDecimal::from(2u32)
            }
        } else {
            BigDecimal::zero()
        }
    }
}

pub fn asset_amount_to_shares(
    pool: &Pool,
    available_shares: Shares,
    asset_amount: &AssetAmount,
    inverse_round_direction: bool,
) -> (Shares, Balance) {
    let (shares, amount) = if let Some(amount) = &asset_amount.amount {
        (
            pool.amount_to_shares(amount.0, !inverse_round_direction),
            amount.0,
        )
    } else if let Some(max_amount) = &asset_amount.max_amount {
        let shares = std::cmp::min(
            available_shares.0,
            pool.amount_to_shares(max_amount.0, !inverse_round_direction)
                .0,
        )
        .into();
        (
            shares,
            std::cmp::min(
                pool.shares_to_amount(shares, inverse_round_direction),
                max_amount.0,
            ),
        )
    } else {
        (
            available_shares,
            pool.shares_to_amount(available_shares, inverse_round_direction),
        )
    };
    assert!(shares.0 > 0, "Shares can't be 0");
    assert!(amount > 0, "Amount can't be 0");
    (shares, amount)
}

#[near_bindgen]
impl Contract {
    /// Executes a given list actions on behalf of the predecessor account without price.
    /// - Requires one yoctoNEAR.
    #[payable]
    pub fn execute(&mut self, actions: Vec<Action>) {
        assert_one_yocto();
        let account_id = env::predecessor_account_id();
        let mut account = self.internal_unwrap_account(&account_id);
        self.internal_execute(&account_id, &mut account, actions, Prices::new());
        self.internal_set_account(&account_id, account);
    }

    /// A simple withdraw interface that return a Promise (actual do transfer) or false (nothing transferred),
    /// and the final return value in promise indicate success (with true value) or failure (with false value).
    #[payable]
    pub fn simple_withdraw(&mut self, token_id: AccountId, amount_with_inner_decimal: U128, recipient_id: Option<AccountId>) -> Promise {
        assert_one_yocto();
        
        let account_id = env::predecessor_account_id();
        let recipient_id = recipient_id.unwrap_or(account_id.clone());
        let mut account = self.internal_unwrap_account(&account_id);

        assert!(!token_id.to_string().starts_with(SHADOW_V1_TOKEN_PREFIX));
        if account.supplied.get(&token_id).is_some() {
            let asset_amount = AssetAmount { 
                token_id, 
                amount: Some(amount_with_inner_decimal), 
                max_amount: None 
            };
            let (amount, ft_amount) = self.internal_withdraw(&mut account, &asset_amount);
            assert_eq!(amount, amount_with_inner_decimal.0, "Not enough balance in user's supply");
            assert!(ft_amount > 0, "Withdraw amount can't be 0");

            let promise = self.internal_ft_transfer(&account_id, &asset_amount.token_id, amount, ft_amount, false, &recipient_id);
            events::emit::withdraw_started(&account_id, amount, &asset_amount.token_id);
            self.internal_account_apply_affected_farms(&mut account);
            self.internal_set_account(&account_id, account);
            promise
        } else {
            env::panic_str("Not enough balance in user's supply");
        }
    }
}
