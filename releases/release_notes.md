# Release Notes

Version 0.14.1
```bash=
# codehash: 4rJN4v168Uu5aTaVhEu9k4DDATdpfZrgd9w2MYvK3gQC
```
- add protocol debts storage

Version 0.14.0
```bash=
# codehash: 7g3ExMPR2kVzaqawGuUTwh38CuzBnTMeX3oowoHBMVon
```
- fix audit recommendation

- add fee events

- replace workspaces with near-workspaces

- add update_max_active_user_margin_position and update_asset_holding_position_fee_rate

- add some account view funcs

- if the withdraw amount is zero, only log without panic

Version 0.13.0
```bash=
# codehash: DgDSGmJygsCbtWuAbytjgSH99pPTQhDzRkcxrqTnj4xa 
```
- margin trading

Version 0.12.0
```bash=
# codehash: MozRqmM6agrrtLNdWaW852iuATCnRcL3PgxfYdzmb77 
```
- Improve gas usage for querying pyth oracle. 

- Add TokenNetBalance farm type.

- Improve boost algorithm  
  Even with the max log base (u128::MAX), 1M xboost can have the lowest boost ratio of 29.2%, which couldn't meet the product side demand.  
  To address this issue, we introduce a config item boost_suppress_factor: u128, and let xboost be divided by this factor before participating in the boost algorithm.  
  Say if we set boost_suppress_factor to 1K, we could get 14.6% as the lowest boost ratio for 1M xboost.
 
- Adjustment of boost staking period  
  When we reduce the boost token staking period, there should be a way to deal with those existing staking.
  We will re-calculate xboost and follow these criteria:
  - assume users staking their boost token as early as possible;
  - the unlock ts shouldn't exceed the max staking period in the current configuration;
  - the new xboost shouldn't exceed the max xboost in the current configuration;
  - Users get the best xboost if not violate previous criteria;
 
- farmer blacklist  
  Those accounts in the farmer blacklist won't get any further reward from farming. This is used for the possible requirement that relative parties won't compete for farm rewards with regular farmers.

Version 0.10.0
```bash=
# codehash: GcntYxNjD6y4XhiJuyd6ar4FQoTY3ZA1wQ3VJfraX4pC 
```
- Support pyth oracle and switch between pyth and priceoracle.

Version 0.9.1
```bash=
# codehash: 8wSzoqHRtNXdV1xTwT6JvD7HYXLqKwqJyskeR9BkCdcv 
```
- fix an old account auto-upgradation issue in liquidation.

Version 0.9.0
```bash=
# codehash: DUBWfFT1h3NNtvngw22SenyDpeUGN5PRLcNxaopMUpNe 
```
- lp as collateral.

Version 0.8.0
```bash=
# codehash: 7b2DjxtjCHJA5wDRgpMEQVRp2qUTZrL8eZWLkE9wrxXh
```
- fix bug in func account_into_detailed_view.
- set a portion of interest of reserve to protocol fee.
- enable transfer between reserve and protocol supply.

Version 0.7.0
```bash=
# codehash: 8EoF8mXSAYV3HTGyZkSJJJ65xPgzTuj9D442LAjXidXr
```
- baseline.
