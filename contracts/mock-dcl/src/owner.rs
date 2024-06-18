use crate::*;

impl Contract {
    pub fn assert_owner(&self) {
        // TODO: make global config referenced from param
        require!(
            env::predecessor_account_id() == self.internal_get_global_config().owner_id,
            E002_NOT_ALLOWED
        );
    }

    fn check_next_owner_deadline(&mut self) {
        let mut global_config = self.internal_get_global_config();
        if let Some(deadline) = global_config.next_owner_accept_deadline {
            // check if an existing transfer has expired
            if env::block_timestamp_ms() > deadline {
                global_config.next_owner_id = None;
                global_config.next_owner_accept_deadline = None;
            }
        }
        self.internal_set_global_config(global_config);
    }
}

#[near_bindgen]
impl Contract {

    #[payable]
    pub fn grant_next_owner(&mut self, next_owner_id: AccountId) {
        assert_one_yocto();
        self.assert_owner();
        let mut global_config = self.internal_get_global_config();
        require!(global_config.owner_id != next_owner_id, E002_NOT_ALLOWED);

        self.check_next_owner_deadline();
        
        require!(global_config.next_owner_id.is_none(), E002_NOT_ALLOWED);
        global_config.next_owner_id = Some(next_owner_id);
        global_config.next_owner_accept_deadline = Some(env::block_timestamp_ms() + AVAILABLE_MS_FOR_NEXT_OWNER_ACCEPT);
        self.internal_set_global_config(global_config);
    }

    #[payable]
    pub fn accept_next_owner(&mut self) {
        assert_one_yocto();
        self.check_next_owner_deadline();
        
        let mut global_config = self.internal_get_global_config();
        let next_owner_id = global_config.next_owner_id.clone();
        require!(next_owner_id.is_some() && next_owner_id.unwrap() == env::predecessor_account_id(), E002_NOT_ALLOWED);
        require!(global_config.next_owner_accept_deadline.is_some(), E008_ALREADY_ACCEPTED);
        
        global_config.next_owner_accept_deadline = None;
        self.internal_set_global_config(global_config);
    }

    #[payable]
    pub fn confirm_next_owner(&mut self) {
        assert_one_yocto();
        self.assert_owner();
        self.check_next_owner_deadline();

        let mut global_config = self.internal_get_global_config();
        require!(global_config.next_owner_accept_deadline.is_none(), E002_NOT_ALLOWED);
        require!(global_config.next_owner_id.is_some(), E002_NOT_ALLOWED);

        if let Some(next_owner_id) = global_config.next_owner_id.clone() {
            global_config.owner_id = next_owner_id.clone();
            // make sure owner is an user
            if !self.data().users.contains_key(&next_owner_id) {
                self.data_mut().users.insert(
                    &next_owner_id,
                    &User::new(&next_owner_id, &env::current_account_id(), STORAGE_BALANCE_MIN_BOUND).into(),
                );
                self.data_mut().user_count += 1;
            }
        }
        
        global_config.next_owner_id = None;
        self.internal_set_global_config(global_config);
    }

    #[payable]
    pub fn cancel_next_owner(&mut self) {
        assert_one_yocto();
        self.assert_owner();
        self.check_next_owner_deadline();
        
        let mut global_config = self.internal_get_global_config();
        require!(global_config.next_owner_id.is_some(), E002_NOT_ALLOWED);
        
        global_config.next_owner_id = None;
        global_config.next_owner_accept_deadline = None;
        self.internal_set_global_config(global_config);
    }

    #[payable]
    pub fn set_farming_contract_id(&mut self, farming_contract_id: AccountId) {
        assert_one_yocto();
        self.assert_owner();

        if !self.data().users.contains_key(&farming_contract_id) {
            self.data_mut().users.insert(
                &farming_contract_id,
                &User::new(&farming_contract_id, &env::current_account_id(), STORAGE_BALANCE_MIN_BOUND).into(),
            );
            self.data_mut().user_count += 1;
        }
        let current_farming_contract_id = self.data().farming_contract_id.clone();
        self.data_mut().farming_contract_id_history.push(current_farming_contract_id);
        self.data_mut().farming_contract_id = farming_contract_id;
    }

    #[payable]
    pub fn pause_contract(&mut self) -> bool {
        assert_one_yocto();
        self.assert_owner();
        let is_update = if self.data().state == RunningState::Running {
            log!("Contract paused by {}", env::predecessor_account_id());
            self.data_mut().state = RunningState::Paused;
            true
        } else {
            log!("Contract state is already in Paused");
            false
        };
        is_update
    }

    #[payable]
    pub fn resume_contract(&mut self) -> bool {
        assert_one_yocto();
        self.assert_owner();
        let is_update = if self.data().state == RunningState::Paused {
            log!("Contract resumed by {}", env::predecessor_account_id());
            self.data_mut().state = RunningState::Running;
            true
        } else {
            log!("Contract state is already in Running");
            false
        };
        is_update
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
            require!(is_success, E006_INVALID_OPERATOR);
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
        contract.data = match contract.data {
            VersionedContractData::V1000(data) => VersionedContractData::V1003(data.into()),
            VersionedContractData::V1001(data) => VersionedContractData::V1003(data.into()),
            VersionedContractData::V1002(data) => VersionedContractData::V1003(data.into()),
            VersionedContractData::V1003(data) => VersionedContractData::V1003(data),
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
        let get_config_method_name = b"get_metadata".to_vec();
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
