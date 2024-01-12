# Feature: LP Token as Collateral

### Requirement
To let those liquidity in ref.finance could be take part into Burrow activities, enable lp token could be used as collateral to borrow assets out.    

Meanwhile, those lp token could still be staked into farm to earn reward.

### Basic Design
To enable "double spend", those lp tokens are not really transferred into burrow or farm contract. Only their shadows will be casted out.  

Now, there are 2 kinds of shadows available in ref lp token. One is used in burrow as collateral, we can name it B-Shadow. Another is used in boost-farm as staking seed, we can call it F-Shadow. LP token can have mulitiple kinds shadows at the same time.  

Only those lp token without any shadow could be transferred. So, shadow also acts as a special locker.  

Let's say Alice have 100 lp token. Then, she can have 80 F-Shadow used in boost-farm, and meanwhile have 95 B-Shadow to used in burrow as collateral to borrow Near out. In this case, Alice would have only 5 lp token that can be transfer freely.  

If Alice's debt in burrow encounters a liquidation and the liquidator want to claim 45 lp token from Alice, the protocol will notify ref dex as a trustee entity, to uncast 45 B-Shadow from Alice, and prepare 45 transferrable lp token. So, ref dex would figure out how many other shadow will be uncasted (in this case, 25 F-Shadow from the boost-farm contract), and do the uncast action. After liquidation, the liquidator would got 45 lp token without any shadow, and Alice has 55 lp token with the same amount of F-Shadow in boost-farm contract and 50 B-Shadow in the burrow contract as collateral.  

Another issue we need to take care of is the lp token value. The core idea is to breakdown the lp token into 2 or more backend assets, use values of those assets to evaluate lp token value, of coure with a fluctuation rate. So, the lp token contract, that is ref dex in this case, would act as a kind of Oracle, to provide lp token real-time inside tokens portion information on chain. 

As LP token has a little different model of asset, we take it separately from user's regular debt. Now, a user could have multiple debt instead a whole one before. He could have one regular debt, mutilple lp token debts (which use a lp token as collateral). Each debt has its own collateral asset, own health factor, therefore, the liquidation would cast debt by debt. Once a lp token debt was liquidated, it won't effect the user's regular debt or other lp token debt.

### regular work flow
The work flow is arround this action:
```rust
pub enum ShadowActions {
    ToFarming,
    FromFarming,
    ToBurrowland,
    FromBurrowland,
}
```
ref_dex, which is the lp token contract, is responsible for management of shadows.

#### deposit lp token to Burrow

- LP call ref_dex::shadow_action with ShadowActions::ToBurrowland
- ref_dex call Burrow::on_cast_shadow
- Burrow takes it as a lp token deposit
- ref_dex check the cross contract call result  

It's something like ft_transfer_call.

#### withdraw lp token from Burrow
- LP call ref_dex::shadow_action with ShadowActions::FromBurrowland
- ref_dex call Burrow::on_remove_shadow
- Burrow takes it as a lp token withdraw
- ref_dex check the cross contract call result  

#### stake lp token to Farm

- LP call ref_dex::shadow_action with ShadowActions::ToFarming
- ref_dex call boost_farm::on_cast_shadow
- boost_farm takes it as a lp token stake
- ref_dex check the cross contract call result  

It's something like ft_transfer_call.

#### unstake lp token from Farm
- LP call ref_dex::shadow_action with ShadowActions::FromFarming
- ref_dex call boost_farm::on_remove_shadow
- boost_farm takes it as a lp token unstake
- ref_dex check the cross contract call result  

### liquidation and forceclose work flow
- liquidator finds a liquidatable lp token debt
- liquidator call Burrow with liquidate sub command
- Burrow execute liquidation logic 
- Burrow call ref_dex::on_burrow_liquidation to claim debt collateral
- ref_dex uncast shadows to ensure enough free lp token and transfer to liquidator
- Burrow check the cross contract call result