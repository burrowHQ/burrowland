
use mock_ref_exchange::{ContractMetadata, StablePoolInfo, ShadowRecordInfo, RefStorageState};

use crate::*;

pub struct RefExchange(pub Contract);

impl RefExchange {
    pub async fn storage_deposit(
        &self,
        account: &Account,
    ) -> Result<ExecutionFinalResult> {
        self.0
            .call("storage_deposit")
            .args_json(json!({
                "account_id": Some(account.id()),
                "registration_only": Option::<bool>::None,
            }))
            .gas(20_000_000_000_000)
            .deposit(parse_near!("1 N"))
            .transact()
            .await
    }

    pub async fn mft_transfer_call(
        &self,
        caller: &Account,
        token_id: String,
        receiver_id: &AccountId,
        amount: U128,
        msg: String
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "mft_transfer_call")
            .args_json(json!({
                "token_id": token_id,
                "receiver_id": receiver_id,
                "amount": amount,
                "msg": msg,
            }))
            .gas(300_000_000_000_000)
            .deposit(1)
            .transact()
            .await
    }

    pub async fn deposit(
        &self,
        token_contract: &FtContract,
        caller: &Account,
        amount: u128,
    ) -> Result<ExecutionFinalResult> {
        token_contract.ft_transfer_call(caller, self.0.id(), amount, "".to_string()).await
    }

    pub async fn extend_whitelisted_tokens(
        &self,
        caller: &Account,
        tokens: Vec<&AccountId>
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "extend_whitelisted_tokens")
            .args_json(json!({
                "tokens": tokens,
            }))
            .max_gas()
            .deposit(1)
            .transact()
            .await
    }

    pub async fn mft_register(
        &self,
        caller: &Account,
        token_id: String, 
        account_id: &AccountId
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "mft_register")
            .args_json(json!({
                "token_id": token_id, 
                "account_id": account_id
            }))
            .max_gas()
            .deposit(parse_near!("1 N"))
            .transact()
            .await
    }

    pub async fn add_stable_swap_pool(
        &self,
        caller: &Account,
        tokens: Vec<&AccountId>,
        decimals: Vec<u8>,
        fee: u32,
        amp_factor: u64,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "add_stable_swap_pool")
            .args_json(json!({
                "tokens": tokens,
                "decimals": decimals,
                "fee": fee,
                "amp_factor": amp_factor,
            }))
            .max_gas()
            .deposit(parse_near!("0.01 N"))
            .transact()
            .await
    }

    pub async fn add_stable_liquidity(
        &self,
        caller: &Account,
        pool_id: u64,
        amounts: Vec<U128>,
        min_shares: U128,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "add_stable_liquidity")
            .args_json(json!({
                "pool_id": pool_id,
                "amounts": amounts,
                "min_shares": min_shares,
            }))
            .max_gas()
            .deposit(parse_near!("0.01 N"))
            .transact()
            .await
    }

    pub async fn shadow_farming(
        &self,
        caller: &Account,
        pool_id: u64, 
        amount: Option<U128>,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "shadow_farming")
            .args_json(json!({
                "pool_id": pool_id,
                "amount": amount,
            }))
            .deposit(d(1, 23))
            .max_gas()
            .transact()
            .await
    }

    pub async fn shadow_cancel_farming(
        &self,
        caller: &Account,
        pool_id: u64, 
        amount: Option<U128>
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "shadow_cancel_farming")
            .args_json(json!({
                "pool_id": pool_id,
                "amount": amount,
            }))
            .max_gas()
            .transact()
            .await
    }

    pub async fn shadow_burrowland_deposit(
        &self,
        caller: &Account,
        pool_id: u64, 
        amount: Option<U128>,
        after_deposit_actions_msg: Option<String>
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "shadow_burrowland_deposit")
            .args_json(json!({
                "pool_id": pool_id,
                "amount": amount,
                "after_deposit_actions_msg": after_deposit_actions_msg
            }))
            .max_gas()
            .transact()
            .await
    }

    pub async fn shadow_burrowland_withdraw(
        &self,
        caller: &Account,
        pool_id: u64, 
        amount: Option<U128>,
        after_deposit_actions_msg: Option<String>
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "shadow_burrowland_withdraw")
            .args_json(json!({
                "pool_id": pool_id,
                "amount": amount,
                "before_withdraw_actions_msg": after_deposit_actions_msg
            }))
            .max_gas()
            .transact()
            .await
    }
    
}

impl RefExchange {
    pub async fn metadata(
        &self,
    ) -> Result<ContractMetadata> {
        self.0
            .call("metadata")
            .view()
            .await?
            .json::<ContractMetadata>()
    }
    
    pub async fn get_deposits(
        &self,
        account: &Account
    ) -> Result<HashMap<AccountId, U128>> {
        self.0
            .call("get_deposits")
            .args_json(json!({
                "account_id": account.id()
            }))
            .view()
            .await?
            .json::<HashMap<AccountId, U128>>()
    }

    pub async fn get_pool_shares(
        &self,
        pool_id: u64, 
        account: &Account
    ) -> Result<U128> {
        self.0
            .call("get_pool_shares")
            .args_json(json!({
                "pool_id": pool_id, 
                "account_id": account.id()
            }))
            .view()
            .await?
            .json::<U128>()
    }

    pub async fn get_stable_pool(
        &self,
        pool_id: u64,
    ) -> Result<StablePoolInfo> {
        self.0
            .call("get_stable_pool")
            .args_json(json!({
                "pool_id": pool_id
            }))
            .view()
            .await?
            .json::<StablePoolInfo>()
    }

    pub async fn get_shadow_records(
        &self,
        account: &Account,
    ) -> Result<HashMap<u64, ShadowRecordInfo>> {
        self.0
            .call("get_shadow_records")
            .args_json(json!({
                "account_id": account.id()
            }))
            .view()
            .await?
            .json::<HashMap<u64, ShadowRecordInfo>>()
    }

    pub async fn get_user_storage_state(
        &self,
        account: &Account,
    ) -> Result<Option<RefStorageState>> {
        self.0
            .call("get_user_storage_state")
            .args_json(json!({
                "account_id": account.id()
            }))
            .view()
            .await?
            .json::<Option<RefStorageState>>()
    }

    // pub async fn sync_lp_infos(
    //     &self,
    //     pool_ids: Vec<u64>,
    // ) -> Result<HashMap<String, UnitShareTokens>> {
    //     self.0
    //         .call("sync_lp_infos")
    //         .args_json(json!({
    //             "pool_ids": pool_ids
    //         }))
    //         .view()
    //         .await?
    //         .json::<HashMap<String, UnitShareTokens>>()
    // }
}
