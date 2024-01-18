use mock_boost_farming::{Seed, FarmerSeed, FarmerSeedOld};

use crate::*;

pub struct BoostFarmingContract(pub Contract);

impl BoostFarmingContract {
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

    pub async fn create_seed(
        &self,
        caller: &Account,
        seed_id: &String, 
        seed_decimal: u32,
        min_deposit: Option<U128>,
        min_locking_duration_sec: Option<u32>
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "create_seed")
            .args_json(json!({
                "seed_id": seed_id, 
                "seed_decimal": seed_decimal,
                "min_deposit": min_deposit,
                "min_locking_duration_sec": min_locking_duration_sec
            }))
            .max_gas()
            .deposit(1)
            .transact()
            .await
    }

    pub async fn stake_free_seed(
        &self,
        caller: &Account,
        ref_exchange: &RefExchange,
        token_id: String,
        amount: u128,
    ) -> Result<ExecutionFinalResult> {
        ref_exchange.mft_transfer_call(caller, token_id, self.0.id(), amount.into(), "\"Free\"".to_string()).await
    }

    pub async fn stake_free_shadow_seed(
        &self,
        caller: &Account,
        farmer_id: &AccountId, 
        seed_id: &String, 
        amount: U128
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "stake_free_shadow_seed")
            .args_json(json!({
                "farmer_id": farmer_id, 
                "seed_id": seed_id,
                "amount": amount,
            }))
            .max_gas()
            .transact()
            .await
    }

    pub async fn withdraw_shadow_seed(
        &self,
        caller: &Account,
        seed_id: &String, 
        withdraw_amount: U128
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "withdraw_shadow_seed")
            .args_json(json!({
                "seed_id": seed_id,
                "withdraw_amount": withdraw_amount,
            }))
            .max_gas()
            .deposit(1)
            .transact()
            .await
    }
}

impl BoostFarmingContract {
    pub async fn get_seed(
        &self,
        seed_id: &String, 
    ) -> Result<Option<Seed>> {
        self.0
            .call("get_seed")
            .args_json(json!({
                "seed_id": seed_id, 
            }))
            .view()
            .await?
            .json::<Option<Seed>>()
    }

    pub async fn get_farmer_seed(
        &self,
        farmer_id: &Account, 
        seed_id: &String
    ) -> Result<Option<FarmerSeed>> {
        self.0
            .call("get_farmer_seed")
            .args_json(json!({
                "farmer_id": farmer_id.id(), 
                "seed_id": seed_id, 
            }))
            .view()
            .await?
            .json::<Option<FarmerSeed>>()
    }

    pub async fn list_farmer_seeds(
        &self,
        farmer_id: &Account, 
        from_index: Option<u64>,
        limit: Option<u64>,
    ) -> Result<HashMap<String, FarmerSeed>> {
        self.0
            .call("list_farmer_seeds")
            .args_json(json!({
                "farmer_id": farmer_id.id(), 
                "from_index": from_index, 
                "limit": limit, 
            }))
            .view()
            .await?
            .json::<HashMap<String, FarmerSeed>>()
    }

    pub async fn list_farmer_seeds1(
        &self,
        farmer_id: &Account, 
        from_index: Option<u64>,
        limit: Option<u64>,
    ) -> Result<serde_json::Value> {
        self.0
            .call("list_farmer_seeds")
            .args_json(json!({
                "farmer_id": farmer_id.id(), 
                "from_index": from_index, 
                "limit": limit, 
            }))
            .view()
            .await?
            .json::<serde_json::Value>()
    }

    pub async fn get_farmer_seed_v0(
        &self,
        farmer_id: &Account, 
        seed_id: &String
    ) -> Result<Option<FarmerSeedOld>> {
        self.0
            .call("get_farmer_seed")
            .args_json(json!({
                "farmer_id": farmer_id.id(), 
                "seed_id": seed_id, 
            }))
            .view()
            .await?
            .json::<Option<FarmerSeedOld>>()
    }
}