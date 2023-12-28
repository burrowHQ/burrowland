use crate::*;

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
#[serde(crate = "near_sdk::serde")]
pub struct MarginConfig {
    pub pending_debt_scale: u32,
    pub max_slippage_rate: u32,
    pub min_safty_buffer: u32,
    /// dex account id and its version (1 - RefV1, 2 - RefV2)
    pub registered_dexes: HashMap<AccountId, u8>,
    /// token and its party side, such as 1 and 2 are in different parties,
    /// hence they can be a debt and a position. In other word,
    /// Tokens in the same party, can NOT exist in the same position.
    pub registered_tokens: HashMap<AccountId, u8>,
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
}
