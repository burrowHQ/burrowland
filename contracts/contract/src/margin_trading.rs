use crate::{*, events::emit::{EventDataMarginOpenResult,EventDataMarginDecreaseResult}};
use near_sdk::serde_json;

/// clients use this to indicate how to trading
#[derive(Serialize, Deserialize, Clone)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
#[serde(crate = "near_sdk::serde")]
pub struct SwapIndication {
    pub dex_id: AccountId,
    pub swap_action_text: String,
}

/// ref-v1 swap instruction
#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct RefV1SwapAction {
    /// Pool which should be used for swapping.
    pub pool_id: u64,
    /// Token to swap from.
    pub token_in: AccountId,
    /// Amount to exchange.
    /// If amount_in is None, it will take amount_out from previous step.
    /// Will fail if amount_in is None on the first step.
    pub amount_in: Option<U128>,
    /// Token to swap into.
    pub token_out: AccountId,
    /// Required minimum amount of token_out.
    pub min_amount_out: U128,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
#[serde(untagged)]
pub enum RefV1Action {
    Swap(RefV1SwapAction),
}

impl RefV1Action {
    pub fn get_token_in(&self) -> AccountId {
        match self {
            RefV1Action::Swap(action) => {
                action.token_in.clone()
            }
        }
    }
    pub fn get_amount_in(&self) -> u128 {
        match self {
            RefV1Action::Swap(action) => {
                action.amount_in.expect("Action missing amount_in").0
            }
        }
    }
    
    pub fn get_token_out(&self) -> AccountId {
        match self {
            RefV1Action::Swap(action) => {
                action.token_out.clone()
            }
        }
    }
    pub fn get_min_amount_out(&self) -> u128 {
        match self {
            RefV1Action::Swap(action) => {
                action.min_amount_out.0
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
#[serde(untagged)]
pub enum RefV1TokenReceiverMessage {
    /// Alternative to deposit + execute actions call.
    Execute {
        referral_id: Option<AccountId>,
        /// List of sequential actions.
        actions: Vec<RefV1Action>,
        /// If not None, dex would use ft_transfer_call
        /// to send token_out back to predecessor with this msg.
        client_echo: Option<String>,
        skip_degen_price_sync: Option<bool>,
    },
}

impl RefV1TokenReceiverMessage {
    /// get token_in, amount_in
    pub fn get_token_in(&self) -> (AccountId, Balance) {
        let RefV1TokenReceiverMessage::Execute {
            referral_id: _,
            actions,
            client_echo: _,
            skip_degen_price_sync: _,
        } = self;
        let action = actions.first().unwrap();
        let token_in = action.get_token_in();
        let amount_in = actions.iter().filter(|a| a.get_token_in() == token_in).map(|a| a.get_amount_in()).sum();
        (token_in, amount_in)
    }

    // get token_out, min_amount_out
    pub fn get_token_out(&self) -> (AccountId, Balance) {
        let RefV1TokenReceiverMessage::Execute {
            referral_id: _,
            actions,
            client_echo: _,
            skip_degen_price_sync: _,
        } = self;
        let action = actions.last().unwrap();
        let token_out = action.get_token_out();
        let min_amount_out = actions.iter().filter(|a| a.get_token_out() == token_out).map(|a| a.get_min_amount_out()).sum();
        (token_out, min_amount_out)
    }

    pub fn get_client_echo(&self) -> Option<String> {
        match self {
            RefV1TokenReceiverMessage::Execute {
                referral_id: _,
                actions: _,
                client_echo,
                skip_degen_price_sync: _,
            } => client_echo.clone(),
        }
    }

    pub fn set_client_echo(&mut self, echo: &String) {
        match self {
            RefV1TokenReceiverMessage::Execute {
                referral_id: _,
                actions: _,
                ref mut client_echo,
                skip_degen_price_sync: _,
            } => *client_echo = Some(echo.clone()),
        }
    }

    pub fn to_msg_string(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}

/// Message parameters to receive via token function call.
#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub enum RefV2TokenReceiverMessage {
    Deposit,
    Swap {
        pool_ids: Vec<String>,
        output_token: AccountId,
        min_output_amount: U128,
        skip_unwrap_near: Option<bool>,
        /// If not None, dex would use ft_transfer_call
        /// to send token_out back to predecessor with this msg.
        client_echo: Option<String>,
    },
}

impl RefV2TokenReceiverMessage {
    pub fn get_token_out(&self) -> (AccountId, Balance, Option<bool>) {
        if let RefV2TokenReceiverMessage::Swap {
            pool_ids: _,
            output_token,
            min_output_amount,
            skip_unwrap_near,
            client_echo: _,
        } = self
        {
            (
                output_token.clone(),
                min_output_amount.0,
                skip_unwrap_near.clone(),
            )
        } else {
            env::panic_str("Invalid RefV2TokenReceiverMessage");
        }
    }

    pub fn get_client_echo(&self) -> Option<String> {
        match self {
            RefV2TokenReceiverMessage::Swap {
                pool_ids: _,
                output_token: _,
                min_output_amount: _,
                skip_unwrap_near: _,
                client_echo,
            } => client_echo.clone(),
            RefV2TokenReceiverMessage::Deposit => {
                env::panic_str("Invalid RefV2TokenReceiverMessage")
            }
        }
    }

    pub fn set_client_echo(&mut self, echo: &String) {
        match self {
            RefV2TokenReceiverMessage::Swap {
                pool_ids: _,
                output_token: _,
                min_output_amount: _,
                skip_unwrap_near: _,
                ref mut client_echo,
            } => *client_echo = Some(echo.clone()),
            RefV2TokenReceiverMessage::Deposit => {
                env::panic_str("Invalid RefV2TokenReceiverMessage")
            }
        }
    }

    pub fn to_msg_string(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}

pub enum MsgToDex {
    RefV1(RefV1TokenReceiverMessage),
    RefV2(RefV2TokenReceiverMessage),
}

pub struct SwapDetail {
    pub dex_id: AccountId,
    pub dex_msg: MsgToDex,
}

impl SwapDetail {
    pub fn verify_token_in(&self, token_in: &AccountId, amount_in: Balance) -> bool {
        match &self.dex_msg {
            MsgToDex::RefV1(refv1_msg) => {
                let (msg_token_in, msg_amount_in) = refv1_msg.get_token_in();
                &msg_token_in == token_in && msg_amount_in == amount_in
            }
            MsgToDex::RefV2(_refv2_msg) => true,
        }
    }

    pub fn verify_token_out(&self, token_out: &AccountId, min_amount_out: Balance) -> bool {
        match &self.dex_msg {
            MsgToDex::RefV1(refv1_msg) => {
                let (msg_token_out, msg_min_amount_out) = refv1_msg.get_token_out();
                &msg_token_out == token_out && msg_min_amount_out == min_amount_out
            }
            MsgToDex::RefV2(refv2_msg) => {
                let (msg_token_out, msg_min_amount_out, skip_unwrap_near) =
                    refv2_msg.get_token_out();
                &msg_token_out == token_out
                    && msg_min_amount_out == min_amount_out
                    && skip_unwrap_near.is_some()
                    && skip_unwrap_near.unwrap()
            }
        }
    }

    pub fn get_client_echo(&self) -> Option<String> {
        match &self.dex_msg {
            MsgToDex::RefV1(refv1_msg) => refv1_msg.get_client_echo(),
            MsgToDex::RefV2(refv2_msg) => refv2_msg.get_client_echo(),
        }
    }

    pub fn set_client_echo(&mut self, echo: &String) {
        match &mut self.dex_msg {
            MsgToDex::RefV1(refv1_msg) => refv1_msg.set_client_echo(echo),
            MsgToDex::RefV2(refv2_msg) => refv2_msg.set_client_echo(echo),
        }
    }

    pub fn to_msg_string(&self) -> String {
        match &self.dex_msg {
            MsgToDex::RefV1(refv1_msg) => refv1_msg.to_msg_string(),
            MsgToDex::RefV2(refv2_msg) => refv2_msg.to_msg_string(),
        }
    }
}

/// Protocol set info to this structure along with swap msg to dex,
/// and dex would echo back when transfer token_out back to protocol.
#[derive(Serialize, Deserialize, Clone)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
#[serde(crate = "near_sdk::serde")]
pub struct SwapReference {
    pub account_id: AccountId,
    pub pos_id: String,
    pub amount_in: U128,
    pub action_ts: U64,
    pub op: String,
    pub liquidator_id: Option<AccountId>,
}

impl SwapReference {
    pub(crate) fn to_msg_string(&self) -> String {
        let a = TokenReceiverMsg::SwapReference {
            swap_ref: self.clone(),
        };
        serde_json::to_string(&a).unwrap()
    }
}

/// Operation types for margin position decrease
#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
pub enum DecreaseOperation {
    /// User voluntarily decreasing position
    Decrease,
    /// User closing entire position
    Close,
    /// Liquidator closing unhealthy position
    Liquidate,
    /// Force closing underwater position
    ForceClose,
    /// Keeper executing stop order
    Stop,
}

impl DecreaseOperation {
    pub fn from_str(s: &str) -> Self {
        match s {
            "decrease" => Self::Decrease,
            "close" => Self::Close,
            "liquidate" => Self::Liquidate,
            "forceclose" => Self::ForceClose,
            "stop" => Self::Stop,
            _ => env::panic_str(&format!("Unknown decrease operation: {}", s)),
        }
    }

    /// Returns true if this operation should fully close the position
    pub fn is_full_close(&self) -> bool {
        matches!(self, Self::Close | Self::Liquidate | Self::ForceClose | Self::Stop)
    }

    /// Returns true if remaining debt should be repaid from collateral
    pub fn should_repay_from_collateral(&self) -> bool {
        self.is_full_close()
    }

    /// Returns true if protocol reserve can cover bad debt
    pub fn can_use_protocol_reserve(&self) -> bool {
        matches!(self, Self::ForceClose)
    }

    /// Returns true if benefits go to protocol owner instead of position owner
    pub fn benefits_to_protocol_owner(&self) -> bool {
        matches!(self, Self::Liquidate | Self::ForceClose)
    }
}

/// Result of debt repayment calculation
pub struct DebtRepaymentResult {
    /// Amount of debt token repaid
    pub repay_amount: Balance,
    /// Shares of debt repaid
    pub repay_shares: Shares,
    /// Amount left over after full debt repayment
    pub leftover_amount: Balance,
    /// Holding position fee paid from this repayment
    pub holding_fee_paid: Balance,
    /// Remaining debt cap after this repayment
    pub remaining_debt_cap: Balance,
}

/// Accumulated benefits from position settlement
#[derive(Default)]
pub struct SettlementBenefits {
    /// Collateral token shares (benefit_m_shares)
    pub collateral_shares: u128,
    /// Debt token shares from leftover (benefit_d_shares)
    pub debt_token_shares: u128,
    /// Position token shares (benefit_p_shares)
    pub position_token_shares: u128,
}

impl SettlementBenefits {
    pub fn has_any(&self) -> bool {
        self.collateral_shares > 0
            || self.debt_token_shares > 0
            || self.position_token_shares > 0
    }

    pub fn to_margin_updates(&self, position: &MarginTradingPosition) -> MarginAccountUpdates {
        MarginAccountUpdates {
            token_c_update: (position.token_c_id.clone(), self.collateral_shares),
            token_d_update: (position.token_d_id.clone(), self.debt_token_shares),
            token_p_update: (position.token_p_id.clone(), self.position_token_shares),
        }
    }
}

/// Stop service fee info extracted during settlement
pub struct StopServiceFeeInfo {
    pub token_id: TokenId,
    pub amount: Balance,
}

impl Contract {
    pub(crate) fn parse_swap_indication(&self, swap_indication: &SwapIndication) -> SwapDetail {
        let margin_config = self.internal_margin_config();
        let ver = margin_config
            .registered_dexes
            .get(&swap_indication.dex_id)
            .expect("Unregistered dex");
        if ver == &1_u8 {
            let v1msg = serde_json::from_str::<RefV1TokenReceiverMessage>(
                &swap_indication.swap_action_text,
            )
            .expect("Invalid swap_action_text");
            SwapDetail {
                dex_id: swap_indication.dex_id.clone(),
                dex_msg: MsgToDex::RefV1(v1msg),
            }
        } else if ver == &2_u8 {
            let v2msg = serde_json::from_str::<RefV2TokenReceiverMessage>(
                &swap_indication.swap_action_text,
            )
            .expect("Invalid swap_action_text");
            SwapDetail {
                dex_id: swap_indication.dex_id.clone(),
                dex_msg: MsgToDex::RefV2(v2msg),
            }
        } else {
            env::panic_str("Invalid dex version");
        }
    }
}

impl Contract {
    pub(crate) fn on_open_trade_return(
        &mut self,
        mut account: MarginAccount,
        amount: Balance,
        sr: &SwapReference,
    ) {
        let account_id = account.account_id.clone();
        let margin_config = self.internal_margin_config();
        let mut mt = account.margin_positions.get(&sr.pos_id).unwrap().clone();
        let mut asset_debt = self.internal_unwrap_asset(&mt.token_d_id);
        let mut asset_position = self.internal_unwrap_asset(&mt.token_p_id);

        asset_debt.margin_pending_debt -= sr.amount_in.0;
        let debt_shares = asset_debt
            .margin_debt
            .amount_to_shares(sr.amount_in.0, true);
        asset_debt.margin_debt.deposit(debt_shares, sr.amount_in.0);
        asset_position.margin_position += amount;

        let open_fee_shares = u128_ratio(
            mt.token_c_shares.0,
            margin_config.open_position_fee_rate as u128,
            MAX_RATIO as u128,
        );
        let open_fee_amount = if mt.token_c_id == mt.token_d_id {
            let open_fee_amount = asset_debt
                .supplied
                .shares_to_amount(open_fee_shares.into(), false);
            asset_debt
                .supplied
                .withdraw(open_fee_shares.into(), open_fee_amount);
            asset_debt.prot_fee += open_fee_amount;
            open_fee_amount
        } else {
            let open_fee_amount = asset_position
                .supplied
                .shares_to_amount(open_fee_shares.into(), false);
            asset_position
                .supplied
                .withdraw(open_fee_shares.into(), open_fee_amount);
            asset_position.prot_fee += open_fee_amount;
            open_fee_amount
        };

        mt.token_c_shares.0 -= open_fee_shares;
        mt.debt_cap = sr.amount_in.0;
        mt.uahpi_at_open = asset_debt.unit_acc_hp_interest;
        mt.token_d_shares.0 += debt_shares.0;
        mt.token_p_amount += amount;
        mt.is_locking = false;
        // Update existing margin_position storage
        account.margin_positions.insert(&sr.pos_id, &mt);

        self.internal_set_asset_without_asset_basic_check(&mt.token_d_id, asset_debt);
        self.internal_set_asset_without_asset_basic_check(&mt.token_p_id, asset_position);

        let event = EventDataMarginOpenResult {
            account_id: account.account_id.clone(),
            pos_id: sr.pos_id.clone(),
            token_c_id: mt.token_c_id.clone(),
            token_c_shares: mt.token_c_shares,
            token_d_id: mt.token_d_id.clone(),
            token_d_amount: sr.amount_in.0,
            token_d_shares: mt.token_d_shares,
            token_p_id: mt.token_p_id.clone(),
            token_p_amount: mt.token_p_amount,
            open_fee: open_fee_amount,
        };
        events::emit::margin_open_succeeded(event);
        self.internal_force_set_margin_account(&account_id, account);
    }

    /// Calculates debt repayment amounts including holding position fee
    ///
    /// Returns a DebtRepaymentResult containing:
    /// - repay_amount: actual amount of debt being repaid
    /// - repay_shares: shares of debt being repaid
    /// - leftover_amount: amount left after full debt repayment
    /// - holding_fee_paid: holding position fee deducted
    /// - remaining_debt_cap: debt cap remaining after repayment
    fn calculate_debt_repayment(
        &self,
        position: &MarginTradingPosition,
        asset_debt: &Asset,
        swap_amount: Balance,
    ) -> DebtRepaymentResult {
        let debt_amount = asset_debt
            .margin_debt
            .shares_to_amount(position.token_d_shares, true);

        let holding_fee = u128_ratio(
            position.debt_cap,
            asset_debt.unit_acc_hp_interest - position.uahpi_at_open,
            UNIT,
        );

        let repay_cap = u128_ratio(position.debt_cap, swap_amount, debt_amount + holding_fee);

        if repay_cap >= position.debt_cap {
            // Full repayment: can pay all debt + holding fee
            DebtRepaymentResult {
                repay_amount: debt_amount,
                repay_shares: position.token_d_shares,
                leftover_amount: swap_amount - debt_amount - holding_fee,
                holding_fee_paid: holding_fee,
                remaining_debt_cap: 0,
            }
        } else {
            // Partial repayment: proportional holding fee
            let proportional_fee = u128_ratio(holding_fee, repay_cap, position.debt_cap);
            let repay_amount = swap_amount - proportional_fee;
            DebtRepaymentResult {
                repay_amount,
                repay_shares: asset_debt
                    .margin_debt
                    .amount_to_shares(repay_amount, false),
                leftover_amount: 0,
                holding_fee_paid: proportional_fee,
                remaining_debt_cap: position.debt_cap - repay_cap,
            }
        }
    }

    /// Attempts to repay remaining debt using collateral (when token_c == token_d)
    ///
    /// Returns (debt_shares_repaid, collateral_shares_used, amount_repaid)
    fn repay_debt_from_collateral(
        &self,
        position: &MarginTradingPosition,
        asset_debt: &Asset,
    ) -> (Shares, Shares, Balance) {
        // Only applicable when collateral and debt are same token and there's remaining debt
        if position.token_d_shares.0 == 0 || position.token_d_id != position.token_c_id {
            return (U128(0), U128(0), 0);
        }

        let remaining_debt = asset_debt
            .margin_debt
            .shares_to_amount(position.token_d_shares, true);

        let collateral_shares_needed = asset_debt
            .supplied
            .amount_to_shares(remaining_debt, true);

        if collateral_shares_needed <= position.token_c_shares {
            // Can repay all remaining debt
            (position.token_d_shares, collateral_shares_needed, remaining_debt)
        } else {
            // Use all collateral for partial repayment
            let collateral_amount = asset_debt
                .supplied
                .shares_to_amount(position.token_c_shares, false);
            let debt_shares = asset_debt
                .margin_debt
                .amount_to_shares(collateral_amount, false);
            (debt_shares, position.token_c_shares, collateral_amount)
        }
    }

    /// Covers remaining debt using protocol reserve (for forceclose operations)
    fn cover_debt_from_protocol_reserve(
        &mut self,
        position: &mut MarginTradingPosition,
        asset_debt: &mut Asset,
    ) {
        if position.token_d_shares.0 == 0 {
            return;
        }

        let remaining_debt = asset_debt
            .margin_debt
            .shares_to_amount(position.token_d_shares, true);

        if asset_debt.reserved >= remaining_debt {
            asset_debt.reserved -= remaining_debt;
        } else {
            let uncovered_debt = remaining_debt - asset_debt.reserved;
            asset_debt.reserved = 0;
            let mut protocol_debts = read_protocol_debts_from_storage();
            protocol_debts
                .entry(position.token_d_id.clone())
                .and_modify(|v| *v += uncovered_debt)
                .or_insert(uncovered_debt);
            write_protocol_debts_to_storage(protocol_debts);
            events::emit::new_protocol_debts(&position.token_d_id, uncovered_debt);
        }

        events::emit::forceclose_protocol_loss(&position.token_d_id, remaining_debt);
        asset_debt
            .margin_debt
            .withdraw(position.token_d_shares, remaining_debt);
        position.token_d_shares.0 = 0;
    }

    /// Settles a closed position: converts remaining assets to benefits
    ///
    /// Returns optional StopServiceFeeInfo if stop order was attached
    fn settle_closed_position(
        &mut self,
        account: &mut MarginAccount,
        position: &MarginTradingPosition,
        asset_position: &mut Asset,
        pos_id: &String,
        benefits: &mut SettlementBenefits,
    ) -> Option<StopServiceFeeInfo> {
        // Collateral becomes benefit
        if position.token_c_shares.0 > 0 {
            benefits.collateral_shares = position.token_c_shares.0;
        }

        // Position token becomes supply shares benefit
        if position.token_p_amount > 0 {
            let position_shares = asset_position
                .supplied
                .amount_to_shares(position.token_p_amount, false);
            asset_position
                .supplied
                .deposit(position_shares, position.token_p_amount);
            asset_position.margin_position -= position.token_p_amount;
            benefits.position_token_shares = position_shares.0;
        }

        // Remove margin position from storage
        account.storage_tracker.start();
        account.margin_positions.remove(pos_id);
        account.storage_tracker.stop();

        // Extract stop service fee if present
        account.stops.remove(pos_id).map(|margin_stop| StopServiceFeeInfo {
            token_id: margin_stop.service_token_id,
            amount: margin_stop.service_token_amount.0,
        })
    }

    /// Settles stop service fee to appropriate recipient
    fn settle_stop_service_fee(
        &mut self,
        fee_info: StopServiceFeeInfo,
        operation: DecreaseOperation,
        position_owner_id: &AccountId,
        keeper_id: Option<&AccountId>,
    ) {
        let mut asset = self.internal_unwrap_asset(&fee_info.token_id);

        // Determine recipient: keeper for stop operation, owner otherwise
        let recipient_id = if operation == DecreaseOperation::Stop {
            keeper_id.unwrap_or(position_owner_id)
        } else {
            position_owner_id
        };

        // Get recipient account or fallback to owner's account
        let mut recipient_account = self
            .internal_get_margin_account(recipient_id)
            .unwrap_or_else(|| self.internal_unwrap_margin_account(position_owner_id));

        let final_recipient = recipient_account.account_id.clone();
        let shares = asset.supplied.amount_to_shares(fee_info.amount, false);

        recipient_account.deposit_supply_shares(&fee_info.token_id, &shares);
        asset.supplied.deposit(shares, fee_info.amount);

        self.internal_force_set_margin_account(&final_recipient, recipient_account);
        self.internal_set_asset_without_asset_basic_check(&fee_info.token_id, asset);
    }

    pub(crate) fn on_decrease_trade_return(
        &mut self,
        mut account: MarginAccount,
        amount: Balance,
        sr: &SwapReference,
    ) -> EventDataMarginDecreaseResult {
        let account_id = account.account_id.clone();
        let operation = DecreaseOperation::from_str(&sr.op);
        let mut position = account.margin_positions.get(&sr.pos_id).unwrap().clone();
        let mut asset_debt = self.internal_unwrap_asset(&position.token_d_id);
        let mut asset_position = self.internal_unwrap_asset(&position.token_p_id);
        let mut benefits = SettlementBenefits::default();

        // === Section 1: Calculate and apply debt repayment ===
        let repayment = self.calculate_debt_repayment(&position, &asset_debt, amount);

        // Apply debt repayment to asset and position
        asset_debt.margin_debt.withdraw(repayment.repay_shares, repayment.repay_amount);
        position.token_d_shares.0 -= repayment.repay_shares.0;
        position.debt_cap = repayment.remaining_debt_cap;
        asset_debt.prot_fee += repayment.holding_fee_paid;

        // Handle leftover as benefit (deposit to supply)
        if repayment.leftover_amount > 0 {
            let supply_shares = asset_debt.supplied.amount_to_shares(repayment.leftover_amount, false);
            if supply_shares.0 > 0 {
                asset_debt.supplied.deposit(supply_shares, repayment.leftover_amount);
                benefits.debt_token_shares = supply_shares.0;
            }
        }

        // === Section 2: Additional debt repayment from collateral (for full-close operations) ===
        if operation.should_repay_from_collateral() {
            let (repay_debt_shares, used_supply_shares, repay_amount) =
                self.repay_debt_from_collateral(&position, &asset_debt);

            if repay_amount > 0 {
                asset_debt.supplied.withdraw(used_supply_shares, repay_amount);
                asset_debt.margin_debt.withdraw(repay_debt_shares, repay_amount);
                position.token_d_shares.0 -= repay_debt_shares.0;
                position.token_c_shares.0 -= used_supply_shares.0;
            }
        }

        // === Section 3: Protocol reserve coverage (forceclose only) ===
        if operation.can_use_protocol_reserve() {
            self.cover_debt_from_protocol_reserve(&mut position, &mut asset_debt);
        }

        // === Section 4: Position settlement ===
        position.is_locking = false;
        account.margin_positions.insert(&sr.pos_id, &position);

        // Build result event before potential position removal
        let event = EventDataMarginDecreaseResult {
            account_id: account.account_id.clone(),
            pos_id: sr.pos_id.clone(),
            liquidator_id: sr.liquidator_id.clone(),
            token_c_id: position.token_c_id.clone(),
            token_c_shares: position.token_c_shares,
            token_d_id: position.token_d_id.clone(),
            token_d_shares: position.token_d_shares,
            token_p_id: position.token_p_id.clone(),
            token_p_amount: position.token_p_amount,
            holding_fee: repayment.holding_fee_paid,
        };

        // Try to settle (close) the position if debt is fully repaid
        let stop_fee_info = if position.token_d_shares.0 == 0 {
            self.settle_closed_position(
                &mut account,
                &position,
                &mut asset_position,
                &sr.pos_id,
                &mut benefits,
            )
        } else {
            if operation.is_full_close() {
                env::log_str(&format!(
                    "{} failed due to insufficient fund, user {}, pos_id {}",
                    sr.op, account.account_id, sr.pos_id
                ));
            }
            None
        };

        // === Section 5: Distribute benefits ===
        let mut account_updates = None;

        if benefits.has_any() {
            match operation {
                DecreaseOperation::Liquidate => {
                    // Liquidation: distribute benefits among owner, liquidator, and user
                    let liquidator_id = sr.liquidator_id.clone().expect("Liquidator required");
                    let owner_id = self.internal_config().owner_id;

                    if let Some(mut liquidator_account) = self.internal_get_margin_account(&liquidator_id) {
                        let mut owner_account = self.internal_unwrap_margin_account(&owner_id);
                        let pd = PositionDirection::new(
                            &position.token_c_id,
                            &position.token_d_id,
                            &position.token_p_id,
                        );
                        let mbtl = self.internal_unwrap_margin_base_token_limit_or_default(pd.get_base_token_id());
                        let (owner_updates, liquidator_updates, user_updates) = account_supplied_updates(
                            &mut owner_account,
                            &mut liquidator_account,
                            &mut account,
                            &position,
                            &mbtl,
                            (benefits.collateral_shares, benefits.debt_token_shares, benefits.position_token_shares),
                        );
                        events::emit::margin_benefits(&owner_id, &owner_updates);
                        self.internal_force_set_margin_account(&owner_id, owner_account);
                        events::emit::margin_benefits(&liquidator_id, &liquidator_updates);
                        self.internal_force_set_margin_account(&liquidator_id, liquidator_account);
                        account_updates = Some(user_updates);
                    } else {
                        // Liquidator account not found: all benefits go to owner
                        let mut owner_account = self.internal_unwrap_margin_account(&owner_id);
                        deposit_benefit_to_account(&mut owner_account, &position.token_c_id, benefits.collateral_shares);
                        deposit_benefit_to_account(&mut owner_account, &position.token_d_id, benefits.debt_token_shares);
                        deposit_benefit_to_account(&mut owner_account, &position.token_p_id, benefits.position_token_shares);
                        let owner_updates = benefits.to_margin_updates(&position);
                        events::emit::margin_benefits(&owner_id, &owner_updates);
                        self.internal_force_set_margin_account(&owner_id, owner_account);
                    }
                }
                DecreaseOperation::ForceClose => {
                    // Force close: all benefits go to protocol owner
                    let owner_id = self.internal_config().owner_id;
                    let mut owner_account = self.internal_unwrap_margin_account(&owner_id);
                    deposit_benefit_to_account(&mut owner_account, &position.token_c_id, benefits.collateral_shares);
                    deposit_benefit_to_account(&mut owner_account, &position.token_d_id, benefits.debt_token_shares);
                    deposit_benefit_to_account(&mut owner_account, &position.token_p_id, benefits.position_token_shares);
                    let owner_updates = benefits.to_margin_updates(&position);
                    events::emit::margin_benefits(&owner_id, &owner_updates);
                    self.internal_force_set_margin_account(&owner_id, owner_account);
                }
                DecreaseOperation::Decrease | DecreaseOperation::Close | DecreaseOperation::Stop => {
                    // Normal operations: benefits go to position owner
                    deposit_benefit_to_account(&mut account, &position.token_c_id, benefits.collateral_shares);
                    deposit_benefit_to_account(&mut account, &position.token_d_id, benefits.debt_token_shares);
                    deposit_benefit_to_account(&mut account, &position.token_p_id, benefits.position_token_shares);
                    account_updates = Some(benefits.to_margin_updates(&position));
                }
            }
        }

        // Finalize user account and emit benefits event if any
        if let Some(ref account_updates) = account_updates {
            events::emit::margin_benefits(&account_id, account_updates);
        }
        self.internal_force_set_margin_account(&account_id, account);

        // === Section 6: Save assets ===
        self.internal_set_asset_without_asset_basic_check(&position.token_d_id, asset_debt);
        self.internal_set_asset_without_asset_basic_check(&position.token_p_id, asset_position);

        // === Section 7: Service fee settlement ===
        if let Some(fee_info) = stop_fee_info {
            self.settle_stop_service_fee(
                fee_info,
                operation,
                &account_id,
                sr.liquidator_id.as_ref(),
            );
        }

        event
    }
}

pub struct MarginAccountUpdates {
    pub token_c_update: (AccountId, u128),
    pub token_d_update: (AccountId, u128),
    pub token_p_update: (AccountId, u128),
}

impl MarginAccountUpdates {
    pub fn get_token_d_amount(&self) -> u128 {
        if self.token_c_update.0 == self.token_d_update.0 {
            self.token_c_update.1 + self.token_d_update.1
        } else {
            self.token_d_update.1
        }
    }

    pub fn get_token_p_amount(&self) -> u128 {
        if self.token_c_update.0 == self.token_p_update.0 {
            self.token_c_update.1 + self.token_p_update.1
        } else {
            self.token_p_update.1
        }
    }

    pub fn to_hash_map(&self) -> HashMap<AccountId, U128> {
        HashMap::from([
            (self.token_d_update.0.clone(), U128(self.get_token_d_amount())), 
            (self.token_p_update.0.clone(), U128(self.get_token_p_amount())),
        ])
    }
}

pub fn account_supplied_updates(
    owner_account: &mut MarginAccount,
    liquidator_account: &mut MarginAccount,
    account: &mut MarginAccount,
    mt: &MarginTradingPosition,
    mbtl: &MarginBaseTokenLimit,
    (benefit_m_shares, benefit_d_shares, benefit_p_shares): (u128, u128, u128),
) -> (MarginAccountUpdates, MarginAccountUpdates, MarginAccountUpdates) {
    let (benefit_c_shares_to_owner, benefit_c_shares_to_liquidator, benefit_c_shares_to_user) = benefit_distribution(owner_account, liquidator_account, account, &mt.token_c_id, benefit_m_shares, mbtl.liq_benefit_protocol_rate, mbtl.liq_benefit_liquidator_rate);
    let (benefit_d_shares_to_owner, benefit_d_shares_to_liquidator, benefit_d_shares_to_user) = benefit_distribution(owner_account, liquidator_account, account, &mt.token_d_id, benefit_d_shares, mbtl.liq_benefit_protocol_rate, mbtl.liq_benefit_liquidator_rate);
    let (benefit_p_shares_to_owner, benefit_p_shares_to_liquidator, benefit_p_shares_to_user) = benefit_distribution(owner_account, liquidator_account, account, &mt.token_p_id, benefit_p_shares, mbtl.liq_benefit_protocol_rate, mbtl.liq_benefit_liquidator_rate);
    (MarginAccountUpdates{
        token_c_update: (mt.token_c_id.clone(), benefit_c_shares_to_owner),
        token_d_update: (mt.token_d_id.clone(), benefit_d_shares_to_owner),
        token_p_update: (mt.token_p_id.clone(), benefit_p_shares_to_owner),
    }, MarginAccountUpdates{
        token_c_update: (mt.token_c_id.clone(), benefit_c_shares_to_liquidator),
        token_d_update: (mt.token_d_id.clone(), benefit_d_shares_to_liquidator),
        token_p_update: (mt.token_p_id.clone(), benefit_p_shares_to_liquidator),
    }, MarginAccountUpdates{
        token_c_update: (mt.token_c_id.clone(), benefit_c_shares_to_user),
        token_d_update: (mt.token_d_id.clone(), benefit_d_shares_to_user),
        token_p_update: (mt.token_p_id.clone(), benefit_p_shares_to_user),
    })
}

pub fn benefit_distribution(
    owner_account: &mut MarginAccount,
    liquidator_account: &mut MarginAccount,
    account: &mut MarginAccount,
    token_id: &AccountId, 
    total_benefit_shares: u128,
    liq_benefit_protocol_rate: u32, 
    liq_benefit_liquidator_rate: u32
) -> (u128, u128, u128) {
    if total_benefit_shares > 0 {
        let (benefit_shares_to_owner, benefit_shares_to_liquidator, benefit_shares_to_user) = 
            calculate_benefit_distribution(total_benefit_shares, liq_benefit_protocol_rate, liq_benefit_liquidator_rate);
        deposit_benefit_to_account(owner_account, token_id, benefit_shares_to_owner);
        deposit_benefit_to_account(liquidator_account, token_id, benefit_shares_to_liquidator);
        deposit_benefit_to_account(account, token_id, benefit_shares_to_user);
        (benefit_shares_to_owner, benefit_shares_to_liquidator, benefit_shares_to_user)
    } else {
        (0, 0, 0)
    }
}

pub fn calculate_benefit_distribution(total_benefit_shares: u128, liq_benefit_protocol_rate: u32, liq_benefit_liquidator_rate: u32) -> (u128, u128, u128) {
    if liq_benefit_protocol_rate + liq_benefit_liquidator_rate == MAX_RATIO {
        let benefit_shares_to_owner = u128_ratio(total_benefit_shares, liq_benefit_protocol_rate as u128, MAX_RATIO as u128);
        let benefit_shares_to_liquidator = total_benefit_shares - benefit_shares_to_owner;
        (benefit_shares_to_owner, benefit_shares_to_liquidator, 0)
    } else {
        let benefit_shares_to_owner = u128_ratio(total_benefit_shares, liq_benefit_protocol_rate as u128, MAX_RATIO as u128);
        let benefit_shares_to_liquidator = u128_ratio(total_benefit_shares, liq_benefit_liquidator_rate as u128, MAX_RATIO as u128);
        let benefit_shares_to_user = total_benefit_shares - benefit_shares_to_owner - benefit_shares_to_liquidator;
        (benefit_shares_to_owner, benefit_shares_to_liquidator, benefit_shares_to_user)   
    }
}

pub fn deposit_benefit_to_account(margin_account: &mut MarginAccount, token_id: &AccountId, benefit_shares: u128) {
    if benefit_shares > 0 {
        margin_account.deposit_supply_shares(token_id, &U128(benefit_shares));
    }
}