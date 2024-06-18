
use mock_ref_exchange::{ContractMetadata, StablePoolInfo, ShadowRecordInfo, RefStorageState, ShadowActions, AccountBaseInfo, UnitShareCumulativeInfoView, PoolInfo};

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

    pub async fn swap(
        &self,
        token_contract: &FtContract,
        caller: &Account,
        amount: u128,
        pool_id: u64,
        token_out_id: &AccountId
    ) -> Result<ExecutionFinalResult> {
        token_contract.ft_transfer_call(caller, self.0.id(), amount,
        format!("{{\"actions\": [{{\"pool_id\": {}, \"token_in\": \"{}\", \"token_out\": \"{}\", \"min_amount_out\": \"1\"}}]}}", pool_id, token_contract.0.id().to_string(), token_out_id.to_string())
        ).await
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

    pub async fn add_simple_swap_pool(
        &self,
        caller: &Account,
        tokens: Vec<&AccountId>,
        fee: u32,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "add_simple_pool")
            .args_json(json!({
                "tokens": tokens,
                "fee": fee,
            }))
            .max_gas()
            .deposit(parse_near!("0.01 N"))
            .transact()
            .await
    }

    pub async fn add_simple_liquidity(
        &self,
        caller: &Account,
        pool_id: u64,
        amounts: Vec<U128>,
        min_amounts: Option<Vec<U128>>,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "add_liquidity")
            .args_json(json!({
                "pool_id": pool_id,
                "amounts": amounts,
                "min_amounts": min_amounts,
            }))
            .max_gas()
            .deposit(parse_near!("0.01 N"))
            .transact()
            .await
    }

    pub async fn register_pool_twap_record(
        &self,
        caller: &Account,
        pool_id: u64,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "register_pool_twap_record")
            .args_json(json!({
                "pool_id": pool_id,
            }))
            .max_gas()
            .deposit(1)
            .transact()
            .await
    }

    pub async fn modify_cumulative_info_record_interval_sec (
        &self,
        caller: &Account,
        record_interval_sec: u32
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "modify_cumulative_info_record_interval_sec")
            .args_json(json!({
                "record_interval_sec": record_interval_sec,
            }))
            .deposit(1)
            .max_gas()
            .transact()
            .await
    }

    pub async fn sync_pool_twap_record(
        &self,
        caller: &Account,
        pool_id: u64,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "sync_pool_twap_record")
            .args_json(json!({
                "pool_id": pool_id,
            }))
            .max_gas()
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

    pub async fn shadow_action(
        &self,
        caller: &Account,
        action: ShadowActions, 
        pool_id: u64, 
        amount: Option<U128>, 
        msg: String,
        deposit: u128
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "shadow_action")
            .args_json(json!({
                "action": action,
                "pool_id": pool_id,
                "amount": amount,
                "msg": msg,
            }))
            .deposit(deposit)
            .max_gas()
            .transact()
            .await
    }

    pub async fn shadow_farming(
        &self,
        caller: &Account,
        pool_id: u64, 
        amount: Option<U128>,
    ) -> Result<ExecutionFinalResult> {
        self.shadow_action(caller, ShadowActions::ToFarming, pool_id, amount, "".to_string(), d(1, 23)).await
    }

    pub async fn shadow_cancel_farming(
        &self,
        caller: &Account,
        pool_id: u64, 
        amount: Option<U128>
    ) -> Result<ExecutionFinalResult> {
        self.shadow_action(caller, ShadowActions::FromFarming, pool_id, amount, "".to_string(), 0).await
    }

    pub async fn shadow_burrowland_deposit(
        &self,
        caller: &Account,
        pool_id: u64, 
        amount: Option<U128>,
        after_deposit_actions_msg: Option<String>
    ) -> Result<ExecutionFinalResult> {
        self.shadow_action(caller, ShadowActions::ToBurrowland, pool_id, amount, after_deposit_actions_msg.unwrap_or("".to_string()), d(1, 23)).await
    }

    pub async fn shadow_burrowland_withdraw(
        &self,
        caller: &Account,
        pool_id: u64, 
        amount: Option<U128>,
        after_deposit_actions_msg: Option<String>
    ) -> Result<ExecutionFinalResult> {
        self.shadow_action(caller, ShadowActions::FromBurrowland, pool_id, amount, after_deposit_actions_msg.unwrap_or("".to_string()), 0).await
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

    pub async fn get_pool(
        &self,
        pool_id: u64,
    ) -> Result<PoolInfo> {
        self.0
            .call("get_pool")
            .args_json(json!({
                "pool_id": pool_id
            }))
            .view()
            .await?
            .json::<PoolInfo>()
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

    pub async fn get_account_basic_info(
        &self,
        account: &Account,
    ) -> Result<Option<AccountBaseInfo>> {
        self.0
            .call("get_account_basic_info")
            .args_json(json!({
                "account_id": account.id()
            }))
            .view()
            .await?
            .json::<Option<AccountBaseInfo>>()
    }

    pub async fn get_pool_twap_info_view(
        &self,
        pool_id: u64
    ) -> Result<Option<UnitShareCumulativeInfoView>> {
        self.0
            .call("get_pool_twap_info_view")
            .args_json(json!({
                "pool_id": pool_id
            }))
            .view()
            .await?
            .json::<Option<UnitShareCumulativeInfoView>>()
    }
}
