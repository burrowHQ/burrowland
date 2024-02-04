use crate::*;

#[derive(Serialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, Deserialize))]
#[serde(crate = "near_sdk::serde")]
pub struct AssetDetailedView {
    pub token_id: TokenId,
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
    /// The amount belongs to the protocol. This amount can also be borrowed and affects
    /// borrowing rate.
    #[serde(with = "u128_dec_format")]
    pub prot_fee: Balance,
    /// The amount of global unit accumulated holding-position interest.
    #[serde(with = "u128_dec_format")]
    pub uahpi: Balance,
    /// When the asset was last updated. It's always going to be the current block timestamp.
    #[serde(with = "u64_dec_format")]
    pub last_update_timestamp: Timestamp,
    /// The asset config.
    pub config: AssetConfig,
    /// Current APR excluding farms for supplying the asset.
    pub supply_apr: BigDecimal,
    /// Current APR excluding farms for borrowing the asset.
    pub borrow_apr: BigDecimal,
    /// Asset farms
    pub farms: Vec<AssetFarmView>,
}

#[derive(Serialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, Deserialize))]
#[serde(crate = "near_sdk::serde")]
pub struct AssetFarmView {
    pub farm_id: FarmId,
    /// Active rewards for the farm
    pub rewards: HashMap<TokenId, AssetFarmReward>,
}

impl Contract {
    pub fn asset_into_detailed_view(&self, token_id: TokenId, asset: Asset) -> AssetDetailedView {
        let farms = self
            .get_asset_farms(vec![
                FarmId::Supplied(token_id.clone()),
                FarmId::Borrowed(token_id.clone()),
            ])
            .into_iter()
            .map(|(farm_id, asset_farm)| AssetFarmView {
                farm_id,
                rewards: asset_farm.rewards,
            })
            .collect();
        let supply_apr = asset.get_supply_apr(self.internal_margin_config().margin_debt_discount_rate);
        let borrow_apr = asset.get_borrow_apr();
        let Asset {
            supplied,
            borrowed,
            margin_debt,
            margin_pending_debt,
            margin_position,
            reserved,
            prot_fee,
            unit_acc_hp_interest,
            last_update_timestamp,
            config,
        } = asset;
        AssetDetailedView {
            token_id,
            supplied,
            borrowed,
            margin_debt,
            margin_pending_debt,
            margin_position,
            reserved,
            prot_fee,
            uahpi: unit_acc_hp_interest,
            last_update_timestamp,
            config,
            supply_apr,
            borrow_apr,
            farms,
        }
    }
}
