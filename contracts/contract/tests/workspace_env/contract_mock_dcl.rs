use mock_dcl::RunningState;

use crate::*;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(crate = "near_sdk::serde")]
pub struct QuoteResult {
    pub amount: U128, 
    pub tag: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(crate = "near_sdk::serde")]
pub struct Pool {
    pub pool_id: String,
    pub token_x: AccountId,
    pub token_y: AccountId,
    pub fee: u32,
    pub point_delta: i32,

    pub current_point: i32,
    pub liquidity: U128,
    pub liquidity_x: U128,
    pub max_liquidity_per_point: U128,

    pub volume_x_in: String,
    pub volume_y_in: String,
    pub volume_x_out: String,
    pub volume_y_out: String,

    pub state: RunningState,
}


pub struct DclExchange(pub Contract);

impl DclExchange {
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

    pub async fn deposit(
        &self,
        token_contract: &FtContract,
        caller: &Account,
        amount: u128,
    ) -> Result<ExecutionFinalResult> {
        token_contract.ft_transfer_call(caller, self.0.id(), amount, "\"Deposit\"".to_string()).await
    }

    pub async fn swap(
        &self,
        token_contract: &FtContract,
        caller: &Account,
        amount: u128,
        pool_id: String,
        token_out_id: &AccountId,
    ) -> Result<ExecutionFinalResult> {
        token_contract.ft_transfer_call(caller, self.0.id(), amount,
        format!("{{\"Swap\": {{\"pool_ids\": [\"{}\"], \"output_token\": \"{}\", \"min_output_amount\": \"1\"}}}}", pool_id, token_out_id.to_string())
        ).await
    }

    pub async fn create_pool(
        &self,
        caller: &Account,
        token_a_id: &AccountId,
        token_b_id: &AccountId,
        fee: u32,
        init_point: i32,
        deposit: u128,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "create_pool")
            .args_json(json!({
                "token_a": token_a_id,
                "token_b": token_b_id,
                "fee": fee,
                "init_point": init_point,
            }))
            .max_gas()
            .deposit(deposit)
            .transact()
            .await
    }

    pub async fn add_liquidity(
        &self,
        caller: &Account,
        pool_id: &String,
        left_point: i32,
        right_point: i32,
        amount_x: u128,
        amount_y: u128,
        min_amount_x: u128,
        min_amount_y: u128,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "add_liquidity")
            .args_json(json!({
                "pool_id": pool_id,
                "left_point": left_point,
                "right_point": right_point,
                "amount_x": U128(amount_x),
                "amount_y": U128(amount_y),
                "min_amount_x": U128(min_amount_x),
                "min_amount_y": U128(min_amount_y),
            }))
            .max_gas()
            .transact()
            .await
    }
}

impl DclExchange {
    pub async fn quote(
        &self,
        pool_ids: Vec<&String>,
        input_token: &AccountId,
        output_token: &AccountId,
        input_amount: u128,
        tag: Option<String>,
    ) -> Result<QuoteResult> {
        self.0
            .call("quote")
            .args_json(json!({
                "pool_ids": pool_ids,
                "input_token": input_token,
                "output_token": output_token,
                "input_amount": U128(input_amount),
                "tag": tag,
            }))
            .view()
            .await?
            .json::<QuoteResult>()
    }

    pub async fn get_pool(
        &self,
        pool_id: &String,
    ) -> Result<Pool> {
        self.0
            .call("get_pool")
            .args_json(json!({
                "pool_id": pool_id,
            }))
            .view()
            .await?
            .json::<Pool>()
    }

    pub async fn list_user_assets(
        &self,
        account: &Account,
    ) -> Result<HashMap<AccountId, U128>> {
        self.0
            .call("list_user_assets")
            .args_json(json!({
                "account_id": account.id(),
            }))
            .view()
            .await?
            .json::<HashMap<AccountId, U128>>()
    }
}