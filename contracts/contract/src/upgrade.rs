use crate::*;

#[near_bindgen]
impl Contract {
    /// A method to migrate a state during the contract upgrade.
    /// Can only be called after upgrade method.
    #[private]
    #[init(ignore_state)]
    pub fn migrate_state() -> Self {
        let mut root_state: Contract = env::state_read().unwrap();
        let eth_old_account_id = get_eth_old_account_id();
        let eth_new_account_id = get_eth_new_account_id();
        // FIX-ETH: We take care of all the replacement in the root stucture in contract upgrading phase.
        // FIX-ETH: replace eth tokenID in assets lookupmap key
        let asset = root_state.assets.remove(&eth_old_account_id).unwrap();
        require!(root_state.assets.insert(&eth_new_account_id, &asset).is_none());
        // FIX-ETH: replace eth tokenID in asset_ids unorderedset key
        require!(root_state.asset_ids.remove(&eth_old_account_id));
        require!(root_state.asset_ids.insert(&eth_new_account_id));
        // FIX-ETH: replace eth tokenID in last_prices hashmap key
        let price = root_state.last_prices.remove(&eth_old_account_id).unwrap();
        require!(root_state.last_prices.insert(eth_new_account_id.clone(), price).is_none());
        // FIX-ETH: replace eth tokenID in token_pyth_info hashmap key
        let pyth_info = root_state.token_pyth_info.remove(&eth_old_account_id).unwrap();
        require!(root_state.token_pyth_info.insert(eth_new_account_id.clone(), pyth_info).is_none());
        // FIX-ETH: replace eth tokenID in asset_farms hashmap key
        root_state.update_asset_farms_eth_token_id(&eth_old_account_id, &eth_new_account_id);
        root_state
    }

    /// Returns semver of this contract.
    pub fn get_version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }
}

impl Contract {
    pub fn update_asset_farms_eth_token_id(&mut self, eth_old_account_id: &AccountId, eth_new_account_id: &AccountId) {
        // FarmId::NetTvl has no eth in either rewards or inactive_rewards.
        // No update is needed.

        // FarmId::Supplied(eth) has no eth in rewards.
        // Only the key and inactive_rewards need to be updated.
        let mut supplied_farm = self.asset_farms.remove(&FarmId::Supplied(eth_old_account_id.clone())).unwrap();
        match &mut supplied_farm {
            VAssetFarm::Current(asset_farm) => {
                let eth_inactive_reward = asset_farm.inactive_rewards.remove(&eth_old_account_id).unwrap();  
                require!(asset_farm.inactive_rewards.insert(eth_new_account_id, &eth_inactive_reward).is_none());
            },
        }
        require!(self.asset_farms.insert(&FarmId::Supplied(eth_new_account_id.clone()), &supplied_farm).is_none());

        // FarmId::Borrowed(eth) has no eth in either rewards or inactive_rewards.
        // Only the key needs to be updated.
        let borrowed_farm = self.asset_farms.remove(&FarmId::Borrowed(eth_old_account_id.clone())).unwrap();
        require!(self.asset_farms.insert(&FarmId::Borrowed(eth_new_account_id.clone()), &borrowed_farm).is_none());

        // FarmId::TokenNetBalance(eth) has no eth in either rewards or inactive_rewards.
        // Only the key needs to be updated.
        let token_net_balance_farm = self.asset_farms.remove(&FarmId::TokenNetBalance(eth_old_account_id.clone())).unwrap();
        require!(self.asset_farms.insert(&FarmId::TokenNetBalance(eth_new_account_id.clone()), &token_net_balance_farm).is_none());
    }
}

mod upgrade {
    use near_sdk::{require, Gas};

    use super::*;
    use near_sys as sys;

    const GAS_TO_COMPLETE_UPGRADE_CALL: Gas = Gas(Gas::ONE_TERA.0 * 10);
    const GAS_FOR_GET_CONFIG_CALL: Gas = Gas(Gas::ONE_TERA.0 * 5);
    const MIN_GAS_FOR_MIGRATE_STATE_CALL: Gas = Gas(Gas::ONE_TERA.0 * 10);

    /// Self upgrade and call migrate, optimizes gas by not loading into memory the code.
    /// Takes as input non serialized set of bytes of the code.
    #[no_mangle]
    pub extern "C" fn upgrade() {
        env::setup_panic_hook();
        let contract: Contract = env::state_read().expect("ERR_CONTRACT_IS_NOT_INITIALIZED");
        contract.assert_owner();
        let current_account_id = env::current_account_id().as_bytes().to_vec();
        let migrate_method_name = b"migrate_state".to_vec();
        let get_config_method_name = b"get_config".to_vec();
        let empty_args = b"{}".to_vec();
        unsafe {
            sys::input(0);
            let promise_id = sys::promise_batch_create(
                current_account_id.len() as _,
                current_account_id.as_ptr() as _,
            );
            sys::promise_batch_action_deploy_contract(promise_id, u64::MAX as _, 0);
            // Gas required to complete this call.
            let required_gas =
                env::used_gas() + GAS_TO_COMPLETE_UPGRADE_CALL + GAS_FOR_GET_CONFIG_CALL;
            require!(
                env::prepaid_gas() >= required_gas + MIN_GAS_FOR_MIGRATE_STATE_CALL,
                "Not enough gas to complete state migration"
            );
            let migrate_state_attached_gas = env::prepaid_gas() - required_gas;
            // Scheduling state migration.
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
