# Margin Trading Stop-Loss/Stop-Profit Feature

## Overview

The stop-loss/stop-profit feature enables automated position management for margin trading on Burrowland. Users can set price-based triggers that automatically close their positions when profit or loss thresholds are reached, similar to stop orders on traditional trading platforms.

This feature leverages a decentralized keeper model where anyone can monitor and execute stop orders in exchange for a service fee, ensuring timely execution without requiring centralized infrastructure.

## Key Concepts

### Stop-Loss
A stop-loss protects traders from excessive losses by automatically closing a position when the remaining collateral value falls to a specified percentage of the original collateral.

**Example:** User sets a 9000 BPS (90%) stop-loss on a position with 100 USDC collateral. If the position value drops such that only ~90 USDC worth of collateral would remain after closing, the stop triggers.

### Stop-Profit
A stop-profit (take-profit) locks in gains by automatically closing a position when the remaining collateral value reaches a target percentage above the original collateral.

**Example:** User sets a 12000 BPS (120%) stop-profit on a position with 100 USDC collateral. If the position grows such that ~120 USDC worth of collateral would remain after closing, the stop triggers.

### Service Fee
To incentivize keepers to monitor and execute stops, users pay a service fee (configurable by contract admin) when setting a stop. This fee is:
- Deducted from the user's margin supply when the stop is set
- Paid to the keeper who executes the stop
- Refunded to the user if the position is closed normally or the stop is removed

## Architecture

### Data Structures

#### MarginStop
```rust
pub struct MarginStop {
    /// profit rate to collateral in BPS
    pub stop_profit: Option<u32>,  // Target profit in BPS (e.g., 12000 = 120%)
    /// loss rate to collateral in BPS
    pub stop_loss: Option<u32>,     // Stop-loss threshold in BPS (e.g., 9000 = 90%)
    pub service_token_id: TokenId,  // Token used for service fee
    pub service_token_amount: U128, // Amount of service fee locked
}
```

#### MarginStopServiceFee
```rust
pub struct MarginStopServiceFee {
    pub token_id: AccountId,  // Token to charge for service fee
    pub amount: U128,         // Amount to charge (in burrow inner decimals)
}
```

#### DecreaseOperation
Enum representing the different types of position decrease operations:
```rust
pub enum DecreaseOperation {
    Decrease,    // User voluntarily decreasing position
    Close,       // User closing entire position
    Liquidate,   // Liquidator closing unhealthy position
    ForceClose,  // Force closing underwater position
    Stop,        // Keeper executing stop order
}
```

**Helper methods:**
- `from_str(s: &str)` - Converts string operation type to enum
- `is_full_close()` - Returns true for Close, Liquidate, ForceClose, Stop
- `should_repay_from_collateral()` - Returns true for full-close operations
- `can_use_protocol_reserve()` - Returns true only for ForceClose
- `benefits_to_protocol_owner()` - Returns true for Liquidate and ForceClose

#### Helper Structures

**DebtRepaymentResult** - Result of debt repayment calculation:
```rust
pub struct DebtRepaymentResult {
    pub repay_amount: Balance,        // Amount of debt token repaid
    pub repay_shares: Shares,         // Shares of debt repaid
    pub leftover_amount: Balance,     // Amount left over after full debt repayment
    pub holding_fee_paid: Balance,    // Holding position fee paid from this repayment
    pub remaining_debt_cap: Balance,  // Remaining debt cap after this repayment
}
```

#### SettlementBenefits
Accumulated benefits from position settlement:
```rust
#[derive(Default)]
pub struct SettlementBenefits {
    pub collateral_shares: u128,      // Collateral token shares (benefit_m_shares)
    pub debt_token_shares: u128,      // Debt token shares from leftover (benefit_d_shares)
    pub position_token_shares: u128,  // Position token shares (benefit_p_shares)
}
```

**Methods:**
- `has_any()` - Returns true if any benefit shares are non-zero
- `to_margin_updates(&position)` - Converts benefits to `MarginAccountUpdates` keyed by the position's token IDs

**StopServiceFeeInfo** - Stop service fee info extracted during settlement:
```rust
pub struct StopServiceFeeInfo {
    pub token_id: TokenId,
    pub amount: Balance,
}
```

**MarginAccountUpdates** - Tracks account balance updates for each token type:
```rust
pub struct MarginAccountUpdates {
    pub token_c_update: (AccountId, u128),
    pub token_d_update: (AccountId, u128),
    pub token_p_update: (AccountId, u128),
}
```

### Storage

- **Account Level:** Each `MarginAccount` contains `stops: HashMap<PosId, MarginStop>`
- **Account View:** `MarginAccountDetailedView` exposes `stops: HashMap<PosId, MarginStop>` for queries
- **Global Config:** `MARGIN_STOP_SERVICE_FEE` (storage key: "mssf") stores the current fee policy
- **Migration:** Account data migrates from `MarginAccountV0`/`MarginAccountV1` to handle the new `stops` field via `VMarginAccount` enum

### Core Functions

#### Setting Stops

**During Position Opening:**
```rust
MarginAction::OpenPosition {
    // ... existing fields ...
    stop_profit: Option<u32>,
    stop_loss: Option<u32>,
}
```

**After Position is Open:**
```rust
MarginAction::SetStop {
    pos_id: PosId,
    stop_profit: Option<u32>,
    stop_loss: Option<u32>,
}
```

`process_set_stop` handles three cases:
1. **Existing stop, both values null** → Remove stop, refund service fee to user's margin supply
2. **Existing stop, at least one value set** → Refund old fee, charge new fee from current policy, store new stop
3. **No existing stop, at least one value set** → Assert fee policy exists, charge fee, store new stop

#### Executing Stops

```rust
MarginAction::StopMTPosition {
    pos_owner_id: AccountId,
    pos_id: PosId,
    token_p_amount: U128,
    min_token_d_amount: U128,
    swap_indication: SwapIndication,
}
```

### Stop Condition Logic

The `is_stop_active()` function determines if a stop should trigger:

```rust
pub(crate) fn is_stop_active(
    &self,
    mt: &MarginTradingPosition,
    prices: &Prices,
    stop: &MarginStop,
    slippage: u32,
) -> bool
```

**Note:** Currently called with `slippage = 0` when checking stop conditions during `process_decrease_margin_position`.

**Stop-Loss Check:**
```
// target remain collateral: value_collateral.mul_ratio(stop_loss)
// current remain: (value_position + value_collateral).mul_ratio(10000-slippage) - (value_debt + total_hp_fee)

Triggers if: (value_position + value_collateral) * (10000 - slippage) / 10000
           < value_collateral * stop_loss / 10000 + value_debt + total_hp_fee
```

**Stop-Profit Check:**
```
// target gross profit: value_collateral.mul_ratio(stop_profit)
// current remain: (value_position + value_collateral).mul_ratio(10000-slippage) - (value_debt + total_hp_fee)

Triggers if: (value_position + value_collateral) * (10000 - slippage) / 10000
           > value_collateral * stop_profit / 10000 + value_debt + total_hp_fee
```

The calculation accounts for:
- Position token value at current prices (`get_mtp_position_value`)
- Debt token value including accrued interest (`get_mtp_debt_value`)
- Collateral value (`get_mtp_collateral_value`)
- Holding position fees (`get_mtp_hp_fee_value`)
- Slippage when selling position tokens (currently 0)

### Validation Rules

Stop settings are validated with `validate_stop_settings()`:

```rust
fn validate_stop_settings(stop_profit: &Option<u32>, stop_loss: &Option<u32>) {
    if let Some(sl) = stop_loss {
        assert!(
            *sl > 0 && *sl < 10000,
            "Stop loss must be between 1 and 9999 BPS (0.01%-99.99%)"
        );
    }
    if let Some(sp) = stop_profit {
        assert!(
            *sp > 10000,
            "Stop profit must be greater than 10000 BPS (>100%)"
        );
    }
}
```

**Note:** An explicit `stop_loss < stop_profit` check is not needed because the individual bounds enforce it implicitly: `stop_loss < 10000 < stop_profit`.

## Workflow Examples

### Example 1: Setting Stop on New Position

1. User calls `execute_margin_actions` with:
   ```json
   {
     "OpenPosition": {
       "token_c_id": "usdc.token",
       "token_c_amount": "100000000",  // 100 USDC
       "token_d_id": "usdc.token",
       "token_d_amount": "200000000",  // Borrow 200 USDC
       "token_p_id": "eth.token",
       "min_token_p_amount": "150000000000000000",
       "swap_indication": {...},
       "stop_profit": 12000,  // 120% take profit
       "stop_loss": 9000      // 90% stop loss
     }
   }
   ```

2. Contract validates service fee policy exists
3. Service fee (e.g., 1 USDC) is deducted from user's margin supply
4. Position opens with stop settings stored
5. `MarginStop` entry created in account's `stops` HashMap

### Example 2: Keeper Executes Stop-Loss

1. ETH price drops, position becomes unprofitable
2. Keeper monitors and detects stop-loss condition met
3. Keeper calls:
   ```json
   {
     "StopMTPosition": {
       "pos_owner_id": "alice.near",
       "pos_id": "position_123",
       "token_p_amount": "150000000000000000",
       "min_token_d_amount": "180000000",
       "swap_indication": {...}
     }
   }
   ```

4. Contract validates stop condition using `is_stop_active()`
5. Position is closed (swap position tokens, repay debt)
6. Service fee is transferred to keeper's margin supply
7. Remaining value is returned to user's margin supply

### Example 3: Removing Stop

1. User decides to remove stop and manage manually:
   ```json
   {
     "SetStop": {
       "pos_id": "position_123",
       "stop_profit": null,
       "stop_loss": null
     }
   }
   ```

2. Contract removes `MarginStop` entry
3. Service fee is refunded to user's margin supply

### Example 4: Failed Position Open with Stop Set

1. User opens a position with stop settings, but the DEX swap fails
2. During `callback_dex_trade` for failed open:
   - Position is removed from `margin_positions`
   - Collateral is returned to user's margin supply
   - If a `MarginStop` was set, it is removed from `stops`
   - Service fee is refunded to user's margin supply

### Example 5: Normal Position Close with Stop Set

1. User closes position normally before stop triggers
2. During `callback_dex_trade` for position close:
   - Position is removed from `margin_positions`
   - Associated `MarginStop` is removed from `stops`
   - Service fee is refunded to user's margin supply

## Settlement Logic

The `on_decrease_trade_return` callback handles position settlement with a structured approach:

### Settlement Sections

1. **Section 1: Debt Repayment Calculation**
   - Uses `calculate_debt_repayment()` to compute repay amount, shares, leftover, and holding fee
   - Applies debt repayment to asset and position
   - Converts leftover to supply shares as benefit

2. **Section 2: Collateral-Based Repayment**
   - For full-close operations (Close, Liquidate, ForceClose, Stop)
   - Uses `repay_debt_from_collateral()` when token_c == token_d
   - Attempts to repay remaining debt using collateral

3. **Section 3: Protocol Reserve Coverage**
   - Only for ForceClose operations
   - Uses `cover_debt_from_protocol_reserve()` to cover bad debt
   - Tracks protocol debts if reserve is insufficient

4. **Section 4: Position Settlement**
   - For positions with zero debt remaining:
     - `settle_closed_position()` converts assets to benefits
     - Removes position from storage
     - Extracts stop service fee info if present

5. **Section 5: Benefits Distribution**
   - **Liquidate:** Distributed among owner, liquidator, and user based on `liq_benefit_protocol_rate` and `liq_benefit_liquidator_rate`
   - **ForceClose:** All benefits go to protocol owner
   - **Decrease/Close/Stop:** Benefits go to position owner

6. **Section 6: Asset Updates**
   - Saves updated asset state without basic checks

7. **Section 7: Service Fee Settlement**
   - `settle_stop_service_fee()` distributes fee to appropriate recipient
   - For Stop operations: fee goes to keeper (liquidator_id)
   - For other operations: fee refunded to position owner

### Helper Functions

```rust
// Calculate debt repayment with holding position fee
fn calculate_debt_repayment(&self, position, asset_debt, swap_amount) -> DebtRepaymentResult

// Repay remaining debt using collateral (when token_c == token_d)
fn repay_debt_from_collateral(&self, position, asset_debt) -> (Shares, Shares, Balance)

// Cover remaining debt using protocol reserve (forceclose only)
fn cover_debt_from_protocol_reserve(&mut self, position, asset_debt)

// Convert remaining assets to benefits, remove position
fn settle_closed_position(&mut self, account, position, asset_position, pos_id, benefits) -> Option<StopServiceFeeInfo>

// Distribute service fee to keeper or refund to owner
fn settle_stop_service_fee(&mut self, fee_info, operation, position_owner_id, keeper_id)
```

## Economic Model

### Fee Structure

The service fee creates a sustainable keeper ecosystem:

- **Admin Control:** Fee policy set via `set_mssf(token_id, amount)`
- **Locked Capital:** Fee is locked when stop is set, not charged upfront
- **Incentive Alignment:** Keeper only receives fee when successfully executing a valid stop
- **Refund Policy:** Users get fee back if they manage position manually

### Keeper Economics

Keepers profit from:
1. Monitoring positions with active stops
2. Executing stops when conditions are met
3. Collecting service fees

Costs for keepers:
- Gas fees for execution
- Infrastructure for monitoring
- Price oracle data access

### User Economics

Benefits:
- Automated risk management
- Protection from extreme losses
- Ability to lock in profits without constant monitoring
- No upfront cost (fee is refundable)

Costs:
- Service fee (only if stop is executed by a keeper; fee is refunded for normal close/decrease operations)
- Potential slippage when stop executes

## Security Considerations

### Access Control

- **Owner Only:** Only position owner can set/modify stops via `SetStop`
- **Anyone Can Execute:** Any account with a margin account can trigger `StopMTPosition` (similar to liquidation)
- **Admin Only:** Only contract owner can set service fee policy via `set_mssf()` (requires 1 yoctoNEAR)
- **Margin Execute:** All margin actions require exactly 1 yoctoNEAR attached deposit via `margin_execute()`

### Validation

1. **Stop Conditions:** `is_stop_active()` validates mathematical conditions with slippage=0 before allowing execution
2. **Position Locking:** Cannot modify stops while position has pending swap (`is_locking`)
3. **Fee Policy:** Stop setting fails with "Margin stop service fee policy is not set." if `MARGIN_STOP_SERVICE_FEE` is not configured
4. **Sufficient Balance:** User must have enough margin supply to pay service fee
5. **Stop Value Ranges:**
   - Stop loss: Must be between 1 and 9999 BPS (0.01%-99.99%)
   - Stop profit: Must be greater than 10000 BPS (>100%)
6. **Self-Stop Prevention:** Users cannot execute stops on their own positions ("Can't stop yourself")

### Storage Safety

- **Force Updates:** `internal_force_set_margin_account()` handles storage updates in critical callbacks to prevent state inconsistency
- **Fallback Recipient:** If service fee recipient lacks a margin account, fee goes to position owner's account instead
- **Atomic Operations:** Stop settings and fee handling are atomic within transactions
- **Saturating Math:** Storage balance calculations use `saturating_sub()` to prevent underflow issues

### Edge Cases

1. **Slippage Protection:** `min_token_d_amount` prevents excessive slippage during stop execution
2. **Fee Refunds:** Multiple refund paths ensure users don't lose fees unnecessarily:
   - Normal close/decrease of a position with stop set → refunded
   - Stop removal via `SetStop` with both values null → refunded
   - Failed position open (DEX swap failure) with stop set → refunded
3. **Migration Safety:** `MarginAccountV0` → `MarginAccountV1` → `Current` provides backward compatibility
4. **Stop Removal:** Removing both profit and loss stops triggers full refund
5. **Keeper Margin Account Required:** Keeper executing `StopMTPosition` must have a margin account to receive the service fee; otherwise the transaction panics

## Admin Operations

### Setting Service Fee Policy

Requires exactly 1 yoctoNEAR attached and must be called by the contract owner.

```rust
contract.set_mssf(MarginStopServiceFee {
    token_id: "usdc.token".parse().unwrap(),
    amount: U128(1000000), // 1 USDC (6 decimals, in burrow inner decimals)
});
```

### Querying Current Policy

```rust
let fee_policy = contract.get_mssf();
// Returns: Option<MarginStopServiceFee>
```

### Updating Fee Policy

Simply call `set_mssf()` again with new values. Existing positions retain their original fee amounts.

## Events

### set_stop Event

Emitted when a user sets or modifies stop settings:

```json
{
  "event": "set_stop",
  "data": {
    "account_id": "alice.near",
    "stop_profit": 12000,
    "stop_loss": 9000,
    "position": "position_123"
  }
}
```

### margin_stop_started Event

Emitted when a keeper initiates a stop execution (uses existing `margin_decrease_started` infrastructure with `"margin_stop_started"` event name).

## Implementation Notes

### Files Modified

- `contracts/contract/src/margin_stop_service_fee.rs` - Fee policy management (`MarginStopServiceFee`, `set_mssf`, `get_mssf`)
- `contracts/contract/src/margin_accounts.rs` - `MarginStop` struct, `stops` field, `MarginAccountDetailedView` with stops
- `contracts/contract/src/margin_actions.rs` - `StopMTPosition` and `SetStop` actions, processing logic
- `contracts/contract/src/margin_position.rs` - `is_stop_active()`, `validate_stop_settings()`, `process_set_stop()`, stop fee handling during open, failed-open refund in `callback_dex_trade`
- `contracts/contract/src/margin_trading.rs` - Extensively refactored with:
  - `DecreaseOperation` enum
  - `DebtRepaymentResult`, `SettlementBenefits`, `StopServiceFeeInfo`, `MarginAccountUpdates` structs
  - `calculate_debt_repayment()`, `repay_debt_from_collateral()`, `cover_debt_from_protocol_reserve()`
  - `settle_closed_position()`, `settle_stop_service_fee()`
  - Refactored `on_decrease_trade_return()` with clear section-based logic
- `contracts/contract/src/fungible_token.rs` - DEX callback echo handling for stop operations
- `contracts/contract/src/margin_pyth.rs` - Pyth oracle price handling for stop execution
- `contracts/contract/src/events.rs` - `set_stop` event
- `contracts/contract/src/legacy.rs` - `MarginAccountV0`, `MarginAccountV1` migration paths
- `contracts/contract/src/storage_keys.rs` - `MARGIN_STOP_SERVICE_FEE` constant ("mssf")
- `contracts/contract/src/lib.rs` - Module registration

### Testing Recommendations

1. **Unit Tests:**
   - `is_stop_active()` with various price scenarios
   - Service fee calculation and refund logic
   - Storage migration from V1 to Current

2. **Integration Tests:**
   - End-to-end stop-loss execution
   - End-to-end stop-profit execution
   - Fee refund on normal close
   - Fee refund on stop removal
   - Storage failure handling

3. **Simulation Tests:**
   - Keeper competition scenarios
   - Multiple positions with different stops
   - Price movement edge cases
   - Gas cost analysis

## Future Enhancements

Potential improvements for future versions:

1. **Trailing Stops:** Dynamic stop-loss that moves with price
2. **Time-based Stops:** Execute after a specific duration
3. **Partial Stops:** Close only a percentage of position
4. **Stop Ladder:** Multiple stop levels at different thresholds
5. **Gas Rebates:** Return excess gas to keepers
6. **Priority Fees:** Allow users to offer higher fees for faster execution

## Conclusion

The stop-loss/stop-profit feature brings automated risk management to Burrowland's margin trading system. By leveraging a decentralized keeper model with economic incentives, it provides users with set-and-forget protection while maintaining the protocol's trustless architecture.

The implementation carefully balances user protection, keeper incentives, and protocol sustainability through a well-designed service fee mechanism and robust validation logic.
