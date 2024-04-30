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
        let RefV1Action::Swap(sa) = action;
        (sa.token_in.clone(), sa.amount_in.unwrap().0)
    }

    // get token_out, min_amount_out
    pub fn get_token_out(&self) -> (AccountId, Balance) {
        let RefV1TokenReceiverMessage::Execute {
            referral_id: _,
            actions,
            client_echo: _,
        } = self;
        let action = actions.last().unwrap();
        let RefV1Action::Swap(sa) = action;
        (sa.token_out.clone(), sa.min_amount_out.0)
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
        account: &mut MarginAccount,
        amount: Balance,
        sr: &SwapReference,
    ) {
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
        account
            .margin_positions
            .insert(&sr.pos_id, &mt);

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
    }

    pub(crate) fn on_decrease_trade_return(
        &mut self,
        account: &mut MarginAccount,
        amount: Balance,
        sr: &SwapReference,
    ) -> EventDataMarginDecreaseResult {
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
                    asset_debt
                        .margin_debt
                        .withdraw(mt.token_d_shares, remain_debt_balance);
                    mt.token_d_shares.0 = 0;
                }
            }
        }

        mt.is_locking = false;
        account
            .margin_positions
            .insert(&sr.pos_id, &mt);

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
            // TODO: change to directly send assets back to user
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
            account.margin_positions.remove(&sr.pos_id);
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
        if benefit_d_shares > 0 || benefit_m_shares > 0 || benefit_p_shares > 0 {
            if sr.op == "liquidate" || sr.op == "forceclose" {
                let mut liquidator_account =
                    if let Some(ref liquidator_account_id) = sr.liquidator_id {
                        if let Some(x) = self.internal_get_margin_account(&liquidator_account_id) {
                            x
                        } else {
                            self.internal_unwrap_margin_account(&self.internal_config().owner_id)
                        }
                    } else {
                        self.internal_unwrap_margin_account(&self.internal_config().owner_id)
                    };
                if benefit_d_shares > 0 {
                    liquidator_account
                        .deposit_supply_shares(&mt.token_d_id, &U128(benefit_d_shares));
                }
                if benefit_m_shares > 0 {
                    liquidator_account
                        .deposit_supply_shares(&mt.token_c_id, &U128(benefit_m_shares));
                }
                if benefit_p_shares > 0 {
                    liquidator_account
                        .deposit_supply_shares(&mt.token_p_id, &U128(benefit_p_shares));
                }
                self.internal_set_margin_account(
                    &liquidator_account.account_id.clone(),
                    liquidator_account,
                );
            } else {
                if benefit_d_shares > 0 {
                    account.deposit_supply_shares(&mt.token_d_id, &U128(benefit_d_shares));
                }
                if benefit_m_shares > 0 {
                    account.deposit_supply_shares(&mt.token_c_id, &U128(benefit_m_shares));
                }
                if benefit_p_shares > 0 {
                    account.deposit_supply_shares(&mt.token_p_id, &U128(benefit_p_shares));
                }
            }
        }

        self.internal_set_asset(&mt.token_d_id, asset_debt);
        self.internal_set_asset(&mt.token_p_id, asset_position);

        event
    }
}
