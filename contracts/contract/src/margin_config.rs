use crate::*;

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
#[serde(crate = "near_sdk::serde")]
pub struct MarginConfig {
    /// base token default value
    /// When open a position or decrease collateral, the new leverage rate should less than this,
    /// Eg: 5 means 5 times collateral value should more than debt value.
    pub max_leverage_rate: u8, 
    /// Ensure pending debt less than this portion of availabe amount, 
    /// Eg: 1000 means pending debt amount should less than 10% of available amount.
    pub pending_debt_scale: u32,
    /// base token default value
    /// Ensure the slippage in SwapIndication less than this one,
    /// Eg: 1000 means we allow a max slippage of 10%.
    pub max_slippage_rate: u32,
    /// base token default value
    /// The position will be liquidated when (margin + position) is less than 
    ///   (debt + hp_fee) * (1 + min_safety_buffer_rate).
    pub min_safety_buffer: u32,
    /// Compare to regular borrowing, margin borrow enjoy a discount.
    /// Eg: 7000 means margin debt equals 70% of regular debt.
    pub margin_debt_discount_rate: u32,
    /// Open fee is on the margin asset.
    pub open_position_fee_rate: u32,
    /// Dex account id and its version (1 - RefV1, 2 - RefV2)
    pub registered_dexes: HashMap<AccountId, u8>,
    /// Token and its party side, such as 1 and 2 are in different parties,
    /// hence they can be a debt and a position. In other word,
    /// Tokens in the same party, can NOT exist in the same position.
    pub registered_tokens: HashMap<AccountId, u8>, 
    /// Maximum amount of margin position allowed for users to hold.
    pub max_active_user_margin_position: u8,
    /// base token default value
    /// The rate of liquidation benefits allocated to the protocol.
    pub liq_benefit_protocol_rate: u32,
    /// base token default value
    /// The rate of liquidation benefits allocated to the liquidator.
    pub liq_benefit_liquidator_rate: u32,
}

impl MarginConfig {
    pub fn check_pair(
        &self,
        debt_token_id: &AccountId,
        position_token_id: &AccountId,
        margin_token_id: &AccountId,
    ) {
        let position_party = self
            .registered_tokens
            .get(position_token_id)
            .expect("Illegal position token");
        let debt_party = self
            .registered_tokens
            .get(debt_token_id)
            .expect("Illegal debt token");
        assert!(position_party != debt_party, "Illegal debt<>position pairs");
        assert!(
            margin_token_id == debt_token_id || margin_token_id == position_token_id,
            "Margin token should be same as debt or position"
        );
    }
}

impl Contract {
    pub(crate) fn internal_margin_config(&self) -> MarginConfig {
        self.margin_config.get().unwrap()
    }
}

#[near_bindgen]
impl Contract {
    /// Returns the current margin config.
    pub fn get_margin_config(&self) -> MarginConfig {
        self.internal_margin_config()
    }

    #[payable]
    pub fn update_max_leverage_rate(&mut self, max_leverage_rate: u8) {
        assert_one_yocto();
        self.assert_owner();
        assert!(max_leverage_rate > 1, "Invalid max_leverage_rate");
        let mut mc = self.internal_margin_config();
        mc.max_leverage_rate = max_leverage_rate;
        self.margin_config.set(&mc);
    }

    #[payable]
    pub fn update_pending_debt_scale(&mut self, pending_debt_scale: u32) {
        assert_one_yocto();
        self.assert_owner();
        assert!(pending_debt_scale < MAX_RATIO, "Invalid pending_debt_scale");
        let mut mc = self.internal_margin_config();
        mc.pending_debt_scale = pending_debt_scale;
        self.margin_config.set(&mc);
    }

    #[payable]
    pub fn update_max_slippage_rate(&mut self, max_slippage_rate: u32) {
        assert_one_yocto();
        self.assert_owner();
        assert!(max_slippage_rate < MAX_RATIO, "Invalid max_slippage_rate");
        let mut mc = self.internal_margin_config();
        mc.max_slippage_rate = max_slippage_rate;
        self.margin_config.set(&mc);
    }

    #[payable]
    pub fn update_min_safety_buffer(&mut self, min_safety_buffer: u32) {
        assert_one_yocto();
        self.assert_owner();
        assert!(min_safety_buffer < MAX_RATIO, "Invalid min_safety_buffer");
        let mut mc = self.internal_margin_config();
        mc.min_safety_buffer = min_safety_buffer;
        self.margin_config.set(&mc);
    }

    #[payable]
    pub fn update_margin_debt_discount_rate(&mut self, margin_debt_discount_rate: u32) {
        assert_one_yocto();
        self.assert_owner();
        // The debt interest rate for margin positions may be higher than that of regular positions.
        assert!(margin_debt_discount_rate <= 3 * MAX_RATIO, "Invalid margin_debt_discount_rate");
        let mut mc = self.internal_margin_config();
        mc.margin_debt_discount_rate = margin_debt_discount_rate;
        self.margin_config.set(&mc);
    }

    #[payable]
    pub fn update_open_position_fee_rate(&mut self, open_position_fee_rate: u32) {
        assert_one_yocto();
        self.assert_owner();
        assert!(open_position_fee_rate < MAX_RATIO, "Invalid open_position_fee_rate");
        let mut mc = self.internal_margin_config();
        mc.open_position_fee_rate = open_position_fee_rate;
        self.margin_config.set(&mc);
    }

    #[payable]
    pub fn register_margin_dex(&mut self, dex_id: AccountId, dex_version: u8) {
        assert_one_yocto();
        self.assert_owner();
        let mut mc = self.internal_margin_config();
        if mc.registered_dexes.insert(dex_id, dex_version).is_some() {
            env::panic_str("margin dex already exists.");
        }
        self.margin_config.set(&mc);
    }

    #[payable]
    pub fn update_margin_dex(&mut self, dex_id: AccountId, dex_version: u8) {
        assert_one_yocto();
        self.assert_owner();
        let mut mc = self.internal_margin_config();
        if mc.registered_dexes.insert(dex_id, dex_version).is_none() {
            env::panic_str("margin dex does NOT exist.");
        }
        self.margin_config.set(&mc);
    }

    #[payable]
    pub fn unregister_margin_dex(&mut self, dex_id: AccountId) {
        assert_one_yocto();
        self.assert_owner();
        let mut mc = self.internal_margin_config();
        if mc.registered_dexes.remove(&dex_id).is_none() {
            env::panic_str("margin dex does NOT exist.");
        }
        self.margin_config.set(&mc);
    }

    #[payable]
    pub fn register_margin_token(&mut self, token_id: AccountId, token_party: u8) {
        assert_one_yocto();
        self.assert_owner();
        let mut mc = self.internal_margin_config();
        if mc.registered_tokens.insert(token_id, token_party).is_some() {
            env::panic_str("margin token already exists.");
        }
        self.margin_config.set(&mc);
    }

    #[payable]
    pub fn update_margin_token(&mut self, token_id: AccountId, token_party: u8) {
        assert_one_yocto();
        self.assert_owner();
        let mut mc = self.internal_margin_config();
        if mc.registered_tokens.insert(token_id, token_party).is_none() {
            env::panic_str("margin token does NOT exist.");
        }
        self.margin_config.set(&mc);
    }

    #[payable]
    pub fn unregister_margin_token(&mut self, token_id: AccountId) {
        assert_one_yocto();
        self.assert_owner();
        let mut mc = self.internal_margin_config();
        if mc.registered_tokens.remove(&token_id).is_none() {
            env::panic_str("margin token does NOT exist.");
        }
        self.margin_config.set(&mc);
    }

    #[payable]
    pub fn update_max_active_user_margin_position(&mut self, max_active_user_margin_position: u8) {
        assert_one_yocto();
        self.assert_owner();
        let mut mc = self.internal_margin_config();
        mc.max_active_user_margin_position = max_active_user_margin_position;
        self.margin_config.set(&mc);
    }

    #[payable]
    pub fn update_liquidation_benefits_rates(&mut self, liq_benefit_protocol_rate: u32, liq_benefit_liquidator_rate: u32) {
        assert_one_yocto();
        self.assert_owner();
        let mut mc = self.internal_margin_config();
        assert!(liq_benefit_protocol_rate + liq_benefit_liquidator_rate <= MAX_RATIO, "require: liq_benefit_protocol_rate + liq_benefit_liquidator_rate <= {}", MAX_RATIO);
        mc.liq_benefit_protocol_rate = liq_benefit_protocol_rate;
        mc.liq_benefit_liquidator_rate = liq_benefit_liquidator_rate;
        self.margin_config.set(&mc);
    }
}
