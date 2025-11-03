use crate::*;

impl Contract {
    /// Use only transferred tokens to increase collateral
    pub fn internal_only_increase_collateral(
        &mut self,
        account_id: &AccountId,
        token_id: &AccountId,
        amount: Balance,  // inner decimal precision
    ) {
        // 1. check stage
        let asset = self.internal_unwrap_asset(&token_id);
        assert!(
            asset.config.can_use_as_collateral,
            "This asset can't be used as a collateral"
        );
        // check if supply limit has hit, then need panic here
        if let Some(supplied_limit) = asset.config.supplied_limit {
            assert!(
                asset.supplied.balance + amount <= supplied_limit.0, 
                "Asset {} has hit supply limit, increasing collateral is not allowed", token_id
            );
        }
        let shares: Shares = asset.supplied.amount_to_shares(amount, false);

        // 2. on asset level, supply (including collateral) increased
        let mut asset = self.internal_unwrap_asset(token_id);
        asset.supplied.deposit(shares, amount);
        
        // 3. on account level, supply (excluding collateral) untouched but position changed
        let mut account = self.internal_unwrap_account(account_id);
        assert!(!account.is_locked, "Account is locked!");
        // account_asset untouched cause no pure supply change, but farms affected
        account.add_affected_farm(FarmId::Supplied(token_id.clone()));
        account.add_affected_farm(FarmId::TokenNetBalance(token_id.clone()));
        let position = REGULAR_POSITION.to_string();
        account.increase_collateral(&position, &token_id, shares);
        assert!(
            account.get_assets_num() <= self.internal_config().max_num_assets
        );

        // 4. updates
        // update asset
        self.internal_set_asset(token_id, asset);
        // update farms
        self.internal_account_apply_affected_farms(&mut account);
        // update user account
        self.internal_set_account(&account_id, account);

        // 5. emit event
        events::emit::increase_collateral(account_id, amount, token_id, &position);
    }

    /// Use only transferred tokens to repay debt, remains go to supply.
    pub fn internal_only_repay(
        &mut self,
        account_id: &AccountId,
        token_id: &AccountId,
        amount: Balance,  // inner decimal precision
    ) {
        let mut account = self.internal_unwrap_account(&account_id);
        assert!(!account.is_locked, "Account is locked!");
        let mut asset = self.internal_unwrap_asset(token_id);

        // 1. repay
        let position = REGULAR_POSITION.to_string();
        let full_repay_shares = account.internal_unwrap_borrowed(&position, token_id);
        let full_repay_amount = asset.borrowed.shares_to_amount(full_repay_shares, true);
        let (repay_shares, repay_amount) = if amount >= full_repay_amount {
            // full repayment
            (full_repay_shares, full_repay_amount)
        } else {
            // partial repayment
            (asset.borrowed.amount_to_shares(amount, false), amount)
        };
        asset.borrowed.withdraw(repay_shares, repay_amount);
        account.decrease_borrowed(&position, token_id, repay_shares);
        account.add_affected_farm(FarmId::Borrowed(token_id.clone()));
        events::emit::repay(&account_id, repay_amount, token_id, &position);

        // 2. remaining supply
        let remain_amount = amount - repay_amount;
        if remain_amount > 0 {
            let mut account_asset = account.internal_get_asset_or_default(token_id);
            let shares: Shares = asset.supplied.amount_to_shares(remain_amount, false);
            account_asset.deposit_shares(shares);
            account.internal_set_asset(&token_id, account_asset);
            asset.supplied.deposit(shares, remain_amount);
            events::emit::deposit(&account_id, remain_amount, &token_id);
        }

        // udate asset
        self.internal_set_asset(token_id, asset);
        // update farms
        self.internal_account_apply_affected_farms(&mut account);
        // update user account
        self.internal_set_account(&account_id, account);
    }
}