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
        margin_asset_id: AccountId,
        margin_amount: U128,
        debt_asset_id: AccountId,
        debt_amount: U128,
        position_asset_id: AccountId,
        min_position_amount: U128,
        swap_indication: SwapIndication,
    },
    DecreaseMTPosition {
        pos_id: PosId,
        position_amount: U128,
        min_debt_amount: U128,
        swap_indication: SwapIndication,
    },
    CloseMTPosition {
        pos_id: PosId,
        position_amount: U128,
        min_debt_amount: U128,
        swap_indication: SwapIndication,
    },
    LiquidateMTPosition {
        pos_owner_id: AccountId,
        pos_id: PosId,
        position_amount: U128,
        min_debt_amount: U128,
        swap_indication: SwapIndication,
    },
    ForceCloseMTPosition {
        pos_owner_id: AccountId,
        pos_id: PosId,
        position_amount: U128,
        min_debt_amount: U128,
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
        self.internal_set_prices(&prices);
        let ts = env::block_timestamp();
        for action in actions {
            match action {
                MarginAction::OpenPosition {
                    margin_asset_id,
                    margin_amount,
                    debt_asset_id,
                    debt_amount,
                    position_asset_id,
                    min_position_amount,
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
                    position_amount,
                    min_debt_amount,
                    swap_indication,
                } => {
                    let event = self.internal_margin_decrease_position(
                        account,
                        &pos_id,
                        position_amount.into(),
                        min_debt_amount.into(),
                        &swap_indication,
                        &prices,
                    );
                    events::emit::margin_decrease_started("margin_decrease_started", event);
                }
                MarginAction::CloseMTPosition {
                    pos_id,
                    position_amount,
                    min_debt_amount,
                    swap_indication,
                } => {
                    let event = self.internal_margin_close_position(
                        account,
                        &pos_id,
                        position_amount.into(),
                        min_debt_amount.into(),
                        &swap_indication,
                        &prices,
                    );
                    events::emit::margin_decrease_started("margin_close_started", event);
                }
                MarginAction::LiquidateMTPosition {
                    pos_owner_id,
                    pos_id,
                    position_amount,
                    min_debt_amount,
                    swap_indication,
                } => {
                    let event = self.internal_margin_liquidate_position(
                        account_id,
                        &pos_owner_id,
                        &pos_id,
                        position_amount.into(),
                        min_debt_amount.into(),
                        &swap_indication,
                        &prices,
                    );
                    events::emit::margin_decrease_started("margin_liquidate_started", event);
                }
                MarginAction::ForceCloseMTPosition {
                    pos_owner_id,
                    pos_id,
                    position_amount,
                    min_debt_amount,
                    swap_indication,
                } => {
                    let event = self.internal_margin_forceclose_position(
                        &pos_owner_id,
                        &pos_id,
                        position_amount.into(),
                        min_debt_amount.into(),
                        &swap_indication,
                        &prices,
                    );
                    events::emit::margin_decrease_started("margin_forceclose_started", event);
                }
                MarginAction::Withdraw { token_id, amount } => {
                    let amount = self.internal_margin_withdraw_supply(
                        account,
                        &token_id,
                        amount.map(|a| a.into()),
                    );
                    self.internal_ft_transfer(account_id, &token_id, amount);
                    events::emit::margin_withdraw_started(&account_id, amount, &token_id);
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
        let asset_id = mt.margin_asset.clone();
        let asset = self.internal_unwrap_asset(&mt.margin_asset);
        let shares = asset.supplied.amount_to_shares(amount, false);
        let actual_amount = asset.supplied.shares_to_amount(shares, false);
        account.withdraw_supply_shares(&mt.margin_asset, &shares);
        mt.margin_shares.0 += shares.0;
        account.margin_positions.insert(pos_id.clone(), mt);
        (asset_id, actual_amount)
    }

    pub(crate) fn internal_margin_decrease_collateral(
        &mut self,
        account: &mut MarginAccount,
        pos_id: &PosId,
        amount: Balance,
        prices: &Prices,
    ) -> AccountId {
        let margin_config = self.internal_margin_config();
        let mut mt = account
            .margin_positions
            .get(pos_id)
            .expect("Position not exist")
            .clone();
        let token_id = mt.margin_asset.clone();
        let mut asset = self.internal_unwrap_asset(&mt.margin_asset);
        let shares = asset.supplied.amount_to_shares(amount, true);

        asset.supplied.withdraw(shares, amount);

        // collateral can NOT decrease to 0
        assert!(
            mt.margin_shares.0 > shares.0,
            "Not enough collateral to decrease"
        );
        mt.margin_shares.0 -= shares.0;

        let total_cap =
            self.get_mtp_margin_value(&mt, prices) + self.get_mtp_position_value(&mt, prices);
        let total_debt = self.get_mtp_debt_value(&mt, prices);
        let tbd_safty_buffer = 1000_u32;
        assert!(
            total_cap.mul_ratio(tbd_safty_buffer) + total_cap > total_debt,
            "Margin position would be below liquidation line"
        );

        assert!(
            self.get_mtp_lr(&mt, prices).unwrap()
                <= BigDecimal::from(margin_config.max_leverage_rate as u32),
            "Leverage rate is too high"
        );

        self.internal_set_asset(&mt.margin_asset, asset);
        account.margin_positions.insert(pos_id.clone(), mt);

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

    pub(crate) fn internal_margin_withdraw_supply(
        &mut self,
        account: &mut MarginAccount,
        token_id: &AccountId,
        amount: Option<Balance>,
    ) -> Balance {
        let mut asset = self.internal_unwrap_asset(token_id);
        let (withdraw_shares, withdraw_amount) = if let Some(amount) = amount {
            (asset.supplied.amount_to_shares(amount, true), amount)
        } else {
            let shares = account.supplied.get(token_id).unwrap().clone();
            let amount = asset.supplied.shares_to_amount(shares, false);
            (shares, amount)
        };
        account.withdraw_supply_shares(token_id, &withdraw_shares);
        asset.supplied.withdraw(withdraw_shares, withdraw_amount);
        self.internal_set_asset(token_id, asset);

        withdraw_amount
    }
}

#[near_bindgen]
impl Contract {
    /// Executes a given list actions on behalf of the predecessor account.
    /// - Requires one yoctoNEAR.
    #[payable]
    pub fn margin_execute(&mut self, actions: Vec<MarginAction>) {
        assert_one_yocto();
        let account_id = env::predecessor_account_id();
        let mut account = self.internal_unwrap_margin_account(&account_id);
        self.internal_margin_execute(&account_id, &mut account, actions, Prices::new());
        self.internal_set_margin_account(&account_id, account);
    }
}
