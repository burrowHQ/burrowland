# LSD Support Branch (feat/lsd-support)

This document describes the changes introduced in the `feat/lsd-support` branch compared to `main`.

## Overview

This branch introduces two main enhancements:

1. **Client Echo Support** - A new mechanism enabling Liquid Staking Derivative (LSD) protocols to integrate with Burrowland by receiving callbacks after deposit and withdrawal operations.

2. **Improved User Storage Management** - Refactored storage handling in async callbacks to prevent permanent state inconsistency by allowing temporary storage overdraft.

---

## 1. Client Echo Support

### Purpose

Client Echo enables external protocols (primarily LSD protocols) to be notified when deposits or withdrawals are completed on their behalf. This allows LSD contracts to track user positions and trigger downstream operations.

### New Endpoints

#### Deposit with Client Echo

A new message type for `ft_transfer_call`:

```rust
TokenReceiverMsg::ClientEchoDeposit { client_echo: String }
```

When a whitelisted contract deposits tokens with this message:
1. The deposit is processed normally
2. A callback is made to the sender with the following JSON payload:
   ```json
   {
     "token_id": "<token contract address>",
     "supplied_shares": "<shares amount as U128>",
     "supplied_ft_amount": "<original FT amount as U128>",
     "msg": "<client_echo string>"
   }
   ```
3. The callback method is `on_burrowland_supply_client_echo` and receives 50 TGas for processing

**Usage:**
```json
{
  "ClientEchoDeposit": {
    "client_echo": "<custom message for the client>"
  }
}
```

#### Withdrawal with Client Echo

A new public method:

```rust
pub fn client_echo_withdraw_by_shares(
    &mut self,
    token_id: TokenId,
    shares: Shares,
    client_echo: String
) -> PromiseOrValue<U128>
```

When called by a whitelisted contract:
1. Shares are converted to tokens and withdrawn
2. Tokens are sent via `ft_transfer_call` to the caller with the `client_echo` string as the message
3. The caller receives the tokens and can process the custom message

### Security: Sender Whitelist

Only whitelisted contracts can use Client Echo features. The whitelist:
- Is managed by the contract owner
- Supports wildcard matching (e.g., `*.example.near` matches all subaccounts)
- Is checked via `in_client_echo_sender_whitelist()`

**Admin Methods:**
- `append_client_echo_sender_whitelist(sender_list: Vec<String>)` - Add senders
- `remove_client_echo_sender_whitelist(sender_list: Vec<String>)` - Remove senders
- `get_client_echo_sender_whitelist() -> Vec<String>` - List current whitelist

### Files Changed

| File | Changes |
|------|---------|
| `fungible_token.rs` | Added `ClientEchoDeposit` message handling |
| `actions.rs` | Added `client_echo_withdraw_by_shares()` method |
| `client_echo.rs` | Whitelist management (existing file, unchanged in this branch) |

---

## 2. Improved User Storage Management

### Problem

In async callback scenarios (after DEX trades, FT transfers, liquidations), if storage check fails:
- The transaction would panic
- This could leave positions permanently locked
- Tokens could be lost in failed rollback scenarios

### Solution: Force-Save Pattern

Introduced "force-save" functions that skip storage coverage checks in critical callbacks, allowing temporary storage overdraft:

#### New Functions

| Function | Location | Purpose |
|----------|----------|---------|
| `internal_force_set_storage()` | `storage.rs` | Force saves storage, skips balance check |
| `internal_force_set_account()` | `account.rs` | Force saves account state |
| `internal_force_set_margin_account()` | `margin_accounts.rs` | Force saves margin account state |

#### Removed Functions/Code

| Removed | Reason |
|---------|--------|
| `internal_set_storage_without_panic()` | Replaced by `internal_force_set_storage()` |
| `internal_set_margin_account_without_panic()` | Replaced by `internal_force_set_margin_account()` |
| `check_storage_covered()` | No longer needed |
| `margin_account_token_shares_to_lostfound()` | Lostfound mechanism removed |
| `LostfoundSupplyShares` event | No longer emitted |

### Where Force-Save is Used

Force-save is now used in all critical async callbacks:

| Callback | File | Scenario |
|----------|------|----------|
| `after_ft_transfer()` | `fungible_token.rs` | FT transfer failed, re-deposit tokens |
| `after_ft_transfer_call()` | `fungible_token.rs` | FT transfer_call returned unused tokens |
| `after_margin_asset_ft_transfer()` | `fungible_token.rs` | Margin asset transfer failed |
| `on_open_trade_return()` | `margin_trading.rs` | Opening margin position callback |
| `on_decrease_trade_return()` | `margin_trading.rs` | Closing margin position callback |
| `callback_dex_trade()` | `margin_position.rs` | DEX trade callback |
| `callback_process_shadow_liquidate_result()` | `shadow_actions.rs` | Shadow liquidation callback |
| `callback_process_shadow_force_close_result()` | `shadow_actions.rs` | Shadow force close callback |

### Impact

- Users may temporarily exceed their storage balance after certain callbacks
- The storage overdraft is a temporary state - users can deposit more storage balance
- Prevents permanent account/position locks that could occur on storage panic
- Removes the complex "lostfound" mechanism that was previously used as a fallback

---

## 3. Gas Optimization

### Change

Added `.with_unused_gas_weight(0)` to DEX trade callbacks:

```rust
Self::ext(env::current_account_id())
    .with_static_gas(GAS_FOR_FT_TRANSFER_CALL_CALLBACK)
    .with_unused_gas_weight(0)  // NEW
    .callback_dex_trade(...)
```

### Purpose

When `unused_gas_weight` is set to 0, unused gas from the cross-contract call is not forwarded to the callback. This:
- Improves gas efficiency
- Prevents the callback from receiving excessive gas it doesn't need
- Ensures more predictable gas usage

### Files Changed

| File | Location |
|------|----------|
| `margin_position.rs` | Two instances in `internal_open_margin_trade()` and `internal_increase_margin_trade()` |

---

## Commit History

| Commit | Description |
|--------|-------------|
| `60c2d44` | feat(lsd): add client echo support for deposits and withdrawals |
| `747d399` | feat(lsd): increase gas for on_burrowland_supply_client_echo |
| `34992db` | improve user storage management, allow slightly excess storage limit in some callbacks |
| `767b1ed` | chore: set unused gas weight to 0 for dex trade callbacks |

---

## Migration Notes

### For Protocol Operators

No migration needed. The changes are backwards compatible.

### For LSD Integrators

To use Client Echo:
1. Request to be added to the client echo sender whitelist
2. Implement `on_burrowland_supply_client_echo(token_id, supplied_shares, supplied_ft_amount, msg)` in your contract
3. Use `ClientEchoDeposit` message for deposits
4. Use `client_echo_withdraw_by_shares()` for withdrawals

### For Users

No action required. The storage management improvements are transparent and only affect edge cases where async callbacks previously could have failed.
