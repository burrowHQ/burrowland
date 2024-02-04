use crate::*;

impl Contract {

    pub fn internal_quote(
        &self,
        pool_cache: &mut HashMap<PoolId, Pool>,
        vip_info: Option<HashMap<PoolId, u32>>,
        pool_ids: Vec<PoolId>,
        input_token: AccountId,
        output_token: AccountId,
        input_amount: U128,
        tag: Option<String>,
    ) -> QuoteResult {
        let quote_failed = QuoteResult {
            amount: 0.into(),
            tag: tag.clone(),
        };
        if self.data().state == RunningState::Paused {
            return quote_failed;
        }

        let protocol_fee_rate = self.data().protocol_fee_rate;
        
        let (actual_output_token, actual_output_amount) = {
            let mut next_input_token_or_last_output_token = input_token;
            let mut next_input_amount_or_actual_output = input_amount.0;
            for pool_id in pool_ids {
                let mut pool = pool_cache.remove(&pool_id).unwrap_or(self.internal_unwrap_pool(&pool_id));
                if pool.state == RunningState::Paused || 
                    self.data().frozenlist.contains(&pool.token_x) || self.data().frozenlist.contains(&pool.token_y) {
                    return quote_failed;
                }

                let pool_fee = pool.get_pool_fee_by_user(&vip_info);
                
                let is_finished = if next_input_token_or_last_output_token.eq(&pool.token_x) {
                    let (_, out_amount, is_finished, _, _) =
                        pool.internal_x_swap_y(pool_fee, protocol_fee_rate, next_input_amount_or_actual_output, -799999, true);
                    next_input_token_or_last_output_token = pool.token_y.clone();
                    next_input_amount_or_actual_output = out_amount;
                    is_finished
                } else if next_input_token_or_last_output_token.eq(&pool.token_y) {
                    let (_, out_amount, is_finished, _, _) =
                        pool.internal_y_swap_x(pool_fee, protocol_fee_rate, next_input_amount_or_actual_output, 799999, true);
                    next_input_token_or_last_output_token = pool.token_x.clone();
                    next_input_amount_or_actual_output = out_amount;
                    is_finished
                } else {
                    return quote_failed;
                };
                if !is_finished {
                    return quote_failed;
                }
                pool_cache.insert(pool_id, pool);
            }
            (
                next_input_token_or_last_output_token,
                next_input_amount_or_actual_output,
            )
        };
        if output_token != actual_output_token {
            return quote_failed;
        }
        QuoteResult {
            amount: actual_output_amount.into(),
            tag,
        }
    }

    /// @param account_id
    /// @param pool_ids: all pools participating in swap
    /// @param input_token: the swap-in token, must be in pool_ids[0].tokens
    /// @param input_amount: the amount of swap-in token
    /// @param output_token: the swap-out token, must be in pool_ids[-1].tokens
    /// @param min_output_amount: minimum number of swap-out token to be obtained
    /// @return actual got output token amount
    pub fn internal_swap(
        &mut self,
        account_id: &AccountId,
        pool_ids: Vec<PoolId>,
        input_token: &AccountId,
        input_amount: Balance,
        output_token: &AccountId,
        min_output_amount: Balance,
    ) -> Balance {
        pool_ids.iter().for_each(|pool_id| {
            self.assert_pool_running(&self.internal_unwrap_pool(pool_id));
            let (token_x, token_y, _) = pool_id.parse_pool_id();
            self.assert_no_frozen_tokens(&[token_x, token_y]);
        });
        let protocol_fee_rate = self.data().protocol_fee_rate;
        let vip_info = self.data().vip_users.get(account_id);
        let (actual_output_token, actual_output_amount) = {
            let mut next_input_token_or_last_output_token = input_token.clone();
            let mut next_input_amount_or_actual_output = input_amount;
            for pool_id in pool_ids.iter() {
                let mut pool = self.internal_unwrap_pool(&pool_id);
                
                let pool_fee = pool.get_pool_fee_by_user(&vip_info);

                if next_input_token_or_last_output_token.eq(&pool.token_x) {
                    let (actual_cost, out_amount, is_finished, total_fee, protocol_fee) =
                        pool.internal_x_swap_y(pool_fee, protocol_fee_rate, next_input_amount_or_actual_output, -799999, false);
                    if !is_finished {
                        env::panic_str(&format!("ERR_TOKEN_{}_NOT_ENOUGH", pool.token_y.to_string().to_uppercase()));
                    }

                    pool.total_x += actual_cost;
                    pool.total_y -= out_amount;
                    pool.volume_x_in += U256::from(actual_cost);
                    pool.volume_y_out += U256::from(out_amount);

                    next_input_token_or_last_output_token = pool.token_y.clone();
                    next_input_amount_or_actual_output = out_amount;

                    Event::Swap {
                        swapper: account_id,
                        token_in: &pool.token_x,
                        token_out: &pool.token_y,
                        amount_in: &U128(actual_cost),
                        amount_out: &U128(out_amount),
                        pool_id: &pool.pool_id,
                        total_fee: &U128(total_fee),
                        protocol_fee: &U128(protocol_fee),
                    }
                    .emit();
                } else if next_input_token_or_last_output_token.eq(&pool.token_y) {
                    let (actual_cost, out_amount, is_finished, total_fee, protocol_fee) =
                        pool.internal_y_swap_x(pool_fee, protocol_fee_rate, next_input_amount_or_actual_output, 799999, false);
                    if !is_finished {
                        env::panic_str(&format!("ERR_TOKEN_{}_NOT_ENOUGH", pool.token_x.to_string().to_uppercase()));
                    }

                    pool.total_y += actual_cost;
                    pool.total_x -= out_amount;
                    pool.volume_y_in += U256::from(actual_cost);
                    pool.volume_x_out += U256::from(out_amount);

                    next_input_token_or_last_output_token = pool.token_x.clone();
                    next_input_amount_or_actual_output = out_amount;

                    Event::Swap {
                        swapper: account_id,
                        token_in: &pool.token_y,
                        token_out: &pool.token_x,
                        amount_in: &U128(actual_cost),
                        amount_out: &U128(out_amount),
                        pool_id: &pool.pool_id,
                        total_fee: &U128(total_fee),
                        protocol_fee: &U128(protocol_fee),
                    }
                    .emit();
                } else {
                    env::panic_str(E404_INVALID_POOL_IDS);
                }
                self.internal_set_pool(&pool_id, pool);
            }
            (
                next_input_token_or_last_output_token,
                next_input_amount_or_actual_output,
            )
        };

        require!(output_token == &actual_output_token, E212_INVALID_OUTPUT_TOKEN);
        require!(actual_output_amount >= min_output_amount, E204_SLIPPAGE_ERR);

        actual_output_amount
    }

    /// @param account_id
    /// @param pool_ids: all pools participating in swap
    /// @param input_token: the swap-in token, must be in pool_ids[-1].tokens
    /// @param max_input_amount: maximum amount of swap-in token to pay
    /// @param output_token: the swap-out token, must be in pool_ids[0].tokens
    /// @param output_amount: the amount of swap-out token
    /// @return actual used input token amount
    pub fn internal_swap_by_output(
        &mut self,
        account_id: &AccountId,
        pool_ids: Vec<PoolId>,
        input_token: &AccountId,
        max_input_amount: Balance,
        output_token: &AccountId,
        output_amount: Balance,
        skip_unwrap_near: Option<bool>
    ) -> Balance {
        pool_ids.iter().for_each(|pool_id| {
            self.assert_pool_running(&self.internal_unwrap_pool(pool_id));
            let (token_x, token_y, _) = pool_id.parse_pool_id();
            self.assert_no_frozen_tokens(&[token_x, token_y]);
        });
        
        let protocol_fee_rate = self.data().protocol_fee_rate;
        let vip_info = self.data().vip_users.get(account_id);
        let (actual_input_token, actual_input_amount, actual_output_amount) = {
            let mut next_desire_token = output_token.clone();
            let mut next_desire_amount = output_amount;
            let mut actual_output_amount = output_amount;
            for pool_id in pool_ids.iter() {
                let mut pool = self.internal_unwrap_pool(&pool_id);
                
                let pool_fee = pool.get_pool_fee_by_user(&vip_info);

                if next_desire_token.eq(&pool.token_x) {
                    let (need_amount, acquire_amount, is_finished, total_fee, protocol_fee) = pool.internal_y_swap_x_desire_x(pool_fee, protocol_fee_rate, next_desire_amount, 800001, false);
                    if !is_finished {
                        env::panic_str(&format!("ERR_TOKEN_{}_NOT_ENOUGH", pool.token_x.to_string().to_uppercase()));
                    }

                    pool.total_y += need_amount;
                    pool.total_x -= acquire_amount;
                    pool.volume_y_in += U256::from(need_amount);
                    pool.volume_x_out += U256::from(acquire_amount);

                    actual_output_amount = acquire_amount;
                    next_desire_token = pool.token_y.clone();
                    next_desire_amount = need_amount;

                    Event::SwapDesire {
                        swapper: account_id,
                        token_in: &pool.token_y,
                        token_out: &pool.token_x,
                        amount_in: &U128(need_amount),
                        amount_out: &U128(acquire_amount),
                        pool_id: &pool.pool_id,
                        total_fee: &U128(total_fee),
                        protocol_fee: &U128(protocol_fee),
                    }
                    .emit();
                } else if next_desire_token.eq(&pool.token_y) {
                    let (need_amount, acquire_amount, is_finished, total_fee, protocol_fee) = pool.internal_x_swap_y_desire_y(pool_fee, protocol_fee_rate, next_desire_amount, -800001, false);
                    if !is_finished {
                        env::panic_str(&format!("ERR_TOKEN_{}_NOT_ENOUGH", pool.token_y.to_string().to_uppercase()));
                    }

                    pool.total_x += need_amount;
                    pool.total_y -= acquire_amount;
                    pool.volume_x_in += U256::from(need_amount);
                    pool.volume_y_out += U256::from(acquire_amount);

                    actual_output_amount = acquire_amount;
                    next_desire_token = pool.token_x.clone();
                    next_desire_amount = need_amount;

                    Event::SwapDesire {
                        swapper: account_id,
                        token_in: &pool.token_x,
                        token_out: &pool.token_y,
                        amount_in: &U128(need_amount),
                        amount_out: &U128(acquire_amount),
                        pool_id: &pool.pool_id,
                        total_fee: &U128(total_fee),
                        protocol_fee: &U128(protocol_fee),
                    }
                    .emit();
                } else {
                    env::panic_str(E404_INVALID_POOL_IDS);
                }
                self.internal_set_pool(&pool_id, pool); 
            }
            (next_desire_token, next_desire_amount, actual_output_amount)
        };
        require!(input_token == &actual_input_token, E213_INVALID_INPUT_TOKEN);
        require!(actual_input_amount <= max_input_amount, E204_SLIPPAGE_ERR);

        if actual_output_amount > 0 {
            self.process_transfer(account_id, &output_token, actual_output_amount, skip_unwrap_near);
        }
        
        actual_input_amount
    }

    /// @param account_id
    /// @param pool_id
    /// @param input_token: the swap-in token
    /// @param input_amount: the amount of swap-in token
    /// @param stop_point: low_boundary_point or hight_boundary_point
    /// @return actual cost input token amount
    pub fn internal_swap_by_stop_point(
        &mut self,
        account_id: &AccountId,
        pool_id: &PoolId,
        input_token: &AccountId,
        input_amount: Balance,
        stop_point: i32,
        skip_unwrap_near: Option<bool>
    ) -> Balance {
        let mut pool = self.internal_unwrap_pool(pool_id);
        self.assert_pool_running(&pool);
        self.assert_no_frozen_tokens(&[pool.token_x.clone(), pool.token_y.clone()]);
        
        let protocol_fee_rate = self.data().protocol_fee_rate;

        let vip_info = self.data().vip_users.get(account_id);
        let pool_fee = pool.get_pool_fee_by_user(&vip_info);

        let (output_token, actual_input_amount, actual_output_amount) = if input_token.eq(&pool.token_x) {
            let (actual_input_amount, actual_output_amount, _, total_fee, protocol_fee) = pool.internal_x_swap_y(pool_fee, protocol_fee_rate, input_amount, stop_point, false);
            
            pool.total_x += actual_input_amount;
            pool.total_y -= actual_output_amount;
            pool.volume_x_in += U256::from(actual_input_amount);
            pool.volume_y_out += U256::from(actual_output_amount);

            Event::Swap {
                swapper: account_id,
                token_in: &pool.token_x,
                token_out: &pool.token_y,
                amount_in: &U128(actual_input_amount),
                amount_out: &U128(actual_output_amount),
                pool_id: &pool.pool_id,
                total_fee: &U128(total_fee),
                protocol_fee: &U128(protocol_fee),
            }
            .emit();

            (pool.token_y.clone(), actual_input_amount, actual_output_amount)
        } else if input_token.eq(&pool.token_y) {
            let (actual_input_amount, actual_output_amount, _, total_fee, protocol_fee) = pool.internal_y_swap_x(pool_fee, protocol_fee_rate, input_amount, stop_point, false);
            
            pool.total_y += actual_input_amount;
            pool.total_x -= actual_output_amount;
            pool.volume_y_in += U256::from(actual_input_amount);
            pool.volume_x_out += U256::from(actual_output_amount);

            Event::Swap {
                swapper: account_id,
                token_in: &pool.token_y,
                token_out: &pool.token_x,
                amount_in: &U128(actual_input_amount),
                amount_out: &U128(actual_output_amount),
                pool_id: &pool.pool_id,
                total_fee: &U128(total_fee),
                protocol_fee: &U128(protocol_fee),
            }
            .emit();

            (pool.token_x.clone(), actual_input_amount, actual_output_amount)
        } else {
            env::panic_str(E404_INVALID_POOL_IDS);
        };
        self.internal_set_pool(pool_id, pool);

        if actual_output_amount > 0 {
            self.process_transfer(account_id, &output_token, actual_output_amount, skip_unwrap_near);
        }
        actual_input_amount
    }
}
