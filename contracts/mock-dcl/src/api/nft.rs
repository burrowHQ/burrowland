use crate::*;

use near_contract_standards::non_fungible_token::core::NonFungibleTokenCore;
use near_contract_standards::non_fungible_token::core::NonFungibleTokenResolver;
use near_contract_standards::non_fungible_token::enumeration::NonFungibleTokenEnumeration;
use near_contract_standards::non_fungible_token::events::NftTransfer;
use near_contract_standards::non_fungible_token::metadata::{
    NFTContractMetadata, NonFungibleTokenMetadataProvider, NFT_METADATA_SPEC,
};
use near_contract_standards::non_fungible_token::{Token, TokenId};

const GAS_FOR_RESOLVE_TRANSFER: Gas = Gas(5_000_000_000_000);
const GAS_FOR_NFT_TRANSFER_CALL: Gas = Gas(25_000_000_000_000 + GAS_FOR_RESOLVE_TRANSFER.0);

#[allow(unused)]
#[ext_contract(ext_self)]
trait NFTResolver {
    fn nft_resolve_transfer(
        &mut self,
        previous_owner_id: AccountId,
        receiver_id: AccountId,
        token_id: TokenId,
        approved_account_ids: Option<HashMap<AccountId, u64>>,
    ) -> bool;
}

#[ext_contract(ext_receiver)]
pub trait NonFungibleTokenReceiver {
    /// Returns true if token should be returned to `sender_id`
    fn nft_on_transfer(
        &mut self,
        sender_id: AccountId,
        previous_owner_id: AccountId,
        token_id: TokenId,
        msg: String,
    ) -> PromiseOrValue<bool>;
}

/// should NOT panic at any case, so all verify work should be done by caller
fn internal_change_owner_without_check(
    liquidity: &mut UserLiquidity,
    prev_user: &mut User,
    user: &mut User,
) {
    liquidity.owner_id = user.user_id.clone();
    prev_user.liquidity_keys.remove(&liquidity.lpt_id);
    user.liquidity_keys.insert(&liquidity.lpt_id);
}

impl Contract {
    /// Transfer token_id from `from` to `to`
    fn internal_transfer(&mut self, token_id: &TokenId, from: &AccountId, to: &AccountId, approval_id: Option<u64>, memo: Option<String>) -> (AccountId, Option<HashMap<AccountId, u64>>) {
        let mut liquidity = self.internal_unwrap_user_liquidity(&token_id);
        require!(!liquidity.is_mining(), E218_USER_LIQUIDITY_IS_MINING);

        let owner_id = liquidity.owner_id.clone();

        // clear approvals, this will be rolled back by a panic if sending fails
        let approved_account_ids = self.data_mut().approvals_by_id.remove(token_id);

        // check if authorized
        let sender_id = if from != &owner_id {
            // if token has no approved accounts
            let app_acc_ids =
                approved_account_ids.as_ref().unwrap_or_else(|| env::panic_str(E505_SENDER_NOT_APPROVED));

            // Approval extension is being used; get approval_id for sender.
            let actual_approval_id = app_acc_ids.get(from);

            // Panic if sender not approved at all
            if actual_approval_id.is_none() {
                env::panic_str(E505_SENDER_NOT_APPROVED);
            }

            // If approval_id included, check that it matches
            require!(
                approval_id.is_none() || actual_approval_id == approval_id.as_ref(),
                format!(
                    "The actual approval_id {:?} is different from the given approval_id {:?}",
                    actual_approval_id, approval_id
                )
            );
            Some(from)
        } else {
            None
        };

        require!(&owner_id != to, E506_FORBIDDEN_SELF_TRANSFER);

        // require!(liquidity.owner_id == from.clone(), E500_NOT_NFT_OWNER);
        let mut prev_user = self.internal_unwrap_user(&owner_id);
        let mut user = self.internal_unwrap_user(to);
        let global_config = self.internal_get_global_config();
        require!(user.get_available_slots(global_config.storage_price_per_slot, global_config.storage_for_asset) > 0, E107_NOT_ENOUGH_STORAGE_FOR_SLOTS);
        internal_change_owner_without_check(&mut liquidity, &mut prev_user, &mut user);

        self.internal_set_user(&owner_id, prev_user);
        self.internal_set_user(to, user);
        self.internal_set_user_liquidity(token_id, liquidity);

        Self::emit_transfer(
            &owner_id,
            to,
            &token_id,
            sender_id,
            memo,
        );
        
        // return previous owner & approvals
        (owner_id, approved_account_ids)
    }

    fn emit_transfer(
        owner_id: &AccountId,
        receiver_id: &AccountId,
        token_id: &str,
        sender_id: Option<&AccountId>,
        memo: Option<String>,
    ) {
        NftTransfer {
            old_owner_id: owner_id,
            new_owner_id: receiver_id,
            token_ids: &[token_id],
            authorized_id: sender_id.filter(|sender_id| *sender_id == owner_id),
            memo: memo.as_deref(),
        }
        .emit();
    }
}

#[near_bindgen]
impl NonFungibleTokenCore for Contract {
    #[payable]
    fn nft_transfer(
        &mut self,
        receiver_id: AccountId,
        token_id: TokenId,
        approval_id: Option<u64>,
        memo: Option<String>,
    ) {
        assert_one_yocto();
        self.assert_contract_running();
        let sender_id = env::predecessor_account_id();
        self.internal_transfer(&token_id, &sender_id, &receiver_id, approval_id, memo);
    }

    #[payable]
    fn nft_transfer_call(
        &mut self,
        receiver_id: AccountId,
        token_id: TokenId,
        approval_id: Option<u64>,
        memo: Option<String>,
        msg: String,
    ) -> PromiseOrValue<bool> {
        assert_one_yocto();
        require!(
            env::prepaid_gas() > GAS_FOR_NFT_TRANSFER_CALL,
            E501_MORE_GAS_IS_REQUIRED
        );
        self.assert_contract_running();
        let sender_id = env::predecessor_account_id();
        let (prev_owner, old_approvals) = self.internal_transfer(&token_id, &sender_id, &receiver_id, approval_id, memo);
        // Initiating receiver's call and the callback
        ext_receiver::ext(receiver_id.clone())
            .with_attached_deposit(NO_DEPOSIT)
            .with_static_gas(env::prepaid_gas() - GAS_FOR_NFT_TRANSFER_CALL)
            .nft_on_transfer(sender_id.clone(), prev_owner.clone(), token_id.clone(), msg)
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_FOR_RESOLVE_TRANSFER)
                    .nft_resolve_transfer(
                        prev_owner,
                        receiver_id,
                        token_id,
                        old_approvals,
                    ),
            )
            .into()
    }

    fn nft_token(&self, token_id: TokenId) -> Option<Token> {
        let liquidity = self.internal_unwrap_user_liquidity(&token_id);
        let approved_account_ids = self.data()
            .approvals_by_id
            .get(&token_id).or_else(|| Some(HashMap::new()));
        Some(Token {
            token_id,
            owner_id: liquidity.owner_id,
            metadata: None,
            approved_account_ids,
        })
    }
}

#[near_bindgen]
impl NonFungibleTokenResolver for Contract {
    #[private]
    fn nft_resolve_transfer(
        &mut self,
        previous_owner_id: AccountId,
        receiver_id: AccountId,
        token_id: TokenId,
        #[allow(unused_variables)] approved_account_ids: Option<
            std::collections::HashMap<AccountId, u64>,
        >,
    ) -> bool {
        let must_revert = match env::promise_result(0) {
            PromiseResult::NotReady => env::abort(),
            PromiseResult::Successful(value) => {
                if let Ok(yes_or_no) = near_sdk::serde_json::from_slice::<bool>(&value) {
                    yes_or_no
                } else {
                    true
                }
            }
            PromiseResult::Failed => true,
        };

        // if call succeeded, return early
        if !must_revert {
            return true;
        }

        // OTHERWISE, try to set owner back to previous_owner_id and restore approved_account_ids
        // Check that receiver didn't already transfer it away or burn it.
        if let Some(mut liquidity) = self.internal_get_user_liquidity(&token_id) {
            if liquidity.owner_id != receiver_id {
                // The token is not owned by the receiver anymore. Can't return it.
                true
            } else {
                // reset approvals to what previous owner had set before call to nft_transfer_call
                if let Some(previous_owner_approvals) = approved_account_ids {
                    self.data_mut().approvals_by_id.insert(&token_id, &previous_owner_approvals);
                }

                let mut prev_user = self.internal_unwrap_user(&receiver_id);
                // if original owner has unregistered, put this nft to contract owner's hand
                let mut user = self
                    .internal_get_user(&previous_owner_id)
                    .unwrap_or_else(|| self.internal_unwrap_user(&self.internal_get_global_config().owner_id));
                internal_change_owner_without_check(&mut liquidity, &mut prev_user, &mut user);
                self.internal_set_user(&receiver_id, prev_user);
                self.internal_set_user(&previous_owner_id, user);
                self.internal_set_user_liquidity(&token_id, liquidity);
                Self::emit_transfer(&receiver_id, &previous_owner_id, &token_id, None, None);
                false
            }
        } else {
            // The token was burned and doesn't exist anymore.
            true
        }
        // false
    }
}

#[near_bindgen]
impl NonFungibleTokenEnumeration for Contract {
    fn nft_total_supply(&self) -> near_sdk::json_types::U128 {
        (self.data().liquidity_count as u128).into()
    }

    fn nft_tokens(
        &self,
        #[allow(unused_variables)] from_index: Option<near_sdk::json_types::U128>,
        #[allow(unused_variables)] limit: Option<u64>,
    ) -> Vec<Token> {
        vec![]
    }

    fn nft_supply_for_owner(&self, account_id: AccountId) -> near_sdk::json_types::U128 {
        self.internal_get_user(&account_id)
            .and_then(|user| U128(user.liquidity_keys.len() as u128).into())
            .unwrap_or(U128(0))
    }

    fn nft_tokens_for_owner(
        &self,
        account_id: AccountId,
        from_index: Option<near_sdk::json_types::U128>,
        limit: Option<u64>,
    ) -> Vec<Token> {
        let token_set: Vec<String> = if let Some(user) = self.internal_get_user(&account_id) {
            user.liquidity_keys.to_vec()
        } else {
            return vec![];
        };
        let limit = limit.map(|v| v as usize).unwrap_or(usize::MAX);
        require!(limit != 0, E502_CANNOT_PROVIDE_LIMIT_OF_ZERO);
        let start_index: u128 = from_index.map(From::from).unwrap_or_default();
        require!(
            token_set.len() as u128 > start_index,
            E503_OUT_OF_BOUND
        );
        token_set
            .iter()
            .skip(start_index as usize)
            .take(limit)
            .map(|token_id| Token {
                token_id: token_id.clone(),
                owner_id: account_id.clone(),
                metadata: None,
                approved_account_ids: Some(self.data().approvals_by_id.get(&token_id).unwrap_or_default()),
            })
            .collect()
    }
}

#[near_bindgen]
impl NonFungibleTokenMetadataProvider for Contract {
    fn nft_metadata(&self) -> NFTContractMetadata {
        NFTContractMetadata {
            spec: NFT_METADATA_SPEC.to_string(),
            name: "REF DCL LIQUIDITY NFT".to_string(),
            symbol: "RDL".to_string(),
            icon: None,
            base_uri: None,
            reference: None,
            reference_hash: None,
        }
    }
}
