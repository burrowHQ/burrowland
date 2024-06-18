use crate::*;

impl User {
    pub fn add_asset(&mut self, token_id: &AccountId, amount: Balance) {
        self.assets.insert(
            token_id,
            &(amount + self.assets.get(token_id).unwrap_or(0_u128)).clone(),
        );
    }

    pub fn sub_asset(&mut self, token_id: &AccountId, amount: Balance) {
        if amount != 0 {
            if let Some(prev) = self.assets.remove(token_id) {
                require!(amount <= prev, E101_INSUFFICIENT_BALANCE);
                let remain = prev - amount;
                if remain > 0 {
                    self.assets.insert(token_id, &remain);
                }
            } else {
                env::panic_str(E101_INSUFFICIENT_BALANCE);
            }
        }
    }
}

impl Contract {
    pub fn deposit_asset(&mut self, user_id: &AccountId, token_id: &AccountId, amount: Balance) {
        self.assert_no_frozen_tokens(&[token_id.clone()]);
        let mut user = self.internal_unwrap_user(user_id);
        user.add_asset(token_id, amount);
        self.internal_set_user(user_id, user);
    }

    pub fn process_ft_transfer(&mut self, user_id: &AccountId, token_id: &AccountId, amount: Balance) -> Promise {
        ext_fungible_token::ext(token_id.clone())
            .with_attached_deposit(1)
            .with_static_gas(GAS_FOR_ASSET_TRANSFER)
            .ft_transfer(user_id.clone(), amount.into(), None)
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_FOR_RESOLVE_ASSET_TRANSFER)
                    .callback_post_withdraw_asset(
                        token_id.clone(),
                        user_id.clone(),
                        amount.into(),
                    ),
            )
    }

    pub fn process_ft_transfer_call(&mut self, user_id: &AccountId, token_id: &AccountId, amount: Balance, msg: String) -> Promise {
        ext_fungible_token::ext(token_id.clone())
            .with_attached_deposit(1)
            .with_static_gas(GAS_FOR_ASSET_TRANSFER)
            .ft_transfer_call(user_id.clone(), amount.into(), None, msg)
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_FOR_RESOLVE_ASSET_TRANSFER)
                    .callback_post_withdraw_asset(
                        token_id.clone(),
                        user_id.clone(),
                        amount.into(),
                    ),
            )
    }

    pub fn process_near_transfer(&mut self, user_id: &AccountId, wnear_id: AccountId, amount: Balance) -> Promise {
        ext_wrap_near::ext(wnear_id)
            .with_attached_deposit(1)
            .with_static_gas(GAS_FOR_NEAR_WITHDRAW)
            .near_withdraw(amount.into())
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_FOR_RESOLVE_NEAR_WITHDRAW)
                    .callback_post_withdraw_near(
                        user_id.clone(),
                        amount.into(),
                    ),
            )
    }

    pub fn process_transfer(&mut self, user_id: &AccountId, token_id: &AccountId, amount: Balance, skip_unwrap_near: Option<bool>) -> Promise {
        let wnear_id = self.internal_get_global_config().wnear_id.clone();
        if token_id == &wnear_id && !skip_unwrap_near.unwrap_or_default() {
            self.process_near_transfer(&user_id, wnear_id, amount)
        } else {
            self.process_ft_transfer(&user_id, token_id, amount)
        }
    }
}
