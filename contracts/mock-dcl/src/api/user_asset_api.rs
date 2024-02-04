use crate::*;

#[near_bindgen]
impl Contract {
    pub fn get_user_asset(&self, account_id: AccountId, token_id: AccountId) -> U128 {
        self.internal_get_user(&account_id)
            .and_then(|user| user.assets.get(&token_id))
            .unwrap_or(0_u128)
            .into()
    }

    pub fn list_user_assets(
        &self,
        account_id: AccountId,
        from_index: Option<u64>,
        limit: Option<u64>,
    ) -> HashMap<AccountId, U128> {
        if let Some(user) = self.internal_get_user(&account_id) {
            let keys = user.assets.keys_as_vector();

            let from_index = from_index.unwrap_or(0);
            let limit = limit.unwrap_or(keys.len());

            (from_index..std::cmp::min(from_index + limit, keys.len()))
                .map(|index| {
                    (
                        keys.get(index).unwrap(),
                        user.assets.get(&keys.get(index).unwrap()).unwrap().into(),
                    )
                })
                .collect()
        } else {
            HashMap::new()
        }
    }

    /// Withdraws given token of given user.
    /// when amount is None, withdraw all balance of the token.
    pub fn withdraw_asset(
        &mut self,
        token_id: AccountId,
        amount: Option<U128>,
        skip_unwrap_near: Option<bool>
    ) -> PromiseOrValue<bool> {
        self.assert_contract_running();
        self.assert_no_frozen_tokens(&[token_id.clone()]);

        let user_id = env::predecessor_account_id();
        let mut user = self.internal_unwrap_user(&user_id);

        let total = user.assets.get(&token_id).unwrap_or(0_u128);
        let amount: u128 = amount.map(|v| v.into()).unwrap_or(total);

        if amount > 0 {
            // Note: subtraction, will be reverted if the promise fails.
            user.sub_asset(&token_id, amount);
            self.internal_set_user(&user_id, user);

            self.process_transfer(&user_id, &token_id, amount, skip_unwrap_near).into()
        } else {
            PromiseOrValue::Value(true)
        }
    }

    #[private]
    pub fn callback_post_withdraw_asset(
        &mut self,
        token_id: AccountId,
        user_id: AccountId,
        amount: U128,
    ) -> bool {
        require!(
            env::promise_results_count() == 1,
            E001_PROMISE_RESULT_COUNT_INVALID
        );
        let amount: Balance = amount.into();
        match env::promise_result(0) {
            PromiseResult::NotReady => unreachable!(),
            PromiseResult::Successful(_) => {
                true
            }
            PromiseResult::Failed => {
                // This reverts the changes from withdraw function.
                if let Some(mut user) = self.internal_get_user(&user_id) {
                    user.add_asset(&token_id, amount);
                    self.internal_set_user(&user_id, user);

                    Event::Lostfound {
                        user: &user_id,
                        token: &token_id,
                        amount: &U128(amount),
                        locked: &false,
                    }
                    .emit();
                } else {
                    Event::Lostfound {
                        user: &user_id,
                        token: &token_id,
                        amount: &U128(amount),
                        locked: &true,
                    }
                    .emit();
                }
                false
            }
        }
    }

    #[private]
    pub fn callback_post_withdraw_near(
        &mut self,
        user_id: AccountId,
        amount: U128,
    ) -> bool {
        require!(
            env::promise_results_count() == 1,
            E001_PROMISE_RESULT_COUNT_INVALID
        );
        let amount: Balance = amount.into();
        match env::promise_result(0) {
            PromiseResult::NotReady => unreachable!(),
            PromiseResult::Successful(_) => {
                Promise::new(user_id).transfer(amount);
                true
            }
            PromiseResult::Failed => {
                // This reverts the changes from withdraw function.
                let wnear_id = self.internal_get_global_config().wnear_id;
                if let Some(mut user) = self.internal_get_user(&user_id) {
                    user.add_asset(&wnear_id, amount);
                    self.internal_set_user(&user_id, user);

                    Event::Lostfound {
                        user: &user_id,
                        token: &wnear_id,
                        amount: &U128(amount),
                        locked: &false,
                    }
                    .emit();
                } else {
                    Event::Lostfound {
                        user: &user_id,
                        token: &wnear_id,
                        amount: &U128(amount),
                        locked: &true,
                    }
                    .emit();
                }
                false
            }
        }
    }
}
