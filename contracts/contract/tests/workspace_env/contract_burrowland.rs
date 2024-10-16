use crate::*;

use contract::{Config, AssetConfig, AssetDetailedView, PriceReceiverMsg, AccountDetailedView, AssetAmount, Action, TokenReceiverMsg};

pub struct Burrowland(pub Contract);

impl Burrowland {
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
            .gas(Gas::from_tgas(20))
            .deposit(NearToken::from_near(1))
            .transact()
            .await
    }

    pub async fn enable_oracle(
        &self,
        caller: &Account,
        enable_price_oracle: bool,
        enable_pyth_oracle: bool
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "enable_oracle")
            .args_json(json!({
                "enable_price_oracle": enable_price_oracle,
                "enable_pyth_oracle": enable_pyth_oracle
            }))
            .max_gas()
            .deposit(NearToken::from_yoctonear(1))
            .transact()
            .await
    }

    pub async fn update_config(
        &self,
        caller: &Account,
        config: &Config
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "update_config")
            .args_json(json!({
                "config": config,
            }))
            .max_gas()
            .deposit(NearToken::from_yoctonear(1))
            .transact()
            .await
    }

    pub async fn update_asset(
        &self,
        caller: &Account,
        token_id: &AccountId, 
        asset_config: AssetConfig
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "update_asset")
            .args_json(json!({
                "token_id": token_id, 
                "asset_config": asset_config
            }))
            .max_gas()
            .deposit(NearToken::from_yoctonear(1))
            .transact()
            .await
    }

    pub async fn add_asset(
        &self,
        caller: &Account,
        token_id: &AccountId, 
        asset_config: AssetConfig
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "add_asset")
            .args_json(json!({
                "token_id": token_id,
                "asset_config": asset_config,
            }))
            .max_gas()
            .deposit(NearToken::from_yoctonear(1))
            .transact()
            .await
    }

    pub async fn account_stake_booster(
        &self,
        caller: &Account,
        amount: Option<U128>, 
        duration: u32
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "account_stake_booster")
            .args_json(json!({
                "amount": amount,
                "duration": duration,
            }))
            .max_gas()
            .deposit(NearToken::from_yoctonear(1))
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

    pub async fn deposit_rated(
        &self,
        token_contract: &RatedTokenContract,
        caller: &Account,
        amount: u128,
    ) -> Result<ExecutionFinalResult> {
        token_contract.ft_transfer_call(caller, self.0.id(), amount, "".to_string()).await
    }


    pub async fn deposit_to_reserve(
        &self,
        token_contract: &FtContract,
        caller: &Account,
        amount: u128,
    ) -> Result<ExecutionFinalResult> {
        token_contract.ft_transfer_call(caller, self.0.id(), amount, "\"DepositToReserve\"".to_string()).await
    }

    pub async fn borrow (
        &self,
        caller: &Account,
        oracle: &Oralce,
        price_data: PriceData,
        token_id: &AccountId,
        borrow_amount: u128,
    ) -> Result<ExecutionFinalResult> {
        oracle.oracle_call(caller, self.0.id(), price_data, PriceReceiverMsg::Execute {
            actions: vec![
                Action::Borrow(asset_amount(token_id, borrow_amount)),
            ],
        }).await
    }

    pub async fn withdraw(
        &self,
        caller: &Account,
        token_id: &AccountId,
        withdraw_amount: u128,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "execute")
            .args_json(json!({
                "actions": vec![
                    Action::Withdraw(asset_amount(token_id, withdraw_amount)),
                ]
            }))
            .max_gas()
            .deposit(NearToken::from_yoctonear(1))
            .transact()
            .await
    }

    pub async fn borrow_and_withdraw(
        &self,
        caller: &Account,
        oracle: &Oralce,
        receiver_id: &AccountId,
        price_data: PriceData,
        token_id: &AccountId,
        borrow_amount: u128,
    ) -> Result<ExecutionFinalResult> {
        oracle.oracle_call(caller, receiver_id, price_data, PriceReceiverMsg::Execute {
            actions: vec![
                Action::Borrow(asset_amount(token_id, borrow_amount)),
                Action::Withdraw(asset_amount(token_id, borrow_amount)),
            ],
        }).await
    }

    pub async fn borrow_and_withdraw_with_pyth(
        &self,
        caller: &Account,
        token_id: &AccountId,
        borrow_amount: u128,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "execute_with_pyth")
            .args_json(json!({
                "actions": vec![
                    Action::Borrow(asset_amount(token_id, borrow_amount)),
                    Action::Withdraw(asset_amount(token_id, borrow_amount)),
                ],
            }))
            .max_gas()
            .deposit(NearToken::from_yoctonear(1))
            .transact()
            .await
    }

    pub async fn deposit_increase_collateral_borrow_withdraw_with_pyth (
        &self,
        token_contract: &FtContract,
        caller: &Account,
        amount: u128,
        borrow_token_id: &AccountId,
        borrow_amount: u128 
    ) -> Result<ExecutionFinalResult> {
        token_contract.ft_transfer_call(caller, self.0.id(), amount, serde_json::to_string(&TokenReceiverMsg::ExecuteWithPyth {
            actions: vec![
                Action::IncreaseCollateral(asset_amount(token_contract.0.id(), 0)),
                Action::Borrow(asset_amount(borrow_token_id, borrow_amount)),
                Action::Withdraw(asset_amount(borrow_token_id, 0)),
            ]
        }).unwrap()).await
    }
    
    pub async fn deposit_repay_decrease_collateral_withdraw_with_pyth (
        &self,
        token_contract: &FtContract,
        caller: &Account,
        amount: u128,
        repay_token_id: &AccountId,
        repay_amount: u128,
        decrease_token_id: &AccountId,
        decrease_amount: u128 
    ) -> Result<ExecutionFinalResult> {
        token_contract.ft_transfer_call(caller, self.0.id(), amount, serde_json::to_string(&TokenReceiverMsg::ExecuteWithPyth {
            actions: vec![
                Action::Repay(asset_amount(repay_token_id, repay_amount)),
                Action::DecreaseCollateral(asset_amount(decrease_token_id, decrease_amount)),
                Action::Withdraw(asset_amount(decrease_token_id, 0)),
            ]
        }).unwrap()).await
    }

    pub async fn supply_to_collateral(
        &self,
        token_contract: &FtContract,
        caller: &Account,
        amount: u128,
    ) -> Result<ExecutionFinalResult> {
        token_contract.ft_transfer_call(caller, self.0.id(), amount, serde_json::to_string(&TokenReceiverMsg::Execute {
            actions: vec![
                Action::IncreaseCollateral(asset_amount(token_contract.0.id(), 0))
            ]
        }).unwrap()).await
    }

    pub async fn increase_collateral(
        &self,
        caller: &Account,
        token_id: &AccountId,
        amount: u128
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "execute")
            .args_json(json!({
                "actions": vec![
                    Action::IncreaseCollateral(asset_amount(token_id, amount)),
                ],
            }))
            .max_gas()
            .deposit(NearToken::from_yoctonear(1))
            .transact()
            .await
    }

    pub async fn increase_collateral_with_pyth(
        &self,
        caller: &Account,
        token_id: &AccountId,
        amount: u128
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "execute_with_pyth")
            .args_json(json!({
                "actions": vec![
                    Action::IncreaseCollateral(asset_amount(token_id, amount)),
                ],
            }))
            .max_gas()
            .deposit(NearToken::from_yoctonear(1))
            .transact()
            .await
    }

    pub async fn position_increase_collateral(
        &self,
        caller: &Account,
        token_id: &AccountId,
        amount: u128
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "execute")
            .args_json(json!({
                "actions": vec![
                    Action::PositionIncreaseCollateral{
                        position: token_id.to_string(),
                        asset_amount: asset_amount(token_id, amount)
                    },
                ],
            }))
            .max_gas()
            .deposit(NearToken::from_yoctonear(1))
            .transact()
            .await
    }

    pub async fn position_borrow_and_withdraw(
        &self,
        caller: &Account,
        oracle: &Oralce,
        receiver_id: &AccountId,
        price_data: PriceData,
        position: String,
        token_id: &AccountId,
        borrow_amount: u128,
        withdraw_amount: u128
    ) -> Result<ExecutionFinalResult> {
        oracle.oracle_call(caller, receiver_id, price_data, PriceReceiverMsg::Execute {
            actions: vec![
                Action::PositionBorrow{
                    position,
                    asset_amount: asset_amount(token_id, borrow_amount)
                },
                Action::Withdraw(asset_amount(token_id, withdraw_amount)),
            ],
        }).await
    }

    pub async fn position_borrow_and_withdraw_with_pyth(
        &self,
        caller: &Account,
        position: String,
        token_id: &AccountId,
        borrow_amount: u128,
        withdraw_amount: u128
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "execute_with_pyth")
            .args_json(json!({
                "actions": vec![
                    Action::PositionBorrow{
                        position,
                        asset_amount: asset_amount(token_id, borrow_amount)
                    },
                    Action::Withdraw(asset_amount(token_id, withdraw_amount)),
                ],
            }))
            .max_gas()
            .deposit(NearToken::from_yoctonear(1))
            .transact()
            .await
    }

    pub async fn liquidate(
        &self,
        caller: &Account,
        oracle: &Oralce,
        receiver_id: &AccountId,
        liquidation_account_id: &AccountId,
        price_data: PriceData,
        in_assets: Vec<AssetAmount>,
        out_assets: Vec<AssetAmount>,
        position: Option<String>,
        min_token_amounts: Option<Vec<U128>>
    ) -> Result<ExecutionFinalResult> {
        oracle.oracle_call(caller, receiver_id, price_data, PriceReceiverMsg::Execute {
            actions: vec![
                Action::Liquidate { 
                    account_id: near_sdk::AccountId::new_unchecked(liquidation_account_id.to_string()), 
                    in_assets, out_assets, position, min_token_amounts
                },
            ],
        }).await
    }

    pub async fn liquidate_with_pyth(
        &self,
        caller: &Account,
        liquidation_account_id: &AccountId,
        in_assets: Vec<AssetAmount>,
        out_assets: Vec<AssetAmount>,
        position: Option<String>,
        min_token_amounts: Option<Vec<U128>>
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "execute_with_pyth")
            .args_json(json!({
                "actions": vec![
                    Action::Liquidate { 
                        account_id: near_sdk::AccountId::new_unchecked(liquidation_account_id.to_string()), 
                        in_assets, out_assets, position, min_token_amounts
                    },
                ],
            }))
            .max_gas()
            .deposit(NearToken::from_yoctonear(1))
            .transact()
            .await
    }

    pub async fn force_close(
        &self,
        caller: &Account,
        oracle: &Oralce,
        force_close_account_id: &AccountId,
        price_data: PriceData,
        position: Option<String>,
        min_token_amounts: Option<Vec<U128>>
    ) -> Result<ExecutionFinalResult> {
        oracle.oracle_call(caller, self.0.id(), price_data, PriceReceiverMsg::Execute {
            actions: vec![
                Action::ForceClose { 
                    account_id: near_sdk::AccountId::new_unchecked(force_close_account_id.to_string()),
                    position, min_token_amounts
                },
            ],
        }).await
    }

    pub async fn force_close_with_pyth(
        &self,
        caller: &Account,
        force_close_account_id: &AccountId,
        position: Option<String>,
        min_token_amounts: Option<Vec<U128>>
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "execute_with_pyth")
            .args_json(json!({
                "actions": vec![
                    Action::ForceClose { 
                        account_id: near_sdk::AccountId::new_unchecked(force_close_account_id.to_string()),
                        position, min_token_amounts
                    },
                ],
            }))
            .max_gas()
            .deposit(NearToken::from_yoctonear(1))
            .transact()
            .await
    }
    
    pub async fn claim_prot_fee(
        &self,
        caller: &Account,
        token_id: &AccountId, 
        stdd_amount: Option<U128>
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "claim_prot_fee")
            .args_json(json!({
                "token_id": token_id, 
                "stdd_amount": stdd_amount
            }))
            .max_gas()
            .deposit(NearToken::from_yoctonear(1))
            .transact()
            .await
    }

    pub async fn decrease_reserved(
        &self,
        caller: &Account,
        token_id: &AccountId, 
        stdd_amount: Option<U128>
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "decrease_reserved")
            .args_json(json!({
                "token_id": token_id, 
                "stdd_amount": stdd_amount
            }))
            .max_gas()
            .deposit(NearToken::from_yoctonear(1))
            .transact()
            .await
    }

    pub async fn sync_ref_exchange_lp_token_infos(
        &self,
        caller: &Account,
        token_ids: Option<Vec<String>>
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "sync_ref_exchange_lp_token_infos")
            .args_json(json!({
                "token_ids": token_ids
            }))
            .max_gas()
            .transact()
            .await
    }

    pub async fn add_asset_farm_reward(
        &self,
        caller: &Account,
        farm_id: FarmId,
        reward_token_id: &AccountId,
        new_reward_per_day: U128,
        new_booster_log_base: U128,
        reward_amount: U128,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "add_asset_farm_reward")
            .args_json(json!({
                "farm_id": farm_id,
                "reward_token_id": reward_token_id,
                "new_reward_per_day": new_reward_per_day,
                "new_booster_log_base": new_booster_log_base,
                "reward_amount": reward_amount,
            }))
            .max_gas()
            .deposit(NearToken::from_yoctonear(1))
            .transact()
            .await
    }

    pub async fn account_farm_claim_all(
        &self,
        caller: &Account,
        account_id: Option<AccountId>
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "account_farm_claim_all")
            .args_json(json!({
                "account_id": account_id,
            }))
            .max_gas()
            .transact()
            .await
    }

    pub async fn extend_guardians(
        &self,
        caller: &Account,
        guardians: Vec<&AccountId>
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "extend_guardians")
            .args_json(json!({
                "guardians": guardians,
            }))
            .deposit(NearToken::from_yoctonear(1))
            .max_gas()
            .transact()
            .await
    }

    pub async fn remove_guardians(
        &self,
        caller: &Account,
        guardians: Vec<&AccountId>
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "remove_guardians")
            .args_json(json!({
                "guardians": guardians,
            }))
            .deposit(NearToken::from_yoctonear(1))
            .max_gas()
            .transact()
            .await
    }

    pub async fn update_asset_prot_ratio(
        &self,
        caller: &Account,
        token_id: &AccountId,
        prot_ratio: u32
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "update_asset_prot_ratio")
            .args_json(json!({
                "token_id": token_id,
                "prot_ratio": prot_ratio
            }))
            .deposit(NearToken::from_yoctonear(1))
            .max_gas()
            .transact()
            .await
    }

    pub async fn update_asset_net_tvl_multiplier(
        &self,
        caller: &Account,
        token_id: &AccountId,
        net_tvl_multiplier: u32
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "update_asset_net_tvl_multiplier")
            .args_json(json!({
                "token_id": token_id,
                "net_tvl_multiplier": net_tvl_multiplier
            }))
            .deposit(NearToken::from_yoctonear(1))
            .max_gas()
            .transact()
            .await
    }

    pub async fn enable_asset_capacity(
        &self,
        caller: &Account,
        token_id: &AccountId,
        can_deposit: Option<bool>, 
        can_withdraw: Option<bool>, 
        can_use_as_collateral: Option<bool>, 
        can_borrow: Option<bool>
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "enable_asset_capacity")
            .args_json(json!({
                "token_id": token_id,
                "can_deposit": can_deposit, 
                "can_withdraw": can_withdraw, 
                "can_use_as_collateral": can_use_as_collateral, 
                "can_borrow": can_borrow
            }))
            .deposit(NearToken::from_yoctonear(1))
            .max_gas()
            .transact()
            .await
    }

    pub async fn disable_asset_capacity(
        &self,
        caller: &Account,
        token_id: &AccountId,
        can_deposit: Option<bool>, 
        can_withdraw: Option<bool>, 
        can_use_as_collateral: Option<bool>, 
        can_borrow: Option<bool>
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "disable_asset_capacity")
            .args_json(json!({
                "token_id": token_id,
                "can_deposit": can_deposit, 
                "can_withdraw": can_withdraw, 
                "can_use_as_collateral": can_use_as_collateral, 
                "can_borrow": can_borrow
            }))
            .deposit(NearToken::from_yoctonear(1))
            .max_gas()
            .transact()
            .await
    }

    pub async fn add_token_pyth_info(
        &self,
        caller: &Account,
        token_id: &AccountId,
        decimals: u8,
        fraction_digits: u8,
        price_identifier: &str,
        extra_call: Option<String>,
        default_price: Option<Price>
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "add_token_pyth_info")
            .args_json(json!({
                "token_id": token_id,
                "token_pyth_info": {
                    "decimals": decimals,
                    "fraction_digits": fraction_digits,
                    "price_identifier": price_identifier,
                    "extra_call": extra_call,
                    "default_price": default_price
                }
            }))
            .max_gas()
            .deposit(NearToken::from_yoctonear(1))
            .transact()
            .await
    }

    pub async fn update_token_pyth_info(
        &self,
        caller: &Account,
        token_id: &AccountId,
        decimals: u8,
        fraction_digits: u8,
        price_identifier: &str,
        extra_call: Option<String>,
        default_price: Option<Price>
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "update_token_pyth_info")
            .args_json(json!({
                "token_id": token_id,
                "token_pyth_info": {
                    "decimals": decimals,
                    "fraction_digits": fraction_digits,
                    "price_identifier": price_identifier,
                    "extra_call": extra_call,
                    "default_price": default_price
                }
            }))
            .max_gas()
            .deposit(NearToken::from_yoctonear(1))
            .transact()
            .await
    }

    pub async fn execute_with_pyth(
        &self,
        caller: &Account,
        actions: Vec<Action>,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "execute_with_pyth")
            .args_json(json!({
                "actions": actions,
            }))
            .max_gas()
            .deposit(NearToken::from_yoctonear(1))
            .transact()
            .await
    }
}

impl Burrowland {
    pub async fn get_asset(
        &self,
        token_id: &AccountId
    ) -> Result<AssetDetailedView> {
        self.0
            .call("get_asset")
            .args_json(json!({
                "token_id": token_id
            }))
            .view()
            .await?
            .json::<AssetDetailedView>()
    }

    pub async fn get_account(
        &self,
        account: &Account
    ) -> Result<Option<AccountDetailedView>> {
        self.0
            .call("get_account")
            .args_json(json!({
                "account_id": account.id()
            }))
            .view()
            .await?
            .json::<Option<AccountDetailedView>>()
    }

    pub async fn get_account_all_positions(
        &self,
        account: &Account
    ) -> Result<Option<AccountAllPositionsDetailedView>> {
        self.0
            .call("get_account_all_positions")
            .args_json(json!({
                "account_id": account.id()
            }))
            .view()
            .await?
            .json::<Option<AccountAllPositionsDetailedView>>()
    }

    pub async fn get_config(
        &self,
    ) -> Result<Config> {
        self.0
            .call("get_config")
            .view()
            .await?
            .json::<Config>()
    }

    pub async fn get_config_v0(
        &self,
    ) -> Result<ConfigV0> {
        self.0
            .call("get_config")
            .view()
            .await?
            .json::<ConfigV0>()
    }

    pub async fn get_config_v1(
        &self,
    ) -> Result<ConfigV1> {
        self.0
            .call("get_config")
            .view()
            .await?
            .json::<ConfigV1>()
    }

    pub async fn get_config_v2(
        &self,
    ) -> Result<ConfigV2> {
        self.0
            .call("get_config")
            .view()
            .await?
            .json::<ConfigV2>()
    }

    pub async fn get_config_v3(
        &self,
    ) -> Result<ConfigV3> {
        self.0
            .call("get_config")
            .view()
            .await?
            .json::<ConfigV3>()
    }

    pub async fn get_version(
        &self,
    ) -> Result<String> {
        self.0
            .call("get_version")
            .view()
            .await?
            .json::<String>()
    }

    pub async fn get_last_lp_token_infos(
        &self,
    ) -> Result<HashMap<String, UnitShareTokens>> {
        self.0
            .call("get_last_lp_token_infos")
            .view()
            .await?
            .json::<HashMap<String, UnitShareTokens>>()
    }
    
    pub async fn get_token_pyth_info(
        &self,
        token_id: &AccountId
    ) -> Result<TokenPythInfo> {
        self.0
            .call("get_token_pyth_info")
            .args_json(json!({
                "token_id": token_id
            }))
            .view()
            .await?
            .json::<TokenPythInfo>()
    }
}


impl Burrowland {
    pub async fn add_asset_handler(
        &self,
        root: &Account,
        token: &FtContract, 
    ) -> Result<ExecutionFinalResult> {
        let token_id = token.0.id();
        let asset_config = match token_id.to_string().as_str() {
            "booster.test.near" => AssetConfig {
                reserve_ratio: 2500,
                prot_ratio: 0,
                target_utilization: 8000,
                target_utilization_rate: U128(1000000000008319516250272147),
                max_utilization_rate: U128(1000000000039724853136740579),
                holding_position_fee_rate: U128(1000000000000000000000000000),
                volatility_ratio: 2000,
                extra_decimals: 0,
                can_deposit: true,
                can_withdraw: true,
                can_use_as_collateral: false,
                can_borrow: false,
                net_tvl_multiplier: 10000,
                max_change_rate: None,
                supplied_limit: Some(u128::MAX.into()),
                borrowed_limit: Some(u128::MAX.into()),
                min_borrowed_amount: Some(1u128.into()),
            },
            "linear.test.near" => AssetConfig {
                reserve_ratio: 2500,
                prot_ratio: 0,
                target_utilization: 8000,
                target_utilization_rate: U128(1000000000003593629036885046),
                max_utilization_rate: U128(1000000000039724853136740579),
                holding_position_fee_rate: U128(1000000000000000000000000000),
                volatility_ratio: 5000,
                extra_decimals: 0,
                can_deposit: true,
                can_withdraw: true,
                can_use_as_collateral: true,
                can_borrow: true,
                net_tvl_multiplier: 2500,
                max_change_rate: None,
                supplied_limit: Some(u128::MAX.into()),
                borrowed_limit: Some(u128::MAX.into()),
                min_borrowed_amount: Some(1u128.into()),
            },
            "stnear.test.near" => AssetConfig {
                reserve_ratio: 2500,
                prot_ratio: 0,
                target_utilization: 8000,
                target_utilization_rate: U128(1000000000003593629036885046),
                max_utilization_rate: U128(1000000000039724853136740579),
                holding_position_fee_rate: U128(1000000000000000000000000000),
                volatility_ratio: 7000,
                extra_decimals: 0,
                can_deposit: true,
                can_withdraw: true,
                can_use_as_collateral: true,
                can_borrow: true,
                net_tvl_multiplier: 2500,
                max_change_rate: None,
                supplied_limit: Some(u128::MAX.into()),
                borrowed_limit: Some(u128::MAX.into()),
                min_borrowed_amount: Some(1u128.into()),
            },
            "nearx.test.near" => AssetConfig {
                reserve_ratio: 2500,
                prot_ratio: 0,
                target_utilization: 8000,
                target_utilization_rate: U128(1000000000003593629036885046),
                max_utilization_rate: U128(1000000000039724853136740579),
                holding_position_fee_rate: U128(1000000000000000000000000000),
                volatility_ratio: 7000,
                extra_decimals: 0,
                can_deposit: true,
                can_withdraw: true,
                can_use_as_collateral: true,
                can_borrow: false,
                net_tvl_multiplier: 0,
                max_change_rate: None,
                supplied_limit: Some(u128::MAX.into()),
                borrowed_limit: Some(u128::MAX.into()),
                min_borrowed_amount: Some(1u128.into()),
            },
            "wrap.test.near" => AssetConfig {
                reserve_ratio: 2500,
                prot_ratio: 0,
                target_utilization: 8000,
                target_utilization_rate: U128(1000000000003593629036885046),
                max_utilization_rate: U128(1000000000039724853136740579),
                holding_position_fee_rate: U128(1000000000000000000000000000),
                volatility_ratio: 6000,
                extra_decimals: 0,
                can_deposit: true,
                can_withdraw: true,
                can_use_as_collateral: true,
                can_borrow: true,
                net_tvl_multiplier: 10000,
                max_change_rate: None,
                supplied_limit: Some(u128::MAX.into()),
                borrowed_limit: Some(u128::MAX.into()),
                min_borrowed_amount: Some(1u128.into()),
            },
            "neth.test.near" => AssetConfig {
                reserve_ratio: 2500,
                prot_ratio: 0,
                target_utilization: 8000,
                target_utilization_rate: U128(1000000000001547125956667610),
                max_utilization_rate: U128(1000000000039724853136740579),
                holding_position_fee_rate: U128(1000000000000000000000000000),
                volatility_ratio: 6000,
                extra_decimals: 0,
                can_deposit: true,
                can_withdraw: true,
                can_use_as_collateral: true,
                can_borrow: true,
                net_tvl_multiplier: 10000,
                max_change_rate: None,
                supplied_limit: Some(u128::MAX.into()),
                borrowed_limit: Some(u128::MAX.into()),
                min_borrowed_amount: Some(1u128.into()),
            },
            "ndai.test.near" => AssetConfig {
                reserve_ratio: 2500,
                prot_ratio: 0,
                target_utilization: 8000,
                target_utilization_rate: U128(1000000000002440418605283556),
                max_utilization_rate: U128(1000000000039724853136740579),
                holding_position_fee_rate: U128(1000000000000000000000000000),
                volatility_ratio: 9500,
                extra_decimals: 0,
                can_deposit: true,
                can_withdraw: true,
                can_use_as_collateral: true,
                can_borrow: true,
                net_tvl_multiplier: 10000,
                max_change_rate: None,
                supplied_limit: Some(u128::MAX.into()),
                borrowed_limit: Some(u128::MAX.into()),
                min_borrowed_amount: Some(1u128.into()),
            },
            "nusdt.test.near" => AssetConfig {
                reserve_ratio: 2500,
                prot_ratio: 0,
                target_utilization: 8000,
                target_utilization_rate: U128(1000000000002440418605283556),
                max_utilization_rate: U128(1000000000039724853136740579),
                holding_position_fee_rate: U128(1000000000000000000000000000),
                volatility_ratio: 9500,
                extra_decimals: 12,
                can_deposit: true,
                can_withdraw: true,
                can_use_as_collateral: true,
                can_borrow: true,
                net_tvl_multiplier: 10000,
                max_change_rate: None,
                supplied_limit: Some(u128::MAX.into()),
                borrowed_limit: Some(u128::MAX.into()),
                min_borrowed_amount: Some(1u128.into()),
            },
            "nusdc.test.near" => AssetConfig {
                reserve_ratio: 2500,
                prot_ratio: 0,
                target_utilization: 8000,
                target_utilization_rate: U128(1000000000002440418605283556),
                max_utilization_rate: U128(1000000000039724853136740579),
                holding_position_fee_rate: U128(1000000000000000000000000000),
                volatility_ratio: 9500,
                extra_decimals: 12,
                can_deposit: true,
                can_withdraw: true,
                can_use_as_collateral: true,
                can_borrow: true,
                net_tvl_multiplier: 10000,
                max_change_rate: None,
                supplied_limit: Some(u128::MAX.into()),
                borrowed_limit: Some(u128::MAX.into()),
                min_borrowed_amount: Some(1u128.into()),
            },
            _ => {
                panic!("unsupported token: {:?}", token_id);
            }
        };
        self.add_asset(root, token_id, asset_config).await
    }

    pub async fn add_rated_asset_handler(
        &self,
        root: &Account,
        token: &RatedTokenContract, 
    ) -> Result<ExecutionFinalResult> {
        let token_id = token.0.id();
        let asset_config = match token_id.to_string().as_str() {
            "linear.test.near" => AssetConfig {
                reserve_ratio: 2500,
                prot_ratio: 0,
                target_utilization: 8000,
                target_utilization_rate: U128(1000000000003593629036885046),
                max_utilization_rate: U128(1000000000039724853136740579),
                holding_position_fee_rate: U128(1000000000000000000000000000),
                volatility_ratio: 5000,
                extra_decimals: 0,
                can_deposit: true,
                can_withdraw: true,
                can_use_as_collateral: true,
                can_borrow: true,
                net_tvl_multiplier: 2500,
                max_change_rate: None,
                supplied_limit: Some(u128::MAX.into()),
                borrowed_limit: Some(u128::MAX.into()),
                min_borrowed_amount: Some(1u128.into()),
            },
            "stnear.test.near" => AssetConfig {
                reserve_ratio: 2500,
                prot_ratio: 0,
                target_utilization: 8000,
                target_utilization_rate: U128(1000000000003593629036885046),
                max_utilization_rate: U128(1000000000039724853136740579),
                holding_position_fee_rate: U128(1000000000000000000000000000),
                volatility_ratio: 7000,
                extra_decimals: 0,
                can_deposit: true,
                can_withdraw: true,
                can_use_as_collateral: true,
                can_borrow: true,
                net_tvl_multiplier: 2500,
                max_change_rate: None,
                supplied_limit: Some(u128::MAX.into()),
                borrowed_limit: Some(u128::MAX.into()),
                min_borrowed_amount: Some(1u128.into()),
            },
            "nearx.test.near" => AssetConfig {
                reserve_ratio: 2500,
                prot_ratio: 0,
                target_utilization: 8000,
                target_utilization_rate: U128(1000000000003593629036885046),
                max_utilization_rate: U128(1000000000039724853136740579),
                holding_position_fee_rate: U128(1000000000000000000000000000),
                volatility_ratio: 7000,
                extra_decimals: 0,
                can_deposit: true,
                can_withdraw: true,
                can_use_as_collateral: true,
                can_borrow: false,
                net_tvl_multiplier: 0,
                max_change_rate: None,
                supplied_limit: Some(u128::MAX.into()),
                borrowed_limit: Some(u128::MAX.into()),
                min_borrowed_amount: Some(1u128.into()),
            },
            _ => {
                panic!("unsupported token: {:?}", token_id);
            }
        };
        self.add_asset(root, token_id, asset_config).await
    }
}

// margin trading
impl Burrowland {
    pub async fn deposit_to_margin(
        &self,
        token_contract: &FtContract,
        caller: &Account,
        amount: u128,
    ) -> Result<ExecutionFinalResult> {
        token_contract.ft_transfer_call(caller, self.0.id(), amount, "\"DepositToMargin\"".to_string()).await
    }

    pub async fn register_margin_token(&self, 
        caller: &Account,
        token_id: &AccountId, 
        token_party: u8
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "register_margin_token")
            .args_json(json!({
                "token_id": token_id, 
                "token_party": token_party
            }))
            .deposit(NearToken::from_yoctonear(1))
            .max_gas()
            .transact()
            .await
    }

    pub async fn register_margin_dex(
        &self, 
        caller: &Account,
        dex_id: &AccountId, 
        dex_version: u8
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "register_margin_dex")
            .args_json(json!({
                "dex_id": dex_id, 
                "dex_version": dex_version
            }))
            .deposit(NearToken::from_yoctonear(1))
            .max_gas()
            .transact()
            .await
    }

    pub async fn margin_execute_with_pyth(
        &self,
        caller: &Account,
        actions: Vec<MarginAction>,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "margin_execute_with_pyth")
            .args_json(json!({
                "actions": actions,
            }))
            .max_gas()
            .deposit(NearToken::from_yoctonear(1))
            .transact()
            .await
    }

    pub async fn margin_trading_open_position_by_oracle_call(
        &self,
        oracle: &Oralce,
        price_data: PriceData,
        caller: &Account,
        token_c_id: &AccountId,
        token_c_amount: U128,
        token_d_id: &AccountId,
        token_d_amount: U128,
        token_p_id: &AccountId,
        min_token_p_amount: U128,
        swap_indication: SwapIndication,
    ) -> Result<ExecutionFinalResult> {
        oracle.oracle_call(caller, self.0.id(), price_data, PriceReceiverMsg::MarginExecute {
            actions: vec![
                MarginAction::OpenPosition { 
                    token_c_id: near_sdk::AccountId::new_unchecked(token_c_id.to_string()), 
                    token_c_amount, 
                    token_d_id: near_sdk::AccountId::new_unchecked(token_d_id.to_string()), 
                    token_d_amount, 
                    token_p_id: near_sdk::AccountId::new_unchecked(token_p_id.to_string()), 
                    min_token_p_amount, 
                    swap_indication 
                }
            ],
        }).await
    }

    pub async fn margin_trading_open_position_by_pyth(
        &self,
        caller: &Account,
        token_c_id: &AccountId,
        token_c_amount: U128,
        token_d_id: &AccountId,
        token_d_amount: U128,
        token_p_id: &AccountId,
        min_token_p_amount: U128,
        swap_indication: SwapIndication,
    ) -> Result<ExecutionFinalResult> {
        self.margin_execute_with_pyth(caller, vec![
            MarginAction::OpenPosition { 
                token_c_id: near_sdk::AccountId::new_unchecked(token_c_id.to_string()), 
                token_c_amount, 
                token_d_id: near_sdk::AccountId::new_unchecked(token_d_id.to_string()), 
                token_d_amount, 
                token_p_id: near_sdk::AccountId::new_unchecked(token_p_id.to_string()), 
                min_token_p_amount, 
                swap_indication 
            }
        ]).await
    }

    pub async fn margin_trading_decrease_mtposition_by_oracle_call(
        &self,
        oracle: &Oralce,
        price_data: PriceData,
        caller: &Account,
        pos_id: &String,
        token_p_amount: u128,
        min_token_d_amount: u128,
        swap_indication: SwapIndication,
    ) -> Result<ExecutionFinalResult> {
        oracle.oracle_call(caller, self.0.id(), price_data, PriceReceiverMsg::MarginExecute {
            actions: vec![
                MarginAction::DecreaseMTPosition { 
                    pos_id: pos_id.clone(),
                    token_p_amount: token_p_amount.into(),
                    min_token_d_amount: min_token_d_amount.into(),
                    swap_indication: swap_indication,
                }
            ],
        }).await
    }

    pub async fn margin_trading_decrease_mtposition_by_pyth(
        &self,
        caller: &Account,
        pos_id: &String,
        token_p_amount: u128,
        min_token_d_amount: u128,
        swap_indication: SwapIndication,
    ) -> Result<ExecutionFinalResult> {
        self.margin_execute_with_pyth(caller, vec![
            MarginAction::DecreaseMTPosition { 
                pos_id: pos_id.clone(),
                token_p_amount: token_p_amount.into(),
                min_token_d_amount: min_token_d_amount.into(),
                swap_indication: swap_indication,
            }
        ]).await
    }

    pub async fn margin_trading_close_mtposition_by_oracle_call(
        &self,
        oracle: &Oralce,
        price_data: PriceData,
        caller: &Account,
        pos_id: &String,
        token_p_amount: u128,
        min_token_d_amount: u128,
        swap_indication: SwapIndication,
    ) -> Result<ExecutionFinalResult> {
        oracle.oracle_call(caller, self.0.id(), price_data, PriceReceiverMsg::MarginExecute {
            actions: vec![
                MarginAction::CloseMTPosition { 
                    pos_id: pos_id.clone(),
                    token_p_amount: token_p_amount.into(),
                    min_token_d_amount: min_token_d_amount.into(),
                    swap_indication: swap_indication,
                }
            ],
        }).await
    }

    pub async fn margin_trading_withdraw(
        &self,
        caller: &Account,
        token_id: &AccountId,
        amount: Option<U128>,
    ) -> Result<ExecutionFinalResult> {
        self.margin_execute_with_pyth(caller, vec![
            MarginAction::Withdraw { 
                token_id: near_sdk::AccountId::new_unchecked(token_id.to_string()),
                amount: amount,
            }
        ]).await
    }

    pub async fn margin_trading_close_mtposition_by_pyth(
        &self,
        caller: &Account,
        pos_id: &String,
        token_p_amount: u128,
        min_token_d_amount: u128,
        swap_indication: SwapIndication,
    ) -> Result<ExecutionFinalResult> {
        self.margin_execute_with_pyth(caller,  vec![
            MarginAction::CloseMTPosition {
                pos_id: pos_id.clone(),
                token_p_amount: token_p_amount.into(),
                min_token_d_amount: min_token_d_amount.into(),
                swap_indication: swap_indication,
            }
        ]).await
    }

    pub async fn margin_trading_liquidate_mtposition_by_oracle_call(
        &self,
        oracle: &Oralce,
        price_data: PriceData,
        caller: &Account,
        pos_owner_id: &AccountId,
        pos_id: &String,
        token_p_amount: u128,
        min_token_d_amount: u128,
        swap_indication: SwapIndication,
    ) -> Result<ExecutionFinalResult> {
        oracle.oracle_call(caller, self.0.id(), price_data, PriceReceiverMsg::MarginExecute {
            actions: vec![
                MarginAction::LiquidateMTPosition { 
                    pos_owner_id: near_sdk::AccountId::new_unchecked(pos_owner_id.to_string()),
                    pos_id: pos_id.clone(),
                    token_p_amount: token_p_amount.into(),
                    min_token_d_amount: min_token_d_amount.into(),
                    swap_indication: swap_indication,
                }
            ],
        }).await
    }

    pub async fn margin_trading_liquidate_mtposition_by_pyth(
        &self,
        caller: &Account,
        pos_owner_id: &AccountId,
        pos_id: &String,
        token_p_amount: u128,
        min_token_d_amount: u128,
        swap_indication: SwapIndication,
    ) -> Result<ExecutionFinalResult> {
        self.margin_execute_with_pyth(caller, vec![
            MarginAction::LiquidateMTPosition {
                pos_owner_id: near_sdk::AccountId::new_unchecked(pos_owner_id.to_string()),
                pos_id: pos_id.clone(),
                token_p_amount: token_p_amount.into(),
                min_token_d_amount: min_token_d_amount.into(),
                swap_indication: swap_indication,
            }
        ]).await
    }

    pub async fn margin_trading_force_close_mtposition_by_oracle_call(
        &self,
        oracle: &Oralce,
        price_data: PriceData,
        caller: &Account,
        pos_owner_id: &AccountId,
        pos_id: &String,
        token_p_amount: u128,
        min_token_d_amount: u128,
        swap_indication: SwapIndication,
    ) -> Result<ExecutionFinalResult> {
        oracle.oracle_call(caller, self.0.id(), price_data, PriceReceiverMsg::MarginExecute {
            actions: vec![
                MarginAction::ForceCloseMTPosition { 
                    pos_owner_id: near_sdk::AccountId::new_unchecked(pos_owner_id.to_string()),
                    pos_id: pos_id.clone(),
                    token_p_amount: token_p_amount.into(),
                    min_token_d_amount: min_token_d_amount.into(),
                    swap_indication: swap_indication,
                }
            ],
        }).await
    }

    pub async fn margin_trading_force_close_mtposition_by_pyth(
        &self,
        caller: &Account,
        pos_owner_id: &AccountId,
        pos_id: &String,
        token_p_amount: u128,
        min_token_d_amount: u128,
        swap_indication: SwapIndication,
    ) -> Result<ExecutionFinalResult> {
        self.margin_execute_with_pyth(caller, vec![
            MarginAction::ForceCloseMTPosition { 
                pos_owner_id: near_sdk::AccountId::new_unchecked(pos_owner_id.to_string()),
                pos_id: pos_id.clone(),
                token_p_amount: token_p_amount.into(),
                min_token_d_amount: min_token_d_amount.into(),
                swap_indication: swap_indication,
            }
        ]).await
    }

    pub async fn margin_trading_increase_collateral_by_ft_transfer_call(
        &self,
        token_contract: &FtContract,
        caller: &Account,
        ft_amount: u128,
        pos_id: &String,
        amount: u128,
    ) -> Result<ExecutionFinalResult> {
        token_contract.ft_transfer_call(caller, self.0.id(), ft_amount, serde_json::to_string(&TokenReceiverMsg::MarginExecute {
            actions: vec![
                MarginAction::IncreaseCollateral { pos_id: pos_id.clone(), amount: amount.into() }
            ]
        }).unwrap()).await
    }

    pub async fn margin_trading_decrease_collateral_by_oracle_call(
        &self,
        oracle: &Oralce,
        price_data: PriceData,
        caller: &Account,
        pos_id: &String,
        amount: u128,
    ) -> Result<ExecutionFinalResult> {
        oracle.oracle_call(caller, self.0.id(), price_data, PriceReceiverMsg::MarginExecute {
            actions: vec![
                MarginAction::DecreaseCollateral { 
                    pos_id: pos_id.clone(), amount: amount.into()
                }
            ],
        }).await
    }

    pub async fn margin_trading_decrease_collateral_by_pyth(
        &self,
        caller: &Account,
        pos_id: &String,
        amount: u128,
    ) -> Result<ExecutionFinalResult> {
        self.margin_execute_with_pyth(caller, vec![
            MarginAction::DecreaseCollateral { 
                pos_id: pos_id.clone(), amount: amount.into()
            }
        ]).await
    }

}



impl Burrowland {
    pub async fn get_margin_account(
        &self,
        account: &Account
    ) -> Result<Option<MarginAccountDetailedView>> {
        self.0
            .call("get_margin_account")
            .args_json(json!({
                "account_id": account.id()
            }))
            .view()
            .await?
            .json::<Option<MarginAccountDetailedView>>()
    }

}