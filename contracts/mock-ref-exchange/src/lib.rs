use std::convert::TryInto;
use std::fmt;

use near_contract_standards::storage_management::{
    StorageBalance, StorageBalanceBounds, StorageManagement,
};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, UnorderedSet, Vector, UnorderedMap};
use near_sdk::json_types::U128;
use near_sdk::{
    assert_one_yocto, env, log, near_bindgen, AccountId, Balance, PanicOnDefault, Promise,
    PromiseResult, StorageUsage, BorshStorageKey, PromiseOrValue, ext_contract
};
use utils::GAS_FOR_BASIC_OP;

use crate::account_deposit::{VAccount, Account};
pub use crate::action::{SwapAction, Action, ActionResult, get_tokens_in_actions};
use crate::errors::*;
use crate::admin_fee::AdminFees;
use crate::pool::Pool;
use crate::simple_pool::SimplePool;
use crate::stable_swap::StableSwapPool;
use crate::rated_swap::{RatedSwapPool, rate::{RateTrait, global_get_rate, global_set_rate}};
use crate::utils::{check_token_duplicates, TokenCache};
pub use crate::custom_keys::*;
pub use crate::views::{PoolInfo, RatedPoolInfo, ContractMetadata, RatedTokenInfo, AddLiquidityPrediction};
pub use crate::token_receiver::AddLiquidityInfo;

mod account_deposit;
mod action;
mod errors;
mod admin_fee;
mod legacy;
mod multi_fungible_token;
mod owner;
mod pool;
mod simple_pool;
mod stable_swap;
mod rated_swap;
mod storage_impl;
mod token_receiver;
mod utils;
mod views;
mod custom_keys;

#[derive(BorshStorageKey, BorshSerialize)]
pub(crate) enum StorageKey {
    Pools,
    Accounts,
    Shares { pool_id: u32 },
    Whitelist,
    Guardian,
    AccountTokens {account_id: AccountId},
    Frozenlist,
    Referral,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Eq, PartialEq, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
pub enum RunningState {
    Running, Paused
}

impl fmt::Display for RunningState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RunningState::Running => write!(f, "Running"),
            RunningState::Paused => write!(f, "Paused"),
        }
    }
}

#[ext_contract(ext_self)]
pub trait SelfCallbacks {
    fn update_token_rate_callback(&mut self, token_id: AccountId);
}

#[near_bindgen]
#[derive(BorshSerialize, BorshDeserialize, PanicOnDefault)]
pub struct Contract {
    /// Account of the owner.
    owner_id: AccountId,
    /// Admin fee rate in total fee.
    admin_fee_bps: u32,
    /// List of all the pools.
    pools: Vector<Pool>,
    /// Accounts registered, keeping track all the amounts deposited, storage and more.
    accounts: LookupMap<AccountId, VAccount>,
    /// Set of whitelisted tokens by "owner".
    whitelisted_tokens: UnorderedSet<AccountId>,
    /// Set of guardians.
    guardians: UnorderedSet<AccountId>,
    /// Running state
    state: RunningState,
    /// Set of frozenlist tokens
    frozen_tokens: UnorderedSet<AccountId>,
    /// Map of referrals
    referrals: UnorderedMap<AccountId, u32>,
}

#[near_bindgen]
impl Contract {
    #[init]
    pub fn new(owner_id: AccountId, exchange_fee: u32, referral_fee: u32) -> Self {
        Self {
            owner_id: owner_id.clone(),
            admin_fee_bps: exchange_fee + referral_fee,
            pools: Vector::new(StorageKey::Pools),
            accounts: LookupMap::new(StorageKey::Accounts),
            whitelisted_tokens: UnorderedSet::new(StorageKey::Whitelist),
            guardians: UnorderedSet::new(StorageKey::Guardian),
            state: RunningState::Running,
            frozen_tokens: UnorderedSet::new(StorageKey::Frozenlist),
            referrals: UnorderedMap::new(StorageKey::Referral),
        }
    }

    /// Adds new "Simple Pool" with given tokens and given fee.
    /// Attached NEAR should be enough to cover the added storage.
    #[payable]
    pub fn add_simple_pool(&mut self, tokens: Vec<AccountId>, fee: u32) -> u64 {
        self.assert_contract_running();
        check_token_duplicates(&tokens);
        self.internal_add_pool(Pool::SimplePool(SimplePool::new(
            self.pools.len() as u32,
            tokens,
            fee,
        )))
    }

    /// Adds new "Stable Pool" with given tokens, decimals, fee and amp.
    /// It is limited to owner or guardians, cause a complex and correct config is needed.
    /// tokens: pool tokens in this stable swap.
    /// decimals: each pool tokens decimal, needed to make them comparable.
    /// fee: total fee of the pool, admin fee is inclusive.
    /// amp_factor: algorithm parameter, decide how stable the pool will be.
    #[payable]
    pub fn add_stable_swap_pool(
        &mut self,
        tokens: Vec<AccountId>,
        decimals: Vec<u8>,
        fee: u32,
        amp_factor: u64,
    ) -> u64 {
        assert!(self.is_owner_or_guardians(), "{}", ERR100_NOT_ALLOWED);
        check_token_duplicates(&tokens);
        self.internal_add_pool(Pool::StableSwapPool(StableSwapPool::new(
            self.pools.len() as u32,
            tokens,
            decimals,
            amp_factor as u128,
            fee,
        )))
    }

    ///
    #[payable]
    pub fn add_rated_swap_pool(
        &mut self,
        tokens: Vec<AccountId>,
        decimals: Vec<u8>,
        fee: u32,
        amp_factor: u64,
    ) -> u64 {
        assert!(self.is_owner_or_guardians(), "{}", ERR100_NOT_ALLOWED);
        check_token_duplicates(&tokens);
        self.internal_add_pool(Pool::RatedSwapPool(RatedSwapPool::new(
            self.pools.len() as u32,
            tokens,
            decimals,
            amp_factor as u128,
            fee,
        )))
    }

    /// [AUDIT_03_reject(NOPE action is allowed by design)]
    /// [AUDIT_04]
    /// Executes generic set of actions.
    /// If referrer provided, pays referral_fee to it.
    /// If no attached deposit, outgoing tokens used in swaps must be whitelisted.
    #[payable]
    pub fn execute_actions(
        &mut self,
        actions: Vec<Action>,
        referral_id: Option<AccountId>,
    ) -> ActionResult {
        self.assert_contract_running();
        let sender_id = env::predecessor_account_id();
        let mut account = self.internal_unwrap_account(&sender_id);
        // Validate that all tokens are whitelisted if no deposit (e.g. trade with access key).
        if env::attached_deposit() == 0 {
            for action in &actions {
                for token in action.tokens() {
                    assert!(
                        account.get_balance(&token).is_some() 
                            || self.whitelisted_tokens.contains(&token),
                        "{}",
                        // [AUDIT_05]
                        ERR27_DEPOSIT_NEEDED
                    );
                }
            }
        }

        let referral_info :Option<(AccountId, u32)> = referral_id
            .as_ref().and_then(|rid| self.referrals.get(&rid))
            .map(|fee| (referral_id.unwrap().into(), fee));
        
        let result =
            self.internal_execute_actions(&mut account, &referral_info, &actions, ActionResult::None);
        self.internal_save_account(&sender_id, account);
        result
    }

    /// Execute set of swap actions between pools.
    /// If referrer provided, pays referral_fee to it.
    /// If no attached deposit, outgoing tokens used in swaps must be whitelisted.
    #[payable]
    pub fn swap(&mut self, actions: Vec<SwapAction>, referral_id: Option<AccountId>) -> U128 {
        self.assert_contract_running();
        assert_ne!(actions.len(), 0, "{}", ERR72_AT_LEAST_ONE_SWAP);
        U128(
            self.execute_actions(
                actions
                    .into_iter()
                    .map(|swap_action| Action::Swap(swap_action))
                    .collect(),
                referral_id,
            )
            .to_amount(),
        )
    }

    /// Add liquidity from already deposited amounts to given pool.
    #[payable]
    pub fn add_liquidity(
        &mut self,
        pool_id: u64,
        amounts: Vec<U128>,
        min_amounts: Option<Vec<U128>>,
    ) -> U128 {
        self.assert_contract_running();
        assert!(
            env::attached_deposit() > 0,
            "{}", ERR35_AT_LEAST_ONE_YOCTO
        );
        let prev_storage = env::storage_usage();
        let sender_id = env::predecessor_account_id();
        let mut amounts: Vec<u128> = amounts.into_iter().map(|amount| amount.into()).collect();
        let mut pool = self.pools.get(pool_id).expect(ERR85_NO_POOL);
        // feature frozenlist
        self.assert_no_frozen_tokens(pool.tokens());
        // Add amounts given to liquidity first. It will return the balanced amounts.
        let shares = pool.add_liquidity(
            &sender_id,
            &mut amounts,
            false
        );
        if let Some(min_amounts) = min_amounts {
            // Check that all amounts are above request min amounts in case of front running that changes the exchange rate.
            for (amount, min_amount) in amounts.iter().zip(min_amounts.iter()) {
                assert!(amount >= &min_amount.0, "{}", ERR86_MIN_AMOUNT);
            }
        }
        // [AUDITION_AMENDMENT] 2.3.7 Code Optimization (I)
        let mut deposits = self.internal_unwrap_account(&sender_id);
        let tokens = pool.tokens();
        // Subtract updated amounts from deposits. This will fail if there is not enough funds for any of the tokens.
        for i in 0..tokens.len() {
            deposits.withdraw(&tokens[i], amounts[i]);
        }
        self.internal_save_account(&sender_id, deposits);
        self.pools.replace(pool_id, &pool);
        self.internal_check_storage(prev_storage);

        U128(shares)
    }

    /// For stable swap pool, user can add liquidity with token's combination as his will.
    /// But there is a little fee according to the bias of token's combination with the one in the pool.
    /// pool_id: stable pool id. If simple pool is given, panic with unimplement.
    /// amounts: token's combination (in pool tokens sequence) user want to add into the pool, a 0 means absent of that token.
    /// min_shares: Slippage, if shares mint is less than it (cause of fee for too much bias), panic with  ERR68_SLIPPAGE
    #[payable]
    pub fn add_stable_liquidity(
        &mut self,
        pool_id: u64,
        amounts: Vec<U128>,
        min_shares: U128,
    ) -> U128 {
        self.assert_contract_running();
        assert!(
            env::attached_deposit() > 0,
            "{}", ERR35_AT_LEAST_ONE_YOCTO
        );
        let prev_storage = env::storage_usage();
        let sender_id = env::predecessor_account_id();
        let amounts: Vec<u128> = amounts.into_iter().map(|amount| amount.into()).collect();
        let mut pool = self.pools.get(pool_id).expect(ERR85_NO_POOL);
        // feature frozenlist
        self.assert_no_frozen_tokens(pool.tokens());
        // Add amounts given to liquidity first. It will return the balanced amounts.
        let mint_shares = pool.add_stable_liquidity(
            &sender_id,
            &amounts,
            min_shares.into(),
            AdminFees::new(self.admin_fee_bps),
            false
        );
        // [AUDITION_AMENDMENT] 2.3.7 Code Optimization (I)
        let mut deposits = self.internal_unwrap_account(&sender_id);
        let tokens = pool.tokens();
        // Subtract amounts from deposits. This will fail if there is not enough funds for any of the tokens.
        for i in 0..tokens.len() {
            deposits.withdraw(&tokens[i], amounts[i]);
        }
        self.internal_save_account(&sender_id, deposits);
        self.pools.replace(pool_id, &pool);
        self.internal_check_storage(prev_storage);

        mint_shares.into()
    }

    // #[payable]
    // pub fn add_rated_liquidity(
    //     &mut self,
    //     pool_id: u64,
    //     amounts: Vec<U128>,
    //     min_shares: U128,
    // ) -> U128 {
    //     self.add_stable_liquidity(pool_id, amounts, min_shares)
    // }

    /// Remove liquidity from the pool into general pool of liquidity.
    #[payable]
    pub fn remove_liquidity(&mut self, pool_id: u64, shares: U128, min_amounts: Vec<U128>) -> Vec<U128> {
        assert_one_yocto();
        self.assert_contract_running();
        let prev_storage = env::storage_usage();
        let sender_id = env::predecessor_account_id();
        let mut pool = self.pools.get(pool_id).expect(ERR85_NO_POOL);
        // feature frozenlist
        self.assert_no_frozen_tokens(pool.tokens());
        let amounts = pool.remove_liquidity(
            &sender_id,
            shares.into(),
            min_amounts
                .into_iter()
                .map(|amount| amount.into())
                .collect(),
            false
        );
        self.pools.replace(pool_id, &pool);
        let tokens = pool.tokens();
        // [AUDITION_AMENDMENT] 2.3.7 Code Optimization (I)
        let mut deposits = self.internal_unwrap_account(&sender_id);
        for i in 0..tokens.len() {
            deposits.deposit(&tokens[i], amounts[i]);
        }
        // Freed up storage balance from LP tokens will be returned to near_balance.
        if prev_storage > env::storage_usage() {
            deposits.near_amount +=
                (prev_storage - env::storage_usage()) as Balance * env::storage_byte_cost();
        }
        self.internal_save_account(&sender_id, deposits);

        amounts
            .into_iter()
            .map(|amount| amount.into())
            .collect()
    }

    /// For stable swap pool, LP can use it to remove liquidity with given token amount and distribution.
    /// pool_id: the stable swap pool id. If simple pool is given, panic with Unimplement.
    /// amounts: Each tokens (in pool tokens sequence) amounts user want get, a 0 means user don't want to get that token back.
    /// max_burn_shares: This is slippage protection, if user request would burn shares more than it, panic with ERR68_SLIPPAGE
    #[payable]
    pub fn remove_liquidity_by_tokens(
        &mut self, pool_id: u64, 
        amounts: Vec<U128>, 
        max_burn_shares: U128
    ) -> U128 {
        assert_one_yocto();
        self.assert_contract_running();
        let prev_storage = env::storage_usage();
        let sender_id = env::predecessor_account_id();
        let mut pool = self.pools.get(pool_id).expect(ERR85_NO_POOL);
        // feature frozenlist
        self.assert_no_frozen_tokens(pool.tokens());
        let burn_shares = pool.remove_liquidity_by_tokens(
            &sender_id,
            amounts
                .clone()
                .into_iter()
                .map(|amount| amount.into())
                .collect(),
            max_burn_shares.into(),
            AdminFees::new(self.admin_fee_bps),
            false
        );
        self.pools.replace(pool_id, &pool);
        let tokens = pool.tokens();
        // [AUDITION_AMENDMENT] 2.3.7 Code Optimization (I)
        let mut deposits = self.internal_unwrap_account(&sender_id);
        for i in 0..tokens.len() {
            deposits.deposit(&tokens[i], amounts[i].into());
        }
        // Freed up storage balance from LP tokens will be returned to near_balance.
        if prev_storage > env::storage_usage() {
            deposits.near_amount +=
                (prev_storage - env::storage_usage()) as Balance * env::storage_byte_cost();
        }
        self.internal_save_account(&sender_id, deposits);

        burn_shares.into()
    }

    /// anyone can trigger an update for some rated token
    pub fn update_token_rate(& self, token_id: AccountId) -> PromiseOrValue<bool> {
        let caller = env::predecessor_account_id();
        let token_id: AccountId = token_id.into();
        if let Some(rate) = global_get_rate(&token_id) {
            log!("Caller {} invokes token {} rait async-update.", caller, token_id);
            rate.async_update().then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_FOR_BASIC_OP)
                    .update_token_rate_callback(
                        token_id,
                    )
            ).into()
        } else {
            log!("Caller {} invokes token {} rait async-update but it is not a valid token.", caller, token_id);
            PromiseOrValue::Value(true)
        }
    }

    /// the async return of update_token_rate
    #[private]
    pub fn update_token_rate_callback(&mut self, token_id: AccountId) {
        assert_eq!(env::promise_results_count(), 1, "{}", ERR123_ONE_PROMISE_RESULT);
        let cross_call_result = match env::promise_result(0) {
            PromiseResult::Successful(result) => result,
            _ => env::panic_str(ERR124_CROSS_CALL_FAILED),
        };

        if let Some(mut rate) = global_get_rate(&token_id) {
            let new_rate = rate.set(&cross_call_result);
            global_set_rate(&token_id, &rate);
            log!(
                "Token {} got new rate {} from cross-contract call.",
                token_id, new_rate
            );
        }
    }
}

/// Internal methods implementation.
impl Contract {

    fn assert_contract_running(&self) {
        match self.state {
            RunningState::Running => (),
            _ => env::panic_str(ERR51_CONTRACT_PAUSED),
        };
    }

    fn assert_no_frozen_tokens(&self, tokens: &[AccountId]) {
        let frozens: Vec<&AccountId> = tokens.iter()
        .filter(
            |token| self.frozen_tokens.contains(*token)
        )
        .collect();
        assert_eq!(frozens.len(), 0, "{}", ERR52_FROZEN_TOKEN);
    }

    /// Check how much storage taken costs and refund the left over back.
    fn internal_check_storage(&self, prev_storage: StorageUsage) {
        let storage_cost = env::storage_usage()
            .checked_sub(prev_storage)
            .unwrap_or_default() as Balance
            * env::storage_byte_cost();

        let refund = env::attached_deposit()
            .checked_sub(storage_cost)
            .expect(
                format!(
                    "ERR_STORAGE_DEPOSIT need {}, attatched {}", 
                    storage_cost, env::attached_deposit()
                ).as_str()
            );
        if refund > 0 {
            Promise::new(env::predecessor_account_id()).transfer(refund);
        }
    }

    /// Adds given pool to the list and returns it's id.
    /// If there is not enough attached balance to cover storage, fails.
    /// If too much attached - refunds it back.
    fn internal_add_pool(&mut self, mut pool: Pool) -> u64 {
        let prev_storage = env::storage_usage();
        let id = self.pools.len() as u64;
        // exchange share was registered at creation time
        pool.share_register(&env::current_account_id());
        self.pools.push(&pool);
        self.internal_check_storage(prev_storage);
        id
    }

    /// Execute sequence of actions on given account. Modifies passed account.
    /// Returns result of the last action.
    fn internal_execute_actions(
        &mut self,
        account: &mut Account,
        referral_info: &Option<(AccountId, u32)>,
        actions: &[Action],
        prev_result: ActionResult,
    ) -> ActionResult {
        // fronzen token feature
        // [AUDITION_AMENDMENT] 2.3.8 Code Optimization (II)
        self.assert_no_frozen_tokens(
            &get_tokens_in_actions(actions)
            .into_iter()
            .map(|token| token)
            .collect::<Vec<AccountId>>()
        );

        let mut result = prev_result;
        for action in actions {
            result = self.internal_execute_action(account, referral_info, action, result);
        }
        result
    }

    /// Executes single action on given account. Modifies passed account. Returns a result based on type of action.
    fn internal_execute_action(
        &mut self,
        account: &mut Account,
        referral_info: &Option<(AccountId, u32)>,
        action: &Action,
        prev_result: ActionResult,
    ) -> ActionResult {
        match action {
            Action::Swap(swap_action) => {
                let amount_in = swap_action
                    .amount_in
                    .map(|value| value.0)
                    .unwrap_or_else(|| prev_result.to_amount());
                account.withdraw(&swap_action.token_in, amount_in);
                let amount_out = self.internal_pool_swap(
                    swap_action.pool_id,
                    &swap_action.token_in,
                    amount_in,
                    &swap_action.token_out,
                    swap_action.min_amount_out.0,
                    referral_info,
                );
                account.deposit(&swap_action.token_out, amount_out);
                // [AUDIT_02]
                ActionResult::Amount(U128(amount_out))
            }
        }
    }

    /// Swaps given amount_in of token_in into token_out via given pool.
    /// Should be at least min_amount_out or swap will fail (prevents front running and other slippage issues).
    fn internal_pool_swap(
        &mut self,
        pool_id: u64,
        token_in: &AccountId,
        amount_in: u128,
        token_out: &AccountId,
        min_amount_out: u128,
        referral_info: &Option<(AccountId, u32)>,
    ) -> u128 {
        let mut pool = self.pools.get(pool_id).expect(ERR85_NO_POOL);
        let amount_out = pool.swap(
            token_in,
            amount_in,
            token_out,
            min_amount_out,
            AdminFees {
                admin_fee_bps: self.admin_fee_bps,
                exchange_id: env::current_account_id(),
                referral_info: referral_info.clone(),
            },
            false
        );
        self.pools.replace(pool_id, &pool);
        amount_out
    }
}

use std::collections::HashMap;

impl Contract {
    fn internal_execute_actions_by_cache(
        &self,
        pool_cache: &mut HashMap<u64, Pool>,
        token_cache: &mut TokenCache,
        referral_info: &Option<(AccountId, u32)>,
        actions: &[Action],
        prev_result: ActionResult,
    ) {
        self.assert_no_frozen_tokens(
            &get_tokens_in_actions(actions)
            .into_iter()
            .map(|token| token)
            .collect::<Vec<AccountId>>()
        );

        let mut result = prev_result;
        for action in actions {
            result = self.internal_execute_action_by_cache(pool_cache, token_cache, referral_info, action, result);
        }
    }

    fn internal_execute_action_by_cache(
        &self,
        pool_cache: &mut HashMap<u64, Pool>,
        token_cache: &mut TokenCache,
        referral_info: &Option<(AccountId, u32)>,
        action: &Action,
        prev_result: ActionResult,
    ) -> ActionResult {
        match action {
            Action::Swap(swap_action) => {
                let amount_in = swap_action
                    .amount_in
                    .map(|value| value.0)
                    .unwrap_or_else(|| prev_result.to_amount());
                token_cache.sub(&swap_action.token_in, amount_in);
                let amount_out = self.internal_pool_swap_by_cache(
                    pool_cache,
                    swap_action.pool_id,
                    &swap_action.token_in,
                    amount_in,
                    &swap_action.token_out,
                    0,
                    referral_info,
                );
                token_cache.add(&swap_action.token_out, amount_out);
                ActionResult::Amount(U128(amount_out))
            }
        }
    }

    fn internal_pool_swap_by_cache(
        &self,
        pool_cache: &mut HashMap<u64, Pool>,
        pool_id: u64,
        token_in: &AccountId,
        amount_in: u128,
        token_out: &AccountId,
        min_amount_out: u128,
        referral_info: &Option<(AccountId, u32)>,
    ) -> u128 {
        let mut pool = pool_cache.remove(&pool_id).unwrap_or(self.pools.get(pool_id).expect(ERR85_NO_POOL));
        let amount_out = pool.swap(
            token_in,
            amount_in,
            token_out,
            min_amount_out,
            AdminFees {
                admin_fee_bps: self.admin_fee_bps,
                exchange_id: env::current_account_id(),
                referral_info: referral_info.clone(),
            },
            true
        );
        pool_cache.insert(pool_id, pool);
        amount_out
    }
}
