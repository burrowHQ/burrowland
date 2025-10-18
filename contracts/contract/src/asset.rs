use crate::*;
use crate::events::emit::{FeeDetail, fee_detail};

pub const MS_PER_YEAR: u64 = 31536000000;

pub const MIN_RESERVE_SHARES: u128 = 10u128.pow(6);

static ASSETS: Lazy<Mutex<HashMap<TokenId, Option<Asset>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(BorshSerialize, BorshDeserialize, Serialize, Clone)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, Deserialize))]
#[serde(crate = "near_sdk::serde")]
pub struct Asset {
    /// Total supplied including collateral, but excluding reserved.
    pub supplied: Pool,
    /// Total borrowed.
    pub borrowed: Pool,
    /// Total margin debt.
    pub margin_debt: Pool,
    /// borrowed by margin position and currently in trading process
    #[serde(with = "u128_dec_format")]
    pub margin_pending_debt: Balance,
    /// total position in margin
    #[serde(with = "u128_dec_format")]
    pub margin_position: Balance,
    /// The amount reserved for the stability. This amount can also be borrowed and affects
    /// borrowing rate.
    #[serde(with = "u128_dec_format")]
    pub reserved: Balance,
    /// Obsolete: The amount belongs to the protocol. This amount can also be borrowed and affects
    /// borrowing rate.
    #[serde(with = "u128_dec_format")]
    pub prot_fee: Balance,
    /// the amounts belongs to each benficiaries, can not be borrowed, can not affect borrowing rate.
    pub beneficiary_fees: HashMap<AccountId, U128>,
    /// The accumulated holding margin position interests till self.last_update_timestamp.
    #[serde(with = "u128_dec_format")]
    pub unit_acc_hp_interest: Balance,
    /// When the asset was last updated. It's always going to be the current block timestamp.
    #[serde(with = "u64_dec_format")]
    pub last_update_timestamp: Timestamp,
    /// The asset config.
    pub config: AssetConfig,
    /// Total lostfound
    #[serde(with = "u128_dec_format")]
    pub lostfound_shares: Balance,
    /// pending emit fee events
    #[borsh_skip]
    #[serde(skip)]
    pub pending_fee_events: Option<Vec<FeeDetail>>,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub enum VAsset {
    V0(AssetV0),
    V1(AssetV1),
    V2(AssetV2),
    V3(AssetV3),
    V4(AssetV4),
    V5(AssetV5),
    Current(Asset),
}

impl From<VAsset> for Asset {
    fn from(v: VAsset) -> Self {
        match v {
            VAsset::V0(v) => v.into(),
            VAsset::V1(v) => v.into(),
            VAsset::V2(v) => v.into(),
            VAsset::V3(v) => v.into(),
            VAsset::V4(v) => v.into(),
            VAsset::V5(v) => v.into(),
            VAsset::Current(c) => c,
        }
    }
}

impl From<Asset> for VAsset {
    fn from(c: Asset) -> Self {
        VAsset::Current(c)
    }
}

impl Asset {
    pub fn new(timestamp: Timestamp, config: AssetConfig) -> Self {
        Self {
            supplied: Pool::new(),
            borrowed: Pool::new(),
            margin_debt: Pool::new(),
            margin_pending_debt: 0,
            margin_position: 0,
            reserved: 0,
            prot_fee: 0,
            beneficiary_fees: HashMap::new(),
            unit_acc_hp_interest: 0,
            last_update_timestamp: timestamp,
            config,
            lostfound_shares: 0,
            pending_fee_events: None,
        }
    }

    pub fn get_rate(&self) -> BigDecimal {
        self.config.get_rate(
            self.borrowed.balance + self.margin_debt.balance + self.margin_pending_debt,
            self.supplied.balance + self.reserved + self.prot_fee,
        )
    }

    pub fn get_margin_debt_rate(&self, margin_debt_discount_rate: u32) -> BigDecimal {
        (self.get_rate() - BigDecimal::one()) * BigDecimal::from_ratio(margin_debt_discount_rate) + BigDecimal::one()
    }

    pub fn get_margin_debt_apr(&self, margin_debt_discount_rate: u32) -> BigDecimal {
        let rate = self.get_margin_debt_rate(margin_debt_discount_rate);
        rate.pow(MS_PER_YEAR) - BigDecimal::one()
    }

    pub fn get_borrow_apr(&self) -> BigDecimal {
        let rate = self.get_rate();
        rate.pow(MS_PER_YEAR) - BigDecimal::one()
    }

    pub fn get_supply_apr(&self, margin_debt_discount_rate: u32) -> BigDecimal {
        if self.supplied.balance == 0 || (self.borrowed.balance == 0 && self.margin_debt.balance == 0) {
            return BigDecimal::zero();
        }

        let borrow_apr = self.get_borrow_apr();
        let margin_debt_apr = self.get_margin_debt_apr(margin_debt_discount_rate);
        if borrow_apr == BigDecimal::zero() && margin_debt_apr == BigDecimal::zero() {
            return BigDecimal::zero();
        }

        let borrow_interest = borrow_apr.round_mul_u128(self.borrowed.balance);
        let supply_interest_borrow_part = ratio(borrow_interest, MAX_RATIO - self.config.reserve_ratio);
        
        let margin_debt_interest = margin_debt_apr.round_mul_u128(self.margin_debt.balance);
        let supply_interest_margin_debt_part = ratio(margin_debt_interest, MAX_RATIO - self.config.reserve_ratio);
        
        BigDecimal::from(supply_interest_borrow_part + supply_interest_margin_debt_part).div_u128(self.supplied.balance)
    }

    /// return total distributed amount
    fn distribute_beneficiaries(&mut self, reward: u128) -> u128 {
        let mut fees: HashMap<AccountId, u128> = HashMap::new();
        let mut total_fee = 0_u128;
        for (beneficiary, rate) in self.config.beneficiaries.iter() {
            fees.insert(beneficiary.clone(), ratio(reward, *rate));
        }
        for (beneficiary, amount) in fees.iter() {
            total_fee += *amount;
            if let Some(fee) = self.beneficiary_fees.get_mut(beneficiary) {
                *fee = U128(fee.0 + *amount);
            } else {
                self.beneficiary_fees.insert(beneficiary.clone(), U128(*amount));
            }
        }
        total_fee
    }

    // n = 31536000000 ms in a year (365 days)
    //
    // Compute `r` from `X`. `X` is desired APY
    // (1 + r / n) ** n = X (2 == 200%)
    // n * log(1 + r / n) = log(x)
    // log(1 + r / n) = log(x) / n
    // log(1 + r  / n) = log( x ** (1 / n))
    // 1 + r / n = x ** (1 / n)
    // r / n = (x ** (1 / n)) - 1
    // r = n * ((x ** (1 / n)) - 1)
    // n = in millis
    fn compound(&mut self, token_id: &TokenId, time_diff_ms: Duration, margin_debt_discount_rate: u32) {
        // handle common borrowed
        let rate = self.get_rate();
        let interest =
            rate.pow(time_diff_ms).round_mul_u128(self.borrowed.balance) - self.borrowed.balance;
        let mut normal_fee_detail = FeeDetail::new("normal".to_string(), token_id.clone(), interest);
        let reserved = ratio(interest, self.config.reserve_ratio);
        if self.supplied.shares.0 > 0 {
            self.supplied.balance += interest - reserved;
            let prot_fee = self.distribute_beneficiaries(reserved);
            self.reserved += reserved - prot_fee;
            normal_fee_detail.reserved = reserved - prot_fee;
            normal_fee_detail.prot_fee = prot_fee;
        } else {
            let prot_fee = self.distribute_beneficiaries(interest);
            self.reserved += interest - prot_fee;
            normal_fee_detail.reserved = interest - prot_fee;
            normal_fee_detail.prot_fee = prot_fee;
        }
        self.borrowed.balance += interest;
        self.add_pending_fee_events(normal_fee_detail);

        // handle margin debt
        let margin_debt_rate = self.get_margin_debt_rate(margin_debt_discount_rate);
        let interest = margin_debt_rate
            .pow(time_diff_ms)
            .round_mul_u128(self.margin_debt.balance)
            - self.margin_debt.balance;
        let mut margin_fee_detail = FeeDetail::new("margin".to_string(), token_id.clone(), interest);
        let reserved = ratio(interest, self.config.reserve_ratio);
        if self.supplied.shares.0 > 0 {
            self.supplied.balance += interest - reserved;
            let prot_fee = self.distribute_beneficiaries(reserved);
            self.reserved += reserved - prot_fee;
            margin_fee_detail.reserved = reserved - prot_fee;
            margin_fee_detail.prot_fee = prot_fee;
        } else {
            let prot_fee = self.distribute_beneficiaries(interest);
            self.reserved += interest - prot_fee;
            margin_fee_detail.reserved = interest - prot_fee;
            margin_fee_detail.prot_fee = prot_fee;
        }
        self.margin_debt.balance += interest;
        self.add_pending_fee_events(margin_fee_detail);
    }

    pub fn update(&mut self, token_id: &TokenId, margin_debt_discount_rate: u32) {
        let timestamp = env::block_timestamp();
        let time_diff_ms = nano_to_ms(timestamp - self.last_update_timestamp);
        if time_diff_ms > 0 {
            // update
            self.last_update_timestamp += ms_to_nano(time_diff_ms);
            self.compound(token_id, time_diff_ms, margin_debt_discount_rate);
            // update unit accumulated holding position interest
            let hp_rate = BigDecimal::from(self.config.holding_position_fee_rate);
            self.unit_acc_hp_interest += hp_rate.pow(time_diff_ms).round_mul_u128(UNIT) - UNIT;
        }
    }

    pub fn available_amount(&self) -> Balance {
        // old version: self.supplied.balance + self.reserved + self.prot_fee - self.borrowed.balance
        // margin_position can NOT be used for lending, we must ensure a postition can be decreased at any time.
        self.supplied.balance + self.reserved + self.prot_fee
            - self.borrowed.balance
            - self.margin_debt.balance
            - self.margin_pending_debt
    }

    pub fn increase_margin_pending_debt(&mut self, amount: Balance, pending_debt_scale: u32) {
        self.margin_pending_debt += amount;
        assert!(
            u128_ratio(self.available_amount(), pending_debt_scale as u128, MAX_RATIO as u128)
                > self.margin_pending_debt,
            "Pending debt will overflow"
        );
    }

    pub fn add_pending_fee_events(&mut self, fee_detail: FeeDetail) {
        if let Some(pending_fee_events) = self.pending_fee_events.as_mut() {
            pending_fee_events.push(fee_detail);
        } else {
            self.pending_fee_events = Some(vec![fee_detail]);
        }
    }

    pub fn send_pending_fee_events(&mut self) {
        if let Some(pending_fee_events) = self.pending_fee_events.take() {
            for fee_event in pending_fee_events {
                fee_detail(fee_event);
            }
        }
    }
}

impl Contract {
    pub fn internal_unwrap_asset(&self, token_id: &TokenId) -> Asset {
        self.internal_get_asset(token_id).expect("Asset not found")
    }

    pub fn internal_get_asset(&self, token_id: &TokenId) -> Option<Asset> {
        let mut cache = ASSETS.lock().unwrap();
        cache.get(token_id).cloned().unwrap_or_else(|| {
            let asset = self.assets.get(token_id).map(|o| {
                let mut asset: Asset = o.into();
                asset.update(token_id, self.internal_margin_config().margin_debt_discount_rate);
                asset
            });
            cache.insert(token_id.clone(), asset.clone());
            asset
        })
    }

    pub fn internal_set_asset(&mut self, token_id: &TokenId, asset: Asset) {
        if self.is_reliable_liquidator_context {
            self.internal_set_asset_without_asset_basic_check(token_id, asset);
        } else {
            self.internal_set_asset_with_basic_check(token_id, asset);
        }
    }

    pub fn internal_set_asset_with_basic_check(&mut self, token_id: &TokenId, asset: Asset) {
        assert!(asset.supplied.shares.0 == 0 ||
            asset.supplied.shares.0 >= MIN_RESERVE_SHARES, "Asset {} supply shares cannot be less than {}", token_id, MIN_RESERVE_SHARES);
        assert!(asset.borrowed.shares.0 == 0 ||
            asset.borrowed.shares.0 >= MIN_RESERVE_SHARES, "Asset {} borrow shares cannot be less than {}", token_id, MIN_RESERVE_SHARES);
        assert!(asset.margin_debt.shares.0 == 0 ||
            asset.margin_debt.shares.0 >= MIN_RESERVE_SHARES, "Asset {} margin_debt shares cannot be less than {}", token_id, MIN_RESERVE_SHARES);

        // Common save logic
        self.internal_save_asset_common(token_id, asset);
    }

    pub fn internal_set_asset_without_asset_basic_check(&mut self, token_id: &TokenId, asset: Asset) {
        // Common save logic only
        self.internal_save_asset_common(token_id, asset);
    }

    fn internal_save_asset_common(&mut self, token_id: &TokenId, mut asset: Asset) {
        assert!(
            asset.borrowed.shares.0 > 0 || asset.borrowed.balance == 0,
            "Borrowed invariant broken"
        );
        if asset.supplied.shares.0 == 0 && asset.supplied.balance > 0 {
            asset.reserved += asset.supplied.balance;
            asset.supplied.balance = 0;
        }
        asset.supplied.assert_invariant();
        asset.borrowed.assert_invariant();
        asset.send_pending_fee_events();
        ASSETS
            .lock()
            .unwrap()
            .insert(token_id.clone(), Some(asset.clone()));
        self.assets.insert(token_id, &asset.into());
    }
}

#[near_bindgen]
impl Contract {
    /// Returns an asset for a given token_id.
    pub fn get_asset(&self, token_id: AccountId) -> Option<AssetDetailedView> {
        self.internal_get_asset(&token_id)
            .map(|asset| self.asset_into_detailed_view(token_id, asset))
    }

    /// Returns an list of pairs (token_id, asset) for assets a given list of token_id.
    /// Only returns pais for existing assets.
    pub fn get_assets(&self, token_ids: Vec<AccountId>) -> Vec<AssetDetailedView> {
        token_ids
            .into_iter()
            .filter_map(|token_id| {
                self.internal_get_asset(&token_id)
                    .map(|asset| self.asset_into_detailed_view(token_id, asset))
            })
            .collect()
    }

    /// Returns a list of pairs (token_id, asset) for assets from a given index up to a given limit.
    pub fn get_assets_paged(
        &self,
        from_index: Option<u64>,
        limit: Option<u64>,
    ) -> Vec<(TokenId, Asset)> {
        let keys = self.asset_ids.as_vector();
        let from_index = from_index.unwrap_or(0);
        let limit = limit.unwrap_or(keys.len());
        (from_index..std::cmp::min(keys.len(), limit))
            .map(|index| {
                let key = keys.get(index).unwrap();
                let mut asset: Asset = self.assets.get(&key).unwrap().into();
                asset.update(&key, self.internal_margin_config().margin_debt_discount_rate);
                (key, asset)
            })
            .collect()
    }

    pub fn get_assets_paged_detailed(
        &self,
        from_index: Option<u64>,
        limit: Option<u64>,
    ) -> Vec<AssetDetailedView> {
        let keys = self.asset_ids.as_vector();
        let from_index = from_index.unwrap_or(0);
        let limit = limit.unwrap_or(keys.len());
        (from_index..std::cmp::min(keys.len(), limit))
            .map(|index| {
                let token_id = keys.get(index).unwrap();
                let mut asset: Asset = self.assets.get(&token_id).unwrap().into();
                asset.update(&token_id, self.internal_margin_config().margin_debt_discount_rate);
                self.asset_into_detailed_view(token_id, asset)
            })
            .collect()
    }
}

#[cfg(test)]
pub fn clean_assets_cache() {
    let mut cache = ASSETS.lock().unwrap();
    cache.clear();
}
