use crate::*;

pub mod emit {
    use super::*;
    use near_sdk::serde_json::json;

    #[derive(Serialize)]
    #[serde(crate = "near_sdk::serde")]
    struct AccountAmountToken<'a> {
        pub account_id: &'a AccountId,
        #[serde(with = "u128_dec_format")]
        pub amount: Balance,
        pub token_id: &'a TokenId,
    }

    #[derive(Serialize)]
    #[serde(crate = "near_sdk::serde")]
    struct AccountAmountTokenPosition<'a> {
        pub account_id: &'a AccountId,
        #[serde(with = "u128_dec_format")]
        pub amount: Balance,
        pub token_id: &'a TokenId,
        pub position: &'a String,
    }

    fn log_event<T: Serialize>(event: &str, data: T) {
        let event = json!({
            "standard": "burrow",
            "version": "1.0.0",
            "event": event,
            "data": [data]
        });

        log!("EVENT_JSON:{}", event.to_string());
    }

    pub fn deposit_to_reserve(account_id: &AccountId, amount: Balance, token_id: &TokenId) {
        log_event(
            "deposit_to_reserve",
            AccountAmountToken {
                account_id: &account_id,
                amount,
                token_id: &token_id,
            },
        );
    }

    pub fn deposit(account_id: &AccountId, amount: Balance, token_id: &TokenId) {
        log_event(
            "deposit",
            AccountAmountToken {
                account_id: &account_id,
                amount,
                token_id: &token_id,
            },
        );
    }

    pub fn margin_deposit(account_id: &AccountId, amount: Balance, token_id: &TokenId) {
        log_event(
            "margin_deposit",
            AccountAmountToken {
                account_id: &account_id,
                amount,
                token_id: &token_id,
            },
        );
    }

    pub fn withdraw_started(account_id: &AccountId, amount: Balance, token_id: &TokenId) {
        log_event(
            "withdraw_started",
            AccountAmountToken {
                account_id: &account_id,
                amount,
                token_id: &token_id,
            },
        );
    }

    pub fn withdraw_failed(account_id: &AccountId, amount: Balance, token_id: &TokenId) {
        log_event(
            "withdraw_failed",
            AccountAmountToken {
                account_id: &account_id,
                amount,
                token_id: &token_id,
            },
        );
    }

    pub fn withdraw_succeeded(account_id: &AccountId, amount: Balance, token_id: &TokenId) {
        log_event(
            "withdraw_succeeded",
            AccountAmountToken {
                account_id: &account_id,
                amount,
                token_id: &token_id,
            },
        );
    }

    pub fn increase_collateral(account_id: &AccountId, amount: Balance, token_id: &TokenId, position: &String) {
        log_event(
            "increase_collateral",
            AccountAmountTokenPosition {
                account_id,
                amount,
                token_id,
                position
            }
        );
    }

    pub fn decrease_collateral(account_id: &AccountId, amount: Balance, token_id: &TokenId, position: &String) {
        log_event(
            "decrease_collateral",
            AccountAmountTokenPosition {
                account_id,
                amount,
                token_id,
                position
            }
        );
    }

    pub fn borrow(account_id: &AccountId, amount: Balance, token_id: &TokenId, position: &String) {
        log_event(
            "borrow",
            AccountAmountTokenPosition {
                account_id,
                amount,
                token_id,
                position
            }
        );
    }

    pub fn repay(account_id: &AccountId, amount: Balance, token_id: &TokenId, position: &String) {
        log_event(
            "repay",
            AccountAmountTokenPosition {
                account_id,
                amount,
                token_id,
                position
            }
        );
    }

    pub fn liquidate(
        account_id: &AccountId,
        liquidation_account_id: &AccountId,
        collateral_sum: &BigDecimal,
        repaid_sum: &BigDecimal,
        old_discount: &BigDecimal,
        new_discount: &BigDecimal,
        position: &String
    ) {
        log_event(
            "liquidate",
            json!({
                "account_id": account_id,
                "liquidation_account_id": liquidation_account_id,
                "collateral_sum": collateral_sum,
                "repaid_sum": repaid_sum,
                "old_discount": old_discount,
                "new_discount": new_discount,
                "position": position,
            }),
        );
    }

    pub fn force_close(
        liquidation_account_id: &AccountId,
        collateral_sum: &BigDecimal,
        repaid_sum: &BigDecimal,
        old_discount: &BigDecimal,
        position: &String
    ) {
        log_event(
            "force_close",
            json!({
                "liquidation_account_id": liquidation_account_id,
                "collateral_sum": collateral_sum,
                "repaid_sum": repaid_sum,
                "old_discount": old_discount,
                "position": position,
            }),
        );
    }

    pub fn force_close_remain_borrowed(
        liquidation_account_id: &AccountId,
        remain_borrowed: &HashMap<TokenId, Shares>,
        position: &String
    ) {
        log_event(
            "force_close_remain_borrowed",
            json!({
                "liquidation_account_id": liquidation_account_id,
                "remain_borrowed": remain_borrowed,
                "position": position,
            }),
        );
    }

    pub fn booster_stake(
        account_id: &AccountId,
        amount: Balance,
        duration: DurationSec,
        extra_x_booster_amount: Balance,
        booster_staking: &BoosterStaking,
    ) {
        log_event(
            "booster_stake",
            json!({
                "account_id": account_id,
                "booster_amount": U128(amount),
                "duration": duration,
                "x_booster_amount": U128(extra_x_booster_amount),
                "total_booster_amount": U128(booster_staking.staked_booster_amount),
                "total_x_booster_amount": U128(booster_staking.x_booster_amount),
            }),
        );
    }

    pub fn booster_unstake(account_id: &AccountId, booster_staking: &BoosterStaking) {
        log_event(
            "booster_unstake",
            json!({
                "account_id": account_id,
                "total_booster_amount": U128(booster_staking.staked_booster_amount),
                "total_x_booster_amount": U128(booster_staking.x_booster_amount),
            }),
        );
    }

    pub fn claim_prot_fee(account_id: &AccountId, amount: Balance, token_id: &TokenId) {
        log_event(
            "claim_prot_fee",
            AccountAmountToken {
                account_id: &account_id,
                amount,
                token_id: &token_id,
            },
        );
    }

    pub fn increase_reserved(account_id: &AccountId, amount: Balance, token_id: &TokenId) {
        log_event(
            "increase_reserved",
            AccountAmountToken {
                account_id: &account_id,
                amount,
                token_id: &token_id,
            },
        );
    }

    pub fn decrease_reserved(account_id: &AccountId, amount: Balance, token_id: &TokenId) {
        log_event(
            "decrease_reserved",
            AccountAmountToken {
                account_id: &account_id,
                amount,
                token_id: &token_id,
            },
        );
    }

    pub fn margin_asset_withdraw_started(account_id: &AccountId, amount: Balance, token_id: &TokenId) {
        log_event(
            "withdraw_started_margin_asset",
            AccountAmountToken {
                account_id: &account_id,
                amount,
                token_id: &token_id,
            },
        );
    }

    pub fn margin_asset_withdraw_failed(account_id: &AccountId, amount: Balance, token_id: &TokenId) {
        log_event(
            "withdraw_failed_margin_asset",
            AccountAmountToken {
                account_id: &account_id,
                amount,
                token_id: &token_id,
            },
        );
    }

    pub fn margin_asset_withdraw_succeeded(account_id: &AccountId, amount: Balance, token_id: &TokenId) {
        log_event(
            "withdraw_succeeded_margin_asset",
            AccountAmountToken {
                account_id: &account_id,
                amount,
                token_id: &token_id,
            },
        );
    }


    #[derive(Serialize)]
    #[serde(crate = "near_sdk::serde")]
    pub struct EventDataMarginOpen {
        pub account_id: AccountId,
        pub pos_id: String,
        pub token_c_id: TokenId,
        #[serde(with = "u128_dec_format")]
        pub token_c_amount: Balance,
        pub token_c_shares: U128,
        pub token_d_id: TokenId,
        #[serde(with = "u128_dec_format")]
        pub token_d_amount: Balance,
        pub token_p_id: TokenId,
        #[serde(with = "u128_dec_format")]
        pub token_p_amount: Balance,
    }

    pub fn margin_open_started(data: EventDataMarginOpen) {
        log_event(
            "margin_open_started",
            data,
        );
    }

    pub fn margin_open_failed(account_id: &AccountId, pos_id: &String) {
        log_event(
            "margin_open_failed",
            json!({
                "account_id": account_id,
                "pos_id": pos_id,
            }),
        );
    }

    #[derive(Serialize)]
    #[serde(crate = "near_sdk::serde")]
    pub struct EventDataMarginOpenResult {
        pub account_id: AccountId,
        pub pos_id: String,
        pub token_c_id: TokenId,
        pub token_c_shares: U128,
        pub token_d_id: TokenId,
        #[serde(with = "u128_dec_format")]
        pub token_d_amount: Balance,
        pub token_d_shares: U128,
        pub token_p_id: TokenId,
        #[serde(with = "u128_dec_format")]
        pub token_p_amount: Balance,
        #[serde(with = "u128_dec_format")]
        pub open_fee: Balance,
    }

    pub fn margin_open_succeeded(data: EventDataMarginOpenResult) {
        log_event(
            "margin_open_succeeded",
            data,
        );
    }

    #[derive(Serialize)]
    #[serde(crate = "near_sdk::serde")]
    pub struct EventDataMarginDecrease {
        pub account_id: AccountId,
        pub pos_id: String,
        pub liquidator_id: Option<AccountId>,
        pub token_p_id: TokenId,
        #[serde(with = "u128_dec_format")]
        pub token_p_amount: Balance,
        pub token_d_id: TokenId,
        #[serde(with = "u128_dec_format")]
        pub token_d_amount: Balance,
    }

    pub fn margin_decrease_started(event_id: &str, data: EventDataMarginDecrease) {
        log_event(
            event_id,
            data,
        );
    }

    pub fn margin_decrease_failed(account_id: &AccountId, pos_id: &String) {
        log_event(
            "margin_decrease_failed",
            json!({
                "account_id": account_id,
                "pos_id": pos_id,
            }),
        );
    }

    #[derive(Serialize)]
    #[serde(crate = "near_sdk::serde")]
    pub struct EventDataMarginDecreaseResult {
        pub account_id: AccountId,
        pub pos_id: String,
        pub liquidator_id: Option<AccountId>,
        pub token_c_id: TokenId,
        pub token_c_shares: U128,
        pub token_d_id: TokenId,
        pub token_d_shares: U128,
        pub token_p_id: TokenId,
        #[serde(with = "u128_dec_format")]
        pub token_p_amount: Balance,
        #[serde(with = "u128_dec_format")]
        pub holding_fee: Balance,
    }

    pub fn margin_decrease_succeeded(op_id: &str, data: EventDataMarginDecreaseResult) {
        let event_id: &str = if op_id == "decrease" {
            "margin_decrease_succeeded"
        } else if op_id == "close" {
            "margin_close_succeeded"
        } else if op_id == "liquidate" {
            "margin_liquidate_succeeded"
        } else if op_id == "forceclose" {
            "margin_forceclose_succeeded"
        } else {
            op_id
        };

        log_event(
            event_id,
            data,
        );
    }
}
