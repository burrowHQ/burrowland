use crate::*;

#[derive(BorshSerialize, BorshDeserialize, Serialize, Default, Clone)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, Deserialize))]
#[serde(crate = "near_sdk::serde")]
pub struct BoosterStaking {
    /// The amount of Booster token staked.
    #[serde(with = "u128_dec_format")]
    pub staked_booster_amount: Balance,
    /// The amount of xBooster token.
    #[serde(with = "u128_dec_format")]
    pub x_booster_amount: Balance,
    /// When the staked Booster token can be unstaked in nanoseconds.
    #[serde(with = "u64_dec_format")]
    pub unlock_timestamp: u64,
}

#[near_bindgen]
impl Contract {
    /// Stakes a given amount (or all supplied) booster token for a given duration in seconds.
    /// If the previous stake exists, then the new duration should be longer than the previous
    /// remaining staking duration.
    #[payable]
    pub fn account_stake_booster(
        &mut self,
        booster_token_id: AccountId,
        amount: Option<U128>,
        duration: DurationSec,
    ) {
        assert_one_yocto();
        let booster_tokens = read_booster_tokens_from_storage();
        let mut booster_token_info = booster_tokens
            .get(&booster_token_id)
            .cloned()
            .expect(format!("Invalid booster token id {}", booster_token_id).as_str());
        require!(booster_token_info.enable, "Disabled booster token ID");

        require!(
            duration >= booster_token_info.minimum_staking_duration_sec
                && duration <= booster_token_info.maximum_staking_duration_sec,
            "Duration is out of range"
        );

        let account_id = env::predecessor_account_id();

        // Set reliable liquidator context if caller is in whitelist
        self.is_reliable_liquidator_context = in_reliable_liquidator_whitelist(&account_id.to_string());

        require!(
            !self.blacklist_of_farmers.contains(&account_id),
            "Blacklisted account"
        );
        let mut account = self.internal_unwrap_account(&account_id);

        // Computing and withdrawing amount from supplied.
        let mut asset = self.internal_unwrap_asset(&booster_token_id);
        let mut account_asset = account.internal_unwrap_asset(&booster_token_id);

        let (shares, amount) = if let Some(amount) = amount.map(|a| a.0) {
            (asset.supplied.amount_to_shares(amount, true), amount)
        } else {
            (
                account_asset.shares,
                asset.supplied.shares_to_amount(account_asset.shares, false),
            )
        };
        require!(
            shares.0 > 0 && amount > 0,
            "The amount should be greater than zero"
        );

        account_asset.withdraw_shares(shares);
        account.internal_set_asset(&booster_token_id, account_asset);

        asset.supplied.withdraw(shares, amount);
        self.internal_set_asset(&booster_token_id, asset);

        // Computing amount of the new xBooster token and new unlock timestamp.
        let timestamp = env::block_timestamp();
        let new_duration_ns = sec_to_nano(duration);
        let new_unlock_timestamp_ns = timestamp + new_duration_ns;

        account.sync_booster_policy(&booster_tokens);

        let mut booster_staking = account
            .booster_stakings
            .remove(&booster_token_id)
            .map(|mut booster_staking| {
                assert!(
                    booster_staking.unlock_timestamp <= new_unlock_timestamp_ns,
                    "The new staking duration is shorter than the current remaining staking duration"
                );
                let restaked_x_booster_amount = compute_x_booster_amount(
                    &booster_token_info,
                    booster_staking.staked_booster_amount,
                    new_duration_ns,
                );
                booster_staking.x_booster_amount =
                    std::cmp::max(booster_staking.x_booster_amount, restaked_x_booster_amount);
                booster_staking
            })
            .unwrap_or_default();
        booster_staking.unlock_timestamp = new_unlock_timestamp_ns;
        booster_staking.staked_booster_amount += amount;
        let extra_x_booster_amount =
            compute_x_booster_amount(&booster_token_info, amount, new_duration_ns);
        booster_staking.x_booster_amount += extra_x_booster_amount;

        events::emit::booster_stake(
            &account_id,
            &booster_token_id,
            amount,
            duration,
            extra_x_booster_amount,
            &booster_staking,
        );

        booster_token_info.total_stake_amount += amount;
        self.internal_set_booster_token_info(&booster_token_id, booster_token_info);

        account
            .booster_stakings
            .insert(booster_token_id, booster_staking);

        account
            .affected_farms
            .extend(account.get_all_potential_farms());
        self.internal_account_apply_affected_farms(&mut account);
        self.internal_set_account(&account_id, account);
    }

    /// Unstakes all booster token.
    /// The current timestamp must be greater than the unlock_timestamp.
    #[payable]
    pub fn account_unstake_booster(
        &mut self,
        booster_token_id: Option<AccountId>,
        client_echo: Option<String>,
    ) {
        assert_one_yocto();

        let account_id = env::predecessor_account_id();

        // Set reliable liquidator context if caller is in whitelist
        self.is_reliable_liquidator_context = in_reliable_liquidator_whitelist(&account_id.to_string());

        let mut account = self.internal_unwrap_account(&account_id);

        let timestamp = env::block_timestamp();
        let (booster_token_id, booster_staking) = if let Some(booster_token_id) = booster_token_id {
            (
                booster_token_id.clone(),
                account
                    .booster_stakings
                    .remove(&booster_token_id)
                    .expect("No staked booster token"),
            )
        } else {
            (
                self.internal_config().booster_token_id,
                account
                    .booster_staking
                    .take()
                    .expect("No staked booster token"),
            )
        };
        let unstake_amount = booster_staking.staked_booster_amount;

        if let Some(mut booster_token_info) =
            self.internal_get_booster_token_info(&booster_token_id)
        {
            if booster_token_info.enable {
                assert!(
                    booster_staking.unlock_timestamp <= timestamp,
                    "The staking is not unlocked yet"
                );
            }
            booster_token_info.total_stake_amount -= unstake_amount;
            self.internal_set_booster_token_info(&booster_token_id, booster_token_info);
        }

        if let Some(client_echo) = client_echo {
            assert!(in_client_echo_sender_whitelist(account_id.as_str()), "Unauthorized client echo sender: {}", account_id);
            let asset = self.internal_unwrap_asset(&booster_token_id);
            let ft_amount = unstake_amount / 10u128.pow(asset.config.extra_decimals as u32);
            if ft_amount > 0 {
                self.internal_ft_transfer_call(
                    &account_id,
                    &booster_token_id,
                    unstake_amount,
                    ft_amount,
                    client_echo,
                );
                events::emit::withdraw_started(&account_id, unstake_amount, &booster_token_id);
            } else {
                events::emit::withdraw_succeeded(&account_id, unstake_amount, &booster_token_id);
            }
        } else {
            self.internal_deposit(&mut account, &booster_token_id, unstake_amount);
            account
                .affected_farms
                .extend(account.get_all_potential_farms());
            self.internal_account_apply_affected_farms(&mut account);
        }

        events::emit::booster_unstake(&account_id, &booster_token_id, &booster_staking);
        self.internal_set_account(&account_id, account);
    }
}

pub fn compute_x_booster_amount(
    booster_token_info: &BoosterTokenInfo,
    amount: u128,
    duration_ns: Duration,
) -> u128 {
    amount
        + u128_ratio(
            amount,
            u128::from(
                booster_token_info.x_booster_multiplier_at_maximum_staking_duration
                    - MIN_BOOSTER_MULTIPLIER,
            ) * u128::from(duration_ns - to_nano(booster_token_info.minimum_staking_duration_sec)),
            u128::from(to_nano(
                booster_token_info.maximum_staking_duration_sec
                    - booster_token_info.minimum_staking_duration_sec,
            )) * u128::from(MIN_BOOSTER_MULTIPLIER),
        )
}
