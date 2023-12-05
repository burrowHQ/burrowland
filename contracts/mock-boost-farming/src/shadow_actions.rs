use crate::*;

use near_sdk::Gas;

pub const GAS_FOR_SHADOW_SEED_TRANSFER: Gas = Gas(20 * TGAS);
pub const GAS_FOR_SHADOW_SEED_CALLBACK: Gas = Gas(10 * TGAS);

pub const SHADOW_V1_TOKEN_PREFIX: &str = "shadow_ref_v1-";

impl Contract {
    pub fn shadow_id_to_seed_id(&self, shadow_id: &String) -> SeedId {
        if shadow_id.to_string().starts_with(SHADOW_V1_TOKEN_PREFIX) {
            let pool_id = shadow_id.split("-").collect::<Vec<&str>>()[1].parse::<u64>().expect("Invalid shadow_id");
            format!("{}@{}", self.data().ref_exchange_id, pool_id)
        } else {
            unimplemented!()
        }
    }
}

#[near_bindgen]
impl Contract {
    pub fn on_cast_shadow(&mut self, account_id: AccountId, shadow_id: String, amount: U128, msg: String) {
        require!(self.data().ref_exchange_id == env::predecessor_account_id(), E002_NOT_ALLOWED);
        require!(self.data().state == RunningState::Running, E004_CONTRACT_PAUSED);
        require!(msg.is_empty());

        let mut farmer = self.internal_unwrap_farmer(&account_id);
        let seed_id = self.shadow_id_to_seed_id(&shadow_id);
        let mut seed = self.internal_unwrap_seed(&seed_id);
        require!(amount.0 >= seed.min_deposit, E307_BELOW_MIN_DEPOSIT);

        self.internal_do_farmer_claim(&mut farmer, &mut seed);

        let mut farmer_seed = farmer.get_seed_unwrap(&seed_id);
        let increased_seed_power = farmer_seed.add_shadow(amount.0);
        farmer.set_seed(&seed_id, farmer_seed);

        seed.total_seed_amount += amount.0;
        seed.total_seed_power += increased_seed_power;

        self.update_impacted_seeds(&mut farmer, &seed_id);

        self.internal_set_farmer(&account_id, farmer);
        self.internal_set_seed(&seed_id, seed);

        Event::ShadowSeedDeposit {
            farmer_id: &account_id,
            seed_id: &seed_id,
            deposit_amount: &U128(amount.0),
            increased_power: &U128(increased_seed_power),
            duration: 0,
        }
        .emit();
    }

    pub fn on_remove_shadow(&mut self, account_id: AccountId, shadow_id: String, amount: U128, msg: String) {
        require!(self.data().ref_exchange_id == env::predecessor_account_id(), E002_NOT_ALLOWED);
        require!(self.data().state == RunningState::Running, E004_CONTRACT_PAUSED);
        require!(msg.is_empty());

        let withdraw_amount: Balance = amount.into();
        require!(withdraw_amount > 0, "amount must greater than 0!");

        let mut farmer = self.internal_unwrap_farmer(&account_id);
        let seed_id = self.shadow_id_to_seed_id(&shadow_id);
        let mut seed = self.internal_unwrap_seed(&seed_id);

        self.internal_do_farmer_claim(&mut farmer, &mut seed);

        let mut farmer_seed = farmer.get_seed_unwrap(&seed_id);

        let prev = farmer_seed.get_seed_power();

        farmer_seed.withdraw_shadow(withdraw_amount);

        seed.total_seed_amount -= withdraw_amount;
        seed.total_seed_power = seed.total_seed_power - prev + farmer_seed.get_seed_power();

        if farmer_seed.is_empty() {
            farmer.remove_seed(&seed_id);
        } else {
            farmer.set_seed(&seed_id, farmer_seed);
        }

        self.update_impacted_seeds(&mut farmer, &seed_id);

        self.internal_set_farmer(&account_id, farmer);
        self.internal_set_seed(&seed_id, seed);

        Event::ShadowSeedWithdraw {
            farmer_id: &account_id,
            seed_id: &seed_id,
            withdraw_amount: &U128(withdraw_amount),
        }
        .emit();
    }
}