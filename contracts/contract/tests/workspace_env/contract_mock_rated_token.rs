use crate::*;

pub struct RatedTokenContract(pub Contract);

impl RatedTokenContract {

    pub async fn set_price(
        &self,
        caller: &Account,
        price: U128
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "set_price")
            .args_json(json!({
                "price": price
            }))
            .max_gas()
            .transact()
            .await
    }

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
            .gas(20_000_000_000_000)
            .deposit(0)
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
            .gas(20_000_000_000_000)
            .deposit(1)
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
            .gas(300_000_000_000_000)
            .deposit(1)
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
            .gas(20_000_000_000_000)
            .deposit(near_sdk::env::storage_byte_cost() * 125)
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
            .gas(20_000_000_000_000)
            .deposit(1)
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

    pub async fn ft_price(
        &self
    ) -> Result<U128> {
        self.0
            .call("ft_price")
            .view()
            .await?
            .json::<U128>()
    }

    pub async fn get_st_near_price(
        &self
    ) -> Result<U128> {
        self.0
            .call("get_st_near_price")
            .view()
            .await?
            .json::<U128>()
    }

    pub async fn get_nearx_price(
        &self
    ) -> Result<U128> {
        self.0
            .call("get_nearx_price")
            .view()
            .await?
            .json::<U128>()
    }
}