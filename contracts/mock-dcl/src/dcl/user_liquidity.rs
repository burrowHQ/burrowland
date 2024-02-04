use crate::*;

#[derive(BorshSerialize, BorshDeserialize, Serialize)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(test, derive(Clone))]
pub struct UserLiquidity {
    pub lpt_id: LptId,
    pub owner_id: AccountId,
    pub pool_id: PoolId,
    pub left_point: i32,
    pub right_point: i32,
    #[serde(skip_serializing)]
    pub last_fee_scale_x_128: U256,
    #[serde(skip_serializing)]
    pub last_fee_scale_y_128: U256,
    #[serde(with = "u128_dec_format")]
    pub amount: u128,
    pub mft_id: MftId,
    #[serde(with = "u128_dec_format")]
    pub v_liquidity: u128,

    #[borsh_skip]
    pub unclaimed_fee_x: Option<U128>,
    #[borsh_skip]
    pub unclaimed_fee_y: Option<U128>, 
}

#[derive(BorshSerialize, BorshDeserialize)]
pub enum VUserLiquidity {
    Current(UserLiquidity),
}

impl From<VUserLiquidity> for UserLiquidity {
    fn from(v: VUserLiquidity) -> Self {
        match v {
            VUserLiquidity::Current(c) => c,
        }
    }
}

impl From<UserLiquidity> for VUserLiquidity {
    fn from(c: UserLiquidity) -> Self {
        VUserLiquidity::Current(c)
    }
}

impl UserLiquidity {

    pub fn is_mining(&self) -> bool {
        !self.mft_id.is_empty() && self.v_liquidity != 0
    }

    /// 
    /// @param acc_fee_x_in_128
    /// @param acc_fee_y_in_128
    pub fn get_unclaimed_fee(&mut self, acc_fee_x_in_128: U256, acc_fee_y_in_128: U256) {
        self.unclaimed_fee_x = Some(
            // In current algorithm, left point fee_out plus right point fee_out may bigger than total fee, 
            // cause a negative value of last_fee_scale(from acc_fee_x_in), so we use overflowed sub to take U256 as I256
            acc_fee_x_in_128.overflowing_sub(self.last_fee_scale_x_128).0
            .mul_fraction_floor(self.amount.into(), pow_128())
            .as_u128().into());
        self.unclaimed_fee_y = Some(
            acc_fee_y_in_128.overflowing_sub(self.last_fee_scale_y_128).0
            .mul_fraction_floor(self.amount.into(), pow_128())
            .as_u128().into());
    }
}

impl Contract {
    pub fn internal_add_liquidity(
        &self,
        pool: &mut Pool,
        user_id: &AccountId,
        lpt_id: LptId,
        left_point: i32,
        right_point: i32,
        amount_x: U128,
        amount_y: U128,
        min_amount_x: U128,
        min_amount_y: U128,
        is_view: bool
    ) -> (u128, u128, UserLiquidity) {
        let (new_liquidity, need_x, need_y, acc_fee_x_in_128, acc_fee_y_in_128) = pool.internal_add_liquidity(left_point, right_point, amount_x.0, amount_y.0, min_amount_x.0, min_amount_y.0, is_view);
        let liquidity = UserLiquidity {
            lpt_id: lpt_id.clone(),
            owner_id: user_id.clone(),
            pool_id: pool.pool_id.clone(),
            left_point,
            right_point,
            last_fee_scale_x_128: acc_fee_x_in_128,
            last_fee_scale_y_128: acc_fee_y_in_128,
            amount: new_liquidity,
            mft_id: String::new(),
            v_liquidity: 0,
            unclaimed_fee_x: None,
            unclaimed_fee_y: None,
        };
        
        pool.total_liquidity += new_liquidity;
        pool.total_x += need_x;
        pool.total_y += need_y;

        if !is_view {
            Event::LiquidityAdded {
                lpt_id: &lpt_id,
                owner_id: &user_id,
                pool_id: &pool.pool_id,
                left_point: &left_point,
                right_point: &right_point,
                added_amount: &U128(new_liquidity),
                cur_amount: &U128(liquidity.amount),
                paid_token_x: &U128(need_x),
                paid_token_y: &U128(need_y),
            }
            .emit();
        }

        (need_x, need_y, liquidity)
    }

    pub fn internal_check_add_liquidity_infos(
        &self, 
        user: &mut User,
        lpt_ids: &mut Vec<LptId>,
        pool_cache: &mut HashMap<String, Pool>,
        inner_id: &mut u128,
        add_liquidity_infos: &Vec<AddLiquidityInfo>,
    ) {
        add_liquidity_infos.iter().for_each(|add_liquidity_info| {
            let pool = self.internal_unwrap_pool(&add_liquidity_info.pool_id);
            require!(add_liquidity_info.left_point % pool.point_delta == 0  && add_liquidity_info.right_point % pool.point_delta == 0, E200_INVALID_ENDPOINT);
            require!(add_liquidity_info.right_point > add_liquidity_info.left_point, E202_ILLEGAL_POINT);
            require!(add_liquidity_info.right_point - add_liquidity_info.left_point < RIGHT_MOST_POINT, E202_ILLEGAL_POINT);
            require!(add_liquidity_info.left_point >= LEFT_MOST_POINT && add_liquidity_info.right_point <= RIGHT_MOST_POINT, E202_ILLEGAL_POINT);
            user.sub_asset(&pool.token_x, add_liquidity_info.amount_x.0);
            user.sub_asset(&pool.token_y, add_liquidity_info.amount_y.0);
            if !pool_cache.contains_key(&add_liquidity_info.pool_id) {
                self.assert_no_frozen_tokens(&[pool.token_x.clone(), pool.token_y.clone()]);
                pool_cache.insert(add_liquidity_info.pool_id.clone(), pool);
            }
            lpt_ids.push(gen_lpt_id(&add_liquidity_info.pool_id, inner_id));
        });
    }

    pub fn internal_check_add_liquidity_infos_by_cache(
        &self, 
        token_cache: &mut TokenCache,
        lpt_ids: &mut Vec<LptId>,
        pool_cache: &mut HashMap<String, Pool>,
        inner_id: &mut u128,
        add_liquidity_infos: &Vec<AddLiquidityInfo>,
    ) {
        add_liquidity_infos.iter().for_each(|add_liquidity_info| {
            let pool = self.internal_unwrap_pool(&add_liquidity_info.pool_id);
            require!(add_liquidity_info.left_point % pool.point_delta == 0  && add_liquidity_info.right_point % pool.point_delta == 0, E200_INVALID_ENDPOINT);
            require!(add_liquidity_info.right_point > add_liquidity_info.left_point, E202_ILLEGAL_POINT);
            require!(add_liquidity_info.right_point - add_liquidity_info.left_point < RIGHT_MOST_POINT, E202_ILLEGAL_POINT);
            require!(add_liquidity_info.left_point >= LEFT_MOST_POINT && add_liquidity_info.right_point <= RIGHT_MOST_POINT, E202_ILLEGAL_POINT);
            token_cache.sub(&pool.token_x, add_liquidity_info.amount_x.0);
            token_cache.sub(&pool.token_y, add_liquidity_info.amount_y.0);
            if !pool_cache.contains_key(&add_liquidity_info.pool_id) {
                self.assert_no_frozen_tokens(&[pool.token_x.clone(), pool.token_y.clone()]);
                pool_cache.insert(add_liquidity_info.pool_id.clone(), pool);
            }
            lpt_ids.push(gen_lpt_id(&add_liquidity_info.pool_id, inner_id));
        });
    }

    pub fn internal_batch_add_liquidity(
        &self, 
        user_id: &AccountId,
        lpt_ids: &Vec<LptId>,
        pool_cache: &mut HashMap<String, Pool>,
        add_liquidity_infos: Vec<AddLiquidityInfo>,
        is_view: bool
    ) -> (Vec<(u128, u128)>, HashMap<AccountId, u128>, Vec<UserLiquidity>) {
        let mut refund_tokens = HashMap::new();
        let mut liquiditys = vec![];
        let mut need_tokens = vec![];
        for (index, add_liquidity_info) in add_liquidity_infos.into_iter().enumerate() {
            let pool = pool_cache.get_mut(&add_liquidity_info.pool_id).unwrap();
            let (need_x, need_y, liquidity) = self.internal_add_liquidity(pool, &user_id, lpt_ids[index].clone(), add_liquidity_info.left_point, add_liquidity_info.right_point, add_liquidity_info.amount_x, add_liquidity_info.amount_y, add_liquidity_info.min_amount_x, add_liquidity_info.min_amount_y, is_view);
            need_tokens.push((need_x, need_y));
            let refund_x = add_liquidity_info.amount_x.0 - need_x;
            let refund_y = add_liquidity_info.amount_y.0 - need_y;
            if refund_x > 0 {
                refund_tokens.entry(pool.token_x.clone()).and_modify(|v| *v += refund_x).or_insert(refund_x);
            }
            if refund_y > 0 {
                refund_tokens.entry(pool.token_y.clone()).and_modify(|v| *v += refund_y).or_insert(refund_y);
            }
            liquiditys.push(liquidity);
        }

        (need_tokens, refund_tokens, liquiditys)
    }

    pub fn internal_check_remove_liquidity_infos(
        &self, 
        user: &mut User,
        liquiditys: &mut Vec<UserLiquidity>,
        pool_cache: &mut HashMap<String, Pool>,
        remove_liquidity_infos: &Vec<RemoveLiquidityInfo>,
    ) -> (HashMap<MftId, u128>, u64) {
        let mut remove_mft_details = HashMap::new();
        let mut duplicate_checking = HashSet::new();
        let mut release_slots = 0;
        remove_liquidity_infos.iter().for_each(|remove_liquidity_info| {
            require!(duplicate_checking.insert(&remove_liquidity_info.lpt_id), E220_LIQUIDITY_DUPLICATE);
            let mut liquidity = self.internal_unwrap_user_liquidity(&remove_liquidity_info.lpt_id);
            if remove_liquidity_info.amount.0 >= liquidity.amount {
                release_slots += 1;
            }
            require!(user.user_id == liquidity.owner_id, E215_NOT_LIQUIDITY_OWNER);
            if remove_liquidity_info.amount.0 > 0 {
                if liquidity.is_mining() {
                    if user.mft_assets.get(&liquidity.mft_id).unwrap_or_default() >= liquidity.v_liquidity {
                        user.sub_mft_asset(&liquidity.mft_id, liquidity.v_liquidity);
                        remove_mft_details.entry(liquidity.mft_id).and_modify(|v| *v += liquidity.v_liquidity).or_insert(liquidity.v_liquidity);
                        liquidity.mft_id = String::new();
                        liquidity.v_liquidity = 0;
                    }else {
                        env::panic_str(E218_USER_LIQUIDITY_IS_MINING);
                    }
                } 
            }
            if !pool_cache.contains_key(&liquidity.pool_id) {
                let pool = self.internal_unwrap_pool(&liquidity.pool_id);
                self.assert_no_frozen_tokens(&[pool.token_x.clone(), pool.token_y.clone()]);
                pool_cache.insert(liquidity.pool_id.clone(), pool);
            }
            liquiditys.push(liquidity);
        });
        (remove_mft_details, release_slots)
    }

    pub fn internal_remove_liquidity(
        &self,
        user_id: &AccountId,
        pool: &mut Pool,
        liquidity: &mut UserLiquidity,
        remove_liquidity: u128,
        min_amount_x: U128,
        min_amount_y: U128,
    ) -> (u128, u128) {
        let (remove_x, remove_y, acc_fee_x_in_128, acc_fee_y_in_128) = pool.internal_remove_liquidity(remove_liquidity, liquidity.left_point, liquidity.right_point, min_amount_x.0, min_amount_y.0);
        liquidity.get_unclaimed_fee(acc_fee_x_in_128, acc_fee_y_in_128);
        liquidity.amount -= remove_liquidity;
        liquidity.last_fee_scale_x_128 = acc_fee_x_in_128;
        liquidity.last_fee_scale_y_128 = acc_fee_y_in_128;
        
        let new_fee_x = liquidity.unclaimed_fee_x.unwrap_or(U128(0)).0;
        let new_fee_y = liquidity.unclaimed_fee_y.unwrap_or(U128(0)).0;

        let refund_x = remove_x + new_fee_x;
        let refund_y = remove_y + new_fee_y;

        pool.total_liquidity -= remove_liquidity;
        pool.total_x -= refund_x;
        pool.total_y -= refund_y;

        Event::LiquidityRemoved {
            lpt_id: &liquidity.lpt_id,
            owner_id: &user_id,
            pool_id: &liquidity.pool_id,
            left_point: &liquidity.left_point,
            right_point: &liquidity.right_point,
            removed_amount: &U128(remove_liquidity),
            cur_amount: &U128(liquidity.amount),
            refund_token_x: &U128(refund_x),
            refund_token_y: &U128(refund_y),
            claim_fee_token_x: &U128(new_fee_x),
            claim_fee_token_y: &U128(new_fee_y),
        }
        .emit();

        (refund_x, refund_y)
    }

    pub fn internal_batch_remove_liquidity(
        &self, 
        user_id: &AccountId,
        pool_cache: &mut HashMap<String, Pool>,
        liquiditys: &mut Vec<UserLiquidity>,
        remove_liquidity_infos: Vec<RemoveLiquidityInfo>
    ) -> HashMap<AccountId, u128> {
        let mut refund_tokens = HashMap::new();
        for (index, remove_liquidity_info) in remove_liquidity_infos.into_iter().enumerate() {
            let liquidity = liquiditys.get_mut(index).unwrap();
            let pool = pool_cache.get_mut(&liquidity.pool_id).unwrap();

            let remove_liquidity = if remove_liquidity_info.amount.0 < liquidity.amount { remove_liquidity_info.amount.0 } else { liquidity.amount };
            let (refund_x, refund_y) = self.internal_remove_liquidity(user_id, pool, liquidity, remove_liquidity, remove_liquidity_info.min_amount_x, remove_liquidity_info.min_amount_y);
            if refund_x > 0 {
                refund_tokens.entry(pool.token_x.clone()).and_modify(|v| *v += refund_x).or_insert(refund_x);
            }
            if refund_y > 0 {
                refund_tokens.entry(pool.token_y.clone()).and_modify(|v| *v += refund_y).or_insert(refund_y);
            }
        }
        
        refund_tokens
    }
}


impl Contract {
    pub fn internal_mint_liquiditys(&mut self, mut user: User, liquiditys: Vec<UserLiquidity>) {
        for liquidity in liquiditys {
            user.liquidity_keys.insert(&liquidity.lpt_id);
            if self.data_mut().user_liquidities.insert(&liquidity.lpt_id.clone(), &liquidity.into()).is_none() {
                self.data_mut().liquidity_count += 1;
            }
        }
        self.internal_set_user(&user.user_id.clone(), user);   
    }

    pub fn internal_update_or_burn_liquiditys(&mut self, user: &mut User, liquiditys: Vec<UserLiquidity>) {
        for liquidity in liquiditys.into_iter() {
            let lpt_id = liquidity.lpt_id.clone();
            if liquidity.amount > 0 {
                self.internal_set_user_liquidity(&lpt_id, liquidity);
            } else {
                if self.data_mut().user_liquidities.remove(&lpt_id).is_some() {
                    self.data_mut().liquidity_count -= 1;
                }
                user.liquidity_keys.remove(&lpt_id);
            }
        }
    }

    pub fn internal_burn_liquidity(&mut self, mut user: User, liquidity: &UserLiquidity) {
        if self.data_mut().user_liquidities.remove(&liquidity.lpt_id).is_some() {
            self.data_mut().liquidity_count -= 1;
        }
        user.liquidity_keys.remove(&liquidity.lpt_id);
        self.internal_set_user(&user.user_id.clone(), user);   
    }
}

impl Contract {
    pub fn internal_mint_v_liquidity(&mut self, user: &mut User, lpt_id: LptId, dcl_farming_type: DclFarmingType) {
        let mut user_liquidity = self.internal_unwrap_user_liquidity(&lpt_id);
        require!(user_liquidity.owner_id == user.user_id, E500_NOT_NFT_OWNER);
        require!(!user_liquidity.is_mining(), E218_USER_LIQUIDITY_IS_MINING);

        let v_liquidity = match dcl_farming_type {
            DclFarmingType::FixRange { left_point, right_point } => {
                self.calc_fix_range_v_liquidity(left_point, right_point, &user_liquidity)
            }
        };
        require!(v_liquidity > 0, E705_INVALID_V_LIQUIDITY);

        let pool_id = parse_pool_id_from_lpt_id(&lpt_id);
        let mft_id = gen_mft_id(&pool_id, &dcl_farming_type);

        user.add_mft_asset(&mft_id, v_liquidity);
        self.internal_increase_mft_supply(&mft_id, v_liquidity);

        user_liquidity.mft_id = mft_id;
        user_liquidity.v_liquidity = v_liquidity;
        
        self.internal_set_user_liquidity(&lpt_id, user_liquidity);
    }

    pub fn internal_burn_v_liquidity(&mut self, user: &mut User, lpt_id: LptId) {
        let mut user_liquidity = self.internal_unwrap_user_liquidity(&lpt_id);
        require!(user_liquidity.owner_id == user.user_id, E500_NOT_NFT_OWNER);
        require!(user_liquidity.is_mining(), E219_USER_LIQUIDITY_IS_NOT_MINING);

        user.sub_mft_asset(&user_liquidity.mft_id, user_liquidity.v_liquidity);
        self.internal_decrease_mft_supply(&user_liquidity.mft_id, user_liquidity.v_liquidity);
        
        user_liquidity.mft_id = String::new();
        user_liquidity.v_liquidity = 0;

        self.internal_set_user_liquidity(&lpt_id, user_liquidity);
    }
}

impl Contract {
    pub fn internal_get_user_liquidity(&self, lpt_id: &LptId) -> Option<UserLiquidity> {
        self.data().user_liquidities.get(lpt_id).map(|o| o.into())
    }

    pub fn internal_unwrap_user_liquidity(&self, lpt_id: &LptId) -> UserLiquidity {
        self.internal_get_user_liquidity(lpt_id)
            .expect(E207_LIQUIDITY_NOT_FOUND)
    }

    pub fn internal_set_user_liquidity(&mut self, lpt_id: &LptId, user_liquidity: UserLiquidity) {
        self.data_mut().user_liquidities.insert(&lpt_id, &user_liquidity.into());
    }
}