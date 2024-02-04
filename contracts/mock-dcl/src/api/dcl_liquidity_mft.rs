use crate::*;
use near_contract_standards::fungible_token::metadata::FungibleTokenMetadata;

pub const GAS_FOR_RESOLVE_TRANSFER: Gas = Gas(20 * TGAS);
pub const GAS_FOR_FT_TRANSFER_CALL: Gas = Gas(25 * TGAS);

#[ext_contract(ext_share_token_receiver)]
pub trait MFTTokenReceiver {
    fn mft_on_transfer(
        &mut self,
        token_id: String,
        sender_id: AccountId,
        amount: U128,
        msg: String,
    ) -> PromiseOrValue<U128>;
}

#[near_bindgen]
impl Contract {
    pub fn mft_balance_of(&self, token_id: MftId, account_id: AccountId) -> U128 {
        self.internal_mft_balance(token_id, &account_id).into()
    }

    pub fn mft_total_supply(&self, token_id: MftId) -> U128 {
        self.data().mft_supply.get(&token_id).unwrap_or_default().into()
    }

    #[payable]
    pub fn mft_transfer(
        &mut self,
        token_id: String,
        receiver_id: AccountId,
        amount: U128,
        memo: Option<String>,
    ) {
        assert_one_yocto();
        self.assert_contract_running();
        let sender_id = env::predecessor_account_id();
        require!(sender_id == self.data().farming_contract_id 
            || self.data().farming_contract_id_history.contains(&sender_id), E703_SENDER_NOT_FARMING_ACCOUNTID);
        self.internal_mft_transfer(
            token_id,
            &sender_id,
            &receiver_id,
            Some(amount.0),
            memo,
        );
    }

    #[payable]
    pub fn mft_transfer_call(
        &mut self,
        token_id: String,
        receiver_id: AccountId,
        amount: U128,
        memo: Option<String>,
        msg: String,
    ) -> PromiseOrValue<U128> {
        assert_one_yocto();
        self.assert_contract_running();
        require!(receiver_id == self.data().farming_contract_id, E704_RECEIVER_NOT_FARMING_ACCOUNTID);
        let sender_id = env::predecessor_account_id();
        self.internal_mft_transfer(
            token_id.clone(),
            &sender_id,
            &receiver_id,
            Some(amount.0),
            memo,
        );

        ext_share_token_receiver::ext(receiver_id.clone())
            .with_static_gas(GAS_FOR_FT_TRANSFER_CALL)
            .mft_on_transfer(token_id.clone(),
                sender_id.clone(),
                amount,
                msg)
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_FOR_RESOLVE_TRANSFER)
                    .mft_resolve_transfer(
                        token_id,
                        sender_id,
                        &receiver_id,
                        amount,
                    )
            ).into()
    }

    #[payable]
    pub fn mft_transfer_all_call(
        &mut self,
        token_id: String,
        receiver_id: AccountId,
        memo: Option<String>,
        msg: String,
    ) -> PromiseOrValue<U128> {
        assert_one_yocto();
        self.assert_contract_running();
        require!(receiver_id == self.data().farming_contract_id, E704_RECEIVER_NOT_FARMING_ACCOUNTID);
        let sender_id = env::predecessor_account_id();
        let transfer_amount = self.internal_mft_transfer(
            token_id.clone(),
            &sender_id,
            &receiver_id,
            None,
            memo,
        );

        ext_share_token_receiver::ext(receiver_id.clone())
            .with_static_gas(GAS_FOR_FT_TRANSFER_CALL)
            .mft_on_transfer(token_id.clone(),
                sender_id.clone(),
                U128(transfer_amount),
                msg)
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_FOR_RESOLVE_TRANSFER)
                    .mft_resolve_transfer(
                        token_id,
                        sender_id,
                        &receiver_id,
                        U128(transfer_amount),
                    )
            ).into()
    }

    #[private]
    pub fn mft_resolve_transfer(
        &mut self,
        token_id: String,
        sender_id: AccountId,
        receiver_id: &AccountId,
        amount: U128,
    ) -> U128 {
        let unused_amount = match env::promise_result(0) {
            PromiseResult::NotReady => unreachable!(),
            PromiseResult::Successful(value) => {
                if let Ok(unused_amount) = near_sdk::serde_json::from_slice::<U128>(&value) {
                    std::cmp::min(amount.0, unused_amount.0)
                } else {
                    amount.0
                }
            }
            PromiseResult::Failed => amount.0,
        };
        if unused_amount > 0 {
            let receiver_balance = self.internal_mft_balance(token_id.clone(), &receiver_id);
            if receiver_balance > 0 {
                let refund_amount = std::cmp::min(receiver_balance, unused_amount);
                
                let refund_to = if self.internal_get_user(&sender_id).is_some() {
                    sender_id
                } else {
                    // The program will not enter this branch
                    // The fact that the user can access this function means that the user has at least one nft, which means that the user must exist
                    self.internal_get_global_config().owner_id.clone()
                };
                self.internal_mft_transfer(token_id, &receiver_id, &refund_to, Some(refund_amount), None);
            }
        }
        U128(unused_amount)
    }

    pub fn mft_metadata(&self, token_id: String) -> FungibleTokenMetadata {
        FungibleTokenMetadata {
            spec: "mft-1.0.0".to_string(),
            name: format!("dcl-pool-{}", token_id),
            symbol: format!("DCL-POOL-{}", token_id),
            icon: None,
            reference: None,
            reference_hash: None,
            decimals: 6,
        }
    }
}

#[near_bindgen]
impl Contract {
    pub fn get_user_mft_asset(&self, account_id: AccountId, mft_id: MftId) -> U128 {
        self.internal_get_user(&account_id)
            .and_then(|user| user.mft_assets.get(&mft_id))
            .unwrap_or(0_u128)
            .into()
    }

    pub fn list_user_mft_assets(
        &self,
        account_id: AccountId,
        from_index: Option<u64>,
        limit: Option<u64>,
    ) -> HashMap<MftId, U128> {
        if let Some(user) = self.internal_get_user(&account_id) {
            let keys = user.mft_assets.keys_as_vector();

            let from_index = from_index.unwrap_or(0);
            let limit = limit.unwrap_or(keys.len());

            (from_index..std::cmp::min(from_index + limit, keys.len()))
                .map(|index| {
                    (
                        keys.get(index).unwrap(),
                        user.mft_assets.get(&keys.get(index).unwrap()).unwrap().into(),
                    )
                })
                .collect()
        } else {
            HashMap::new()
        }
    }
}