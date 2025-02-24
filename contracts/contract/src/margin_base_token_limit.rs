use near_sdk::require;
use std::fmt::Display;

use crate::*;

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
#[serde(crate = "near_sdk::serde")]
pub struct MarginBaseTokenLimitGur {
    /// Defines the allowed range for guardians to update `min_safety_buffer`.
    /// The first value represents the minimum limit, and the second value represents the maximum limit.
    pub min_safety_buffer_gur: (u32, u32),
    /// Defines the allowed range for guardians to update `max_leverage_rate`.
    /// The first value represents the minimum limit, and the second value represents the maximum limit.
    pub max_leverage_rate_gur: (u8, u8),
    /// Defines the allowed range for guardians to update `max_common_slippage_rate`.
    /// The first value represents the minimum limit, and the second value represents the maximum limit.
    pub max_common_slippage_rate_gur: (u32, u32),
    /// Defines the allowed range for guardians to update `max_forceclose_slippage_rate`.
    /// The first value represents the minimum limit, and the second value represents the maximum limit.
    pub max_forceclose_slippage_rate_gur: (u32, u32),
    /// Defines the allowed range for guardians to update `liq_benefit_protocol_rate`.
    /// The first value represents the minimum limit, and the second value represents the maximum limit.
    pub liq_benefit_protocol_rate_gur: (u32, u32),
    /// Defines the allowed range for guardians to update `liq_benefit_liquidator_rate`.
    /// The first value represents the minimum limit, and the second value represents the maximum limit.
    pub liq_benefit_liquidator_rate_gur: (u32, u32),
    /// Defines the allowed range for guardians to update `min_base_token_short_position`.
    /// The first value represents the minimum limit, and the second value represents the maximum limit.
    pub min_base_token_short_position_gur: (U128, U128),
    /// Defines the allowed range for guardians to update `min_base_token_long_position`.
    /// The first value represents the minimum limit, and the second value represents the maximum limit.
    pub min_base_token_long_position_gur: (U128, U128),
    /// Defines the allowed range for guardians to update `max_base_token_short_position`.
    /// The first value represents the minimum limit, and the second value represents the maximum limit.
    pub max_base_token_short_position_gur: (U128, U128),
    /// Defines the allowed range for guardians to update `max_base_token_long_position`.
    /// The first value represents the minimum limit, and the second value represents the maximum limit.
    pub max_base_token_long_position_gur: (U128, U128),
    /// Defines the allowed range for guardians to update `total_base_token_available_short`.
    /// The first value represents the minimum limit, and the second value represents the maximum limit.
    pub total_base_token_available_short_gur: (U128, U128),
    /// Defines the allowed range for guardians to update `total_base_token_available_long`.
    /// The first value represents the minimum limit, and the second value represents the maximum limit.
    pub total_base_token_available_long_gur: (U128, U128),
}

#[derive(BorshSerialize, BorshDeserialize)]
pub enum VMarginBaseTokenLimitGur {
    Current(MarginBaseTokenLimitGur),
}

impl From<VMarginBaseTokenLimitGur> for MarginBaseTokenLimitGur {
    fn from(v: VMarginBaseTokenLimitGur) -> Self {
        match v {
            VMarginBaseTokenLimitGur::Current(c) => c,
        }
    }
}

impl From<MarginBaseTokenLimitGur> for VMarginBaseTokenLimitGur {
    fn from(c: MarginBaseTokenLimitGur) -> Self {
        VMarginBaseTokenLimitGur::Current(c)
    }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
#[serde(crate = "near_sdk::serde")]
pub struct MarginBaseTokenLimit {
    /// The position will be liquidated when (margin + position) is less than
    ///   (debt + hp_fee) * (1 + min_safety_buffer_rate).
    pub min_safety_buffer: u32,
    /// When open a position or decrease collateral, the new leverage rate should less than this,
    /// Eg: 5 means 5 times collateral value should more than debt value.
    pub max_leverage_rate: u8,
    /// Ensure the slippage in common SwapIndication less than this one,
    /// Eg: 1000 means we allow a max slippage of 10%.
    pub max_common_slippage_rate: u32,
    /// Ensure the slippage in forceclose SwapIndication less than this one,
    /// Eg: 1000 means we allow a max slippage of 10%.
    pub max_forceclose_slippage_rate: u32,
    /// The rate of liquidation benefits allocated to the protocol.
    pub liq_benefit_protocol_rate: u32,
    /// The rate of liquidation benefits allocated to the liquidator.
    pub liq_benefit_liquidator_rate: u32,
    /// The minimum available quantity of base token for a single short position.
    pub min_base_token_short_position: U128,
    /// The minimum available quantity of base token for a single long position.
    pub min_base_token_long_position: U128,
    /// The maximum available quantity of base token for a single short position.
    pub max_base_token_short_position: U128,
    /// The maximum available quantity of base token for a single long position.
    pub max_base_token_long_position: U128,
    /// The total available quantity of base token for short positions in the contract.
    pub total_base_token_available_short: U128,
    /// The total available quantity of base token for long positions in the contract.
    pub total_base_token_available_long: U128,
}

impl MarginBaseTokenLimit {
    pub fn default_limit(margin_config: &MarginConfig) -> Self {
        Self {
            min_safety_buffer: margin_config.min_safety_buffer,
            max_leverage_rate: margin_config.max_leverage_rate,
            max_common_slippage_rate: margin_config.max_slippage_rate,
            max_forceclose_slippage_rate: margin_config.max_slippage_rate,
            liq_benefit_protocol_rate: margin_config.liq_benefit_protocol_rate,
            liq_benefit_liquidator_rate: margin_config.liq_benefit_liquidator_rate,
            min_base_token_short_position: 0.into(),
            min_base_token_long_position: 0.into(),
            max_base_token_short_position: u128::MAX.into(),
            max_base_token_long_position: u128::MAX.into(),
            total_base_token_available_short: u128::MAX.into(),
            total_base_token_available_long: u128::MAX.into(),
        }
    }

    fn assert_validate_within_range<T>(value: T, min: T, max: T, param_name: &str)
    where
        T: PartialOrd + Copy + Display,
    {
        require!(
            value >= min && value <= max,
            format!(
                "{} value {} is out of the allowed range [{} - {}]",
                param_name, value, min, max
            )
        );
    }

    pub fn assert_validate(&self, mbtlg: MarginBaseTokenLimitGur) {
        Self::assert_validate_within_range(
            self.min_safety_buffer,
            mbtlg.min_safety_buffer_gur.0,
            mbtlg.min_safety_buffer_gur.1,
            "min_safety_buffer",
        );
        Self::assert_validate_within_range(
            self.max_leverage_rate,
            mbtlg.max_leverage_rate_gur.0,
            mbtlg.max_leverage_rate_gur.1,
            "max_leverage_rate",
        );
        Self::assert_validate_within_range(
            self.max_common_slippage_rate,
            mbtlg.max_common_slippage_rate_gur.0,
            mbtlg.max_common_slippage_rate_gur.1,
            "max_common_slippage_rate",
        );
        Self::assert_validate_within_range(
            self.max_forceclose_slippage_rate,
            mbtlg.max_forceclose_slippage_rate_gur.0,
            mbtlg.max_forceclose_slippage_rate_gur.1,
            "max_forceclose_slippage_rate",
        );
        Self::assert_validate_within_range(
            self.liq_benefit_protocol_rate,
            mbtlg.liq_benefit_protocol_rate_gur.0,
            mbtlg.liq_benefit_protocol_rate_gur.1,
            "liq_benefit_protocol_rate",
        );
        Self::assert_validate_within_range(
            self.liq_benefit_liquidator_rate,
            mbtlg.liq_benefit_liquidator_rate_gur.0,
            mbtlg.liq_benefit_liquidator_rate_gur.1,
            "liq_benefit_liquidator_rate",
        );
        Self::assert_validate_within_range(
            self.min_base_token_short_position.0,
            mbtlg.min_base_token_short_position_gur.0 .0,
            mbtlg.min_base_token_short_position_gur.1 .0,
            "min_base_token_short_position",
        );
        Self::assert_validate_within_range(
            self.min_base_token_long_position.0,
            mbtlg.min_base_token_long_position_gur.0 .0,
            mbtlg.min_base_token_long_position_gur.1 .0,
            "min_base_token_long_position",
        );
        Self::assert_validate_within_range(
            self.max_base_token_short_position.0,
            mbtlg.max_base_token_short_position_gur.0 .0,
            mbtlg.max_base_token_short_position_gur.1 .0,
            "max_base_token_short_position",
        );
        Self::assert_validate_within_range(
            self.max_base_token_long_position.0,
            mbtlg.max_base_token_long_position_gur.0 .0,
            mbtlg.max_base_token_long_position_gur.1 .0,
            "max_base_token_long_position",
        );
        Self::assert_validate_within_range(
            self.total_base_token_available_short.0,
            mbtlg.total_base_token_available_short_gur.0 .0,
            mbtlg.total_base_token_available_short_gur.1 .0,
            "total_base_token_available_short",
        );
        Self::assert_validate_within_range(
            self.total_base_token_available_long.0,
            mbtlg.total_base_token_available_long_gur.0 .0,
            mbtlg.total_base_token_available_long_gur.1 .0,
            "total_base_token_available_long",
        );
    }

    pub fn assert_base_token_amount_valid(
        &self,
        base_token_amount: u128,
        total_base_token_amount: u128,
        pd: &PositionDirection,
    ) {
        match pd {
            PositionDirection::Long(_) => {
                require!(
                    base_token_amount >= self.min_base_token_long_position.0
                        && base_token_amount <= self.max_base_token_long_position.0,
                    format!(
                        "base_token_amount out of the long position valid range [{}, {}]",
                        self.min_base_token_long_position.0, self.max_base_token_long_position.0
                    )
                );
                require!(
                    total_base_token_amount <= self.total_base_token_available_long.0,
                    format!(
                        "total_base_token_amount exceeds the long position limit {}.",
                        self.total_base_token_available_long.0
                    )
                );
            }
            PositionDirection::Short(_) => {
                require!(
                    base_token_amount >= self.min_base_token_short_position.0
                        && base_token_amount <= self.max_base_token_short_position.0,
                    format!(
                        "base_token_amount out of the short position valid range [{}, {}]",
                        self.min_base_token_short_position.0, self.max_base_token_short_position.0
                    )
                );
                require!(
                    total_base_token_amount <= self.total_base_token_available_short.0,
                    format!(
                        "total_base_token_amount exceeds the short position limit {}.",
                        self.total_base_token_available_short.0
                    )
                );
            }
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize)]
pub enum VMarginBaseTokenLimit {
    Current(MarginBaseTokenLimit),
}

impl From<VMarginBaseTokenLimit> for MarginBaseTokenLimit {
    fn from(v: VMarginBaseTokenLimit) -> Self {
        match v {
            VMarginBaseTokenLimit::Current(c) => c,
        }
    }
}

impl From<MarginBaseTokenLimit> for VMarginBaseTokenLimit {
    fn from(c: MarginBaseTokenLimit) -> Self {
        VMarginBaseTokenLimit::Current(c)
    }
}

pub fn read_margin_base_token_limit_gur_from_storage(
) -> Option<UnorderedMap<TokenId, VMarginBaseTokenLimitGur>> {
    env::storage_read(MARGIN_BASE_TOKEN_LIMIT_GUR.as_bytes()).map(|v| {
        UnorderedMap::try_from_slice(&v).expect("deserialize margin base token limit gur failed.")
    })
}

pub fn write_margin_base_token_limit_gur_to_storage(
    data: UnorderedMap<TokenId, VMarginBaseTokenLimitGur>,
) {
    env::storage_write(
        MARGIN_BASE_TOKEN_LIMIT_GUR.as_bytes(),
        &data.try_to_vec().unwrap(),
    );
}

pub fn read_margin_base_token_limit_from_storage(
) -> Option<UnorderedMap<TokenId, VMarginBaseTokenLimit>> {
    env::storage_read(MARGIN_BASE_TOKEN_LIMIT.as_bytes()).map(|v| {
        UnorderedMap::try_from_slice(&v).expect("deserialize margin base token limit failed.")
    })
}

pub fn write_margin_base_token_limit_to_storage(
    data: UnorderedMap<TokenId, VMarginBaseTokenLimit>,
) {
    env::storage_write(
        MARGIN_BASE_TOKEN_LIMIT.as_bytes(),
        &data.try_to_vec().unwrap(),
    );
}

impl Contract {
    pub fn internal_unwrap_margin_base_token_limit_or_default(
        &self,
        token_id: &TokenId,
    ) -> MarginBaseTokenLimit {
        match read_margin_base_token_limit_from_storage() {
            Some(margin_base_token_limit) => match margin_base_token_limit.get(token_id) {
                Some(limit) => limit.into(),
                None => MarginBaseTokenLimit::default_limit(&self.internal_margin_config()),
            },
            None => MarginBaseTokenLimit::default_limit(&self.internal_margin_config()),
        }
    }

    pub fn internal_set_margin_base_token_limit(
        &mut self,
        token_id: &TokenId,
        mbtl: MarginBaseTokenLimit,
    ) {
        match read_margin_base_token_limit_from_storage() {
            Some(mut margin_base_token_limit) => {
                margin_base_token_limit.insert(token_id, &mbtl.into());
                write_margin_base_token_limit_to_storage(margin_base_token_limit);
            }
            None => {
                let mut margin_base_token_limit =
                    UnorderedMap::new(MARGIN_BASE_TOKEN_LIMIT.as_bytes());
                margin_base_token_limit.insert(token_id, &mbtl.into());
                write_margin_base_token_limit_to_storage(margin_base_token_limit);
            }
        };
    }

    pub fn internal_remove_margin_base_token_limit(&mut self, token_id: &TokenId) {
        match read_margin_base_token_limit_from_storage() {
            Some(mut margin_base_token_limit) => {
                margin_base_token_limit
                    .remove(token_id)
                    .expect(format!("{} MarginBaseTokenLimit not exist", token_id).as_str());
                write_margin_base_token_limit_to_storage(margin_base_token_limit);
            }
            None => env::panic_str(format!("{} MarginBaseTokenLimit not exist", token_id).as_str()),
        }
    }
}

impl Contract {
    pub fn internal_unwrap_margin_base_token_limit_gur(
        &self,
        token_id: &TokenId,
    ) -> MarginBaseTokenLimitGur {
        match read_margin_base_token_limit_gur_from_storage() {
            Some(margin_base_token_limit_gur) => margin_base_token_limit_gur
                .get(token_id)
                .map(|v| v.into())
                .expect(format!("{} MarginBaseTokenLimitGur not exist", token_id).as_str()),
            None => {
                env::panic_str(format!("{} MarginBaseTokenLimitGur not exist", token_id).as_str())
            }
        }
    }

    pub fn internal_set_margin_base_token_limit_gur(
        &mut self,
        token_id: &TokenId,
        mbtlg: MarginBaseTokenLimitGur,
    ) {
        match read_margin_base_token_limit_gur_from_storage() {
            Some(mut margin_base_token_limit_gur) => {
                margin_base_token_limit_gur.insert(token_id, &mbtlg.into());
                write_margin_base_token_limit_gur_to_storage(margin_base_token_limit_gur);
            }
            None => {
                let mut margin_base_token_limit_gur =
                    UnorderedMap::new(MARGIN_BASE_TOKEN_LIMIT_GUR.as_bytes());
                margin_base_token_limit_gur.insert(token_id, &mbtlg.into());
                write_margin_base_token_limit_gur_to_storage(margin_base_token_limit_gur);
            }
        };
    }

    pub fn internal_remove_margin_base_token_limit_gur(&mut self, token_id: &TokenId) {
        match read_margin_base_token_limit_gur_from_storage() {
            Some(mut margin_base_token_limit_gur) => {
                margin_base_token_limit_gur
                    .remove(token_id)
                    .expect(format!("{} MarginBaseTokenLimitGur not exist", token_id).as_str());
                write_margin_base_token_limit_gur_to_storage(margin_base_token_limit_gur);
            }
            None => {
                env::panic_str(format!("{} MarginBaseTokenLimitGur not exist", token_id).as_str())
            }
        }
    }
}

#[near_bindgen]
impl Contract {
    #[payable]
    pub fn set_margin_base_token_limit_gur(
        &mut self,
        token_id: TokenId,
        mbtlg: MarginBaseTokenLimitGur,
    ) {
        assert_one_yocto();
        self.assert_owner();
        self.internal_set_margin_base_token_limit_gur(&token_id, mbtlg);
    }

    #[payable]
    pub fn remove_margin_base_token_limit_gur(&mut self, token_id: TokenId) {
        assert_one_yocto();
        self.assert_owner();
        self.internal_remove_margin_base_token_limit_gur(&token_id);
    }

    #[payable]
    pub fn set_margin_base_token_limit(&mut self, token_id: TokenId, mbtl: MarginBaseTokenLimit) {
        assert_one_yocto();
        self.assert_owner_or_guardians();
        if env::predecessor_account_id() != self.internal_config().owner_id {
            let mbtlg = self.internal_unwrap_margin_base_token_limit_gur(&token_id);
            mbtl.assert_validate(mbtlg);
        }
        require!(mbtl.max_leverage_rate > 1, "Invalid max_leverage_rate");
        require!(mbtl.max_common_slippage_rate < MAX_RATIO, "Invalid max_common_slippage_rate");
        require!(mbtl.max_forceclose_slippage_rate < MAX_RATIO, "Invalid max_forceclose_slippage_rate");
        require!(mbtl.min_safety_buffer < MAX_RATIO, "Invalid min_safety_buffer");
        require!(
            mbtl.min_base_token_long_position <= mbtl.max_base_token_long_position,
            "require: min_base_token_long_position <= max_base_token_long_position"
        );
        require!(
            mbtl.min_base_token_short_position <= mbtl.max_base_token_short_position,
            "require: min_base_token_short_position <= max_base_token_short_position"
        );
        require!(
            mbtl.liq_benefit_protocol_rate + mbtl.liq_benefit_liquidator_rate <= MAX_RATIO,
            format!(
                "require: liq_benefit_protocol_rate + liq_benefit_liquidator_rate <= {}",
                MAX_RATIO
            )
        );
        self.internal_set_margin_base_token_limit(&token_id, mbtl);
    }

    #[payable]
    pub fn remove_margin_base_token_limit(&mut self, token_id: TokenId) {
        assert_one_yocto();
        self.assert_owner();
        self.internal_remove_margin_base_token_limit(&token_id);
    }

    pub fn list_margin_base_token_limit_gur(
        &self,
        token_ids: Vec<TokenId>,
    ) -> HashMap<TokenId, Option<MarginBaseTokenLimitGur>> {
        match read_margin_base_token_limit_gur_from_storage() {
            Some(margin_base_token_limit_gur) => token_ids
                .into_iter()
                .map(|token_id| {
                    (
                        token_id.clone(),
                        margin_base_token_limit_gur.get(&token_id).map(|v| v.into()),
                    )
                })
                .collect(),
            None => Default::default(),
        }
    }

    pub fn get_margin_base_token_limit_gur_paged(
        &self,
        from_index: Option<u64>,
        limit: Option<u64>,
    ) -> HashMap<TokenId, MarginBaseTokenLimitGur> {
        match read_margin_base_token_limit_gur_from_storage() {
            Some(margin_base_token_limit_gur) => {
                let keys = margin_base_token_limit_gur.keys_as_vector();
                let from_index = from_index.unwrap_or(0);
                let limit = limit.unwrap_or(keys.len());
                (from_index..std::cmp::min(keys.len(), from_index + limit))
                    .map(|index| {
                        let key = keys.get(index).unwrap();
                        (
                            key.clone(),
                            margin_base_token_limit_gur.get(&key).unwrap().into(),
                        )
                    })
                    .collect()
            }
            None => Default::default(),
        }
    }

    pub fn list_margin_base_token_limit(
        &self,
        token_ids: Vec<TokenId>,
    ) -> HashMap<TokenId, Option<MarginBaseTokenLimit>> {
        match read_margin_base_token_limit_from_storage() {
            Some(margin_base_token_limit) => token_ids
                .into_iter()
                .map(|token_id| {
                    (
                        token_id.clone(),
                        margin_base_token_limit.get(&token_id).map(|v| v.into()),
                    )
                })
                .collect(),
            None => Default::default(),
        }
    }

    pub fn get_margin_base_token_limit_paged(
        &self,
        from_index: Option<u64>,
        limit: Option<u64>,
    ) -> HashMap<TokenId, MarginBaseTokenLimit> {
        match read_margin_base_token_limit_from_storage() {
            Some(margin_base_token_limit) => {
                let keys = margin_base_token_limit.keys_as_vector();
                let from_index = from_index.unwrap_or(0);
                let limit = limit.unwrap_or(keys.len());
                (from_index..std::cmp::min(keys.len(), from_index + limit))
                    .map(|index| {
                        let key = keys.get(index).unwrap();
                        (
                            key.clone(),
                            margin_base_token_limit.get(&key).unwrap().into(),
                        )
                    })
                    .collect()
            }
            None => Default::default(),
        }
    }

    pub fn get_default_margin_base_token_limit(&self) -> MarginBaseTokenLimit {
        MarginBaseTokenLimit::default_limit(&self.internal_margin_config())
    }
}
