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
    },
}

impl RefV1TokenReceiverMessage {
    /// get token_in, amount_in
    pub fn get_token_in(&self) -> (AccountId, Balance) {
        let RefV1TokenReceiverMessage::Execute {
            referral_id: _,
            actions,
            client_echo: _,
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
            } => client_echo.clone(),
        }
    }

    pub fn set_client_echo(&mut self, echo: &String) {
        match self {
            RefV1TokenReceiverMessage::Execute {
                referral_id: _,
                actions: _,
                ref mut client_echo,
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
        account_id: &AccountId,
        amount: Balance,
        sr: &SwapReference,
    ) {
        let mut account = self.internal_unwrap_margin_account(&account_id);
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

        self.internal_set_asset(&mt.token_d_id, asset_debt);
        self.internal_set_asset(&mt.token_p_id, asset_position);

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
        self.internal_set_margin_account(&account_id, account);
    }

    pub(crate) fn on_decrease_trade_return(
        &mut self,
        account_id: &AccountId,
        amount: Balance,
        sr: &SwapReference,
    ) -> EventDataMarginDecreaseResult {
        let mut account = self.internal_unwrap_margin_account(&account_id);
        let mut mt = account.margin_positions.get(&sr.pos_id).unwrap().clone();
        let mut asset_debt = self.internal_unwrap_asset(&mt.token_d_id);
        let mut asset_position = self.internal_unwrap_asset(&mt.token_p_id);
        let (mut benefit_m_shares, mut benefit_d_shares, mut benefit_p_shares) =
            (0_u128, 0_u128, 0_u128);

        // figure out actual repay amount and shares
        // figure out how many debt_cap been repaid, and charge corresponding holding-position fee from repayment.
        let debt_amount = asset_debt
            .margin_debt
            .shares_to_amount(mt.token_d_shares, true);
        let hp_fee = u128_ratio(
            mt.debt_cap,
            asset_debt.unit_acc_hp_interest - mt.uahpi_at_open,
            UNIT,
        );
        mt.uahpi_at_open = asset_debt.unit_acc_hp_interest;
        
        let repay_cap = u128_ratio(mt.debt_cap, amount, debt_amount + hp_fee);

        let (repay_amount, repay_shares, left_amount, repay_hp_fee) = if repay_cap >= mt.debt_cap {
            (debt_amount, mt.token_d_shares, amount - debt_amount - hp_fee, hp_fee)
        } else {
            let repay_hp_fee = u128_ratio(hp_fee, repay_cap, mt.debt_cap);
            (
                amount - repay_hp_fee,
                asset_debt
                    .margin_debt
                    .amount_to_shares(amount - repay_hp_fee, false),
                0,
                repay_hp_fee,
            )
        };
        asset_debt.margin_debt.withdraw(repay_shares, repay_amount);
        mt.token_d_shares.0 -= repay_shares.0;
        mt.debt_cap = if repay_cap >= mt.debt_cap {
            0
        } else {
            mt.debt_cap - repay_cap
        };
        // distribute hp_fee
        asset_debt.prot_fee += repay_hp_fee;

        // handle possible leftover debt asset, put them into user's supply
        if left_amount > 0 {
            let supply_shares = asset_debt.supplied.amount_to_shares(left_amount, false);
            if supply_shares.0 > 0 {
                asset_debt.supplied.deposit(supply_shares, left_amount);
                benefit_d_shares = supply_shares.0;
            }
        }

        if sr.op != "decrease" {
            // try to repay remaining debt from margin
            if mt.token_d_shares.0 > 0 && mt.token_d_id == mt.token_c_id {
                let remain_debt_balance = asset_debt
                    .margin_debt
                    .shares_to_amount(mt.token_d_shares, true);
                let margin_shares_to_repay = asset_debt
                    .supplied
                    .amount_to_shares(remain_debt_balance, true);
                let (repay_debt_share, used_supply_share, repay_amount) =
                    if margin_shares_to_repay <= mt.token_c_shares {
                        (mt.token_d_shares, margin_shares_to_repay, remain_debt_balance)
                    } else {
                        // use all margin balance to repay
                        let margin_balance = asset_debt
                            .supplied
                            .shares_to_amount(mt.token_c_shares, false);
                        let repay_debt_shares = asset_debt
                            .margin_debt
                            .amount_to_shares(margin_balance, false);
                        (repay_debt_shares, mt.token_c_shares, margin_balance)
                    };
                asset_debt
                    .supplied
                    .withdraw(used_supply_share, repay_amount);
                asset_debt
                    .margin_debt
                    .withdraw(repay_debt_share, repay_amount);
                mt.token_d_shares.0 -= repay_debt_share.0;
                mt.token_c_shares.0 -= used_supply_share.0;
            }
        }

        if sr.op == "forceclose" {
            // try to use protocol reserve to repay remaining debt
            if mt.token_d_shares.0 > 0 {
                let remain_debt_balance = asset_debt
                    .margin_debt
                    .shares_to_amount(mt.token_d_shares, true);
                if asset_debt.reserved >= remain_debt_balance {
                    asset_debt.reserved -= remain_debt_balance;
                } else {
                    let debt = remain_debt_balance - asset_debt.reserved;
                    asset_debt.reserved = 0;
                    let mut protocol_debts = read_protocol_debts_from_storage();
                    protocol_debts.entry(mt.token_d_id.clone())
                        .and_modify(|v| *v += debt)
                        .or_insert(debt);
                    write_protocol_debts_to_storage(protocol_debts);
                    events::emit::new_protocol_debts(&mt.token_d_id, debt);
                }
                events::emit::forceclose_protocol_loss(&mt.token_d_id, remain_debt_balance);
                asset_debt
                    .margin_debt
                    .withdraw(mt.token_d_shares, remain_debt_balance);
                mt.token_d_shares.0 = 0;
            }
        }

        mt.is_locking = false;
        // Update existing margin_position storage
        account.margin_positions.insert(&sr.pos_id, &mt);

        let event = EventDataMarginDecreaseResult {
            account_id: account.account_id.clone(),
            pos_id: sr.pos_id.clone(),
            liquidator_id: sr.liquidator_id.clone(),
            token_c_id: mt.token_c_id.clone(),
            token_c_shares: mt.token_c_shares,
            token_d_id: mt.token_d_id.clone(),
            token_d_shares: mt.token_d_shares,
            token_p_id: mt.token_p_id.clone(),
            token_p_amount: mt.token_p_amount,
            holding_fee: repay_hp_fee,
        };
        
        // try to settle this position
        if mt.token_d_shares.0 == 0 {
            // close this position and remaining asset goes back to user's inner account
            if mt.token_c_shares.0 > 0 {
                benefit_m_shares = mt.token_c_shares.0;
            }
            if mt.token_p_amount > 0 {
                let position_shares = asset_position
                    .supplied
                    .amount_to_shares(mt.token_p_amount, false);
                asset_position
                    .supplied
                    .deposit(position_shares, mt.token_p_amount);
                asset_position.margin_position -= mt.token_p_amount;
                benefit_p_shares = position_shares.0;
            }
            // Remove margin_position storage
            account.storage_tracker.start();
            account.margin_positions.remove(&sr.pos_id);
            account.storage_tracker.stop();
        } else {
            if sr.op != "decrease" {
                env::log_str(&format!(
                    "{} failed due to insufficient fund, user {}, pos_id {}",
                    sr.op.clone(),
                    account.account_id.clone(),
                    sr.pos_id.clone()
                ));
            }
        }

        // distribute benefits
        let mut account_updates = None;
        if benefit_d_shares > 0 || benefit_m_shares > 0 || benefit_p_shares > 0 {
            if sr.op == "liquidate" {
                // The sr.liquidator_id in the liquidate operation must not be None.
                let liquidator_id = sr.liquidator_id.clone().unwrap();
                let mut liquidator_account = self.internal_unwrap_margin_account(&liquidator_id);
                let owner_id = self.internal_config().owner_id;
                let mut owner_account = self.internal_unwrap_margin_account(&owner_id);
                let pd = PositionDirection::new(&mt.token_c_id, &mt.token_d_id, &mt.token_p_id);
                let mbtl = self.internal_unwrap_margin_base_token_limit_or_default(pd.get_base_token_id());
                let (owner_updates, liquidator_updates, user_updates) = account_supplied_updates(
                    &mut owner_account, 
                    &mut liquidator_account, 
                    &mut account, 
                    &mt, 
                    &mbtl,
                    (benefit_m_shares, benefit_d_shares, benefit_p_shares)
                );
                events::emit::margin_benefits(&owner_id, &owner_updates);
                self.internal_set_margin_account_without_panic(
                    &owner_id,
                    owner_account,
                    &mut asset_debt,
                    &mut asset_position,
                    owner_updates
                );
                events::emit::margin_benefits(&liquidator_id, &liquidator_updates);
                self.internal_set_margin_account_without_panic(
                    &liquidator_id,
                    liquidator_account,
                    &mut asset_debt,
                    &mut asset_position,
                    liquidator_updates
                );
                account_updates = Some(user_updates);
            } else if sr.op == "forceclose" {
                let owner_id = self.internal_config().owner_id;
                let mut owner_account = self.internal_unwrap_margin_account(&owner_id);
                deposit_benefit_to_account(&mut owner_account, &mt.token_c_id, benefit_m_shares);
                deposit_benefit_to_account(&mut owner_account, &mt.token_d_id, benefit_d_shares);
                deposit_benefit_to_account(&mut owner_account, &mt.token_p_id, benefit_p_shares);
                let owner_updates = MarginAccountUpdates {
                    token_c_update: (mt.token_c_id.clone(), benefit_m_shares),
                    token_d_update: (mt.token_d_id.clone(), benefit_d_shares),
                    token_p_update: (mt.token_p_id.clone(), benefit_p_shares),
                };
                events::emit::margin_benefits(&owner_id, &owner_updates);
                self.internal_set_margin_account_without_panic(
                    &owner_id, 
                    owner_account,
                    &mut asset_debt,
                    &mut asset_position,
                    owner_updates
                );
            } else {
                deposit_benefit_to_account(&mut account, &mt.token_c_id, benefit_m_shares);
                deposit_benefit_to_account(&mut account, &mt.token_d_id, benefit_d_shares);
                deposit_benefit_to_account(&mut account, &mt.token_p_id, benefit_p_shares);
                account_updates = Some(MarginAccountUpdates {
                    token_c_update: (mt.token_c_id.clone(), benefit_m_shares),
                    token_d_update: (mt.token_d_id.clone(), benefit_d_shares),
                    token_p_update: (mt.token_p_id.clone(), benefit_p_shares),
                });
            }
        }

        if let Some(account_updates) = account_updates{
            events::emit::margin_benefits(&account_id, &account_updates);
            self.internal_set_margin_account_without_panic(
                &account_id,
                account,
                &mut asset_debt,
                &mut asset_position,
                account_updates
            );
        } else {
            self.internal_set_margin_account(&account_id, account);
        }

        self.internal_set_asset(&mt.token_d_id, asset_debt);
        self.internal_set_asset(&mt.token_p_id, asset_position);

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