use crate::*;

#[derive(Deserialize, Serialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
#[serde(crate = "near_sdk::serde")]
pub enum MarginAction {
    Withdraw {
        token_id: AccountId,
        amount: Option<U128>,
    },
    IncreaseCollateral {
        pos_id: PosId,
        amount: U128,
    },
    DecreaseCollateral {
        pos_id: PosId,
        amount: U128,
    },
    OpenPosition {
        token_c_id: AccountId,
        token_c_amount: U128,
        token_d_id: AccountId,
        token_d_amount: U128,
        token_p_id: AccountId,
        min_token_p_amount: U128,
        swap_indication: SwapIndication,
    },
    DecreaseMTPosition {
        pos_id: PosId,
        token_p_amount: U128,
        min_token_d_amount: U128,
        swap_indication: SwapIndication,
    },
    CloseMTPosition {
        pos_id: PosId,
        token_p_amount: U128,
        min_token_d_amount: U128,
        swap_indication: SwapIndication,
    },
    LiquidateMTPosition {
        pos_owner_id: AccountId,
        pos_id: PosId,
        token_p_amount: U128,
        min_token_d_amount: U128,
        swap_indication: SwapIndication,
    },
    ForceCloseMTPosition {
        pos_owner_id: AccountId,
        pos_id: PosId,
        token_p_amount: U128,
        min_token_d_amount: U128,
        swap_indication: SwapIndication,
    },
}

impl Contract {
    pub fn internal_margin_execute(
        &mut self,
        account_id: &AccountId,
        account: &mut MarginAccount,
        actions: Vec<MarginAction>,
        prices: Prices,
    ) {
        // Set reliable liquidator context if signer is in whitelist
        self.is_reliable_liquidator_context = in_reliable_liquidator_whitelist(&env::signer_account_id().to_string());

        self.internal_set_prices(&prices);
        let ts = env::block_timestamp();
        for action in actions {
            match action {
                MarginAction::OpenPosition {
                    token_c_id: margin_asset_id,
                    token_c_amount: margin_amount,
                    token_d_id: debt_asset_id,
                    token_d_amount: debt_amount,
                    token_p_id: position_asset_id,
                    min_token_p_amount: min_position_amount,
                    swap_indication,
                } => {
                    let event = self.internal_margin_open_position(
                        ts,
                        account,
                        &margin_asset_id,
                        margin_amount.into(),
                        &debt_asset_id,
                        debt_amount.into(),
                        &position_asset_id,
                        min_position_amount.into(),
                        &swap_indication,
                        &prices,
                    );
                    events::emit::margin_open_started(event);
                }
                MarginAction::IncreaseCollateral { pos_id, amount } => {
                    let (token_id, actual_amount) = self.internal_margin_increase_collateral(account, &pos_id, amount.into());
                    events::emit::increase_collateral(&account_id, actual_amount, &token_id, &pos_id);
                }
                MarginAction::DecreaseCollateral { pos_id, amount } => {
                    let token_id = self.internal_margin_decrease_collateral(
                        account,
                        &pos_id,
                        amount.into(),
                        &prices,
                    );
                    events::emit::decrease_collateral(&account_id, amount.0, &token_id, &pos_id);
                }
                MarginAction::DecreaseMTPosition {
                    pos_id,
                    token_p_amount: position_amount,
                    min_token_d_amount: min_debt_amount,
                    swap_indication,
                } => {
                    let event = self.process_decrease_margin_position(
                        account,
                        &pos_id,
                        position_amount.into(),
                        min_debt_amount.into(),
                        &swap_indication,
                        &prices,
                        "decrease".to_string(),
                        None,
                    );
                    events::emit::margin_decrease_started("margin_decrease_started", event);
                }
                MarginAction::CloseMTPosition {
                    pos_id,
                    token_p_amount: position_amount,
                    min_token_d_amount: min_debt_amount,
                    swap_indication,
                } => {
                    let event = self.process_decrease_margin_position(
                        account,
                        &pos_id,
                        position_amount.into(),
                        min_debt_amount.into(),
                        &swap_indication,
                        &prices,
                        "close".to_string(),
                        None,
                    );
                    events::emit::margin_decrease_started("margin_close_started", event);
                }
                MarginAction::LiquidateMTPosition {
                    pos_owner_id,
                    pos_id,
                    token_p_amount: position_amount,
                    min_token_d_amount: min_debt_amount,
                    swap_indication,
                } => {
                    assert_ne!(
                        account_id, &pos_owner_id,
                        "Can't liquidate yourself"
                    );
                    let mut pos_owner = self.internal_unwrap_margin_account(&pos_owner_id);
                    let event = self.process_decrease_margin_position(
                        &mut pos_owner,
                        &pos_id,
                        position_amount.into(),
                        min_debt_amount.into(),
                        &swap_indication,
                        &prices,
                        "liquidate".to_string(),
                        Some(account_id.clone()),
                    );
                    self.internal_set_margin_account(&pos_owner_id, pos_owner);
                    events::emit::margin_decrease_started("margin_liquidate_started", event);
                }
                MarginAction::ForceCloseMTPosition {
                    pos_owner_id,
                    pos_id,
                    token_p_amount: position_amount,
                    min_token_d_amount: min_debt_amount,
                    swap_indication,
                } => {
                    assert_ne!(
                        account_id, &pos_owner_id,
                        "Can't liquidate yourself"
                    );
                    let config = self.internal_config();
                    assert!(
                        config.force_closing_enabled,
                        "The force closing is not enabled"
                    );
                    let mut pos_owner = self.internal_unwrap_margin_account(&pos_owner_id);
                    let event = self.process_decrease_margin_position(
                        &mut pos_owner,
                        &pos_id,
                        position_amount.into(),
                        min_debt_amount.into(),
                        &swap_indication,
                        &prices,
                        "forceclose".to_string(),
                        None,
                    );
                    self.internal_set_margin_account(&pos_owner_id, pos_owner);
                    events::emit::margin_decrease_started("margin_forceclose_started", event);
                }
                MarginAction::Withdraw { token_id, amount } => {
                    assert!(!token_id.to_string().starts_with(SHADOW_V1_TOKEN_PREFIX));
                    let asset = self.internal_unwrap_asset(&token_id);
                    assert!(asset.config.can_withdraw, "Withdrawals for this asset are not enabled");
                    if account.supplied.get(&token_id).is_some() {
                        let (amount, ft_amount) = self.internal_margin_withdraw_supply(
                            account,
                            &token_id,
                            amount.map(|a| a.into()),
                        );
                        if ft_amount > 0 {
                            self.internal_ft_transfer(account_id, &token_id, amount, ft_amount, true);
                            events::emit::margin_asset_withdraw_started(&account_id, amount, &token_id);
                        } else {
                            events::emit::margin_asset_withdraw_succeeded(&account_id, amount, &token_id);
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn internal_margin_increase_collateral(
        &mut self,
        account: &mut MarginAccount,
        pos_id: &PosId,
        amount: Balance,
    ) -> (AccountId, Balance) {
        let mut mt = account
            .margin_positions
            .get(pos_id)
            .expect("Position not exist")
            .clone();
        let asset_id = mt.token_c_id.clone();
        let asset = self.internal_unwrap_asset(&mt.token_c_id);
        assert!(asset.config.can_use_as_collateral, "This asset can't be used as a collateral");
        let shares = asset.supplied.amount_to_shares(amount, false);
        let actual_amount = asset.supplied.shares_to_amount(shares, false);
        account.withdraw_supply_shares(&mt.token_c_id, &shares);
        mt.token_c_shares.0 += shares.0;
        // Update existing margin_position storage
        account.margin_positions.insert(&pos_id, &mt);
        (asset_id, actual_amount)
    }

    pub(crate) fn internal_margin_decrease_collateral(
        &mut self,
        account: &mut MarginAccount,
        pos_id: &PosId,
        amount: Balance,
        prices: &Prices,
    ) -> AccountId {
        let mut mt = account
            .margin_positions
            .get(pos_id)
            .expect("Position not exist")
            .clone();
        assert!(
            !mt.is_locking,
            "Position is currently waiting for a trading result."
        );
        let pd = PositionDirection::new(&mt.token_c_id, &mt.token_d_id, &mt.token_p_id);
        let mbtl = self.internal_unwrap_margin_base_token_limit_or_default(pd.get_base_token_id());
        let token_id = mt.token_c_id.clone();
        let asset = self.internal_unwrap_asset(&mt.token_c_id);
        let shares = asset.supplied.amount_to_shares(amount, true);

        // collateral can NOT decrease to 0
        assert!(
            mt.token_c_shares.0 > shares.0,
            "Not enough collateral to decrease"
        );
        mt.token_c_shares.0 -= shares.0;

        assert!(
            !self.is_mt_liquidatable(&mt, prices, mbtl.min_safety_buffer),
            "Margin position would be below liquidation line"
        );
        assert!(
            !self.is_mt_forcecloseable(&mt, prices),
            "Margin position would be below forceclose line"
        );

        assert!(
            self.get_mtp_lr(&mt, prices).unwrap()
                <= BigDecimal::from(mbtl.max_leverage_rate as u32),
            "Leverage rate is too high"
        );

        account.deposit_supply_shares(&mt.token_c_id, &shares);
        // Update existing margin_position storage
        account.margin_positions.insert(&pos_id, &mt);

        token_id
    }

    pub(crate) fn internal_margin_deposit(
        &mut self,
        account: &mut MarginAccount,
        token_id: &TokenId,
        amount: Balance,
    ) -> Shares {
        let mut asset = self.internal_unwrap_asset(token_id);
        let shares: Shares = asset.supplied.amount_to_shares(amount, false);
        account.deposit_supply_shares(token_id, &shares);
        asset.supplied.deposit(shares, amount);
        self.internal_set_asset(token_id, asset);
        shares
    }

    #[allow(unused)]
    pub(crate) fn internal_margin_deposit_without_asset_basic_check(
        &mut self,
        account: &mut MarginAccount,
        token_id: &TokenId,
        amount: Balance,
    ) -> Shares {
        let mut asset = self.internal_unwrap_asset(token_id);
        let shares: Shares = asset.supplied.amount_to_shares(amount, false);
        account.deposit_supply_shares(token_id, &shares);
        asset.supplied.deposit(shares, amount);
        self.internal_set_asset_without_asset_basic_check(token_id, asset);
        shares
    }

    pub(crate) fn internal_margin_withdraw_supply(
        &mut self,
        account: &mut MarginAccount,
        token_id: &AccountId,
        amount: Option<Balance>,
    ) -> (Balance, Balance) {
        let mut asset = self.internal_unwrap_asset(token_id);
        let (withdraw_shares, withdraw_amount) = if let Some(amount) = amount {
            (asset.supplied.amount_to_shares(amount, true), amount)
        } else {
            let shares = account.supplied.get(token_id).unwrap().clone();
            let amount = asset.supplied.shares_to_amount(shares, false);
            (shares, amount)
        };
        let available_amount = asset.available_amount();
        assert!(
            withdraw_amount <= available_amount,
            "Withdraw error: Exceeded available amount {} of {}",
            available_amount,
            token_id
        );

        let withdraw_ft_amount = withdraw_amount / 10u128.pow(asset.config.extra_decimals as u32);
        if withdraw_ft_amount > 0 {
            account.withdraw_supply_shares(token_id, &withdraw_shares);
            asset.supplied.withdraw(withdraw_shares, withdraw_amount);
            self.internal_set_asset(token_id, asset);
            (withdraw_amount, withdraw_ft_amount)
        } else {
            (0, 0)
        }
    }
}

#[near_bindgen]
impl Contract {
    /// Executes a given list margin actions on behalf of the predecessor account.
    /// - Requires one yoctoNEAR.
    #[payable]
    pub fn margin_execute(&mut self, actions: Vec<MarginAction>) {
        assert_one_yocto();
        let account_id = env::predecessor_account_id();

        // move to internal_margin_execute()
        // // Set reliable liquidator context if caller is in whitelist
        // self.is_reliable_liquidator_context = in_reliable_liquidator_whitelist(&account_id.to_string());

        let mut account = self.internal_unwrap_margin_account(&account_id);
        self.internal_margin_execute(&account_id, &mut account, actions, Prices::new());
        self.internal_set_margin_account(&account_id, account);
    }
}
