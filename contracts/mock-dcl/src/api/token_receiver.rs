use crate::*;
use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
use near_sdk::{serde_json, PromiseOrValue};
use std::collections::HashSet;

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct SwapInfo {
    pub pool_ids: Vec<PoolId>,
    pub input_token: AccountId,
    pub amount_in: U128,
    pub output_token: AccountId,
    pub min_output_amount: U128,
}

/// Message parameters to receive via token function call.
#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
enum TokenReceiverMessage {
    Deposit,
    Swap {
        pool_ids: Vec<PoolId>,
        output_token: AccountId,
        min_output_amount: U128,
        skip_unwrap_near: Option<bool>,
        client_echo: Option<String>,
    },
    SwapByOutput {
        pool_ids: Vec<PoolId>,
        output_token: AccountId,
        output_amount: U128,
    },
    SwapByStopPoint {
        pool_id: PoolId,
        stop_point: i32,
        skip_unwrap_near: Option<bool>
    },
    LimitOrder {
        client_id: String,
        pool_id: PoolId,
        buy_token: AccountId,
        point: i32,
    },
    LimitOrderWithSwap {
        client_id: String,
        pool_id: PoolId,
        buy_token: AccountId,
        // should be N * pointdelta
        point: i32,
        skip_unwrap_near: Option<bool>
    },
    HotZap {
        swap_infos: Vec<SwapInfo>,
        add_liquidity_infos: Vec<AddLiquidityInfo>,
    }
}

#[near_bindgen]
impl FungibleTokenReceiver for Contract {
    fn ft_on_transfer(
        &mut self,
        sender_id: AccountId,
        amount: U128,
        msg: String,
    ) -> PromiseOrValue<U128> {
        self.assert_contract_running();

        let amount: u128 = amount.into();
        let token_id = env::predecessor_account_id();
        let message = serde_json::from_str::<TokenReceiverMessage>(&msg).expect(E600_INVALID_MSG);
        match message {
            TokenReceiverMessage::Deposit => {
                let pool_ids = self.data().pools.keys_as_vector();
                let mut allow_tokens = HashSet::new();
                for pool_id in pool_ids.iter() {
                    let (token_x, token_y, _) = pool_id.parse_pool_id();
                    allow_tokens.insert(token_x);
                    allow_tokens.insert(token_y);
                }
                require!(allow_tokens.contains(&token_id), E012_UNSUPPORTED_TOKEN);
                self.deposit_asset(&sender_id, &token_id, amount);
                PromiseOrValue::Value(U128(0))
            }
            TokenReceiverMessage::Swap {
                pool_ids,
                output_token,
                min_output_amount,
                skip_unwrap_near,
                client_echo
            } => {
                let actual_output_amount = self.internal_swap(
                    &sender_id,
                    pool_ids,
                    &token_id,
                    amount,
                    &output_token,
                    min_output_amount.0,
                );
                if actual_output_amount > 0 {
                    if let Some(msg) = client_echo {
                        self.process_ft_transfer_call(&sender_id, &output_token, actual_output_amount, msg);
                    } else {
                        self.process_transfer(&sender_id, &output_token, actual_output_amount, skip_unwrap_near);
                    }
                }
                PromiseOrValue::Value(U128(0))
            }
            TokenReceiverMessage::SwapByOutput {
                ..
            } => {
                env::panic_str("This feature is under construction.");
            }
            // TokenReceiverMessage::SwapByOutput {
            //     pool_ids,
            //     output_token,
            //     output_amount,
            // } => {
            //     let actual_used = self.internal_swap_by_output(
            //         &sender_id,
            //         pool_ids,
            //         &token_id,
            //         amount,
            //         &output_token,
            //         output_amount.0,
            //     );
            //     PromiseOrValue::Value(U128(amount - actual_used))
            // }
            TokenReceiverMessage::SwapByStopPoint {
                pool_id,
                stop_point,
                skip_unwrap_near
            } => {
                let actual_used = self.internal_swap_by_stop_point(
                    &sender_id,
                    &pool_id,
                    &token_id,
                    amount,
                    stop_point,
                    skip_unwrap_near
                );
                PromiseOrValue::Value(U128(amount - actual_used))
            }
            TokenReceiverMessage::LimitOrder {
                client_id,
                pool_id,
                buy_token,
                point,
            } => {
                self.internal_add_order(client_id, &sender_id, &token_id, amount, &pool_id, point, &buy_token, 0, 0);
                PromiseOrValue::Value(U128(0))
            }
            TokenReceiverMessage::LimitOrderWithSwap {
                client_id,
                pool_id,
                buy_token,
                point,
                skip_unwrap_near
            } => {
                self.internal_add_order_with_swap(client_id, &sender_id, &token_id, amount, &pool_id, point, &buy_token, skip_unwrap_near);
                PromiseOrValue::Value(U128(0))
            }
            TokenReceiverMessage::HotZap { 
                swap_infos, 
                add_liquidity_infos, 
            } => {
                require!(swap_infos.len() > 0 && add_liquidity_infos.len() > 0);
                let mut user = self.internal_unwrap_user(&sender_id);
                let global_config = self.internal_get_global_config();
                require!(user.get_available_slots(global_config.storage_price_per_slot, global_config.storage_for_asset) >= add_liquidity_infos.len() as u64, E107_NOT_ENOUGH_STORAGE_FOR_SLOTS);

                let mut token_cache = TokenCache::new();
                token_cache.add(&token_id, amount);
                for swap_info in swap_infos {
                    token_cache.sub(&swap_info.input_token, swap_info.amount_in.0);
                    let actual_output_amount = self.internal_swap(
                        &sender_id,
                        swap_info.pool_ids,
                        &swap_info.input_token,
                        swap_info.amount_in.0,
                        &swap_info.output_token,
                        swap_info.min_output_amount.0,
                    );
                    token_cache.add(&swap_info.output_token, actual_output_amount);
                }
                
                let mut pool_cache: HashMap<String, Pool> = HashMap::new();
                let mut lpt_ids = vec![];
                let mut inner_id = self.data().latest_liquidity_id;
                self.internal_check_add_liquidity_infos_by_cache(&mut token_cache, &mut lpt_ids, &mut pool_cache, &mut inner_id, &add_liquidity_infos);
                self.data_mut().latest_liquidity_id = inner_id;

                let (_, refund_tokens, liquiditys) = self.internal_batch_add_liquidity(&sender_id, &lpt_ids, &mut pool_cache, add_liquidity_infos, false);
                
                for (token_id, amount) in refund_tokens {
                    token_cache.add(&token_id, amount);
                }

                for (token_id, amount) in token_cache.0.iter() {
                    user.add_asset(token_id, *amount);
                }
        
                for (pool_id, pool) in pool_cache {
                    self.internal_set_pool(&pool_id, pool); 
                }
        
                self.internal_mint_liquiditys(user, liquiditys);

                Event::HotZap {
                    account_id: &sender_id,
                    remain_assets: &token_cache.into()
                }
                .emit();

                PromiseOrValue::Value(U128(0))
            }
        }
    }
}
