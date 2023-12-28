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
                    self.internal_margin_open_position(
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
                }
                MarginAction::IncreaseCollateral { pos_id, amount } => {
                    self.internal_margin_increase_collateral(account, &pos_id, amount.into());
                }
                MarginAction::DecreaseCollateral { pos_id, amount } => {
                    self.internal_margin_decrease_collateral(
                        account,
                        &pos_id,
                        amount.into(),
                        &prices,
                    );
                }
                MarginAction::DecreaseMTPosition {
                    pos_id,
                    position_amount,
                    min_debt_amount,
                    swap_indication,
                } => {
                    self.internal_margin_decrease_position(
                        account,
                        &pos_id,
                        position_amount.into(),
                        min_debt_amount.into(),
                        &swap_indication,
                        &prices,
                    );
                }
                MarginAction::CloseMTPosition {
                    pos_id,
                    position_amount,
                    min_debt_amount,
                    swap_indication,
                } => {
                    self.internal_margin_close_position(
                        account,
                        &pos_id,
                        position_amount.into(),
                        min_debt_amount.into(),
                        &swap_indication,
                        &prices,
                    );
                }
                MarginAction::LiquidateMTPosition {
                    pos_owner_id,
                    pos_id,
                    position_amount,
                    min_debt_amount,
                    swap_indication,
                } => {
                    self.internal_margin_liquidate_position(
                        account_id,
                        &pos_owner_id,
                        &pos_id,
                        position_amount.into(),
                        min_debt_amount.into(),
                        &swap_indication,
                        &prices,
                    );
                }
                MarginAction::ForceCloseMTPosition {
                    pos_owner_id,
                    pos_id,
                    position_amount,
                    min_debt_amount,
                    swap_indication,
                } => {
                    self.internal_margin_forceclose_position(
                        &pos_owner_id,
                        &pos_id,
                        position_amount.into(),
                        min_debt_amount.into(),
                        &swap_indication,
                        &prices,
                    );
                }
                MarginAction::Withdraw { token_id, amount } => {
                    self.internal_margin_withdraw_supply(
                        account,
                        &token_id,
                        amount.map(|a| a.into()),
                    );
                }
            }
        }
    }

    pub(crate) fn internal_margin_increase_collateral(
        &mut self,
        account: &mut MarginAccount,
        pos_id: &PosId,
        amount: Balance,
    ) -> Balance {
        let mut mt = account
            .margin_positions
            .get(pos_id)
            .expect("Position not exist")
            .clone();
        let asset = self.internal_unwrap_asset(&mt.margin_asset);
        let shares = asset.supplied.amount_to_shares(amount, false);
        let actual_amount = asset.supplied.shares_to_amount(shares, false);
        account.withdraw_supply_shares(&mt.margin_asset, &shares);
        mt.margin_shares.0 += shares.0;
        account.margin_positions.insert(pos_id.clone(), mt);
        actual_amount
    }

    pub(crate) fn internal_margin_decrease_collateral(
        &mut self,
        account: &mut MarginAccount,
        pos_id: &PosId,
        amount: Balance,
        prices: &Prices,
    ) {
        let mut mt = account
            .margin_positions
            .get(pos_id)
            .expect("Position not exist")
            .clone();
        let mut asset = self.internal_unwrap_asset(&mt.margin_asset);
        let shares = asset.supplied.amount_to_shares(amount, true);

        asset.supplied.withdraw(shares, amount);

        assert!(
            mt.margin_shares.0 >= shares.0,
            "Not enough collateral to decrease"
        );
        mt.margin_shares.0 -= shares.0;

        let total_cap = self.get_mtp_collateral_sum(&mt, prices);
        let total_debt = self.get_mtp_borrowed_sum(&mt, prices);
        let tbd_safty_buffer = 1000_u32;
        assert!(
            total_cap.mul_ratio(tbd_safty_buffer) + total_cap > total_debt,
            "Margin position would be below liquidation line"
        );

        self.internal_set_asset(&mt.margin_asset, asset);
        account.margin_positions.insert(pos_id.clone(), mt);

        // TODO: send token to user by ft_transfer
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
    ) {
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
        

        // TODO: send token back to user
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
