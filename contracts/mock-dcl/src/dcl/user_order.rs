use crate::*;

#[derive(BorshSerialize, BorshDeserialize, Serialize)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(test, derive(Clone, Debug))]
pub struct UserOrder {
    pub client_id: String,
    pub order_id: OrderId,
    pub owner_id: AccountId,
    pub pool_id: PoolId,
    pub point: i32,
    pub sell_token: AccountId,
    pub buy_token: AccountId,
    // amount through ft_transfer_call
    #[serde(with = "u128_dec_format")]
    pub original_deposit_amount: u128,
    // earn token amount through swap before actual place order
    #[serde(with = "u128_dec_format")]
    pub swap_earn_amount: u128,
    // actual original amount of this order
    #[serde(with = "u128_dec_format")]
    pub original_amount: u128,
    // total cancelled amount of this order
    #[serde(with = "u128_dec_format")]
    pub cancel_amount: u128,
    #[serde(with = "u64_dec_format")]
    pub created_at: Timestamp,

    #[serde(skip_serializing)]
    pub last_acc_earn: U256, // lastAccEarn
    #[serde(with = "u128_dec_format")]
    pub remain_amount: u128, // 0 means history order, sellingRemain
    #[serde(with = "u128_dec_format")]
    pub bought_amount: u128, // accumalated amount into inner account, earn + legacyEarn

    #[borsh_skip]
    pub unclaimed_amount: Option<U128>, // claim will push it to inner account,
}

impl UserOrder {
    pub fn is_earn_y(&self) -> bool {
        self.pool_id.is_earn_y(&self.sell_token)
    }
}

#[derive(BorshSerialize, BorshDeserialize)]
pub enum VUserOrder {
    Current(UserOrder),
}

impl From<VUserOrder> for UserOrder {
    fn from(v: VUserOrder) -> Self {
        match v {
            VUserOrder::Current(c) => c,
        }
    }
}

impl From<UserOrder> for VUserOrder {
    fn from(c: UserOrder) -> Self {
        VUserOrder::Current(c)
    }
}

/// Sync user order with point order, try to claim as much earned as possible
/// @param ue: user order
/// @param po: point order
/// @return earned amount this time
pub fn internal_update_order(ue: &mut UserOrder, po: &mut OrderData) -> u128 {
    let is_earn_y = ue.is_earn_y();
    let sqrt_price_96 = get_sqrt_price(ue.point);
    let (total_earn, total_legacy_earn, acc_legacy_earn, cur_acc_earn) = if is_earn_y {
        (
            po.earn_y,
            po.earn_y_legacy,
            po.acc_earn_y_legacy,
            po.acc_earn_y,
        )
    } else {
        (
            po.earn_x,
            po.earn_x_legacy,
            po.acc_earn_x_legacy,
            po.acc_earn_x,
        )
    };

    if ue.last_acc_earn < acc_legacy_earn {
        // this order has been fully filled
        let mut earn = if is_earn_y {
            let liquidity =
                U256::from(ue.remain_amount).mul_fraction_floor(sqrt_price_96, pow_96());
            liquidity.mul_fraction_floor(sqrt_price_96, pow_96())
        } else {
            let liquidity =
                U256::from(ue.remain_amount).mul_fraction_floor(pow_96(), sqrt_price_96);
            liquidity.mul_fraction_floor(pow_96(), sqrt_price_96)
        }
        .as_u128();

        // update po
        if earn > total_legacy_earn {
            // just protect from some rounding errors
            earn = total_legacy_earn;
        }
        if is_earn_y {
            po.earn_y_legacy -= earn;
        } else {
            po.earn_x_legacy -= earn;
        }

        // update ue
        ue.last_acc_earn = cur_acc_earn;
        ue.remain_amount = 0;
        ue.bought_amount += earn;
        ue.unclaimed_amount = Some(U128(earn));

        earn
    } else {
        // this order needs to compete earn
        let mut earn = min((cur_acc_earn - ue.last_acc_earn).as_u128(), total_earn);

        let mut sold = if is_earn_y {
            let liquidity = U256::from(earn).mul_fraction_ceil(pow_96(), sqrt_price_96);
            liquidity.mul_fraction_ceil(pow_96(), sqrt_price_96)
        } else {
            let liquidity = U256::from(earn).mul_fraction_ceil(sqrt_price_96, pow_96());
            liquidity.mul_fraction_ceil(sqrt_price_96, pow_96())
        }
        .as_u128();

        // actual sold should less or equal to remaining, adjust sold and earn if needed
        if sold > ue.remain_amount {
            sold = ue.remain_amount;
            earn = if is_earn_y {
                let liquidity =
                    U256::from(sold).mul_fraction_floor(sqrt_price_96, pow_96());
                liquidity.mul_fraction_floor(sqrt_price_96, pow_96())
            } else {
                let liquidity =
                    U256::from(sold).mul_fraction_floor(pow_96(), sqrt_price_96);
                liquidity.mul_fraction_floor(pow_96(), sqrt_price_96)
            }
            .as_u128();
        }

        // update po
        if earn > total_earn {
            // just protect from some rounding errors
            earn = total_earn;
        }
        if is_earn_y {
            po.earn_y -= earn;
        } else {
            po.earn_x -= earn;
        }

        // update ue
        ue.last_acc_earn = cur_acc_earn;
        ue.remain_amount -= sold;
        ue.bought_amount += earn;
        ue.unclaimed_amount = Some(U128(earn));

        earn
    }
}

impl Contract {
    /// Swap to given point and place order
    /// @param user_id
    /// @param token_id: the selling token
    /// @param amount: the amount of selling token for this order
    /// @param pool_id: pool of this order
    /// @param point
    /// @param buy_token
    /// @return Option<OrderId>
    ///     None: swap has consumed all sell token
    pub fn internal_add_order_with_swap(
        &mut self,
        client_id: String,
        user_id: &AccountId,
        token_id: &AccountId,
        amount: Balance,
        pool_id: &PoolId,
        point: i32,
        buy_token: &AccountId,
        skip_unwrap_near: Option<bool>
    ) -> Option<OrderId> {
        let mut pool = self.internal_get_pool(pool_id).unwrap();
        self.assert_pool_running(&pool);
        self.assert_no_frozen_tokens(&[pool.token_x.clone(), pool.token_y.clone()]);
        require!(point % pool.point_delta as i32 == 0, E202_ILLEGAL_POINT);
        let mut fee_tokens: Vec<AccountId> = Vec::new();
        let mut total_fee_amounts: Vec<U128> = Vec::new();
        let mut protocol_fee_amounts: Vec<U128> = Vec::new();
        let protocol_fee_rate = self.data().protocol_fee_rate;

        let vip_info = self.data().vip_users.get(user_id);
        let pool_fee = pool.get_pool_fee_by_user(&vip_info);

        let (output_token, swapped_amount, swap_earn_amount, is_finished) = if token_id.eq(&pool.token_x) {
            let (actual_input_amount, actual_output_amount, is_finished, total_fee, protocol_fee) =
                pool.internal_x_swap_y(pool_fee, protocol_fee_rate, amount, point, false);
            
            pool.total_x += actual_input_amount;
            pool.total_y -= actual_output_amount;
            pool.volume_x_in += U256::from(actual_input_amount);
            pool.volume_y_out += U256::from(actual_output_amount);

            fee_tokens.push(pool.token_x.clone());
            total_fee_amounts.push(U128(total_fee));
            protocol_fee_amounts.push(U128(protocol_fee));

            Event::Swap {
                swapper: user_id,
                token_in: &pool.token_x,
                token_out: &pool.token_y,
                amount_in: &if is_finished { U128(amount) } else { U128(actual_input_amount) },
                amount_out: &U128(actual_output_amount),
                pool_id: &pool.pool_id,
                total_fee: &U128(total_fee),
                protocol_fee: &U128(protocol_fee),
            }
            .emit();

            (
                pool.token_y.clone(),
                actual_input_amount,
                actual_output_amount,
                is_finished
            )
        } else if token_id.eq(&pool.token_y) {
            let (actual_input_amount, actual_output_amount, is_finished, total_fee, protocol_fee) =
                pool.internal_y_swap_x(pool_fee, protocol_fee_rate, amount, point + 1, false);
            
            pool.total_y += actual_input_amount;
            pool.total_x -= actual_output_amount;
            pool.volume_y_in += U256::from(actual_input_amount);
            pool.volume_x_out += U256::from(actual_output_amount);

            fee_tokens.push(pool.token_y.clone());
            total_fee_amounts.push(U128(total_fee));
            protocol_fee_amounts.push(U128(protocol_fee));

            Event::Swap {
                swapper: user_id,
                token_in: &pool.token_y,
                token_out: &pool.token_x,
                amount_in: &if is_finished { U128(amount) } else { U128(actual_input_amount) },
                amount_out: &U128(actual_output_amount),
                pool_id: &pool.pool_id,
                total_fee: &U128(total_fee),
                protocol_fee: &U128(protocol_fee),
            }
            .emit();

            (
                pool.token_x.clone(),
                actual_input_amount,
                actual_output_amount,
                is_finished
            )
        } else {
            env::panic_str(E305_INVALID_SELLING_TOKEN_ID);
        };

        if swap_earn_amount > 0 {
            self.process_transfer(user_id, &output_token, swap_earn_amount, skip_unwrap_near);
        }

        self.internal_set_pool(pool_id, pool);

        if is_finished {
            return None;
        }
        Some(self.internal_add_order(
            client_id,
            user_id,
            token_id,
            amount,
            pool_id,
            point,
            buy_token,
            swapped_amount,
            swap_earn_amount,
        ))
    }

    /// Place order at given point
    /// @param user_id: the owner of this order
    /// @param token_id: the selling token
    /// @param amount: the amount of selling token for this order
    /// @param pool_id: pool of this order
    /// @param buy_token: the token this order want to buy
    /// @return OrderId
    pub fn internal_add_order(
        &mut self,
        client_id: String,
        user_id: &AccountId,
        token_id: &AccountId,
        amount: Balance,
        pool_id: &PoolId,
        point: i32,
        buy_token: &AccountId,
        swapped_amount: Balance,
        swap_earn_amount: Balance,
    ) -> OrderId {
        let mut pool = self.internal_get_pool(pool_id).unwrap();
        self.assert_no_frozen_tokens(&[pool.token_x.clone(), pool.token_y.clone()]);
        require!(point % pool.point_delta as i32 == 0, E202_ILLEGAL_POINT);
        require!(client_id.len() <= MAX_USER_ORDER_CLIENT_ID_LEN, E306_INVALID_CLIENT_ID);
        require!(amount - swapped_amount > 0, E307_INVALID_SELLING_AMOUNT);

        let mut user = self.internal_unwrap_user(user_id);
        let order_key = gen_user_order_key(pool_id, point);
        require!(
            user.order_keys.get(&order_key).is_none(),
            E301_ACTIVE_ORDER_ALREADY_EXIST
        );

        let global_config = self.internal_get_global_config();
        require!(user.get_available_slots(global_config.storage_price_per_slot, global_config.storage_for_asset) > 0, E107_NOT_ENOUGH_STORAGE_FOR_SLOTS);

        let mut point_data = pool.point_info.0.get(&point).unwrap_or_default();
        let prev_active_order = point_data.has_active_order();
        let mut point_order: OrderData = point_data.order_data.unwrap_or_default();

        let order_id = gen_order_id(pool_id, &mut self.data_mut().latest_order_id);
        let mut order = UserOrder {
            client_id,
            order_id: order_id.clone(),
            owner_id: user_id.clone(),
            pool_id: pool_id.clone(),
            point,
            sell_token: token_id.clone(),
            buy_token: buy_token.clone(),
            original_deposit_amount: amount,
            swap_earn_amount,
            original_amount: amount - swapped_amount,
            created_at: env::block_timestamp(),
            last_acc_earn: U256::zero(),
            remain_amount: amount - swapped_amount,
            cancel_amount: 0_u128,
            bought_amount: 0_u128,
            unclaimed_amount: None,
        };

        let (token_x, token_y, _) = pool_id.parse_pool_id();
        if token_x.eq(token_id) {
            require!(buy_token == &token_y, E303_ILLEGAL_BUY_TOKEN);
            require!(point >= pool.current_point, E202_ILLEGAL_POINT); // greater or equal to current point
            require!(point <= RIGHT_MOST_POINT, E202_ILLEGAL_POINT);
            order.last_acc_earn = point_order.acc_earn_y;
            point_order.selling_x += amount - swapped_amount;
            pool.total_x += amount - swapped_amount;
            pool.total_order_x += amount - swapped_amount;
        } else if token_y.eq(token_id) {
            require!(buy_token == &token_x, E303_ILLEGAL_BUY_TOKEN);
            require!(point <= pool.current_point, E202_ILLEGAL_POINT); // less or equal to current point
            require!(point >= LEFT_MOST_POINT, E202_ILLEGAL_POINT);
            order.last_acc_earn = point_order.acc_earn_x;
            point_order.selling_y += amount - swapped_amount;
            pool.total_y += amount - swapped_amount;
            pool.total_order_y += amount - swapped_amount;
        } else {
            env::panic_str(E305_INVALID_SELLING_TOKEN_ID);
        }
        point_order.user_order_count += 1;
        // update order
        user.order_keys.insert(&order_key, &order.order_id);
        self.internal_set_user(user_id, user);

        // update pool info
        point_data.order_data = Some(point_order);
        pool.point_info.0.insert(&point, &point_data);
        if !prev_active_order && !point_data.has_active_liquidity() {
            pool.slot_bitmap.set_one(point, pool.point_delta);
        }
        self.internal_set_pool(pool_id, pool);

        Event::OrderAdded {
            order_id: &order.order_id,
            created_at: &U64(env::block_timestamp()),
            owner_id: &order.owner_id,
            pool_id: &order.pool_id,
            point: &order.point,
            sell_token: &order.sell_token,
            buy_token: &order.buy_token,
            original_amount: &U128(order.original_amount),
            original_deposit_amount: &U128(order.original_deposit_amount),
            swap_earn_amount: &U128(order.swap_earn_amount),
        }
        .emit();
        self.internal_set_user_order(&order_id, order);

        order_id
    }
}

impl Contract {
    pub fn internal_get_user_order(&self, order_id: &OrderId) -> Option<UserOrder> {
        self.data().user_orders.get(order_id).map(|o| o.into())
    }

    pub fn internal_unwrap_user_order(&self, order_id: &OrderId) -> UserOrder {
        self.internal_get_user_order(order_id)
            .expect(E304_ORDER_NOT_FOUND)
    }

    pub fn internal_set_user_order(&mut self, order_id: &OrderId, user_order: UserOrder) {
        self.data_mut().user_orders.insert(&order_id, &user_order.into());
    }
}
