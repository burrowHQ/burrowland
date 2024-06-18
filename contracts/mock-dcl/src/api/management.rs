use crate::*;

#[near_bindgen]
impl Contract {

    #[payable]
    pub fn pause_pool(&mut self, pool_id: PoolId) -> bool {
        assert_one_yocto();
        require!(self.is_owner_or_operators(), E002_NOT_ALLOWED);
        self.assert_contract_running();

        let mut pool = self.internal_unwrap_pool(&pool_id);
        if pool.state == RunningState::Running {
            pool.state = RunningState::Paused;
            self.internal_set_pool(&pool_id, pool);
            true
        } else {
            false
        }
    }

    #[payable]
    pub fn resume_pool(&mut self, pool_id: PoolId) -> bool {
        assert_one_yocto();
        require!(self.is_owner_or_operators(), E002_NOT_ALLOWED);
        self.assert_contract_running();

        let mut pool = self.internal_unwrap_pool(&pool_id);
        if pool.state == RunningState::Paused {
            pool.state = RunningState::Running;
            self.internal_set_pool(&pool_id, pool);
            true
        } else {
            false
        }
    }

    #[payable]
    pub fn extend_frozenlist_tokens(&mut self, tokens: Vec<AccountId>) {
        assert_one_yocto();
        require!(self.is_owner_or_operators(), E002_NOT_ALLOWED);
        for token in tokens {
            self.data_mut().frozenlist.insert(&token);
        }
    }

    #[payable]
    pub fn remove_frozenlist_tokens(&mut self, tokens: Vec<AccountId>) {
        assert_one_yocto();
        require!(self.is_owner_or_operators(), E002_NOT_ALLOWED);
        for token in tokens {
            let exist = self.data_mut().frozenlist.remove(&token);
            require!(exist, E009_INVALID_FROZEN_TOKEN);
        }
    }

    #[payable]
    pub fn set_vip_user(&mut self, user: AccountId, discount: HashMap<PoolId, u32>) {
        assert_one_yocto();
        require!(self.is_owner_or_operators(), E002_NOT_ALLOWED);
        require!(!discount.is_empty() && 
            discount.iter().all(|(_, &v)| v as u128 <= BP_DENOM), E011_INVALID_VIP_USER_DISCOUNT);
        self.data_mut().vip_users.insert(&user, &discount);
    }

    #[payable]
    pub fn remove_vip_user(&mut self, user: AccountId) {
        assert_one_yocto();
        require!(self.is_owner_or_operators(), E002_NOT_ALLOWED);
        self.data_mut().vip_users.remove(&user);
    }

    #[payable]
    pub fn modify_protocol_fee_rate(&mut self, protocol_fee_rate: u32) {
        assert_one_yocto();
        require!(self.is_owner_or_operators(), E002_NOT_ALLOWED);
        self.assert_contract_running();
        require!(protocol_fee_rate as u128 <= BP_DENOM, E007_INVALID_PROTOCOL_FEE_RATE);
        
        self.data_mut().protocol_fee_rate = protocol_fee_rate;
    }

    #[payable]
    pub fn modify_storage_params(&mut self, storage_price_per_slot: Option<U128>, storage_for_asset: Option<U128>) {
        assert_one_yocto();
        require!(self.is_owner_or_operators(), E002_NOT_ALLOWED);
        self.assert_contract_running();

        let mut global_config = self.internal_get_global_config();
        if let Some(v) = storage_price_per_slot {
            global_config.storage_price_per_slot = v.0;
        }
        if let Some(v) = storage_for_asset {
            global_config.storage_for_asset = v.0;
        }
        self.internal_set_global_config(global_config);
    }

    #[payable]
    pub fn claim_charged_fee(&mut self, pool_id: PoolId, amount_x: U128, amount_y: U128) {
        assert_one_yocto();
        require!(self.is_owner_or_operators(), E002_NOT_ALLOWED);
        self.assert_contract_running();
        let mut pool = self.internal_unwrap_pool(&pool_id);

        let owner_id = self.internal_get_global_config().owner_id.clone();
        let mut user = self.internal_unwrap_user(&owner_id);
        
        let mut amount_x: Balance = amount_x.into();
        let mut amount_y: Balance = amount_y.into();

        if amount_x == 0 {
            amount_x = pool.total_fee_x_charged;
        }
        require!(amount_x <= pool.total_fee_x_charged, E101_INSUFFICIENT_BALANCE);
        if amount_y == 0 {
            amount_y = pool.total_fee_y_charged;
        }
        require!(amount_y <= pool.total_fee_y_charged, E101_INSUFFICIENT_BALANCE);

        pool.total_fee_x_charged -= amount_x;
        pool.total_fee_y_charged -= amount_y;
        pool.total_x -= amount_x;
        pool.total_y -= amount_y;
        
        user.add_asset(&pool.token_x, amount_x);
        user.add_asset(&pool.token_y, amount_y);

        self.internal_set_user(&owner_id, user);
        self.internal_set_pool(&pool_id, pool);

        Event::ClaimChargedFee { 
            user: &owner_id, 
            pool_id: &pool_id, 
            amount_x: &U128(amount_x), 
            amount_y: &U128(amount_y)
        }.emit();
    }
}