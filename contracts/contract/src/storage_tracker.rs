use crate::*;
use near_sdk::StorageUsage;

/// A helper object that tracks changes in state storage.
#[derive(Default, Clone)]
pub struct StorageTracker {
    pub bytes_added: StorageUsage,
    pub bytes_released: StorageUsage,
    pub initial_storage_usage: Option<StorageUsage>,
}

/// Safety guard for the storage tracker.
impl Drop for StorageTracker {
    fn drop(&mut self) {
        assert!(self.is_empty(), "Bug, non-tracked storage change");
    }
}

impl StorageTracker {
    /// Starts tracking the state storage changes.
    pub fn start(&mut self) {
        assert!(
            self.initial_storage_usage
                .replace(env::storage_usage())
                .is_none(),
            "The storage tracker is already tracking"
        );
    }

    /// Stop tracking the state storage changes and record changes in bytes.
    pub fn stop(&mut self) {
        let initial_storage_usage = self
            .initial_storage_usage
            .take()
            .expect("The storage tracker wasn't tracking");
        let storage_usage = env::storage_usage();
        if storage_usage >= initial_storage_usage {
            self.bytes_added += storage_usage - initial_storage_usage;
        } else {
            self.bytes_released += initial_storage_usage - storage_usage;
        }
    }

    /// Consumes the other storage tracker changes.
    pub fn consume(&mut self, other: &mut StorageTracker) {
        self.bytes_added += other.bytes_added;
        other.bytes_added = 0;
        self.bytes_released += other.bytes_released;
        other.bytes_released = 0;
        assert!(
            other.initial_storage_usage.is_none(),
            "Can't merge storage tracker that is tracking storage"
        );
    }

    /// Returns true if no bytes is added or released, and the tracker is not active.
    pub fn is_empty(&self) -> bool {
        self.bytes_added == 0 && self.bytes_released == 0 && self.initial_storage_usage.is_none()
    }

    /// Used when its account is a temp object
    pub fn clean(&mut self) {
        self.bytes_added = 0;
        self.bytes_released = 0;
        self.initial_storage_usage = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a tracker with preset values (no active tracking).
    fn make_tracker(added: StorageUsage, released: StorageUsage) -> StorageTracker {
        StorageTracker {
            bytes_added: added,
            bytes_released: released,
            initial_storage_usage: None,
        }
    }

    // ========== consume: single call ==========

    #[test]
    fn test_consume_single_accumulates_both_fields() {
        let mut target = make_tracker(10, 5);
        let mut source = make_tracker(20, 15);

        target.consume(&mut source);

        assert_eq!(target.bytes_added, 30);
        assert_eq!(target.bytes_released, 20);
        // source must be zeroed
        assert_eq!(source.bytes_added, 0);
        assert_eq!(source.bytes_released, 0);

        target.clean();
    }

    // ========== consume: multiple calls ==========

    #[test]
    fn test_consume_multiple_accumulates_bytes_released() {
        let mut target = make_tracker(0, 0);

        // First consume: releases 200 bytes
        let mut source1 = make_tracker(0, 200);
        target.consume(&mut source1);
        assert_eq!(target.bytes_released, 200);

        // Second consume: releases 50 bytes
        let mut source2 = make_tracker(0, 50);
        target.consume(&mut source2);
        assert_eq!(target.bytes_released, 250, "bytes_released must accumulate across multiple consume() calls");

        target.clean();
    }

    #[test]
    fn test_consume_multiple_accumulates_bytes_added() {
        let mut target = make_tracker(0, 0);

        let mut source1 = make_tracker(100, 0);
        target.consume(&mut source1);
        assert_eq!(target.bytes_added, 100);

        let mut source2 = make_tracker(75, 0);
        target.consume(&mut source2);
        assert_eq!(target.bytes_added, 175);

        target.clean();
    }

    #[test]
    fn test_consume_multiple_mixed_operations() {
        let mut target = make_tracker(10, 5);

        let mut source1 = make_tracker(100, 200);
        target.consume(&mut source1);
        assert_eq!(target.bytes_added, 110);
        assert_eq!(target.bytes_released, 205);

        let mut source2 = make_tracker(50, 30);
        target.consume(&mut source2);
        assert_eq!(target.bytes_added, 160);
        assert_eq!(target.bytes_released, 235);

        let mut source3 = make_tracker(0, 100);
        target.consume(&mut source3);
        assert_eq!(target.bytes_added, 160);
        assert_eq!(target.bytes_released, 335);

        target.clean();
    }

    // ========== consume: source is properly zeroed ==========

    #[test]
    fn test_consume_zeroes_source() {
        let mut target = make_tracker(0, 0);
        let mut source = make_tracker(42, 17);

        target.consume(&mut source);

        assert!(source.is_empty());

        target.clean();
    }

    // ========== consume: zero-value sources ==========

    #[test]
    fn test_consume_zero_source_is_noop() {
        let mut target = make_tracker(10, 20);
        let mut source = make_tracker(0, 0);

        target.consume(&mut source);

        assert_eq!(target.bytes_added, 10);
        assert_eq!(target.bytes_released, 20);

        target.clean();
    }

    // ========== consume: panics if source is actively tracking ==========

    #[test]
    #[should_panic(expected = "Can't merge storage tracker that is tracking storage")]
    fn test_consume_panics_if_source_is_tracking() {
        use std::mem::ManuallyDrop;
        // Use ManuallyDrop to prevent the Drop guard from double-panicking during unwind
        let mut target = ManuallyDrop::new(make_tracker(0, 0));
        let mut source = ManuallyDrop::new(StorageTracker {
            bytes_added: 0,
            bytes_released: 0,
            initial_storage_usage: Some(1000),
        });

        target.consume(&mut source);
    }
}
