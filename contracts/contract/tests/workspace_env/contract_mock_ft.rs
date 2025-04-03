use crate::*;

pub struct FtContract(pub Contract);

impl FtContract {
    pub async fn ft_mint(
        &self,
        caller: &Account,
        receiver: &Account,
        amount: u128,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "mint")
            .args_json(json!({
                "account_id": receiver.id(),
                "amount": U128::from(amount),
            }))
            .gas(Gas::from_tgas(20))
            .transact()
            .await
    }
    
    pub async fn ft_transfer(
        &self,
        sender: &Account,
        receiver: &Account,
        amount: u128,
    ) -> Result<ExecutionFinalResult> {
        sender
            .call(self.0.id(), "ft_transfer")
            .args_json(json!({
                "receiver_id": receiver.id(),
                "amount": U128::from(amount),
                "memo": Option::<String>::None,
            }))
            .gas(Gas::from_tgas(20))
            .deposit(NearToken::from_near(1))
            .transact()
            .await
    }
    
    pub async fn ft_transfer_call(
        &self,
        sender: &Account,
        receiver_id: &AccountId,
        amount: u128,
        msg: String,
    ) -> Result<ExecutionFinalResult> {
        sender
            .call(self.0.id(), "ft_transfer_call")
            .args_json(json!({
                "receiver_id": receiver_id,
                "amount": U128::from(amount),
                "memo": Option::<String>::None,
                "msg": msg.clone(),
            }))
            .max_gas()
            .deposit(NearToken::from_yoctonear(1))
            .transact()
            .await
    }
    
    pub async fn ft_balance_of(
        &self,
        user: &Account,
    ) -> Result<U128> {
        self.0
            .call("ft_balance_of")
            .args_json(json!({
                "account_id": user.id()
            }))
            .view()
            .await?
            .json::<U128>()
    }
    
    pub async fn ft_storage_deposit(
        &self,
        account_id: &AccountId,
    ) -> Result<ExecutionFinalResult> {
        self.0
            .call("storage_deposit")
            .args_json(json!({
                "account_id": Some(account_id),
                "registration_only": Option::<bool>::None,
            }))
            .gas(Gas::from_tgas(20))
            .deposit(NearToken::from_yoctonear(near_sdk::env::storage_byte_cost() * 125))
            .transact()
            .await
    }
    
    pub async fn ft_storage_unregister(
        &self,
        account: &Account,
    ) -> Result<ExecutionFinalResult> {
        account
            .call(self.0.id(), "storage_unregister")
            .args_json(json!({
                "force": Some(true),
            }))
            .gas(Gas::from_tgas(20))
            .deposit(NearToken::from_yoctonear(1))
            .transact()
            .await
    }
    
    pub async fn get_storage_balance_of(
        &self,
        user: &Account,
    ) -> Result<Option<StorageBalance>> {
        self.0
            .call("storage_balance_of")
            .args_json(json!({
                "account_id": user.id()
            }))
            .view()
            .await?
            .json::<Option<StorageBalance>>()
    }
}