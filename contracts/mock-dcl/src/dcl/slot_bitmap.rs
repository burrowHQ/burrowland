use crate::*;
use std::ops::{BitOr, BitAnd};

/// return Some(0) if 0...01
/// return Some(255) if 1...0
fn idx_of_most_left_set_bit(value: U256) -> Option<u8> {
    if value.is_zero() {
        None
    } else {
        Some(255 - value.leading_zeros() as u8)
    }
}

/// return Some(0) if 01...1
/// return Some(255) if 10...0
fn idx_of_most_right_set_bit(value: U256) -> Option<u8> {
    if value.is_zero() {
        None
    } else {
        Some(value.trailing_zeros() as u8)
    }
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct SlotBitmap(pub LookupMap<i16, U256>);

impl SlotBitmap {
    pub fn set_zero(
        &mut self,
        point: i32,
        point_delta: i32
    ) {
        require!(point % point_delta == 0, E200_INVALID_ENDPOINT);
        let map_pt = point / point_delta;
        let word_idx = (map_pt >> 8) as i16;
        let bit_idx = (map_pt % 256) as u8;
        if let Some(value) = self.0.remove(&word_idx) {
            let new_value = value.bitand(!(U256::from(1u128) << bit_idx));
            if !new_value.is_zero() {
                self.0.insert(&word_idx, &new_value);
            }
        }
    }

    pub fn set_one(
        &mut self,
        point: i32,
        point_delta: i32
    ) {
        require!(point % point_delta == 0, E200_INVALID_ENDPOINT);
        let map_pt = point / point_delta;
        let word_idx = (map_pt >> 8) as i16;
        let bit_idx = (map_pt % 256) as u8;
        if let Some(value) = self.0.get(&word_idx) {
            self.0.insert(&word_idx, &value.bitor(U256::from(1u128) << bit_idx));
        } else {
            self.0.insert(&word_idx, &U256::from(0u128).bitor(U256::from(1u128) << bit_idx));
        }
    }

    /// From the given point (including itself), scan to the left, 
    /// find and return the first endpoint or order point,
    /// return None if no valued slot found at the right of stop_slot (including stop_slot)
    pub fn get_nearest_left_valued_slot(
        &self,
        point: i32,
        point_delta: i32,
        stop_slot: i32,
    ) -> Option<i32> {
        let mut slot = point / point_delta;
        if point < 0 && point % point_delta != 0 {
            slot -= 1;
        }; // round towards negative infinity

        let word_idx = (slot >> 8) as i16;
        let bit_idx = (slot % 256) as u8;
        let mut slot_word = {
            if let Some(value) = self.0.get(&word_idx) {
                // from 0001000 to 0001111, then bitand to only remain equal&lower bits
                value.bitand((U256::from(1u128) << bit_idx) - U256::from(1u128) + (U256::from(1u128) << bit_idx))
            } else {
                U256::from(0u128)
            }
        };
        let mut base_slot = slot - bit_idx as i32;
        let mut ret = None;
        while base_slot > stop_slot - 256 {
            if let Some(a) = idx_of_most_left_set_bit(slot_word) {
                let target_slot = base_slot + a as i32;
                if target_slot >= stop_slot {
                    ret = Some(target_slot * point_delta);
                }
                break;
            } else {
                base_slot -= 256;
                slot_word = {
                    if let Some(value) = self.0.get(&((base_slot >> 8) as i16)) {
                        value
                    } else {
                        U256::from(0u128)
                    }
                };
            }
        }
        ret
    }

    /// return start point of a valued (with liquidity or order) slot that beside the given point from right,
    /// NOT including the slot that embrace given point
    /// return None if no valued slot found at the left of stop_slot (including stop_slot)
    pub fn get_nearest_right_valued_slot(
        &self,
        point: i32,
        point_delta: i32,
        stop_slot: i32,
    ) -> Option<i32> {
        let mut slot = point / point_delta;
        if point < 0 && point % point_delta != 0 {
            slot -= 1;
        } // round towards negative infinity

        slot += 1;  // skip to the right next slot
        let word_idx = (slot >> 8) as i16;
        let bit_idx = (slot % 256) as u8;
        let mut slot_word = {
            if let Some(value) = self.0.get(&word_idx) {
                // from 0001000 -> 0000111 to 1111000, then bitand to only remain equal&higher bits
                value.bitand(!((U256::from(1u128) << bit_idx) - U256::from(1u128)))
            } else {
                U256::from(0u128)
            }
        };
        let mut base_slot = slot - bit_idx as i32;
        let mut ret = None;
        while base_slot <= stop_slot {
            if let Some(a) = idx_of_most_right_set_bit(slot_word) {
                let target_slot = base_slot + a as i32;
                if target_slot <= stop_slot {
                    ret = Some(target_slot * point_delta);
                }
                break;
            } else {
                base_slot += 256;
                slot_word = {
                    if let Some(value) = self.0.get(&((base_slot >> 8) as i16)) {
                        value
                    } else {
                        U256::from(0u128)
                    }
                };
            }
        }
        ret
    }
}
