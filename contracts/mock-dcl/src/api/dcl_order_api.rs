use crate::*;

#[near_bindgen]
impl Contract {
    pub fn get_order(&self, order_id: OrderId) -> Option<UserOrder> {
        if let Some(mut order) = self.internal_get_user_order(&order_id) {
            let mut po = self
                .internal_get_pool(&order.pool_id)
                .unwrap()
                .point_info
                .0
                .get(&order.point)
                .unwrap()
                .order_data
                .unwrap();
            internal_update_order(&mut order, &mut po);
            Some(order)
        } else {
            None
        }
    }

    /// Find the user order at the specified point
    /// @param account_id
    /// @param pool_id
    /// @param point
    /// @return Option<UserOrder>
    pub fn find_order(
        &self,
        account_id: AccountId,
        pool_id: PoolId,
        point: i32,
    ) -> Option<UserOrder> {
        if let Some(mut order) = self
            .internal_get_user(&account_id)
            .and_then(|user| user.order_keys.get(&gen_user_order_key(&pool_id, point)))
            .and_then(|order_id| self.internal_get_user_order(&order_id))
        {
            let mut po = self
                .internal_get_pool(&order.pool_id)
                .unwrap()
                .point_info
                .0
                .get(&order.point)
                .unwrap()
                .order_data
                .unwrap();
            internal_update_order(&mut order, &mut po);
            Some(order)
        } else {
            None
        }
    }

    pub fn list_active_orders(&self, account_id: AccountId) -> Vec<UserOrder> {
        if let Some(user) = self.internal_get_user(&account_id) {
            user.order_keys
                .values()
                .map(|order_id| {
                    let mut order = self.internal_unwrap_user_order(&order_id);
                    let mut po = self
                        .internal_get_pool(&order.pool_id)
                        .unwrap()
                        .point_info
                        .0
                        .get(&order.point)
                        .unwrap()
                        .order_data
                        .unwrap();
                    internal_update_order(&mut order, &mut po);
                    order
                })
                .collect()
        } else {
            vec![]
        }
    }

    pub fn list_history_orders(&self, account_id: AccountId) -> Vec<UserOrder> {
        if let Some(user) = self.internal_get_user(&account_id) {
            user.history_orders.to_vec()
        } else {
            vec![]
        }
    }

    #[cfg(test)]
    pub fn list_order_data(&self, pool_id: PoolId, point: i32) -> OrderData {
        let pool = self.internal_get_pool(&pool_id).unwrap();
        require!(point % pool.point_delta as i32 == 0, E202_ILLEGAL_POINT);

        let point_data = pool.point_info.0.get(&point).unwrap_or_default();

        let point_order: OrderData = point_data.order_data.unwrap_or_default();
        
        point_order
    }

    /// @param order_id
    /// @param amount: max cancel amount of selling token
    /// @return (actual removed sell token, bought token till last update)
    /// Note: cancel_order with 0 amount means claim
    pub fn cancel_order(&mut self, order_id: OrderId, amount: Option<U128>, skip_unwrap_near: Option<bool>) -> (U128, U128) {
        self.assert_contract_running();
        let mut order = self.internal_unwrap_user_order(&order_id);

        let user_id = env::predecessor_account_id();
        require!(order.owner_id == user_id, E300_NOT_ORDER_OWNER);

        let mut pool = self.internal_get_pool(&order.pool_id).unwrap();
        self.assert_no_frozen_tokens(&[pool.token_x.clone(), pool.token_y.clone()]);
        let mut point_data = pool.point_info.0.get(&order.point).unwrap();
        let mut point_order: OrderData = point_data.order_data.unwrap();

        let earned = internal_update_order(&mut order, &mut point_order);

        // do cancel
        let actual_cancel_amount = if let Some(expected_cancel_amount) = amount {
            min(expected_cancel_amount.into(), order.remain_amount)
        } else {
            order.remain_amount
        };
        order.cancel_amount += actual_cancel_amount;
        order.remain_amount -= actual_cancel_amount;

        // update point_data
        if order.is_earn_y() {
            pool.total_x -= actual_cancel_amount;
            pool.total_y -= earned;
            pool.total_order_x -= actual_cancel_amount;
            point_order.selling_x -= actual_cancel_amount;
        } else {
            pool.total_x -= earned;
            pool.total_y -= actual_cancel_amount;
            pool.total_order_y -= actual_cancel_amount;
            point_order.selling_y -= actual_cancel_amount;
        }
        point_data.order_data = if order.remain_amount == 0 {
            point_order.user_order_count -= 1;
            if point_order.user_order_count == 0 {
                pool.total_order_x -= point_order.selling_x;
                pool.total_order_y -= point_order.selling_y;
                pool.total_x -= point_order.selling_x;
                pool.total_y -= point_order.selling_y;
                None
            } else {
                Some(point_order)
            }
        } else {
            Some(point_order)
        };
        if !point_data.has_active_liquidity() && !point_data.has_active_order()  {
            pool.slot_bitmap.set_zero(order.point, pool.point_delta);
        }
        if point_data.has_order() || point_data.has_liquidity() {
            pool.point_info.0.insert(&order.point, &point_data);
        } else {
            pool.point_info.0.remove(&order.point);
        }
        self.internal_set_pool(&order.pool_id, pool);

        Event::OrderCancelled {
            order_id: &order.order_id,
            created_at: &U64(order.created_at),
            cancel_at: &U64(env::block_timestamp()),
            owner_id: &order.owner_id,
            pool_id: &order.pool_id,
            point: &order.point,
            sell_token: &order.sell_token,
            buy_token: &order.buy_token,
            request_cancel_amount: &amount,
            actual_cancel_amount: &U128(actual_cancel_amount),
            original_amount: &U128(order.original_amount),
            cancel_amount: &U128(order.cancel_amount),
            remain_amount: &U128(order.remain_amount),
            bought_amount: &U128(order.bought_amount),
        }
        .emit();

        // transfer token to user
        if earned > 0 {
            self.process_transfer(&user_id, &order.buy_token, earned, skip_unwrap_near);
        }

        if actual_cancel_amount > 0 {
            self.process_transfer(&user_id, &order.sell_token, actual_cancel_amount, skip_unwrap_near);
        }

        // deactive order if needed
        if order.remain_amount == 0 {
            // completed order move to user history
            let order_key = gen_user_order_key(&order.pool_id, order.point);
            let mut user = self.internal_unwrap_user(&user_id);
            user.order_keys.remove(&order_key);
            if user.completed_order_count < DEFAULT_USER_ORDER_HISTORY_LEN {
                user.history_orders.push(&order);
            } else {
                let index = user.completed_order_count % DEFAULT_USER_ORDER_HISTORY_LEN;
                user.history_orders.replace(index, &order);
            }
            user.completed_order_count += 1;
            self.internal_set_user(&user_id, user);
            self.data_mut().user_orders.remove(&order_id);
            Event::OrderCompleted {
                order_id: &order.order_id,
                created_at: &U64(order.created_at),
                completed_at: &U64(env::block_timestamp()),
                owner_id: &order.owner_id,
                pool_id: &order.pool_id,
                point: &order.point,
                sell_token: &order.sell_token,
                buy_token: &order.buy_token,
                original_amount: &U128(order.original_amount),
                original_deposit_amount: &U128(order.original_deposit_amount),
                swap_earn_amount: &U128(order.swap_earn_amount),
                cancel_amount: &U128(order.cancel_amount),
                bought_amount: &U128(order.bought_amount),
            }
            .emit();
        } else {
            self.internal_set_user_order(&order_id, order);
        }

        (actual_cancel_amount.into(), earned.into())
    }
}