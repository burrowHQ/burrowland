use near_sdk::{
    AccountId, log,
    serde::{Serialize},
    serde_json::{json},
    json_types::U128,
};

const EVENT_STANDARD: &str = "ref-farming";
const EVENT_STANDARD_VERSION: &str = "1.0.0";

#[derive(Serialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
#[serde(tag = "event", content = "data")]
#[serde(rename_all = "snake_case")]
pub enum Event<'a> {
    SeedCreate {
        caller_id: &'a AccountId,
        seed_id: &'a String,
        min_deposit: &'a U128,
        slash_rate: u32,
        min_locking_duration: u32,
    },
    FarmCreate {
        caller_id: &'a AccountId,
        reward_token: &'a AccountId,
        farm_id: &'a String,
        daily_reward: &'a U128,
        start_at: u32,
    },
    FarmCancel {
        caller_id: &'a AccountId,
        farm_id: &'a String,
    },
    RewardDeposit {
        caller_id: &'a AccountId,
        farm_id: &'a String,
        deposit_amount: &'a U128,
        total_amount: &'a U128,
        start_at: u32,
    },
    SeedDeposit {
        farmer_id: &'a AccountId,
        seed_id: &'a String,
        deposit_amount: &'a U128,
        increased_power: &'a U128,
        duration: u32,
    },
    SeedFreeToLock {
        farmer_id: &'a AccountId,
        seed_id: &'a String,
        amount: &'a U128,
        increased_power: &'a U128,
        duration: u32,
    },
    SeedUnlock {
        farmer_id: &'a AccountId,
        seed_id: &'a String,
        unlock_amount: &'a U128,
        decreased_power: &'a U128,
        slashed_seed: &'a U128,
    },
    SeedWithdraw {
        farmer_id: &'a AccountId,
        seed_id: &'a String,
        withdraw_amount: &'a U128,
        success: bool,
    },
    SeedWithdrawLostfound {
        farmer_id: &'a AccountId,
        seed_id: &'a String,
        withdraw_amount: &'a U128,
        success: bool,
    },
    SeedWithdrawSlashed {
        owner_id: &'a AccountId,
        seed_id: &'a String,
        withdraw_amount: &'a U128,
        success: bool,
    },
    RewardWithdraw {
        farmer_id: &'a AccountId,
        token_id: &'a AccountId,
        withdraw_amount: &'a U128,
        success: bool,
    },
    RewardLostfound {
        farmer_id: &'a AccountId,
        token_id: &'a AccountId,
        withdraw_amount: &'a U128,
    },
    RewardWithdrawBeneficiary {
        owner_id: &'a AccountId,
        farm_id: &'a String,
        withdraw_amount: &'a U128,
        success: bool,
    },
    RewardWithdrawUndistributed {
        owner_id: &'a AccountId,
        farm_id: &'a String,
        withdraw_amount: &'a U128,
        success: bool,
    },
}

impl Event<'_> {
    pub fn emit(&self) {
        emit_event(&self);
    }
}

// Emit event that follows NEP-297 standard: https://nomicon.io/Standards/EventsFormat
// Arguments
// * `standard`: name of standard, e.g. nep171
// * `version`: e.g. 1.0.0
// * `event`: type of the event, e.g. nft_mint
// * `data`: associate event data. Strictly typed for each set {standard, version, event} inside corresponding NEP
pub (crate) fn emit_event<T: ?Sized + Serialize>(data: &T) {
    let result = json!(data);
    let event_json = json!({
        "standard": EVENT_STANDARD,
        "version": EVENT_STANDARD_VERSION,
        "event": result["event"],
        "data": [result["data"]]
    })
    .to_string();
    log!(format!("EVENT_JSON:{}", event_json));
}


#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::{test_utils, AccountId};

    fn alice() -> AccountId {
        AccountId::new_unchecked("alice".to_string())
    }

    fn bob() -> AccountId {
        AccountId::new_unchecked("bob".to_string())
    }

    #[test]
    fn event_reward_deposit() {
        let caller_id = &alice();
        let farm_id = &"fakeseed#0".to_string();
        let deposit_amount = &U128(100);
        let total_amount = &U128(10000);
        let start_at = 500_u32;
        Event::RewardDeposit { caller_id, farm_id, deposit_amount, total_amount, start_at }.emit();
        assert_eq!(
            test_utils::get_logs()[0],
            r#"EVENT_JSON:{"standard":"ref-farming","version":"1.0.0","event":"reward_deposit","data":[{"caller_id":"alice","farm_id":"fakeseed#0","deposit_amount":"100","total_amount":"10000","start_at":500}]}"#
        );
    }

    #[test]
    fn event_seed_create() {
        let caller_id = &alice();
        let seed_id = &"fakeseed".to_string();
        let min_deposit = &U128(1000);
        let slash_rate = 100;
        let min_locking_duration = 100_u32;
        Event::SeedCreate { caller_id, seed_id, min_deposit, slash_rate, min_locking_duration }.emit();
        assert_eq!(
            test_utils::get_logs()[0],
            r#"EVENT_JSON:{"standard":"ref-farming","version":"1.0.0","event":"seed_create","data":[{"caller_id":"alice","seed_id":"fakeseed","min_deposit":"1000","slash_rate":100,"min_locking_duration":100}]}"#
        );
    }

    #[test]
    fn event_farm_create() {
        let caller_id = &alice();
        let reward_token = &bob();
        let farm_id = &"fakeseed#0".to_string();
        let daily_reward = &U128(100);
        let start_at = 0;
        Event::FarmCreate { caller_id, reward_token, farm_id, daily_reward, start_at }.emit();
        assert_eq!(
            test_utils::get_logs()[0],
            r#"EVENT_JSON:{"standard":"ref-farming","version":"1.0.0","event":"farm_create","data":[{"caller_id":"alice","reward_token":"bob","farm_id":"fakeseed#0","daily_reward":"100","start_at":0}]}"#
        );
    }

    #[test]
    fn event_farm_cancel() {
        let caller_id = &alice();
        let farm_id = &"fakeseed#0".to_string();
        Event::FarmCancel { caller_id, farm_id }.emit();
        assert_eq!(
            test_utils::get_logs()[0],
            r#"EVENT_JSON:{"standard":"ref-farming","version":"1.0.0","event":"farm_cancel","data":[{"caller_id":"alice","farm_id":"fakeseed#0"}]}"#
        );
    }

    #[test]
    fn event_seed_withdraw() {
        let farmer_id = &alice();
        let seed_id = &"fakeseed".to_string();
        let withdraw_amount = &U128(100);
        let success = false;
        Event::SeedWithdraw { farmer_id, seed_id, withdraw_amount, success }.emit();
        assert_eq!(
            test_utils::get_logs()[0],
            r#"EVENT_JSON:{"standard":"ref-farming","version":"1.0.0","event":"seed_withdraw","data":[{"farmer_id":"alice","seed_id":"fakeseed","withdraw_amount":"100","success":false}]}"#
        );
    }

    #[test]
    fn event_seed_withdraw_lostfound() {
        let farmer_id = &alice();
        let seed_id = &"fakeseed".to_string();
        let withdraw_amount = &U128(100);
        let success = false;
        Event::SeedWithdrawLostfound { farmer_id, seed_id, withdraw_amount, success }.emit();
        assert_eq!(
            test_utils::get_logs()[0],
            r#"EVENT_JSON:{"standard":"ref-farming","version":"1.0.0","event":"seed_withdraw_lostfound","data":[{"farmer_id":"alice","seed_id":"fakeseed","withdraw_amount":"100","success":false}]}"#
        );
    }

    #[test]
    fn event_seed_withdraw_slashed() {
        let owner_id = &alice();
        let seed_id = &"fakeseed".to_string();
        let withdraw_amount = &U128(100);
        let success = false;
        Event::SeedWithdrawSlashed { owner_id, seed_id, withdraw_amount, success }.emit();
        assert_eq!(
            test_utils::get_logs()[0],
            r#"EVENT_JSON:{"standard":"ref-farming","version":"1.0.0","event":"seed_withdraw_slashed","data":[{"owner_id":"alice","seed_id":"fakeseed","withdraw_amount":"100","success":false}]}"#
        );
    }

    #[test]
    fn event_reward_withdraw() {
        let farmer_id = &alice();
        let token_id = &bob();
        let withdraw_amount = &U128(100);
        let success = false;
        Event::RewardWithdraw { farmer_id, token_id, withdraw_amount, success }.emit();
        assert_eq!(
            test_utils::get_logs()[0],
            r#"EVENT_JSON:{"standard":"ref-farming","version":"1.0.0","event":"reward_withdraw","data":[{"farmer_id":"alice","token_id":"bob","withdraw_amount":"100","success":false}]}"#
        );
    }

    #[test]
    fn event_reward_lostfound() {
        let farmer_id = &alice();
        let token_id = &bob();
        let withdraw_amount = &U128(100);
        Event::RewardLostfound { farmer_id, token_id, withdraw_amount }.emit();
        assert_eq!(
            test_utils::get_logs()[0],
            r#"EVENT_JSON:{"standard":"ref-farming","version":"1.0.0","event":"reward_lostfound","data":[{"farmer_id":"alice","token_id":"bob","withdraw_amount":"100"}]}"#
        );
    }

    #[test]
    fn event_seed_deposit() {
        let farmer_id = &alice();
        let seed_id = &"fakeseed".to_string();
        let deposit_amount = &U128(100);
        let increased_power = &U128(100);
        let duration = 86400_u32;
        Event::SeedDeposit { farmer_id, seed_id, deposit_amount, increased_power, duration }.emit();
        assert_eq!(
            test_utils::get_logs()[0],
            r#"EVENT_JSON:{"standard":"ref-farming","version":"1.0.0","event":"seed_deposit","data":[{"farmer_id":"alice","seed_id":"fakeseed","deposit_amount":"100","increased_power":"100","duration":86400}]}"#
        );
    }

    #[test]
    fn event_seed_free_to_lock() {
        let farmer_id = &alice();
        let seed_id = &"fakeseed".to_string();
        let amount = &U128(100);
        let increased_power = &U128(100);
        let duration = 86400_u32;
        Event::SeedFreeToLock { farmer_id, seed_id, amount, increased_power, duration }.emit();
        assert_eq!(
            test_utils::get_logs()[0],
            r#"EVENT_JSON:{"standard":"ref-farming","version":"1.0.0","event":"seed_free_to_lock","data":[{"farmer_id":"alice","seed_id":"fakeseed","amount":"100","increased_power":"100","duration":86400}]}"#
        );
    }

    #[test]
    fn event_seed_unlock() {
        let farmer_id = &alice();
        let seed_id = &"fakeseed".to_string();
        let unlock_amount = &U128(100);
        let decreased_power = &U128(100);
        let slashed_seed = &U128(100);
        Event::SeedUnlock { farmer_id, seed_id, unlock_amount, decreased_power, slashed_seed }.emit();
        assert_eq!(
            test_utils::get_logs()[0],
            r#"EVENT_JSON:{"standard":"ref-farming","version":"1.0.0","event":"seed_unlock","data":[{"farmer_id":"alice","seed_id":"fakeseed","unlock_amount":"100","decreased_power":"100","slashed_seed":"100"}]}"#
        );
    }
}