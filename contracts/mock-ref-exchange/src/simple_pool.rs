use std::cmp::min;

use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::LookupMap;
use near_sdk::{env, AccountId, Balance, log};
use crate::StorageKey;
use crate::admin_fee::AdminFees;

use crate::errors::*;
use crate::utils::{
    add_to_collection, integer_sqrt, SwapVolume, FEE_DIVISOR, INIT_SHARES_SUPPLY, U256, u128_ratio,
};

const NUM_TOKENS: usize = 2;

/// Implementation of simple pool, that maintains constant product between balances of all the tokens.
/// Similar in design to "Uniswap".
/// Liquidity providers when depositing receive shares, that can be later burnt to withdraw pool's tokens in proportion.
#[derive(BorshSerialize, BorshDeserialize)]
pub struct SimplePool {
    /// List of tokens in the pool.
    pub token_account_ids: Vec<AccountId>,
    /// How much NEAR this contract has.
    pub amounts: Vec<Balance>,
    /// Volumes accumulated by this pool.
    pub volumes: Vec<SwapVolume>,
    /// Fee charged for swap (gets divided by FEE_DIVISOR).
    pub total_fee: u32,
    /// Obsolete, reserve to simplify upgrade.
    pub exchange_fee: u32,
    /// Obsolete, reserve to simplify upgrade.
    pub referral_fee: u32,
    /// Shares of the pool by liquidity providers.
    pub shares: LookupMap<AccountId, Balance>,
    /// Total number of shares.
    pub shares_total_supply: Balance,
}

impl SimplePool {
    pub fn new(
        id: u32,
        token_account_ids: Vec<AccountId>,
        total_fee: u32,
    ) -> Self {
        assert!(
            total_fee < FEE_DIVISOR,
            "{}", ERR90_FEE_TOO_LARGE
        );
        // [AUDIT_10]
        assert_eq!(token_account_ids.len(), NUM_TOKENS, "{}", ERR89_WRONG_TOKEN_COUNT);
        Self {
            token_account_ids: token_account_ids.iter().map(|a| a.clone().into()).collect(),
            amounts: vec![0u128; NUM_TOKENS],
            volumes: vec![SwapVolume::default(); NUM_TOKENS],
            total_fee,
            exchange_fee: 0,
            referral_fee: 0,
            // [AUDIT_11]
            shares: LookupMap::new(StorageKey::Shares {
                pool_id: id,
            }),
            shares_total_supply: 0,
        }
    }

    pub fn modify_total_fee(&mut self, total_fee: u32) {
        self.total_fee = total_fee;
    }

    /// See if the given account has been registered as a LP
    pub fn share_has_registered(&self, account_id: &AccountId) -> bool {
        self.shares.contains_key(account_id)
    }

    /// Register given account with 0 balance in shares.
    /// Storage payment should be checked by caller.
    pub fn share_register(&mut self, account_id: &AccountId) {
        if self.shares.contains_key(account_id) {
            env::panic_str(ERR14_LP_ALREADY_REGISTERED);
        }
        self.shares.insert(account_id, &0);
    }

    /// Transfers shares from predecessor to receiver.
    pub fn share_transfer(&mut self, sender_id: &AccountId, receiver_id: &AccountId, amount: u128) {
        let balance = self.shares.get(&sender_id).expect(ERR13_LP_NOT_REGISTERED);
        if let Some(new_balance) = balance.checked_sub(amount) {
            self.shares.insert(&sender_id, &new_balance);
        } else {
            env::panic_str(ERR91_NOT_ENOUGH_SHARES);
        }
        let balance_out = self
            .shares
            .get(&receiver_id)
            .expect(ERR13_LP_NOT_REGISTERED);
        self.shares.insert(&receiver_id, &(balance_out + amount));
    }

    /// Returns balance of shares for given user.
    pub fn share_balance_of(&self, account_id: &AccountId) -> Balance {
        self.shares.get(account_id).unwrap_or_default()
    }

    /// Returns total number of shares in this pool.
    pub fn share_total_balance(&self) -> Balance {
        self.shares_total_supply
    }

    /// Returns list of tokens in this pool.
    pub fn tokens(&self) -> &[AccountId] {
        &self.token_account_ids
    }

    /// Adds the amounts of tokens to liquidity pool and returns number of shares that this user receives.
    /// Updates amount to amount kept in the pool.
    pub fn add_liquidity(&mut self, sender_id: &AccountId, amounts: &mut Vec<Balance>, is_view: bool) -> Balance {
        assert_eq!(
            amounts.len(),
            NUM_TOKENS,
            "{}", ERR89_WRONG_AMOUNT_COUNT
        );
        let shares = if self.shares_total_supply > 0 {
            let mut fair_supply = U256::max_value();
            for i in 0..self.token_account_ids.len() {
                assert!(amounts[i] > 0, "{}", ERR31_ZERO_AMOUNT);
                fair_supply = min(
                    fair_supply,
                    U256::from(amounts[i] - 1) * U256::from(self.shares_total_supply) / self.amounts[i],
                );
            }
            for i in 0..self.token_account_ids.len() {
                let amount = (U256::from(self.amounts[i]) * fair_supply
                    / U256::from(self.shares_total_supply))
                .as_u128() + 1;
                assert!(amount > 0, "{}", ERR31_ZERO_AMOUNT);
                self.amounts[i] += amount;
                amounts[i] = amount;
            }
            fair_supply.as_u128()
        } else {
            for i in 0..self.token_account_ids.len() {
                assert!(amounts[i] > 0, "{}", ERR31_ZERO_AMOUNT);
                self.amounts[i] += amounts[i];
            }
            INIT_SHARES_SUPPLY
        };
        self.mint_shares(&sender_id, shares, is_view);
        assert!(shares > 0, "{}", ERR32_ZERO_SHARES);
        if !is_view {
            log!("{}",
                format!(
                    "Liquidity added {:?}, minted {} shares",
                    amounts
                        .iter()
                        .zip(self.token_account_ids.iter())
                        .map(|(amount, token_id)| format!("{} {}", amount, token_id))
                        .collect::<Vec<String>>(),
                    shares
                )
            );
        }
        shares
    }

    /// Mint new shares for given user.
    fn mint_shares(&mut self, account_id: &AccountId, shares: Balance, is_view: bool) {
        if shares == 0 {
            return;
        }
        self.shares_total_supply = self.shares_total_supply.checked_add(shares).expect(ERR36_SHARES_TOTAL_SUPPLY_OVERFLOW);
        if !is_view {
            add_to_collection(&mut self.shares, &account_id, shares);
        }
    }

    /// Removes given number of shares from the pool and returns amounts to the parent.
    pub fn remove_liquidity(
        &mut self,
        sender_id: &AccountId,
        shares: Balance,
        min_amounts: Vec<Balance>,
        is_view: bool
    ) -> Vec<Balance> {
        assert_eq!(
            min_amounts.len(),
            NUM_TOKENS,
            "{}", ERR89_WRONG_AMOUNT_COUNT
        );
        if !is_view {
            let prev_shares_amount = self.shares.get(&sender_id).expect(ERR13_LP_NOT_REGISTERED);
            assert!(prev_shares_amount >= shares, "{}", ERR91_NOT_ENOUGH_SHARES);
            if prev_shares_amount == shares {
                // [AUDIT_13] Never unregister a LP when he removed all his liquidity.
                self.shares.insert(&sender_id, &0);
            } else {
                self.shares
                    .insert(&sender_id, &(prev_shares_amount - shares));
            }
        }
        let mut result = vec![];
        for i in 0..self.token_account_ids.len() {
            let amount = (U256::from(self.amounts[i]) * U256::from(shares)
                / U256::from(self.shares_total_supply))
            .as_u128();
            assert!(amount >= min_amounts[i], "{}", ERR68_SLIPPAGE);
            self.amounts[i] -= amount;
            result.push(amount);
        }
        self.shares_total_supply -= shares;
        if !is_view {
            log!("{}",
                format!(
                    "{} shares of liquidity removed: receive back {:?}",
                    shares,
                    result
                        .iter()
                        .zip(self.token_account_ids.iter())
                        .map(|(amount, token_id)| format!("{} {}", amount, token_id))
                        .collect::<Vec<String>>(),
                )
            );
        }
        result
    }

    /// Returns token index for given pool.
    fn token_index(&self, token_id: &AccountId) -> usize {
        self.token_account_ids
            .iter()
            .position(|id| id == token_id)
            .expect(ERR102_INVALID_TOKEN_ID)
    }

    /// Returns number of tokens in outcome, given amount.
    /// Tokens are provided as indexes into token list for given pool.
    fn internal_get_return(
        &self,
        token_in: usize,
        amount_in: Balance,
        token_out: usize,
    ) -> Balance {
        let in_balance = U256::from(self.amounts[token_in]);
        let out_balance = U256::from(self.amounts[token_out]);
        assert!(
            in_balance > U256::zero()
                && out_balance > U256::zero()
                && token_in != token_out
                && amount_in > 0,
            "{}", ERR76_INVALID_PARAMS
        );
        let amount_with_fee = U256::from(amount_in) * U256::from(FEE_DIVISOR - self.total_fee);
        (amount_with_fee * out_balance / (U256::from(FEE_DIVISOR) * in_balance + amount_with_fee))
            .as_u128()
    }

    /// Returns given pool's total fee.
    pub fn get_fee(&self) -> u32 {
        self.total_fee
    }

    /// Returns volumes of the given pool.
    pub fn get_volumes(&self) -> Vec<SwapVolume> {
        self.volumes.clone()
    }

    /// Swap `token_amount_in` of `token_in` token into `token_out` and return how much was received.
    /// Assuming that `token_amount_in` was already received from `sender_id`.
    pub fn swap(
        &mut self,
        token_in: &AccountId,
        amount_in: Balance,
        token_out: &AccountId,
        min_amount_out: Balance,
        admin_fee: &AdminFees,
        is_view: bool
    ) -> Balance {
        assert_ne!(token_in, token_out, "{}", ERR73_SAME_TOKEN);
        let in_idx = self.token_index(token_in);
        let out_idx = self.token_index(token_out);
        let amount_out = self.internal_get_return(in_idx, amount_in, out_idx);
        assert!(amount_out >= min_amount_out, "{}", ERR68_SLIPPAGE);
        if !is_view {
            log!("{}",
                format!(
                    "Swapped {} {} for {} {}",
                    amount_in, token_in, amount_out, token_out
                )
            );
        }

        let prev_invariant =
            integer_sqrt(U256::from(self.amounts[in_idx]) * U256::from(self.amounts[out_idx]));

        self.amounts[in_idx] += amount_in;
        self.amounts[out_idx] -= amount_out;

        // "Invariant" is by how much the dot product of amounts increased due to fees.
        let new_invariant =
            integer_sqrt(U256::from(self.amounts[in_idx]) * U256::from(self.amounts[out_idx]));
        // Invariant can not reduce (otherwise loosing balance of the pool and something it broken).
        assert!(new_invariant >= prev_invariant, "{}", ERR75_INVARIANT_REDUCE);

        // Allocate admin fee as fraction of total fee by issuing LP shares proportionally.
        let numerator = (new_invariant - prev_invariant) * U256::from(self.shares_total_supply);
        if admin_fee.admin_fee_bps > 0 && numerator > U256::zero() {
            // First we convert all admin fee into shares, as invariant increase is caused by total fee, so:
            //   total_fee_shares : prev_shares = (new_invariant - prev_invariant) : prev_invariant
            //   admin_fee_shares = total_fee_shares * admin_fee_rate
            //   Note: only part of total_fee_shares are used to mint share, so we have:
            //   prev_shares / new_shares > prev_invariant / new_invariant
            //   which means per share value increases due to (total_fee - admin_fee) part.
            let denominator = prev_invariant * FEE_DIVISOR / admin_fee.admin_fee_bps;
            let admin_shares = (numerator / denominator).as_u128();

            // Then, if there is valid referral and he has already registered as this pool LP, 
            //   transfer corresponding minted shares to his account. 
            let referral_share = if let Some((referral_id, referral_fee)) = &admin_fee.referral_info {
                if self.shares.contains_key(referral_id) {
                    u128_ratio(admin_shares, *referral_fee as u128, FEE_DIVISOR as u128)
                } else {
                    0
                }
            } else {
                0
            };
            if referral_share > 0 {
                self.mint_shares(&admin_fee.referral_info.as_ref().unwrap().0, referral_share, is_view);
                if !is_view {
                    log!("{}",
                        format!(
                            "Exchange {} got {} shares, Referral {} got {} shares",
                            &admin_fee.exchange_id, admin_shares - referral_share, &admin_fee.referral_info.as_ref().unwrap().0, referral_share,
                        )
                    );
                }
            } else {
                if !is_view {
                    log!("{}",
                        format!(
                            "Exchange {} got {} shares, No referral fee",
                            &admin_fee.exchange_id, admin_shares,
                        )
                    );
                }
            }
            // Finally, remaining admin shares belong to the exchange
            self.mint_shares(&admin_fee.exchange_id, admin_shares - referral_share, is_view);
        }

        // Keeping track of volume per each input traded separately.
        // Reported volume with fees will be sum of `input`, without fees will be sum of `output`.
        self.volumes[in_idx].input.0 += amount_in;
        self.volumes[in_idx].output.0 += amount_out;

        amount_out
    }
}
