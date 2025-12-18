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
    pub stop_profit: Option<u32>,  // Target profit in BPS (e.g., 12000 = 120%)
    pub stop_loss: Option<u32>,     // Stop-loss threshold in BPS (e.g., 9000 = 90%)
    pub service_token_id: TokenId,  // Token used for service fee
    pub service_token_amount: U128, // Amount of service fee locked
}
```

#### MarginStopServiceFee
```rust
pub struct MarginStopServiceFee {
    pub token_id: AccountId,  // Token to charge for service fee
    pub amount: U128,         // Amount to charge
}
```

### Storage

- **Account Level:** Each `MarginAccount` contains `stops: HashMap<PosId, MarginStop>`
- **Global Config:** `MARGIN_STOP_SERVICE_FEE` stores the current fee policy
- **Migration:** Account data migrates from `MarginAccountV1` to handle the new `stops` field

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

**Stop-Loss Check:**
```
current_remain = (value_position + value_collateral) * (1 - slippage) - (value_debt + hp_fee)
target_remain = value_collateral * stop_loss_bps / 10000

Triggers if: current_remain < target_remain
```

**Stop-Profit Check:**
```
current_remain = (value_position + value_collateral) * (1 - slippage) - (value_debt + hp_fee)
target_remain = value_collateral * stop_profit_bps / 10000

Triggers if: current_remain > target_remain
```

The calculation accounts for:
- Position token value at current prices
- Debt token value including accrued interest
- Holding position fees
- Slippage when selling position tokens
- Original collateral value

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

### Example 4: Normal Position Close with Stop Set

1. User closes position normally before stop triggers
2. During `callback_dex_trade` for position close:
   - Position is removed from `margin_positions`
   - Associated `MarginStop` is removed from `stops`
   - Service fee is refunded to user's margin supply

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
- Service fee (only if stop executes or if position closes with stop active)
- Potential slippage when stop executes

## Security Considerations

### Access Control

- **Owner Only:** Only position owner can set/modify stops via `SetStop`
- **Anyone Can Execute:** Any account can trigger `StopMTPosition` (similar to liquidation)
- **Admin Only:** Only contract owner can set service fee policy via `set_mssf()`

### Validation

1. **Stop Conditions:** `is_stop_active()` validates mathematical conditions before allowing execution
2. **Position Locking:** Cannot modify stops while position has pending swap (`is_locking`)
3. **Fee Policy:** Stop setting fails if `MARGIN_STOP_SERVICE_FEE` is not configured
4. **Sufficient Balance:** User must have enough margin supply to pay service fee

### Storage Safety

- **Try-based Updates:** `try_to_set_margin_account()` handles storage payment failures
- **Lostfound Mechanism:** If service fee recipient lacks storage, fee goes to lostfound
- **Atomic Operations:** Stop settings and fee handling are atomic within transactions

### Edge Cases

1. **Slippage Protection:** `min_token_d_amount` prevents excessive slippage during stop execution
2. **Fee Refunds:** Multiple refund paths ensure users don't lose fees unnecessarily
3. **Migration Safety:** `MarginAccountV1` provides backward compatibility
4. **Stop Removal:** Removing both profit and loss stops triggers full refund

## Admin Operations

### Setting Service Fee Policy

```rust
contract.set_mssf(MarginStopServiceFee {
    token_id: "usdc.token".parse().unwrap(),
    amount: U128(1000000), // 1 USDC (6 decimals)
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

- `contracts/contract/src/margin_stop_service_fee.rs` - New module for fee management
- `contracts/contract/src/margin_accounts.rs` - Added `MarginStop` struct and `stops` field
- `contracts/contract/src/margin_actions.rs` - Added `StopMTPosition` and `SetStop` actions
- `contracts/contract/src/margin_position.rs` - Stop condition logic and fee handling
- `contracts/contract/src/margin_trading.rs` - Service fee distribution in callbacks
- `contracts/contract/src/events.rs` - New `set_stop` event
- `contracts/contract/src/legacy.rs` - `MarginAccountV1` migration path
- `contracts/contract/src/storage_keys.rs` - New storage key constant
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
