use crate::*;
use near_contract_standards::non_fungible_token::approval::NonFungibleTokenApproval;
use near_contract_standards::non_fungible_token::approval::ext_nft_approval_receiver;
use near_contract_standards::non_fungible_token::TokenId;

const GAS_FOR_NFT_APPROVE: Gas = Gas(10_000_000_000_000);

#[near_bindgen]
impl NonFungibleTokenApproval for Contract {
    #[payable]
    fn nft_approve(
        &mut self,
        token_id: TokenId,
        account_id: AccountId,
        msg: Option<String>,
    ) -> Option<Promise> {
        require!(env::attached_deposit() >= 1, E601_NEED_DEPOSIT_AT_LEAST_ONE_YOCTO);
        self.assert_contract_running();
        if env::attached_deposit() > 1 {
            // refund deposit if caller attached more than 1 yocto
            Promise::new(account_id.clone()).transfer(env::attached_deposit());
        }
        
        let owner_id = self.internal_unwrap_user_liquidity(&token_id).owner_id;
        let approval_id: u64 = self.data().next_approval_id_by_id.get(&token_id).unwrap_or(1u64);

        let approvals_by_id = &mut self.data_mut().approvals_by_id;
        

        require!(env::predecessor_account_id() == owner_id, E500_NOT_NFT_OWNER);
        
        // update HashMap of approvals for this token
        let approved_account_ids = &mut approvals_by_id.get(&token_id).unwrap_or_default();
        approved_account_ids.insert(account_id.clone(), approval_id);

        // save updated approvals HashMap to contract's LookupMap
        approvals_by_id.insert(&token_id, approved_account_ids);

        // increment next_approval_id for this token
        self.data_mut().next_approval_id_by_id.insert(&token_id, &(approval_id + 1));

        require!(approved_account_ids.len() <= MAX_LIQUIDITY_APPROVAL_COUNT, E504_EXCEED_MAX_APPROVAL_COUNT);
        
        // if given `msg`, schedule call to `nft_on_approve` and return it. Else, return None.
        msg.map(|msg| {
            ext_nft_approval_receiver::ext(account_id)
                .with_static_gas(env::prepaid_gas() - GAS_FOR_NFT_APPROVE)
                .nft_on_approve(token_id, owner_id, approval_id, msg)
        })
    }

    #[payable]
    fn nft_revoke(&mut self, token_id: TokenId, account_id: AccountId) {
        assert_one_yocto();
        self.assert_contract_running();
        let owner_id = self.internal_unwrap_user_liquidity(&token_id).owner_id;
        let approvals_by_id = &mut self.data_mut().approvals_by_id;
        
        let predecessor_account_id = env::predecessor_account_id();

        require!(predecessor_account_id == owner_id, E500_NOT_NFT_OWNER);

        // if token has no approvals, do nothing
        if let Some(approved_account_ids) = &mut approvals_by_id.get(&token_id) {
            // if account_id was already not approved, do nothing
            if approved_account_ids.remove(&account_id).is_some() {
                // if this was the last approval, remove the whole HashMap to save space.
                if approved_account_ids.is_empty() {
                    approvals_by_id.remove(&token_id);
                } else {
                    // otherwise, update approvals_by_id with updated HashMap
                    approvals_by_id.insert(&token_id, approved_account_ids);
                }
            }
        }
    }

    #[payable]
    fn nft_revoke_all(&mut self, token_id: TokenId) {
        assert_one_yocto();
        self.assert_contract_running();
        let owner_id = self.internal_unwrap_user_liquidity(&token_id).owner_id;
        let approvals_by_id = &mut self.data_mut().approvals_by_id;
        
        let predecessor_account_id = env::predecessor_account_id();

        require!(predecessor_account_id == owner_id, E500_NOT_NFT_OWNER);

        approvals_by_id.remove(&token_id);
    }

    fn nft_is_approved(
        &self,
        token_id: TokenId,
        approved_account_id: AccountId,
        approval_id: Option<u64>,
    ) -> bool {

        require!(self.data().user_liquidities.contains_key(&token_id), E207_LIQUIDITY_NOT_FOUND);

        let approved_account_ids = if let Some(ids) = self.data().approvals_by_id.get(&token_id) {
            ids
        } else {
            // token has no approvals
            return false;
        };

        let actual_approval_id = if let Some(id) = approved_account_ids.get(&approved_account_id) {
            id
        } else {
            // account not in approvals HashMap
            return false;
        };

        if let Some(given_approval_id) = approval_id {
            &given_approval_id == actual_approval_id
        } else {
            // account approved, no approval_id given
            true
        }
    }
}
