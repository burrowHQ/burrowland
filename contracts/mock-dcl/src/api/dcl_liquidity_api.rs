use crate::*;

#[derive(Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct AddLiquidityInfo {
    pub pool_id: PoolId,
    pub left_point: i32,
    pub right_point: i32,
    pub amount_x: U128,
    pub amount_y: U128,
    pub min_amount_x: U128,
    pub min_amount_y: U128,
}

#[derive(Serialize, Deserialize, Clone)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
#[serde(crate = "near_sdk::serde")]
pub struct AddLiquidityPrediction {
    pub need_x: U128,
    pub need_y: U128,
    pub liquidity_amount: U128,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct RemoveLiquidityInfo {
    pub lpt_id: LptId,
    pub amount: U128,
    pub min_amount_x: U128,
    pub min_amount_y: U128,
}

#[near_bindgen]
impl Contract {
    
    pub fn batch_add_liquidity(
        &mut self,
        add_liquidity_infos: Vec<AddLiquidityInfo>,
        skip_unwrap_near: Option<bool>
    ) -> Vec<LptId> {
        require!(add_liquidity_infos.len() > 0);
        self.assert_contract_running();
        let user_id = env::predecessor_account_id();
        let mut user = self.internal_unwrap_user(&user_id);
        let global_config = self.internal_get_global_config();
        require!(user.get_available_slots(global_config.storage_price_per_slot, global_config.storage_for_asset) >= add_liquidity_infos.len() as u64, E107_NOT_ENOUGH_STORAGE_FOR_SLOTS);
        
        let mut pool_cache: HashMap<String, Pool> = HashMap::new();
        let mut lpt_ids = vec![];
        let mut inner_id = self.data().latest_liquidity_id;
        self.internal_check_add_liquidity_infos(&mut user, &mut lpt_ids, &mut pool_cache, &mut inner_id, &add_liquidity_infos);
        self.data_mut().latest_liquidity_id = inner_id;

        let (_, refund_tokens, liquiditys) = self.internal_batch_add_liquidity(&user_id, &lpt_ids, &mut pool_cache, add_liquidity_infos, false);
        
        for (token_id, amount) in refund_tokens {
            self.process_transfer(&user_id, &token_id, amount, skip_unwrap_near);
        }

        for (pool_id, pool) in pool_cache {
            self.internal_set_pool(&pool_id, pool); 
        }

        self.internal_mint_liquiditys(user, liquiditys);
        
        lpt_ids
    }

    pub fn batch_remove_liquidity(
        &mut self,
        remove_liquidity_infos: Vec<RemoveLiquidityInfo>,
        skip_unwrap_near: Option<bool>
    ) -> HashMap<AccountId, U128> {
        require!(remove_liquidity_infos.len() > 0);
        self.assert_contract_running();
        let user_id = env::predecessor_account_id();
        let mut user = self.internal_unwrap_user(&user_id);

        let mut pool_cache = HashMap::new();
        let mut liquiditys = vec![];
        let (remove_mft_details, _) = self.internal_check_remove_liquidity_infos(&mut user, &mut liquiditys, &mut pool_cache, &remove_liquidity_infos);
        for (mft_id, v_liquidity) in remove_mft_details {
            self.internal_decrease_mft_supply(&mft_id, v_liquidity);
        }

        let refund_tokens = self.internal_batch_remove_liquidity(&user_id, &mut pool_cache, &mut liquiditys, remove_liquidity_infos);
        
        for (token_id, amount) in refund_tokens.iter() {
            self.process_transfer(&user_id, token_id, *amount, skip_unwrap_near);
        }
        
        for (pool_id, pool) in pool_cache {
            self.internal_set_pool(&pool_id, pool); 
        }

        self.internal_update_or_burn_liquiditys(&mut user, liquiditys); 
        self.internal_set_user(&user_id, user);
        refund_tokens.into_iter().map(|(k, v)| (k, U128(v))).collect()
    }

    pub fn batch_update_liquidity(
        &mut self,
        remove_liquidity_infos: Vec<RemoveLiquidityInfo>,
        add_liquidity_infos: Vec<AddLiquidityInfo>,
        skip_unwrap_near: Option<bool>
    ) {
        require!(remove_liquidity_infos.len() > 0 && add_liquidity_infos.len() > 0);
        self.assert_contract_running();
        let user_id = env::predecessor_account_id();
        let mut user = self.internal_unwrap_user(&user_id);
        let global_config = self.internal_get_global_config();

        let mut pool_cache = HashMap::new();
        let mut liquiditys = vec![];
        let (remove_mft_details, release_slots) = self.internal_check_remove_liquidity_infos(&mut user, &mut liquiditys, &mut pool_cache, &remove_liquidity_infos);
        require!(user.get_available_slots(global_config.storage_price_per_slot, global_config.storage_for_asset) + release_slots >= add_liquidity_infos.len() as u64, E107_NOT_ENOUGH_STORAGE_FOR_SLOTS);
        for (mft_id, v_liquidity) in remove_mft_details {
            self.internal_decrease_mft_supply(&mft_id, v_liquidity);
        }  
        let refund_tokens = self.internal_batch_remove_liquidity(&user_id, &mut pool_cache, &mut liquiditys, remove_liquidity_infos);
        for (token_id, amount) in refund_tokens.into_iter() {
            user.add_asset(&token_id, amount);
        }

        self.internal_update_or_burn_liquiditys(&mut user, liquiditys); 

        let mut lpt_ids = vec![];
        let mut inner_id = self.data_mut().latest_liquidity_id;
        self.internal_check_add_liquidity_infos(&mut user, &mut lpt_ids, &mut pool_cache, &mut inner_id, &add_liquidity_infos);
        self.data_mut().latest_liquidity_id = inner_id;

        let (_, refund_tokens, liquiditys) = self.internal_batch_add_liquidity(&user_id, &lpt_ids, &mut pool_cache, add_liquidity_infos, false);
        
        for (token_id, amount) in refund_tokens {
            self.process_transfer(&user_id, &token_id, amount, skip_unwrap_near);
        }

        for (pool_id, pool) in pool_cache {
            self.internal_set_pool(&pool_id, pool); 
        }

        self.internal_mint_liquiditys(user, liquiditys);
    }
}

#[near_bindgen]
impl Contract {
    /// Add liquidity and get lpt
    /// @param pool_id: a string like token_a|token_b|fee
    /// @param left_point: left point of this range
    /// @param right_point: right point of this range
    /// @param amount_x: the number of token X users expect to add liquidity to use
    /// @param amount_y: the number of token Y users expect to add liquidity to use
    /// @param min_amount_x: the minimum number of token X users expect to add liquidity to use
    /// @param min_amount_y: the minimum number of token Y users expect to add liquidity to use
    /// @return the exist or new-mint lp token id, a string like pool_id|inner_id
    pub fn add_liquidity(
        &mut self,
        pool_id: PoolId,
        left_point: i32,
        right_point: i32,
        amount_x: U128,
        amount_y: U128,
        min_amount_x: U128,
        min_amount_y: U128,
        skip_unwrap_near: Option<bool>
    ) -> LptId {
        self.assert_contract_running();
        let user_id = env::predecessor_account_id();
        let mut user = self.internal_unwrap_user(&user_id);
        let global_config = self.internal_get_global_config();
        require!(user.get_available_slots(global_config.storage_price_per_slot, global_config.storage_for_asset) > 0, E107_NOT_ENOUGH_STORAGE_FOR_SLOTS);
        
        let mut pool = self.internal_unwrap_pool(&pool_id);
        self.assert_no_frozen_tokens(&[pool.token_x.clone(), pool.token_y.clone()]);
        require!(left_point % pool.point_delta == 0  && right_point % pool.point_delta == 0, E200_INVALID_ENDPOINT);
        require!(right_point > left_point, E202_ILLEGAL_POINT);
        require!(right_point - left_point < RIGHT_MOST_POINT, E202_ILLEGAL_POINT);
        require!(left_point >= LEFT_MOST_POINT && right_point <= RIGHT_MOST_POINT, E202_ILLEGAL_POINT);
        
        let lpt_id = gen_lpt_id(&pool_id, &mut self.data_mut().latest_liquidity_id);
        let (need_x, need_y, liquidity) = self.internal_add_liquidity(&mut pool, &user_id, lpt_id.clone(), left_point, right_point, amount_x, amount_y, min_amount_x, min_amount_y, false);
        user.sub_asset(&pool.token_x, amount_x.0);
        user.sub_asset(&pool.token_y, amount_y.0);

        let refund_x = amount_x.0 - need_x;
        let refund_y = amount_y.0 - need_y;
        if refund_x > 0{
            self.process_transfer(&user_id, &pool.token_x, refund_x, skip_unwrap_near);
        }
        if refund_y > 0{
            self.process_transfer(&user_id, &pool.token_y, refund_y, skip_unwrap_near);
        }

        self.internal_set_pool(&pool_id, pool); 
        self.internal_mint_liquiditys(user, vec![liquidity]);
        lpt_id
    }

    /// Append liquidity to the specified lpt
    /// @param lpt_id: a string like pool_id|inner_id
    /// @param amount_x: the number of token X users expect to add liquidity to use
    /// @param amount_y: the number of token Y users expect to add liquidity to use
    /// @param min_amount_x: the minimum number of token X users expect to add liquidity to use
    /// @param min_amount_y: the minimum number of token Y users expect to add liquidity to use
    pub fn append_liquidity(
        &mut self,
        lpt_id: LptId,
        amount_x: U128,
        amount_y: U128,
        min_amount_x: U128,
        min_amount_y: U128,
        skip_unwrap_near: Option<bool>
    ) {
        self.assert_contract_running();
        let user_id = env::predecessor_account_id();
        let mut user = self.internal_unwrap_user(&user_id);
        let mut liquidity = self.internal_unwrap_user_liquidity(&lpt_id);
        require!(!liquidity.is_mining(), E218_USER_LIQUIDITY_IS_MINING);
        require!(user_id == liquidity.owner_id, E215_NOT_LIQUIDITY_OWNER);
        let mut pool = self.internal_unwrap_pool(&liquidity.pool_id);
        self.assert_no_frozen_tokens(&[pool.token_x.clone(), pool.token_y.clone()]);

        let (new_liquidity, need_x, need_y, acc_fee_x_in_128, acc_fee_y_in_128) = pool.internal_add_liquidity(liquidity.left_point, liquidity.right_point, amount_x.0, amount_y.0, min_amount_x.0, min_amount_y.0, false);
        user.sub_asset(&pool.token_x, amount_x.0);
        user.sub_asset(&pool.token_y, amount_y.0);

        liquidity.get_unclaimed_fee(acc_fee_x_in_128, acc_fee_y_in_128);
        let new_fee_x = liquidity.unclaimed_fee_x.unwrap_or(U128(0)).0;
        let new_fee_y = liquidity.unclaimed_fee_y.unwrap_or(U128(0)).0;

        pool.total_liquidity += new_liquidity;
        pool.total_x += need_x;
        pool.total_y += need_y;
        pool.total_x -= new_fee_x;
        pool.total_y -= new_fee_y;

        // refund
        let refund_x = amount_x.0 - need_x + new_fee_x;
        let refund_y = amount_y.0 - need_y + new_fee_y;
        if refund_x > 0{
            self.process_transfer(&user_id, &pool.token_x, refund_x, skip_unwrap_near);
        }
        if refund_y > 0{
            self.process_transfer(&user_id, &pool.token_y, refund_y, skip_unwrap_near);
        }
        // update lpt
        liquidity.amount += new_liquidity;
        liquidity.last_fee_scale_x_128 = acc_fee_x_in_128;
        liquidity.last_fee_scale_y_128 = acc_fee_y_in_128;
        self.internal_set_user(&user.user_id.clone(), user); 
        self.internal_set_pool(&liquidity.pool_id, pool);  
        Event::LiquidityAppend {
            lpt_id: &lpt_id,
            owner_id: &user_id,
            pool_id: &liquidity.pool_id,
            left_point: &liquidity.left_point,
            right_point: &liquidity.right_point,
            added_amount: &U128(new_liquidity),
            cur_amount: &U128(liquidity.amount),
            paid_token_x: &U128(need_x),
            paid_token_y: &U128(need_y),
            claim_fee_token_x: &U128(new_fee_x),
            claim_fee_token_y: &U128(new_fee_y),
        }
        .emit();
        self.internal_set_user_liquidity(&lpt_id, liquidity);
    }

    /// Users can merge lpts with the same left and right boundaries in the same pool
    /// @param lpt_id: a string like pool_id|inner_id
    /// @param lpt_id_list
    pub fn merge_liquidity(
        &mut self,
        lpt_id: LptId,
        lpt_id_list: Vec<LptId>,
        skip_unwrap_near: Option<bool>
    ) {
        self.assert_contract_running();
        require!(lpt_id_list.len() > 0, E216_INVALID_LPT_LIST);
        let user_id = env::predecessor_account_id();
        let mut retain_liquidity = self.internal_unwrap_user_liquidity(&lpt_id);
        require!(!retain_liquidity.is_mining(), E218_USER_LIQUIDITY_IS_MINING);
        require!(retain_liquidity.owner_id == user_id, E215_NOT_LIQUIDITY_OWNER);
        let mut pool = self.internal_unwrap_pool(&retain_liquidity.pool_id);
        self.assert_no_frozen_tokens(&[pool.token_x.clone(), pool.token_y.clone()]);

        let mut remove_token_x = 0;
        let mut remove_token_y = 0;
        let mut remove_fee_x = 0;
        let mut remove_fee_y = 0;

        let mut merge_lpt_ids = String::new();
        for item in lpt_id_list.iter() {
            merge_lpt_ids = format!("{}{}{}", merge_lpt_ids, if merge_lpt_ids.is_empty() { "" } else { "," }, item);
            let user = self.internal_unwrap_user(&user_id);
            let mut liquidity = self.internal_unwrap_user_liquidity(item);
            require!(item != &lpt_id &&
                liquidity.owner_id == retain_liquidity.owner_id &&
                liquidity.pool_id == retain_liquidity.pool_id && 
                liquidity.left_point == retain_liquidity.left_point &&
                liquidity.right_point == retain_liquidity.right_point &&
                !liquidity.is_mining(), E216_INVALID_LPT_LIST);

            let remove_liquidity = liquidity.amount;
            let (remove_x, remove_y, acc_fee_x_in_128, acc_fee_y_in_128) = 
                pool.internal_remove_liquidity(remove_liquidity, liquidity.left_point, liquidity.right_point, 0, 0);
            
            liquidity.get_unclaimed_fee(acc_fee_x_in_128, acc_fee_y_in_128);
            let fee_x = liquidity.unclaimed_fee_x.unwrap_or(U128(0)).0;
            let fee_y = liquidity.unclaimed_fee_y.unwrap_or(U128(0)).0;

            remove_token_x += remove_x;
            remove_token_y += remove_y;
            remove_fee_x += fee_x;
            remove_fee_y += fee_y;

            pool.total_liquidity -= liquidity.amount;
            pool.total_x -= remove_x + fee_x;
            pool.total_y -= remove_y + fee_y;
            self.internal_burn_liquidity(user, &liquidity);
        }

        let (new_liquidity, need_x, need_y, acc_fee_x_in_128, acc_fee_y_in_128) = 
            pool.internal_add_liquidity(retain_liquidity.left_point, retain_liquidity.right_point, remove_token_x, remove_token_y, 0, 0, false);
        retain_liquidity.get_unclaimed_fee(acc_fee_x_in_128, acc_fee_y_in_128);
        let new_fee_x = retain_liquidity.unclaimed_fee_x.unwrap_or(U128(0)).0;
        let new_fee_y = retain_liquidity.unclaimed_fee_y.unwrap_or(U128(0)).0;
    
        pool.total_liquidity += new_liquidity;
        pool.total_x += need_x;
        pool.total_y += need_y;
        pool.total_x -= new_fee_x;
        pool.total_y -= new_fee_y;

        let refund_x = remove_token_x - need_x + new_fee_x + remove_fee_x;
        let refund_y = remove_token_y - need_y + new_fee_y + remove_fee_y;

        if refund_x > 0{
            self.process_transfer(&user_id, &pool.token_x, refund_x, skip_unwrap_near);
        }
        if refund_y > 0{
            self.process_transfer(&user_id, &pool.token_y, refund_y, skip_unwrap_near);
        }

        retain_liquidity.amount += new_liquidity;
        retain_liquidity.last_fee_scale_x_128 = acc_fee_x_in_128;
        retain_liquidity.last_fee_scale_y_128 = acc_fee_y_in_128;

        self.internal_set_pool(&retain_liquidity.pool_id, pool);  
        Event::LiquidityMerge {
            lpt_id: &lpt_id,
            merge_lpt_ids: &merge_lpt_ids,
            owner_id: &user_id,
            pool_id: &retain_liquidity.pool_id,
            left_point: &retain_liquidity.left_point,
            right_point: &retain_liquidity.right_point,
            added_amount: &U128(new_liquidity),
            cur_amount: &U128(retain_liquidity.amount),
            remove_token_x: &U128(remove_token_x),
            remove_token_y: &U128(remove_token_y),
            merge_token_x: &U128(need_x),
            merge_token_y: &U128(need_y),
            claim_fee_token_x: &U128(new_fee_x + remove_fee_x),
            claim_fee_token_y: &U128(new_fee_y + remove_fee_y),
        }
        .emit();
        self.internal_set_user_liquidity(&lpt_id, retain_liquidity);
    }

    /// If all amount in this lp token is removed, 
    /// it will be a burn operation else decrease amount of this lp token
    /// @param lpt_id: a string like pool_id|inner_id
    /// @param amount: amount of liquidity.
    /// @param min_amount_x: removing liquidity will at least give you the number of token X
    /// @param min_amount_y: removing liquidity will at least give you the number of token Y
    /// @return (amount_x, amount_y)
    /// amount_x, balance of tokenX released into inner account (including feeX);
    /// amount_y, balance of tokenY released into inner account (including feeY);
    /// Note: remove_liquidity with 0 amount, 0 min_amount_x, 0 min_amount_y means claim
    pub fn remove_liquidity(
        &mut self,
        lpt_id: LptId,
        amount: U128,
        min_amount_x: U128,
        min_amount_y: U128,
        skip_unwrap_near: Option<bool>
    ) -> (U128, U128) {
        self.assert_contract_running();
        let user_id = env::predecessor_account_id();
        let mut user = self.internal_unwrap_user(&user_id);
        let mut liquidity = self.internal_unwrap_user_liquidity(&lpt_id);
        require!(user_id == liquidity.owner_id, E215_NOT_LIQUIDITY_OWNER);
        let mut pool = self.internal_unwrap_pool(&liquidity.pool_id);
        self.assert_no_frozen_tokens(&[pool.token_x.clone(), pool.token_y.clone()]);

        let remove_liquidity = if amount.0 < liquidity.amount { amount.0 } else { liquidity.amount };
        if remove_liquidity > 0 {
            if liquidity.is_mining() {
                if user.mft_assets.get(&liquidity.mft_id).unwrap_or_default() >= liquidity.v_liquidity {
                    user.sub_mft_asset(&liquidity.mft_id, liquidity.v_liquidity);
                    self.internal_decrease_mft_supply(&liquidity.mft_id, liquidity.v_liquidity);
                    liquidity.mft_id = String::new();
                    liquidity.v_liquidity = 0;
                }else {
                    env::panic_str(E218_USER_LIQUIDITY_IS_MINING);
                }
            } 
        }

        let (refund_x, refund_y) = self.internal_remove_liquidity(&user_id, &mut pool, &mut liquidity, remove_liquidity, min_amount_x, min_amount_y);
        if refund_x > 0{
            self.process_transfer(&user_id, &pool.token_x, refund_x, skip_unwrap_near);
        }
        if refund_y > 0{
            self.process_transfer(&user_id, &pool.token_y, refund_y, skip_unwrap_near);
        }

        self.internal_set_pool(&liquidity.pool_id, pool);  

        if liquidity.amount > 0 {
            self.internal_set_user(&user.user_id.clone(), user); 
            self.internal_set_user_liquidity(&lpt_id, liquidity);
        } else {
            self.internal_burn_liquidity(user, &liquidity);
        }
        
        (refund_x.into(), refund_y.into())
    }

    pub fn get_liquidity(&self, lpt_id: LptId) -> Option<UserLiquidity> {
        if let Some(mut liquidity) = self.internal_get_user_liquidity(&lpt_id) {
            let pool = self.internal_unwrap_pool(&liquidity.pool_id);
            let (acc_fee_x_in_128, acc_fee_y_in_128) = pool.point_info.get_fee_in_range(liquidity.left_point, liquidity.right_point, pool.current_point, pool.fee_scale_x_128, pool.fee_scale_y_128);
            liquidity.get_unclaimed_fee(acc_fee_x_in_128, acc_fee_y_in_128);
            Some(liquidity)
        } else {
            None
        }
    }

    pub fn list_liquidities(
        &self,
        account_id: AccountId,
        from_index: Option<u64>,
        limit: Option<u64>,
    ) -> Vec<UserLiquidity> {
        if let Some(user) = self.internal_get_user(&account_id) {
            let lpt_ids = user.liquidity_keys.as_vector();
            let from_index = from_index.unwrap_or(0);
            let limit = limit.unwrap_or(lpt_ids.len());

            (from_index..std::cmp::min(from_index + limit, lpt_ids.len()))
                .map(|index| {
                    lpt_ids.get(index).unwrap()
                })
                .map(|lpt_id| self.internal_unwrap_user_liquidity(&lpt_id))
                .collect()
        } else {
            vec![]
        }
    }

    pub fn predict_add_liquidity(
        &self,
        pool_id: PoolId,
        left_point: i32,
        right_point: i32,
        amount_x: U128,
        amount_y: U128,
    ) -> AddLiquidityPrediction {
        let view_account = AccountId::new_unchecked("view".to_string());
        let mut pool = self.internal_unwrap_pool(&pool_id);
        let lpt_id = "view_lpt_id".to_string();
        let (need_x, need_y, liquidity) = self.internal_add_liquidity(&mut pool, &view_account, lpt_id, left_point, right_point, amount_x, amount_y, U128(0), U128(0), true);
        AddLiquidityPrediction {
            need_x: U128(need_x),
            need_y: U128(need_y),
            liquidity_amount: U128(liquidity.amount)
        }
    }

    pub fn predict_hotzap(
        &self,
        vip_info: Option<HashMap<PoolId, u32>>,
        token_in: AccountId,
        amount_in: U128,
        swap_infos: Vec<SwapInfo>,
        add_liquidity_infos: Vec<AddLiquidityInfo>,
    )-> Option<(Vec<AddLiquidityPrediction>, HashMap<AccountId, U128>)> {
        if swap_infos.is_empty() || add_liquidity_infos.is_empty() {
            return None
        }
        let view_account = AccountId::new_unchecked("view".to_string());
        let mut pool_cache = HashMap::new();

        let mut token_cache = TokenCache::new();
        token_cache.add(&token_in, amount_in.0);
        for swap_info in swap_infos {
            token_cache.sub(&swap_info.input_token, swap_info.amount_in.0);
            let actual_output_amount = self.internal_quote(&mut pool_cache, vip_info.clone(), swap_info.pool_ids, swap_info.input_token, swap_info.output_token.clone(), swap_info.amount_in, None);
            if actual_output_amount.amount.0 == 0 { return None }
            token_cache.add(&swap_info.output_token, actual_output_amount.amount.0);
        }

        let mut lpt_ids = vec![];
        let mut inner_id = self.data().latest_liquidity_id;
        self.internal_check_add_liquidity_infos_by_cache(&mut token_cache, &mut lpt_ids, &mut pool_cache, &mut inner_id, &add_liquidity_infos);

        let (need_tokens, refund_tokens, liquiditys) = self.internal_batch_add_liquidity(&view_account, &lpt_ids, &mut pool_cache, add_liquidity_infos, true);
        
        for (token_id, amount) in refund_tokens {
            token_cache.add(&token_id, amount);
        }

        Some((
            liquiditys.iter().zip(need_tokens.iter()).map(|(liquidity, need_token)| AddLiquidityPrediction{
                need_x: U128(need_token.0),
                need_y: U128(need_token.1),
                liquidity_amount: U128(liquidity.amount)
            }).collect::<Vec<_>>(),
            token_cache.into()
        ))
    }
}
