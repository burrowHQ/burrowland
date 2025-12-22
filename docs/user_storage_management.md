# User Storage Management

This document explains how user storage management works in the Burrowland lending protocol.

## Overview

NEAR blockchain requires users to pay for on-chain storage. Burrowland implements a storage management system that tracks every byte of contract storage used by each user and ensures users maintain sufficient NEAR balance to cover their storage costs.

## Core Architecture

The system uses a **three-layer tracking approach**:

### Layer 1: StorageTracker

**Location:** `contracts/contract/src/storage_tracker.rs`

A helper object that measures storage changes between `start()` and `stop()` calls:

```rust
pub struct StorageTracker {
    pub bytes_added: StorageUsage,
    pub bytes_released: StorageUsage,
    pub initial_storage_usage: Option<StorageUsage>,
}
```

**Key methods:**
- `start()`: Captures current `env::storage_usage()` as baseline
- `stop()`: Computes the delta and records bytes_added or bytes_released
- `consume()`: Merges one tracker into another (transfers accumulated bytes)
- `is_empty()`: Returns true if no bytes tracked and not currently active
- `clean()`: Resets tracker (used for temporary objects)

**Safety guard:** The `Drop` implementation panics if the tracker is non-empty when destroyed, catching accounting bugs early.

### Layer 2: Storage

**Location:** `contracts/contract/src/storage.rs`

Per-user storage balance tracking:

```rust
pub struct Storage {
    pub storage_balance: Balance,        // NEAR tokens deposited for storage
    pub used_bytes: StorageUsage,        // Bytes currently used in contract storage
    #[borsh_skip]
    pub storage_tracker: StorageTracker, // Tracks changes during current operation
}
```

**Note:** `storage_tracker` is marked `#[borsh_skip]` because it's transient - only used during operation execution, not persisted.

### Layer 3: Account/MarginAccount

Both `Account` and `MarginAccount` structs have their own `storage_tracker` field to track changes to their internal data structures (HashMaps, UnorderedMaps).

## Storage Flow

When a user performs an operation (deposit, borrow, open margin position, etc.):

```
1. User action modifies Account/MarginAccount
   └── account.storage_tracker captures changes to HashMaps

2. internal_set_account() is called
   └── storage.storage_tracker.consume(account.storage_tracker)  // bubble up changes
   └── storage.storage_tracker.start()
   └── self.accounts.insert(...)                                 // persist to storage
   └── storage.storage_tracker.stop()

3. internal_set_storage() finalizes
   └── Calculate net bytes: bytes_added - bytes_released
   └── Update storage.used_bytes
   └── Assert: storage_balance >= (used_bytes × byte_cost)
   └── Panic if insufficient balance
```

## Key Functions

### Contract Methods (storage.rs)

| Method | Purpose |
|--------|---------|
| `internal_get_storage()` | Retrieves Storage for an account |
| `internal_unwrap_storage()` | Same as above but panics if not found |
| `internal_set_storage()` | Saves storage, panics if insufficient balance |
| `internal_force_set_storage()` | Force saves, allows temporary overdraft |
| `internal_storage_balance_of()` | Returns StorageBalance with total and available |

### Account Methods (account.rs)

| Method | Purpose |
|--------|---------|
| `internal_set_account()` | Saves account, panics if insufficient storage |
| `internal_force_set_account()` | Force saves account in critical callbacks |

### Margin Account Methods (margin_accounts.rs)

| Method | Purpose |
|--------|---------|
| `internal_set_margin_account()` | Saves margin account, panics if insufficient storage |
| `internal_force_set_margin_account()` | Force saves margin account in critical callbacks |

### StorageManagement Trait Implementation

| Method | Purpose |
|--------|---------|
| `storage_deposit()` | User deposits NEAR, creates account if new |
| `storage_withdraw()` | User withdraws excess storage balance |
| `storage_unregister()` | User removes account, gets all storage balance back |
| `storage_balance_bounds()` | Returns min/max storage bounds |
| `storage_balance_of()` | Returns user's storage balance info |

## Minimum Balance

Defined in `storage.rs`:

```rust
const MIN_STORAGE_BALANCE: Balance = 10000u128 * env::STORAGE_PRICE_PER_BYTE;
```

Users must deposit at least 10,000 bytes worth of NEAR to register an account.

## Storage Tracking Patterns

### Pattern 1: Implicit HashMap Tracking

For regular account modifications (supply, collateral, borrow), changes to HashMap fields are tracked implicitly:

1. User modifies `account.supplied`, `account.positions`, etc.
2. `internal_set_account()` measures the delta when persisting

### Pattern 2: Explicit UnorderedMap Tracking

For margin positions using `UnorderedMap`, explicit wrapping is required:

```rust
// Opening a position
account.storage_tracker.start();
account.margin_positions.insert(&pos_id, &position);
account.storage_tracker.stop();

// Closing a position
account.storage_tracker.start();
account.margin_positions.remove(&pos_id);
account.storage_tracker.stop();
```

### Pattern 3: Force-Save for Critical Callbacks

In async callback scenarios where a panic would cause permanent inconsistency (locked positions, lost tokens), force-save functions are used:

```rust
// Force save - skips storage coverage check, allows temporary overdraft
pub fn internal_force_set_storage(&mut self, account_id: &AccountId, mut storage: Storage) {
    // Calculate bytes change
    if storage.storage_tracker.bytes_added >= storage.storage_tracker.bytes_released {
        let extra_bytes_used =
            storage.storage_tracker.bytes_added - storage.storage_tracker.bytes_released;
        storage.used_bytes = storage.used_bytes.saturating_add(extra_bytes_used);
        // Note: NO assert_storage_covered() check here
    } else {
        let bytes_released =
            storage.storage_tracker.bytes_released - storage.storage_tracker.bytes_added;
        storage.used_bytes = storage.used_bytes.saturating_sub(bytes_released);
    }
    storage.storage_tracker.bytes_released = 0;
    storage.storage_tracker.bytes_added = 0;
    self.storage.insert(account_id, &storage.into());
}
```

This pattern is used in:
- `on_open_trade_return()` / `on_decrease_trade_return()` - unlocking margin positions
- `callback_process_shadow_liquidate_result()` - unlocking shadow accounts
- `callback_process_shadow_force_close_result()` - unlocking force-closed accounts
- `after_ft_transfer()` / `after_ft_transfer_call()` - restoring tokens on transfer failure
- `after_margin_asset_ft_transfer()` - restoring margin tokens on transfer failure
- `callback_dex_trade()` - reverting position on DEX trade failure

## Account Registration Flow

When a new user calls `storage_deposit()`:

```rust
// 1. Create new Storage with deposited balance
let mut storage = Storage::new();
storage.storage_balance = amount;

// 2. Create new Account
let mut account = Account::new(&account_id);

// 3. HACK: Track the storage cost of the Storage object itself
account.storage_tracker.start();
self.internal_set_storage(&account_id, storage);
account.storage_tracker.stop();

// 4. Save account (this will consume the tracked bytes)
self.internal_set_account(&account_id, account);

// 5. Create margin account
self.internal_set_margin_account(&account_id, MarginAccount::new(&account_id));
```

The "hack" ensures the cost of storing the initial `Storage` object is properly accounted for.

## Account Unregistration

When a user calls `storage_unregister()`:

1. Verify account is not locked
2. Verify all balances are empty (supplied, positions, farms, booster staking)
3. Verify margin account is empty (if exists)
4. Remove all account data from storage
5. Refund entire `storage_balance` to user

## Debugging

The `debug_storage_balance_of()` method provides storage balance info without the minimum storage limit check, useful for debugging storage issues.

## Key Design Principles

1. **Two-level tracking:** Account-level changes bubble up to global storage accounting via `consume()`
2. **Transactional safety:** `start()`/`stop()` pairs ensure atomicity within a single operation
3. **Force-save for critical paths:** Async callbacks use force-save to prevent permanent locks/token loss
4. **Storage cost accountability:** Every persistent storage write is measured and charged against user balance
5. **Explicit measurement:** UnorderedMap operations must be explicitly wrapped in start/stop blocks
6. **Safety guards:** StorageTracker's Drop implementation catches accounting bugs
7. **Temporary overdraft allowed:** Force-save may cause temporary storage overdraft, user can deposit more later

## File Summary

| File | Purpose |
|------|---------|
| `storage_tracker.rs` | Core StorageTracker implementation |
| `storage.rs` | Storage struct, balance management, StorageManagement trait, `internal_force_set_storage()` |
| `account.rs` | Account struct with storage_tracker, `internal_set_account()`, `internal_force_set_account()` |
| `margin_accounts.rs` | MarginAccount struct, `internal_set_margin_account()`, `internal_force_set_margin_account()` |
| `margin_position.rs` | Margin position operations, explicit tracking for UnorderedMap |
| `margin_trading.rs` | Position settlement, position removal tracking |
| `fungible_token.rs` | FT transfer callbacks with force-save |
| `shadow_actions.rs` | Shadow liquidation/force close callbacks with force-save |