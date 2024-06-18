use crate::*;

impl User {
    pub fn add_mft_asset(&mut self, mft_id: &MftId, amount: Balance) {
        self.mft_assets.insert(
            mft_id,
            &(amount + self.mft_assets.get(mft_id).unwrap_or(0_u128)).clone(),
        );
    }

    pub fn sub_mft_asset(&mut self, mft_id: &MftId, amount: Balance) {
        if amount != 0 {
            if let Some(prev) = self.mft_assets.remove(mft_id) {
                require!(amount <= prev, E101_INSUFFICIENT_BALANCE);
                let remain = prev - amount;
                if remain > 0 {
                    self.mft_assets.insert(mft_id, &remain);
                }
            } else {
                env::panic_str(E101_INSUFFICIENT_BALANCE);
            }
        }
    }
}

impl Contract {
    pub fn internal_mft_balance(&self, token_id: String, account_id: &AccountId) -> Balance {
        self.get_user_mft_asset(account_id.clone(), token_id).0
    }

    pub fn internal_mft_transfer(
        &mut self,
        token_id: String,
        sender_id: &AccountId,
        receiver_id: &AccountId,
        amount: Option<u128>,
        memo: Option<String>,
    ) -> u128 {
        require!(sender_id != receiver_id, E706_TRANSFER_TO_SELF);
        let mut sender = self.internal_unwrap_user(sender_id);
        let mut receiver = self.internal_unwrap_user(receiver_id);

        let amount = amount.unwrap_or(sender.mft_assets.get(&token_id).expect(E101_INSUFFICIENT_BALANCE));

        sender.sub_mft_asset(&token_id, amount);
        receiver.add_mft_asset(&token_id, amount);
        
        self.internal_set_user(sender_id, sender);
        self.internal_set_user(receiver_id, receiver);

        if let Some(memo) = memo {
            log!("Memo: {}", memo);
        }
        amount
    }

    pub fn internal_increase_mft_supply(&mut self, mft_id: &MftId, amount: Balance) {
        let (total_amount, overflowing) = self.data().mft_supply.get(mft_id).unwrap_or_default().overflowing_add(amount);
        require!(!overflowing, E702_MFT_SUPPLY_OVERFLOWING);
        self.data_mut().mft_supply.insert(mft_id, &total_amount);
    }

    pub fn internal_decrease_mft_supply(&mut self, mft_id: &MftId, amount: Balance) {
        if amount != 0 {
            if let Some(prev) = self.data_mut().mft_supply.remove(mft_id) {
                require!(amount <= prev, E101_INSUFFICIENT_BALANCE);
                let remain = prev - amount;
                if remain > 0 {
                    self.data_mut().mft_supply.insert(mft_id, &remain);
                }
            } else {
                env::panic_str(E101_INSUFFICIENT_BALANCE);
            }
        }
    }

    pub fn calc_fix_range_v_liquidity(&self, left_point: i32, right_point: i32, user_liquidity: &UserLiquidity) -> u128 {
        require!(user_liquidity.amount >= 10u128.pow(6), E701_LIQUIDITY_TOO_SMALL);
        let valid_range = {
            U256::from(
                std::cmp::max(
                    std::cmp::min(right_point, user_liquidity.right_point) - std::cmp::max(left_point, user_liquidity.left_point),
                    0
                )
            )
        };
        (valid_range * valid_range * U256::from(user_liquidity.amount) / 10u128.pow(6)).as_u128() 
    }
}
