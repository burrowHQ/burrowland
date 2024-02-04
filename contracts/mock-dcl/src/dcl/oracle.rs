use crate::*;

// reserve for farming
#[derive(BorshDeserialize, BorshSerialize)]
pub struct Observation {
    pub timestamp: u32,
    pub acc_point: i64,
    pub init: bool
}