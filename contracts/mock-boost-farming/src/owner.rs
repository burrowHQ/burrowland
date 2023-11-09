use crate::*;

#[derive(Deserialize, Serialize)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(feature = "test", derive(Clone))]
pub struct ImportSeedInfo {
    pub seed_id: String,
    pub seed_decimal: u32,
    pub amount: U128,
    pub min_deposit: U128,
}

#[derive(Deserialize, Serialize)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(feature = "test", derive(Clone))]
pub struct ImportFarmerInfo {
    pub farmer_id: AccountId,
    pub rewards: HashMap<AccountId, U128>,
    pub seeds: HashMap<SeedId, U128>,
}

impl Contract {
    pub fn assert_owner(&self) {
        require!(
            env::predecessor_account_id() == self.data().owner_id,
            E002_NOT_ALLOWED
        );
    }

    fn check_next_owner_deadline(&mut self) {
        if let Some(deadline) = self.data().next_owner_accept_deadline {
            // check if an existing transfer has expired
            if env::block_timestamp_ms() > deadline {
                self.data_mut().next_owner_id = None;
                self.data_mut().next_owner_accept_deadline = None;
            }
        }
    }
}

#[near_bindgen]
impl Contract {
    #[payable]
    pub fn grant_next_owner(&mut self, next_owner_id: AccountId) {
        assert_one_yocto();
        self.assert_owner();
        require!(self.data().owner_id != next_owner_id, E002_NOT_ALLOWED);

        self.check_next_owner_deadline();

        require!(self.data().next_owner_id.is_none(), E002_NOT_ALLOWED);
        self.data_mut().next_owner_id = Some(next_owner_id);
        self.data_mut().next_owner_accept_deadline = Some(env::block_timestamp_ms() + AVAILABLE_MS_FOR_NEXT_OWNER_ACCEPT);
    }

    #[payable]
    pub fn accept_next_owner(&mut self) {
        assert_one_yocto();
        self.check_next_owner_deadline();

        let next_owner_id = self.data().next_owner_id.clone();
        require!(next_owner_id.is_some() && next_owner_id.unwrap() == env::predecessor_account_id(), E002_NOT_ALLOWED);
        require!(self.data().next_owner_accept_deadline.is_some(), E008_ALREADY_ACCEPTED);

        self.data_mut().next_owner_accept_deadline = None;
    }

    #[payable]
    pub fn confirm_next_owner(&mut self) {
        assert_one_yocto();
        self.assert_owner();
        self.check_next_owner_deadline();

        require!(self.data().next_owner_accept_deadline.is_none(), E002_NOT_ALLOWED);
        require!(self.data().next_owner_id.is_some(), E002_NOT_ALLOWED);

        if let Some(next_owner_id) = self.data().next_owner_id.clone() {
            self.data_mut().owner_id = next_owner_id.clone();
        }

        self.data_mut().next_owner_id = None;
    }

    #[payable]
    pub fn cancel_next_owner(&mut self) {
        assert_one_yocto();
        self.assert_owner();
        self.check_next_owner_deadline();

        require!(self.data().next_owner_id.is_some(), E002_NOT_ALLOWED);

        self.data_mut().next_owner_id = None;
        self.data_mut().next_owner_accept_deadline = None;
    }

    #[payable]
    pub fn pause_contract(&mut self) {
        assert_one_yocto();
        self.assert_owner();

        if self.data().state == RunningState::Running {
            log!("Contract paused by {}", env::predecessor_account_id());       
            self.data_mut().state = RunningState::Paused;
        } else {
            log!("Contract state is already in Paused");
        }
    }

    #[payable]
    pub fn resume_contract(&mut self) {
        assert_one_yocto();
        self.assert_owner();

        if self.data().state == RunningState::Paused {
            log!("Contract resumed by {}", env::predecessor_account_id());       
            self.data_mut().state = RunningState::Running;
        } else {
            log!("Contract state is already in Running");
        }
    }

    /// Extend operators. Only can be called by owner.
    #[payable]
    pub fn extend_operators(&mut self, operators: Vec<AccountId>) {
        assert_one_yocto();
        self.assert_owner();
        for operator in operators {
            self.data_mut().operators.insert(&operator);
        }
    }

    /// Remove operators. Only can be called by owner.
    #[payable]
    pub fn remove_operators(&mut self, operators: Vec<AccountId>) {
        assert_one_yocto();
        self.assert_owner();
        for operator in operators {
            let is_success = self.data_mut().operators.remove(&operator);
            require!(is_success, E007_INVALID_OPERATOR);
        }
    }

    /// Should only be called by this contract on migration.
    /// This is NOOP implementation. KEEP IT if you haven't changed contract state.
    /// If you have, you need to implement migration from old state 
    /// (keep the old struct with different name to deserialize it first).
    /// After migration goes live, revert back to this implementation for next updates.
    #[init(ignore_state)]
    #[private]
    pub fn migrate() -> Self {
        let mut contract: Contract = env::state_read().expect(E003_NOT_INIT);
        // see if ContractData need upgrade
        contract.data = 
        match contract.data {
            VersionedContractData::V0100(data) => VersionedContractData::V0102(data.into()),
            VersionedContractData::V0101(data) => VersionedContractData::V0102(data.into()),
            VersionedContractData::V0102(data) => VersionedContractData::V0102(data),
        };
        contract
    }
}

mod upgrade {
    use near_sdk::{require, Gas};
    use near_sys as sys;

    use super::*;

    const GAS_TO_COMPLETE_UPGRADE_CALL: Gas = Gas(Gas::ONE_TERA.0 * 10);
    const GAS_FOR_GET_CONFIG_CALL: Gas = Gas(Gas::ONE_TERA.0 * 5);
    const MIN_GAS_FOR_MIGRATE_STATE_CALL: Gas = Gas(Gas::ONE_TERA.0 * 60);

    /// Self upgrade and call migrate, optimizes gas by not loading into memory the code.
    /// Takes as input non serialized set of bytes of the code.
    #[no_mangle]
    pub extern "C" fn upgrade() {
        env::setup_panic_hook();
        let contract: Contract = env::state_read().expect("ERR_CONTRACT_IS_NOT_INITIALIZED");
        contract.assert_owner();
        let current_account_id = env::current_account_id().as_bytes().to_vec();
        let migrate_method_name = b"migrate".to_vec();
        let get_config_method_name = b"get_config".to_vec();
        let empty_args = b"{}".to_vec();
        // let current_id = env::current_account_id().as_bytes().to_vec();
        // let method_name = "migrate".as_bytes().to_vec();
        unsafe {
            // Load input (wasm code) into register 0.
            sys::input(0);
            // Create batch action promise for the current contract ID
            let promise_id = sys::promise_batch_create(
                current_account_id.len() as _,
                current_account_id.as_ptr() as _,
            );
            // 1st action in the Tx: "deploy contract" (code is taken from register 0)
            sys::promise_batch_action_deploy_contract(promise_id, u64::MAX as _, 0);
            // Gas required to complete this call.
            let required_gas =
                env::used_gas() + GAS_TO_COMPLETE_UPGRADE_CALL + GAS_FOR_GET_CONFIG_CALL;
            require!(
                env::prepaid_gas() >= required_gas + MIN_GAS_FOR_MIGRATE_STATE_CALL,
                "Not enough gas to complete state migration"
            );
            let migrate_state_attached_gas = env::prepaid_gas() - required_gas;
            // 2nd action in the Tx: call this_contract.migrate() with remaining gas
            sys::promise_batch_action_function_call(
                promise_id,
                migrate_method_name.len() as _,
                migrate_method_name.as_ptr() as _,
                empty_args.len() as _,
                empty_args.as_ptr() as _,
                0 as _,
                migrate_state_attached_gas.0,
            );
            // Scheduling to return config after the migration is completed.
            //
            // The upgrade method attaches it as an action, so the entire upgrade including deploy
            // contract action and migration can be rolled back if the config view call can't be
            // returned successfully. The view call deserializes the state and deserializes the
            // config which contains the owner_id. If the contract can deserialize the current config,
            // then it can validate the owner and execute the upgrade again (in case the previous
            // upgrade/migration went badly).
            //
            // It's an extra safety guard for the remote contract upgrades.
            sys::promise_batch_action_function_call(
                promise_id,
                get_config_method_name.len() as _,
                get_config_method_name.as_ptr() as _,
                empty_args.len() as _,
                empty_args.as_ptr() as _,
                0 as _,
                GAS_FOR_GET_CONFIG_CALL.0,
            );
            sys::promise_return(promise_id);
            
        }
    }
}

#[cfg(test)]
mod owner_tests {

    use super::*;

    use near_sdk::test_utils::{accounts, VMContextBuilder};
    use near_sdk::testing_env;

    fn setup_contract() -> (VMContextBuilder, Contract) {
        let mut context = VMContextBuilder::new();
        testing_env!(context.predecessor_account_id(accounts(0)).build());
        let contract = Contract::new(accounts(0));
        (context, contract)
    }

    #[test]
    fn transfer_owner_near_deadline() {
        let (mut context, mut contract) = setup_contract();
        testing_env!(context
            .block_timestamp(0)
            .predecessor_account_id(accounts(0))
            .attached_deposit(1)
            .build()
        );
        contract.grant_next_owner(accounts(2));
        assert_eq!(contract.get_metadata().next_owner_accept_deadline.unwrap(), AVAILABLE_MS_FOR_NEXT_OWNER_ACCEPT);

        // 5 secs before deadline
        testing_env!(context
            .block_timestamp(AVAILABLE_MS_FOR_NEXT_OWNER_ACCEPT * 1_000_000 - 5 * 1_000_000_000)
            .predecessor_account_id(accounts(2))
            .attached_deposit(1)
            .build()
        );
        contract.accept_next_owner();
        assert!(contract.get_metadata().next_owner_accept_deadline.is_none());
    }

    #[test]
    #[should_panic(expected = "E002: not allowed for the caller")]
    fn transfer_owner_after_deadline() {
        let (mut context, mut contract) = setup_contract();
        testing_env!(context
            .block_timestamp(0)
            .predecessor_account_id(accounts(0))
            .attached_deposit(1)
            .build()
        );
        contract.grant_next_owner(accounts(2));
        assert_eq!(contract.get_metadata().next_owner_accept_deadline.unwrap(), AVAILABLE_MS_FOR_NEXT_OWNER_ACCEPT);

        // 5 secs after deadline
        testing_env!(context
            .block_timestamp(AVAILABLE_MS_FOR_NEXT_OWNER_ACCEPT * 1_000_000 + 5 * 1_000_000_000)
            .predecessor_account_id(accounts(2))
            .attached_deposit(1)
            .build()
        );
        contract.accept_next_owner();
    }
}