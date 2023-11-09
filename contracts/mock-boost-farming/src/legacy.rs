use crate::*;

#[derive(BorshDeserialize, BorshSerialize)]
pub struct ContractDataV0100 {
    pub owner_id: AccountId,
    pub operators: UnorderedSet<AccountId>,
    pub config: LazyOption<Config>,
    pub seeds: UnorderedMap<SeedId, VSeed>,
    pub farmers: LookupMap<AccountId, VFarmer>,
    pub outdated_farms: UnorderedMap<FarmId, VSeedFarm>,
    // all slashed seed would recorded in here
    pub seeds_slashed: UnorderedMap<SeedId, Balance>,
    // if unstake seed encounter error, the seed would go to here
    pub seeds_lostfound: UnorderedMap<SeedId, Balance>,

    // for statistic
    farmer_count: u64,
    farm_count: u64,
}

impl From<ContractDataV0100> for ContractData {
    fn from(a: ContractDataV0100) -> Self {
        let ContractDataV0100 {
            owner_id,
            operators,
            config,
            seeds,
            farmers,
            outdated_farms,
            seeds_slashed,
            seeds_lostfound,
            farmer_count,
            farm_count,
        } = a;
        Self {
            owner_id,
            next_owner_id: None,
            next_owner_accept_deadline: None,
            operators,
            config,
            seeds,
            farmers,
            outdated_farms,
            seeds_slashed,
            seeds_lostfound,
            farmer_count,
            farm_count,
            state: RunningState::Running,
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct FarmerV0 {
    /// A copy of an farmer ID. Saves one storage_read when iterating on farmers.
    pub farmer_id: AccountId,
    /// Amounts of various reward tokens the farmer claimed.
    pub rewards: HashMap<AccountId, Balance>,
    /// Various seed tokens the farmer staked.
    pub seeds: UnorderedMap<SeedId, FarmerSeed>,
}

impl From<FarmerV0> for Farmer {
    fn from(a: FarmerV0) -> Self {
        let FarmerV0 {
            farmer_id,
            rewards,
            seeds,
        } = a;
        Self {
            farmer_id: farmer_id.clone(),
            sponsor_id: farmer_id.clone(),
            rewards,
            seeds,
        }
    }
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct ContractDataV0101 {
    pub owner_id: AccountId,
    pub state: RunningState,
    pub operators: UnorderedSet<AccountId>,
    pub config: LazyOption<Config>,
    pub seeds: UnorderedMap<SeedId, VSeed>,
    pub farmers: LookupMap<AccountId, VFarmer>,
    pub outdated_farms: UnorderedMap<FarmId, VSeedFarm>,
    // all slashed seed would recorded in here
    pub seeds_slashed: UnorderedMap<SeedId, Balance>,
    // if unstake seed encounter error, the seed would go to here
    pub seeds_lostfound: UnorderedMap<SeedId, Balance>,

    // for statistic
    farmer_count: u64,
    farm_count: u64,
}

impl From<ContractDataV0101> for ContractData {
    fn from(a: ContractDataV0101) -> Self {
        let ContractDataV0101 {
            owner_id,
            state,
            operators,
            config,
            seeds,
            farmers,
            outdated_farms,
            seeds_slashed,
            seeds_lostfound,
            farmer_count,
            farm_count,
        } = a;
        Self {
            owner_id,
            next_owner_id: None,
            next_owner_accept_deadline: None,
            state,
            operators,
            config,
            seeds,
            farmers,
            outdated_farms,
            seeds_slashed,
            seeds_lostfound,
            farmer_count,
            farm_count,
            
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Clone)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, Deserialize))]
#[serde(crate = "near_sdk::serde")]
pub struct SeedFarmV0 {
    pub farm_id: FarmId,

    pub terms: FarmTerms,

    /// total reward send into this farm by far,
    /// every time reward deposited in, add to this field
    #[serde(with = "u128_dec_format")]
    pub total_reward: Balance,

    #[serde(with = "u64_dec_format")]
    pub distributed_at: Timestamp,

    /// The amount of rewards has been distributed.
    /// remaining_reward = total_reward - distributed_reward
    #[serde(with = "u128_dec_format")]
    pub distributed_reward: Balance,

    /// reward token has been claimed by farmer
    #[serde(with = "u128_dec_format")]
    pub claimed_reward: Balance,

    /// when there is no seed token staked, reward goes to beneficiary
    #[serde(with = "u128_dec_format")]
    pub amount_of_beneficiary: Balance,

    #[serde(skip)]
    pub rps: BigDecimal,

    #[borsh_skip]
    pub status: Option<FarmStatus>,
}

impl SeedFarmV0 {
    pub fn has_ended(&self) -> bool {
        match self.status.as_ref().unwrap() {
            FarmStatus::Ended => true,
            _ => false,
        }
    }

    pub fn internal_update_status(&mut self, block_ts: u64) {
        if self.terms.start_at == 0 || to_nano(self.terms.start_at) >= block_ts {
            self.status = Some(FarmStatus::Created);
        } else if self.total_reward == 0 && to_nano(self.terms.start_at) < block_ts {
            self.status = Some(FarmStatus::Pending);
        } else if self.total_reward > 0 && self.distributed_reward >= self.total_reward {
            self.status = Some(FarmStatus::Ended);
        } else {
            self.status = Some(FarmStatus::Running);
        }
    }

    pub fn update(&mut self, seed_power: Balance) {
        let block_ts = env::block_timestamp();

        self.internal_update_status(block_ts);

        if block_ts <= self.distributed_at {
            // already updated, skip
            return;
        }

        match self.status.as_ref().unwrap() {
            FarmStatus::Ended => {
                self.distributed_at = block_ts;
            },
            FarmStatus::Running => {
                let reward = std::cmp::min(
                    self.total_reward - self.distributed_reward,
                    u128_ratio(
                        self.terms.daily_reward,
                        u128::from(block_ts - self.distributed_at),
                        u128::from(NANOS_PER_DAY),
                    ),
                );
                self.distributed_reward += reward;
                if seed_power > 0 {
                    self.rps = self.rps + BigDecimal::from(reward).div_u128(seed_power);
                } else {
                    self.amount_of_beneficiary += reward;
                }
                self.distributed_at = block_ts;
                self.internal_update_status(block_ts);
            },
            _ => {},
        }
    }

    pub fn add_reward(&mut self, reward_token: &AccountId, amount: Balance) -> (Balance, u32) {
        require!(self.terms.reward_token == reward_token.clone(), E404_UNMATCHED_REWARD_TOKEN);
        if self.terms.start_at == 0 {
            self.terms.start_at = nano_to_sec(env::block_timestamp());
            self.distributed_at = env::block_timestamp();
        }
        self.total_reward += amount;
        (self.total_reward, self.terms.start_at)
    }

    pub fn finalize(&mut self) {
        require!(self.has_ended(), E405_FARM_NOT_ENDED);
        // remaining unclaimed rewards belongs to beneficiary
        self.amount_of_beneficiary = 
            self.distributed_reward - self.claimed_reward;
    }

}

impl From<SeedFarmV0> for SeedFarm {
    fn from(a: SeedFarmV0) -> Self {
        let SeedFarmV0 {
            farm_id,
            terms,
            total_reward,
            distributed_at,
            distributed_reward,
            claimed_reward,
            amount_of_beneficiary,
            rps,
            status
        } = a;
        Self {
            farm_id,
            terms,
            total_reward,
            distributed_at,
            distributed_reward,
            claimed_reward,
            amount_of_beneficiary,
            amount_of_withdrew_beneficiary: 0,
            rps,
            status
        }
    }
}

