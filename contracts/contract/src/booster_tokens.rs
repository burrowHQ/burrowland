use crate::*;

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
#[serde(crate = "near_sdk::serde")]
pub struct BoosterTokenInfo {
    /// The account ID of the booster token contract.
    pub booster_token_id: TokenId,
    /// The number of decimals of the booster fungible token.
    pub booster_decimals: u8,
    /// The minimum duration to stake booster token in seconds.
    pub minimum_staking_duration_sec: DurationSec,
    /// The maximum duration to stake booster token in seconds.
    pub maximum_staking_duration_sec: DurationSec,
    /// The rate of xBooster for the amount of Booster given for the maximum staking duration.
    /// Assuming the 100% multiplier at the minimum staking duration. Should be no less than 100%.
    /// E.g. 20000 means 200% multiplier (or 2X).
    pub x_booster_multiplier_at_maximum_staking_duration: u32,
    /// The factor that suppresses the effect of boost.
    /// E.g. 1000 means that in the calculation, the actual boost amount will be divided by 1000.
    #[serde(with = "u128_dec_format")]
    pub boost_suppress_factor: u128,
    /// Determine whether the booster token takes effect.
    pub enable: bool,
    #[serde(with = "u128_dec_format")]
    pub total_stake_amount: u128,
}

impl BoosterTokenInfo {

    pub fn new(
        booster_token_id: TokenId,
        booster_decimals: u8,
        minimum_staking_duration_sec: DurationSec,
        maximum_staking_duration_sec: DurationSec,
        x_booster_multiplier_at_maximum_staking_duration: u32,
        boost_suppress_factor: u128,
    ) -> Self {
        Self {
            booster_token_id, 
            booster_decimals, 
            minimum_staking_duration_sec, 
            maximum_staking_duration_sec, 
            x_booster_multiplier_at_maximum_staking_duration,
            boost_suppress_factor, 
            enable: true, 
            total_stake_amount: 0,
        }
    }

    pub fn assert_valid(&self) {
        assert!(
            self.minimum_staking_duration_sec <= self.maximum_staking_duration_sec,
            "The maximum staking duration must be greater or equal than minimum staking duration"
        );
        assert!(
            self.x_booster_multiplier_at_maximum_staking_duration >= MIN_BOOSTER_MULTIPLIER,
            "xBooster multiplier should be no less than 100%"
        );
        assert!(
            self.boost_suppress_factor > 0,
            "The boost_suppress_factor must be greater than 0"
        );
    }
}

pub fn read_booster_tokens_from_storage() -> HashMap<TokenId, BoosterTokenInfo> {
    if let Some(content) = env::storage_read(BOOSTER_TOKENS_KEY.as_bytes()) {
        HashMap::try_from_slice(&content).expect("deserialize booster tokens storage failed.")
    } else {
        HashMap::new()
    }
}

pub fn write_booster_tokens_to_storage(data: HashMap<TokenId, BoosterTokenInfo>) {
    env::storage_write(BOOSTER_TOKENS_KEY.as_bytes(), &data.try_to_vec().unwrap());
}

impl Contract {
    pub fn internal_get_booster_token_info(&self, token_id: &TokenId) -> Option<BoosterTokenInfo> {
        let booster_tokens = read_booster_tokens_from_storage();
        booster_tokens
            .get(token_id)
            .cloned()
    }

    pub fn internal_unwrap_booster_token_info(&self, token_id: &TokenId) -> BoosterTokenInfo {
        self.internal_get_booster_token_info(token_id)
            .expect(format!("Invalid booster token id {}", token_id).as_str())
    }

    pub fn internal_set_booster_token_info(
        &mut self,
        token_id: &TokenId,
        booster_token_info: BoosterTokenInfo,
    ) {
        let mut booster_tokens = read_booster_tokens_from_storage();
        booster_tokens.insert(token_id.clone(), booster_token_info);
        write_booster_tokens_to_storage(booster_tokens);
    }
}

#[near_bindgen]
impl Contract {
    #[payable]
    pub fn add_booster_token_info(
        &mut self,
        booster_token_id: TokenId,
        booster_decimals: u8,
        minimum_staking_duration_sec: DurationSec,
        maximum_staking_duration_sec: DurationSec,
        x_booster_multiplier_at_maximum_staking_duration: u32,
        boost_suppress_factor: U128,
    ) {
        assert_one_yocto();
        self.assert_owner();
        let booster_tokens = read_booster_tokens_from_storage();
        require!(
            !booster_tokens.contains_key(&booster_token_id),
            format!("{} already exist.", booster_token_id)
        );
        let booster_token_info = BoosterTokenInfo::new(
            booster_token_id.clone(), 
            booster_decimals, 
            minimum_staking_duration_sec, 
            maximum_staking_duration_sec, 
            x_booster_multiplier_at_maximum_staking_duration,
            boost_suppress_factor.0, 
        );
        booster_token_info.assert_valid();
        self.internal_set_booster_token_info(&booster_token_id, booster_token_info);
    }

    #[payable]
    pub fn update_booster_token_info(
        &mut self,
        booster_token_id: TokenId,
        min_max_staking_duration: Option<(DurationSec, DurationSec)>,
        x_booster_multiplier_at_maximum_staking_duration: Option<u32>,
        boost_suppress_factor: Option<U128>,
        enable: Option<bool>,
    ) {
        assert_one_yocto();
        self.assert_owner();
        let mut booster_token_info = self.internal_unwrap_booster_token_info(&booster_token_id);
        if let Some((minimum_staking_duration_sec, maximum_staking_duration_sec)) = min_max_staking_duration {
            booster_token_info.minimum_staking_duration_sec = minimum_staking_duration_sec;
            booster_token_info.maximum_staking_duration_sec = maximum_staking_duration_sec;
        }
        if let Some(x_booster_multiplier_at_maximum_staking_duration) = x_booster_multiplier_at_maximum_staking_duration {
            booster_token_info.x_booster_multiplier_at_maximum_staking_duration = x_booster_multiplier_at_maximum_staking_duration;
        }
        if let Some(U128(boost_suppress_factor)) = boost_suppress_factor {
            booster_token_info.boost_suppress_factor = boost_suppress_factor;
        }
        if let Some(enable) = enable {
            booster_token_info.enable = enable;
        }
        booster_token_info.assert_valid();
        self.internal_set_booster_token_info(&booster_token_id, booster_token_info);
    }

    #[payable]
    pub fn remove_booster_token_info(
        &mut self,
        booster_token_id: TokenId,
    ) {
        assert_one_yocto();
        self.assert_owner();
        let mut booster_tokens = read_booster_tokens_from_storage();
        let booster_token_info = booster_tokens.remove(&booster_token_id).expect("Invalid booster_token_id");
        require!(booster_token_info.total_stake_amount == 0, "Already has staking");
        write_booster_tokens_to_storage(booster_tokens);
    }

    pub fn get_booster_tokens(&self) -> HashMap<TokenId, BoosterTokenInfo> {
        read_booster_tokens_from_storage()
    }
}
