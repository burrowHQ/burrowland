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
            .gas(20_000_000_000_000)
            .deposit(parse_near!("1 N"))
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
            .deposit(1)
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
            .deposit(1)
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
            .deposit(1)
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
            .deposit(1)
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
            .deposit(1)
            .transact()
            .await
    }

    // pub async fn position_borrow_and_withdraw(
    //     &self,
    //     caller: &Account,
    //     oracle: &Oralce,
    //     receiver_id: &AccountId,
    //     price_data: PriceData,
    //     position: Option<String>,
    //     token_id: &AccountId,
    //     borrow_amount: u128,
    //     withdraw_amount: u128
    // ) -> Result<ExecutionFinalResult> {
    //     oracle.oracle_call(caller, receiver_id, price_data, PriceReceiverMsg::Execute {
    //         actions: vec![
    //             Action::PositionBorrow{
    //                 position,
    //                 asset_amount: asset_amount(token_id, borrow_amount)
    //             },
    //             Action::Withdraw(asset_amount(token_id, withdraw_amount)),
    //         ],
    //     }).await
    // }

    pub async fn liquidate(
        &self,
        caller: &Account,
        oracle: &Oralce,
        receiver_id: &AccountId,
        liquidation_account_id: &AccountId,
        price_data: PriceData,
        in_assets: Vec<AssetAmount>,
        out_assets: Vec<AssetAmount>,
    ) -> Result<ExecutionFinalResult> {
        oracle.oracle_call(caller, receiver_id, price_data, PriceReceiverMsg::Execute {
            actions: vec![
                Action::Liquidate { 
                    account_id: near_sdk::AccountId::new_unchecked(liquidation_account_id.to_string()), 
                    in_assets, out_assets },
            ],
        }).await
    }

    pub async fn force_close(
        &self,
        caller: &Account,
        oracle: &Oralce,
        receiver_id: &AccountId,
        force_close_account_id: &AccountId,
        price_data: PriceData,
    ) -> Result<ExecutionFinalResult> {
        oracle.oracle_call(caller, receiver_id, price_data, PriceReceiverMsg::Execute {
            actions: vec![
                Action::ForceClose { 
                    account_id: near_sdk::AccountId::new_unchecked(force_close_account_id.to_string()), 
                },
            ],
        }).await
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
            .deposit(1)
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
            .deposit(1)
            .transact()
            .await
    }
}

impl Burrowland {
    pub async fn get_asset(
        &self,
        token: &FtContract
    ) -> Result<AssetDetailedView> {
        self.0
            .call("get_asset")
            .args_json(json!({
                "token_id": token.0.id()
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

    pub async fn get_config(
        &self,
    ) -> Result<Config> {
        self.0
            .call("get_config")
            .view()
            .await?
            .json::<Config>()
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
                volatility_ratio: 2000,
                extra_decimals: 0,
                can_deposit: true,
                can_withdraw: true,
                can_use_as_collateral: false,
                can_borrow: false,
                net_tvl_multiplier: 10000,
            },
            "wrap.test.near" => AssetConfig {
                reserve_ratio: 2500,
                prot_ratio: 0,
                target_utilization: 8000,
                target_utilization_rate: U128(1000000000003593629036885046),
                max_utilization_rate: U128(1000000000039724853136740579),
                volatility_ratio: 6000,
                extra_decimals: 0,
                can_deposit: true,
                can_withdraw: true,
                can_use_as_collateral: true,
                can_borrow: true,
                net_tvl_multiplier: 10000,
            },
            "neth.test.near" => AssetConfig {
                reserve_ratio: 2500,
                prot_ratio: 0,
                target_utilization: 8000,
                target_utilization_rate: U128(1000000000001547125956667610),
                max_utilization_rate: U128(1000000000039724853136740579),
                volatility_ratio: 6000,
                extra_decimals: 0,
                can_deposit: true,
                can_withdraw: true,
                can_use_as_collateral: true,
                can_borrow: true,
                net_tvl_multiplier: 10000,
            },
            "ndai.test.near" => AssetConfig {
                reserve_ratio: 2500,
                prot_ratio: 0,
                target_utilization: 8000,
                target_utilization_rate: U128(1000000000002440418605283556),
                max_utilization_rate: U128(1000000000039724853136740579),
                volatility_ratio: 9500,
                extra_decimals: 0,
                can_deposit: true,
                can_withdraw: true,
                can_use_as_collateral: true,
                can_borrow: true,
                net_tvl_multiplier: 10000,
            },
            "nusdt.test.near" => AssetConfig {
                reserve_ratio: 2500,
                prot_ratio: 0,
                target_utilization: 8000,
                target_utilization_rate: U128(1000000000002440418605283556),
                max_utilization_rate: U128(1000000000039724853136740579),
                volatility_ratio: 9500,
                extra_decimals: 12,
                can_deposit: true,
                can_withdraw: true,
                can_use_as_collateral: true,
                can_borrow: true,
                net_tvl_multiplier: 10000,
            },
            "nusdc.test.near" => AssetConfig {
                reserve_ratio: 2500,
                prot_ratio: 0,
                target_utilization: 8000,
                target_utilization_rate: U128(1000000000002440418605283556),
                max_utilization_rate: U128(1000000000039724853136740579),
                volatility_ratio: 9500,
                extra_decimals: 12,
                can_deposit: true,
                can_withdraw: true,
                can_use_as_collateral: true,
                can_borrow: true,
                net_tvl_multiplier: 10000,
            },
            _ => {
                panic!("unsupported token: {:?}", token_id);
            }
        };
        self.add_asset(root, token_id, asset_config).await
    }
}