use crate::*;

use near_sdk::{Gas, ext_contract};

pub const GAS_FOR_SHADOW_SEED_TRANSFER: Gas = Gas(20 * TGAS);
pub const GAS_FOR_SHADOW_SEED_CALLBACK: Gas = Gas(10 * TGAS);

#[ext_contract(ref_exchange_receiver)]
pub trait RefExchangeActions {
    fn shadow_unfarming(&mut self, pool_id: u64, amount: U128);
}

#[near_bindgen]
impl Contract {
    pub fn deposit_free_shadow_seed(&mut self, farmer_id: AccountId, seed_id: String, amount: U128) {
        require!(self.data().ref_exchange_id == env::predecessor_account_id(), E002_NOT_ALLOWED);
        require!(self.data().state == RunningState::Running, E004_CONTRACT_PAUSED);

        let mut farmer = self.internal_unwrap_farmer(&farmer_id);
        let mut seed = self.internal_unwrap_seed(&seed_id);
        require!(amount.0 >= seed.min_deposit, E307_BELOW_MIN_DEPOSIT);

        self.internal_do_farmer_claim(&mut farmer, &mut seed);

        let mut farmer_seed = farmer.get_seed_unwrap(&seed_id);
        let increased_seed_power = farmer_seed.add_shadow(amount.0);
        farmer.set_seed(&seed_id, farmer_seed);

        seed.total_seed_amount += amount.0;
        seed.total_seed_power += increased_seed_power;

        self.update_impacted_seeds(&mut farmer, &seed_id);

        self.internal_set_farmer(&farmer_id, farmer);
        self.internal_set_seed(&seed_id, seed);

        Event::ShadowSeedDeposit {
            farmer_id: &farmer_id,
            seed_id: &seed_id,
            deposit_amount: &U128(amount.0),
            increased_power: &U128(increased_seed_power),
            duration: 0,
        }
        .emit();
    }

    pub fn withdraw_free_shadow_seed(&mut self, farmer_id: AccountId, seed_id: String, amount: U128) {
        require!(self.data().ref_exchange_id == env::predecessor_account_id(), E002_NOT_ALLOWED);
        require!(self.data().state == RunningState::Running, E004_CONTRACT_PAUSED);

        let withdraw_amount: Balance = amount.into();
        require!(withdraw_amount > 0, "amount must greater than 0!");

        let mut farmer = self.internal_unwrap_farmer(&farmer_id);
        let mut seed = self.internal_unwrap_seed(&seed_id);

        self.internal_do_farmer_claim(&mut farmer, &mut seed);

        let mut farmer_seed = farmer.get_seed_unwrap(&seed_id);

        let prev = farmer_seed.get_seed_power();

        farmer_seed.withdraw_shadow(withdraw_amount);

        seed.total_seed_amount -= withdraw_amount;
        seed.total_seed_power = seed.total_seed_power - prev + farmer_seed.get_seed_power();

        if farmer_seed.is_empty() {
            farmer.vseeds.remove(&seed_id);
        } else {
            farmer.set_seed(&seed_id, farmer_seed);
        }

        self.update_impacted_seeds(&mut farmer, &seed_id);

        self.internal_set_farmer(&farmer_id, farmer);
        self.internal_set_seed(&seed_id, seed);

        Event::ShadowSeedWithdraw {
            farmer_id: &farmer_id,
            seed_id: &seed_id,
            withdraw_amount: &U128(withdraw_amount),
        }
        .emit();
    }
}