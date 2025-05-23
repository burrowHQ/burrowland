use crate::*;
use near_sdk::{serde_json, promise_result_as_success, is_promise_success};

pub const GAS_FOR_SYNC_REF_EXCHANGE_LP_INFOS: Gas = Gas(Gas::ONE_TERA.0 * 50);
pub const GAS_FOR_SYNC_REF_EXCHANGE_LP_INFOS_CALLBACK: Gas = Gas(Gas::ONE_TERA.0 * 20);

pub const GAS_FOR_PROCESS_LIQUIDATE_RESULT: Gas = Gas(50 * Gas::ONE_TERA.0);
pub const GAS_FOR_CALLBACK_PROCESS_LIQUIDATE_RESULT: Gas = Gas(40 * Gas::ONE_TERA.0);

pub const GAS_FOR_PROCESS_FORCE_CLOSE_RESULT: Gas = Gas(50 * Gas::ONE_TERA.0);
pub const GAS_FOR_CALLBACK_PROCESS_FORCE_CLOSE_RESULT: Gas = Gas(40 * Gas::ONE_TERA.0);

#[ext_contract(ext_ref_exchange)]
pub trait ExtRefExchange {
    fn on_burrow_liquidation(&mut self, liquidator_account_id: AccountId, liquidation_account_id: AccountId, shadow_id: String, liquidate_share_amount: U128, min_token_amounts: Vec<U128>);
    fn get_unit_lpt_assets(&self, pool_ids: Vec<u64>) -> HashMap<String, UnitShareTokens>;
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct TokenAmount {
    pub token_id: AccountId,
    pub amount: U128,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct UnitShareTokens {
    #[serde(with = "u64_dec_format")]
    pub timestamp: Timestamp,
    pub decimals: u8,
    pub tokens: Vec<TokenAmount>,
}


#[derive(Deserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, Serialize))]
#[serde(crate = "near_sdk::serde")]
pub enum ShadowReceiverMsg {
    Execute { actions: Vec<Action> },
    ExecuteWithPyth { actions: Vec<Action> },
}

#[near_bindgen]
impl Contract {
    pub fn sync_ref_exchange_lp_token_infos(&mut self, token_ids: Option<Vec<String>>) {
        let token_ids = token_ids.unwrap_or_else(|| self.last_lp_token_infos.keys().map(|v| v.clone()).collect());
        assert!(token_ids.len() > 0, "Invalid token_ids");
        let pool_ids: Vec<u64> = token_ids.iter().map(|v| {
            self.internal_unwrap_asset(&AccountId::new_unchecked(v.clone()));
            parse_pool_id(v)
        }).collect();
        ext_ref_exchange::ext(self.internal_config().ref_exchange_id)
            .with_static_gas(shadow_actions::GAS_FOR_SYNC_REF_EXCHANGE_LP_INFOS)
            .get_unit_lpt_assets(pool_ids)
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(shadow_actions::GAS_FOR_SYNC_REF_EXCHANGE_LP_INFOS_CALLBACK)
                    .callback_sync_lp_infos()
            );
    }

    pub fn get_last_lp_token_infos(&self) -> HashMap<String, UnitShareTokens> {
        self.last_lp_token_infos.clone()
    }

    pub fn on_cast_shadow(&mut self, account_id: AccountId, shadow_id: String, amount: U128, msg: String) {
        let config = self.internal_config();
        assert!(env::predecessor_account_id() == config.ref_exchange_id);

        let (actions, with_pyth) = if !msg.is_empty() {
            match near_sdk::serde_json::from_str(&msg).expect("Can't parse ShadowReceiverMsg") {
                ShadowReceiverMsg::Execute { actions } => (actions, false),
                ShadowReceiverMsg::ExecuteWithPyth { actions } => (actions, true),
            }
        } else {
            (vec![], false)
        };

        let token_id = AccountId::new_unchecked(shadow_id);
        let asset = self.internal_unwrap_asset(&token_id);
        assert!(
            asset.config.can_deposit,
            "Deposits for this asset are not enabled"
        );
        let amount = amount.0 * 10u128.pow(asset.config.extra_decimals as u32);
        
        let mut account = self.internal_unwrap_account(&account_id);
        self.internal_deposit(&mut account, &token_id, amount);
        events::emit::deposit(&account_id, amount, &token_id);
        if with_pyth {
            self.internal_execute_with_pyth(&account_id, &mut account, actions);
        } else {
            self.internal_execute(&account_id, &mut account, actions, Prices::new());
        }
        self.internal_set_account(&account_id, account);
    }

    pub fn on_remove_shadow(&mut self, account_id: AccountId, shadow_id: String, amount: U128, msg: String) {
        let config = self.internal_config();
        assert!(env::predecessor_account_id() == config.ref_exchange_id);

        let mut account = self.internal_unwrap_account(&account_id);

        if !msg.is_empty() {
            let actions = match near_sdk::serde_json::from_str(&msg).expect("Can't parse ShadowReceiverMsg") {
                ShadowReceiverMsg::Execute { actions } => actions,
                ShadowReceiverMsg::ExecuteWithPyth { .. } => unimplemented!()
            };
            self.internal_execute(&account_id, &mut account, actions, Prices::new());
        } 

        let token_id = AccountId::new_unchecked(shadow_id);
        let mut asset = self.internal_unwrap_asset(&token_id);
        assert!(
            asset.config.can_withdraw,
            "Withdrawals for this asset are not enabled"
        );
        let withdraw_asset_amount = AssetAmount {
            token_id,
            amount: Some(U128(amount.0 * 10u128.pow(asset.config.extra_decimals as u32))),
            max_amount: None,
        };
        let mut account_asset = account.internal_unwrap_asset(&withdraw_asset_amount.token_id);
        let (shares, amount) =
            asset_amount_to_shares(&asset.supplied, account_asset.shares, &withdraw_asset_amount, false);

        let available_amount = asset.available_amount();
        assert!(
            amount <= available_amount,
            "Withdraw error: Exceeded available amount {} of {}",
            available_amount,
            &withdraw_asset_amount.token_id
        );

        account_asset.withdraw_shares(shares);
        account.internal_set_asset(&withdraw_asset_amount.token_id, account_asset);

        asset.supplied.withdraw(shares, amount);

        self.internal_set_asset(&withdraw_asset_amount.token_id, asset);
        self.internal_set_account(&account_id, account);
        events::emit::withdraw_succeeded(&account_id, amount, &withdraw_asset_amount.token_id);
    }
}

impl Contract {

    pub fn internal_shadow_liquidate(
        &mut self,
        position: &String,
        account_id: &AccountId,
        account: &mut Account,
        prices: &Prices,
        liquidation_account_id: &AccountId,
        in_assets: Vec<AssetAmount>,
        out_assets: Vec<AssetAmount>,
        min_token_amounts: Vec<U128>
    ) {
        let mut liquidation_account = self.internal_get_account(liquidation_account_id, true).expect("Account is not registered");
        let max_discount = self.compute_max_discount(position, &liquidation_account, &prices);
        assert!(
            max_discount > BigDecimal::zero(),
            "The shadow liquidation account is not at risk"
        );

        let mut borrowed_repaid_sum = BigDecimal::zero();

        for asset_amount in in_assets.iter() {
            if asset_amount.token_id == *ETH_OLD_ACCOUNT_ID {
                // The logic is the same as the self.internal_repay_old_eth function, but it does not update the storage.
                let mut account_asset = account.internal_unwrap_asset(&ETH_NEW_ACCOUNT_ID);
                let borrowed_asset = self.internal_unwrap_asset(&ETH_OLD_ACCOUNT_ID);
                let supplied_asset = self.internal_unwrap_asset(&ETH_NEW_ACCOUNT_ID);
                let available_borrowed_shares = liquidation_account.internal_unwrap_borrowed(position, &ETH_OLD_ACCOUNT_ID);

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
                liquidation_account.decrease_borrowed(position, &ETH_OLD_ACCOUNT_ID, borrowed_shares);
                account_asset.withdraw_shares(supplied_shares);

                account.internal_set_asset(&ETH_NEW_ACCOUNT_ID, account_asset);

                borrowed_repaid_sum = borrowed_repaid_sum
                    + BigDecimal::from_balance_price(
                        amount,
                        prices.get_unwrap(&asset_amount.token_id),
                        borrowed_asset.config.extra_decimals,
                    );
            } else {
                // The logic is the same as the self.internal_repay function, but it does not update the storage.
                let mut account_asset = account.internal_unwrap_asset(&asset_amount.token_id);
                let asset = self.internal_unwrap_asset(&asset_amount.token_id);
                let available_borrowed_shares = liquidation_account.internal_unwrap_borrowed(position, &asset_amount.token_id);
    
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
                liquidation_account.decrease_borrowed(position, &asset_amount.token_id, borrowed_shares);
                account_asset.withdraw_shares(supplied_shares);
                
                account.internal_set_asset(&asset_amount.token_id, account_asset);
    
                borrowed_repaid_sum = borrowed_repaid_sum
                    + BigDecimal::from_balance_price(
                        amount,
                        prices.get_unwrap(&asset_amount.token_id),
                        asset.config.extra_decimals,
                    );
            }
        }

        let collateral_asset = self.internal_unwrap_asset(&out_assets[0].token_id);
        let collateral_shares = liquidation_account.internal_unwrap_collateral(position, &out_assets[0].token_id);
        let (shares, amount) =
            asset_amount_to_shares(&collateral_asset.supplied, collateral_shares, &out_assets[0], false);
        liquidation_account.decrease_collateral(position, &out_assets[0].token_id, shares);


        let unit_share_tokens = self.last_lp_token_infos.get(position).expect("lp_token_infos not found");
        let config = self.internal_config();
        assert!(env::block_timestamp() - unit_share_tokens.timestamp <= to_nano(config.lp_tokens_info_valid_duration_sec), "LP token info timestamp is too stale");
        let unit_share = 10u128.pow(unit_share_tokens.decimals as u32);
        let collateral_taken_sum = unit_share_tokens.tokens
            .iter()
            .fold(BigDecimal::zero(), |sum, unit_share_token_value|{
                let token_asset = self.internal_unwrap_asset(&unit_share_token_value.token_id);
                let token_stdd_amount = unit_share_token_value.amount.0 * 10u128.pow(token_asset.config.extra_decimals as u32);
                let token_balance = u128_ratio(token_stdd_amount, amount, 10u128.pow(collateral_asset.config.extra_decimals as u32) * unit_share);
                sum + BigDecimal::from_balance_price(
                    token_balance,
                    prices.get_unwrap(&unit_share_token_value.token_id),
                    token_asset.config.extra_decimals,
                )
            });

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

        ext_ref_exchange::ext(self.internal_config().ref_exchange_id)
            .with_static_gas(GAS_FOR_PROCESS_LIQUIDATE_RESULT)
            .on_burrow_liquidation(
                account_id.clone(), 
                liquidation_account_id.clone(), 
                position.clone(), 
                U128(amount),
                min_token_amounts
            ).then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_FOR_CALLBACK_PROCESS_LIQUIDATE_RESULT)
                    .callback_process_shadow_liquidate_result(
                        account_id.clone(),
                        liquidation_account_id.clone(),
                        position.clone(),
                        in_assets,
                        out_assets,
                        collateral_taken_sum,
                        borrowed_repaid_sum,
                        max_discount,
                        new_max_discount
                    )
            );

        
    }

    pub fn internal_shadow_force_close(&mut self, position: &String, prices: &Prices, liquidation_account_id: &AccountId, min_token_amounts: Vec<U128>) {
        let config = self.internal_config();
        assert!(
            config.force_closing_enabled,
            "The force closing is not enabled"
        );

        let mut borrowed_sum = BigDecimal::zero();
        let mut collateral_assets = HashMap::new();
        let mut borrowed_assets = HashMap::new();

        let liquidation_account = self.internal_get_account(liquidation_account_id, true).expect("Account is not registered");
        let discount = self.compute_max_discount(&position, &liquidation_account, &prices);
        if let Position::LPTokenPosition(position_info) = liquidation_account.positions.get(position).expect("Position not found") {
            let collateral_asset = self.internal_unwrap_asset(&AccountId::new_unchecked(position_info.lpt_id.clone()));
            let collateral_shares = position_info.collateral;
            let collateral_balance = collateral_asset.supplied.shares_to_amount(collateral_shares, false);
            collateral_assets.insert(AccountId::new_unchecked(position_info.lpt_id.clone()), collateral_balance.into());

            let unit_share_tokens = self.last_lp_token_infos.get(position).expect("lp_token_infos not found");
            assert!(env::block_timestamp() - unit_share_tokens.timestamp <= to_nano(config.lp_tokens_info_valid_duration_sec), "LP token info timestamp is too stale");
            let unit_share = 10u128.pow(unit_share_tokens.decimals as u32);
            let collateral_sum = unit_share_tokens.tokens
                .iter()
                .fold(BigDecimal::zero(), |sum, unit_share_token_value|{
                    let token_asset = self.internal_unwrap_asset(&unit_share_token_value.token_id);
                    let token_stdd_amount = unit_share_token_value.amount.0 * 10u128.pow(token_asset.config.extra_decimals as u32);
                    let token_balance = u128_ratio(token_stdd_amount, collateral_balance, 10u128.pow(collateral_asset.config.extra_decimals as u32) * unit_share);
                    
                    sum + BigDecimal::from_balance_price(
                        token_balance,
                        prices.get_unwrap(&unit_share_token_value.token_id),
                        token_asset.config.extra_decimals,
                    )
                });

            for (token_id, shares) in position_info.borrowed.iter() {
                let asset = self.internal_unwrap_asset(&token_id);
                let amount = asset.borrowed.shares_to_amount(*shares, true);
                assert!(
                    asset.reserved >= amount,
                    "Not enough {} in reserve",
                    token_id
                );

                borrowed_assets.insert(token_id.clone(), amount.into());
                borrowed_sum = borrowed_sum
                    + BigDecimal::from_balance_price(
                        amount,
                        prices.get_unwrap(&token_id),
                        asset.config.extra_decimals,
                    );
            }

            assert!(
                borrowed_sum > collateral_sum,
                "Total borrowed sum {} is not greater than total collateral sum {}",
                borrowed_sum,
                collateral_sum
            );
    
            ext_ref_exchange::ext(self.internal_config().ref_exchange_id)
                .with_static_gas(shadow_actions::GAS_FOR_PROCESS_FORCE_CLOSE_RESULT)
                .on_burrow_liquidation(
                    config.owner_id.clone(),
                    liquidation_account_id.clone(),
                    position.clone(), 
                    U128(collateral_balance), 
                    min_token_amounts)
                .then(
                    Self::ext(env::current_account_id())
                        .with_static_gas(shadow_actions::GAS_FOR_CALLBACK_PROCESS_FORCE_CLOSE_RESULT)
                        .callback_process_shadow_force_close_result(
                            liquidation_account_id.clone(),
                            position.clone(),
                            collateral_sum,
                            borrowed_sum,
                            collateral_assets,
                            borrowed_assets,
                            discount
                        )
                );
        } else {
            env::panic_str("Internal error");
        }
    }
}

#[near_bindgen]
impl Contract {
    #[private]
    pub fn callback_sync_lp_infos(&mut self) {
        if let Some(cross_call_result) = promise_result_as_success() {
            if let Ok(lp_token_infos) = serde_json::from_slice::<HashMap<String, UnitShareTokens>>(&cross_call_result) {
                for (key, value) in lp_token_infos {
                    self.last_lp_token_infos.insert(key, value);
                }
            } else {
                log!("Invalid cross-contract result");
            }
        } else {
            log!("Cross-contract call failed");
        }
    }

    #[private]
    pub fn callback_process_shadow_liquidate_result(
        &mut self,
        sender_id: AccountId,
        liquidation_account_id: AccountId,
        position: String, 
        in_assets: Vec<AssetAmount>,
        out_assets: Vec<AssetAmount>,
        collateral_sum: BigDecimal,
        repaid_sum: BigDecimal,
        max_discount: BigDecimal,
        new_max_discount: BigDecimal
    ) {
        let mut account = self.internal_unwrap_account(&sender_id);
        let mut liquidation_account = self.internal_unwrap_account(&liquidation_account_id);
        account.is_locked = false;
        liquidation_account.is_locked = false;

        if is_promise_success() {
            for asset_amount in in_assets {
                liquidation_account.add_affected_farm(FarmId::Borrowed(asset_amount.token_id.clone()));
                liquidation_account.add_affected_farm(FarmId::TokenNetBalance(asset_amount.token_id.clone()));
                self.internal_liquidate_repay(&position, &mut account, &mut liquidation_account, &asset_amount);
            }

            let mut collateral_asset = self.internal_unwrap_asset(&out_assets[0].token_id);
            liquidation_account.add_affected_farm(FarmId::Supplied(out_assets[0].token_id.clone()));
            liquidation_account.add_affected_farm(FarmId::TokenNetBalance(out_assets[0].token_id.clone()));
            let collateral_shares = liquidation_account.internal_unwrap_collateral(&position, &out_assets[0].token_id);
            let (shares, amount) =
                asset_amount_to_shares(&collateral_asset.supplied, collateral_shares, &out_assets[0], false);
            liquidation_account.decrease_collateral(&position, &out_assets[0].token_id, shares);
            collateral_asset.supplied.withdraw(shares, amount);
            self.internal_set_asset_without_asset_basic_check(&out_assets[0].token_id, collateral_asset);

            self.internal_account_apply_affected_farms(&mut account);
            self.internal_account_apply_affected_farms(&mut liquidation_account);

            events::emit::liquidate(
                &sender_id,
                &liquidation_account_id,
                &collateral_sum,
                &repaid_sum,
                &max_discount,
                &new_max_discount,
                &position
            );
        }
        self.internal_set_account(&sender_id, account);
        self.internal_set_account(&liquidation_account_id, liquidation_account);
    }

    #[private]
    pub fn callback_process_shadow_force_close_result (
        &mut self,
        liquidation_account_id: AccountId,
        position: String, 
        collateral_sum: BigDecimal,
        repaid_sum: BigDecimal,
        collateral_assets: HashMap<AccountId, U128>,
        borrowed_assets: HashMap<AccountId, U128>,
        discount: BigDecimal
    ) {
        let mut liquidation_account = self.internal_unwrap_account(&liquidation_account_id);
        liquidation_account.is_locked = false;

        if is_promise_success() {
            if let Position::LPTokenPosition(position_info) = liquidation_account.positions.remove(&position).unwrap(){
                let mut remain_borrowed = HashMap::new();
                liquidation_account.add_affected_farm(FarmId::Supplied(AccountId::new_unchecked(position_info.lpt_id.clone())));
                liquidation_account.add_affected_farm(FarmId::TokenNetBalance(AccountId::new_unchecked(position_info.lpt_id.clone())));
                for (token_id, shares) in position_info.borrowed {
                    let mut asset = self.internal_unwrap_asset(&token_id);
                    let amount = asset.borrowed.shares_to_amount(shares, true);
                    if asset.reserved >= amount {
                        asset.reserved -= amount;
                        asset.borrowed.withdraw(shares, amount);
                        self.internal_set_asset_without_asset_basic_check(&token_id, asset);
                        liquidation_account.add_affected_farm(FarmId::Borrowed(token_id.clone()));
                        liquidation_account.add_affected_farm(FarmId::TokenNetBalance(token_id));
                    } else {
                        remain_borrowed.insert(token_id, shares);
                    }
                }
                if !remain_borrowed.is_empty() {
                    events::emit::force_close_remain_borrowed(&liquidation_account_id, &remain_borrowed, &position);
                    liquidation_account.positions.insert(position.clone(), Position::LPTokenPosition(LPTokenPosition{
                        lpt_id: position_info.lpt_id.clone(),
                        collateral: U128(0),
                        borrowed: remain_borrowed,
                    }));
                }
                self.internal_account_apply_affected_farms(&mut liquidation_account);
                events::emit::force_close(&liquidation_account_id, &collateral_sum, &repaid_sum, collateral_assets, borrowed_assets, &discount, &position);
            }
        }
        self.internal_set_account(&liquidation_account_id, liquidation_account);
    }
}