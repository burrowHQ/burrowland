use near_contract_standards::fungible_token::metadata::{
    FungibleTokenMetadata, FungibleTokenMetadataProvider,
};
use near_contract_standards::fungible_token::FungibleToken;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::json_types::U128;
use near_sdk::{near_bindgen, AccountId, PanicOnDefault, PromiseOrValue};

#[near_bindgen]
#[derive(BorshSerialize, BorshDeserialize, PanicOnDefault)]
pub struct Contract {
    token: FungibleToken,
    name: String,
    symbol: String,
    icon: Option<String>,
    decimals: u8,
}

#[near_bindgen]
impl Contract {
    #[init]
    pub fn new(name: String, symbol: String, decimals: u8) -> Self {
        Self {
            token: FungibleToken::new(b"t".to_vec()),
            name,
            symbol,
            icon: None,
            decimals,
        }
    }

    pub fn mint(&mut self, account_id: AccountId, amount: U128) {
        if self.token.storage_balance_of(account_id.clone()).is_none() {
            self.token.internal_register_account(&account_id);
        }
        self.token.internal_deposit(&account_id, amount.into());
    }

    
    pub fn burn(&mut self, account_id: AccountId, amount: U128) {
        self.token.internal_withdraw(&account_id, amount.into());
    }

    #[private]
    pub fn set_token_name(&mut self, name: String, symbol: String) {
        self.name = name;
        self.symbol = symbol;
    }

    #[private]
    pub fn set_icon(&mut self, icon: String) {
        self.icon = Some(icon);
    }

    #[private]
    pub fn set_decimals(&mut self, dec: u8) {
        self.decimals = dec;
    }
}

near_contract_standards::impl_fungible_token_core!(Contract, token);
near_contract_standards::impl_fungible_token_storage!(Contract, token);

#[near_bindgen]
impl FungibleTokenMetadataProvider for Contract {
    fn ft_metadata(&self) -> FungibleTokenMetadata {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use near_sdk::test_utils::{accounts, VMContextBuilder};
    use near_sdk::{env, testing_env};

    use super::*;

    #[test]
    fn test_basics() {
        let mut context = VMContextBuilder::new();
        testing_env!(context.build());
        let mut contract = Contract::new(String::from("TBD"), String::from("TBD"), 24);
        testing_env!(context
            .attached_deposit(125 * env::storage_byte_cost())
            .build());
        contract.mint(accounts(0), 1_000_000.into());
        assert_eq!(contract.ft_balance_of(accounts(0)), 1_000_000.into());

        testing_env!(context
            .attached_deposit(125 * env::storage_byte_cost())
            .build());
        contract.storage_deposit(Some(accounts(1)), None);
        testing_env!(context
            .attached_deposit(1)
            .predecessor_account_id(accounts(0))
            .build());
        contract.ft_transfer(accounts(1), 1_000.into(), None);
        assert_eq!(contract.ft_balance_of(accounts(1)), 1_000.into());

        contract.burn(accounts(1), 500.into());
        assert_eq!(contract.ft_balance_of(accounts(1)), 500.into());
    }
}