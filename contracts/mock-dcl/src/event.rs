use crate::*;
use near_sdk::serde_json::json;

const EVENT_STANDARD: &str = "dcl.ref";
const EVENT_STANDARD_VERSION: &str = "1.0.0";

#[derive(Serialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
#[serde(tag = "event", content = "data")]
#[serde(rename_all = "snake_case")]
#[must_use = "Don't forget to `.emit()` this event"]
pub enum Event<'a> {
    LiquidityAdded {
        lpt_id: &'a String,
        owner_id: &'a AccountId,
        pool_id: &'a String,
        left_point: &'a i32,
        right_point: &'a i32,
        added_amount: &'a U128,
        cur_amount: &'a U128,
        paid_token_x: &'a U128,
        paid_token_y: &'a U128,
    },
    LiquidityAppend {
        lpt_id: &'a String,
        owner_id: &'a AccountId,
        pool_id: &'a String,
        left_point: &'a i32,
        right_point: &'a i32,
        added_amount: &'a U128,
        cur_amount: &'a U128,
        paid_token_x: &'a U128,
        paid_token_y: &'a U128,
        claim_fee_token_x: &'a U128,
        claim_fee_token_y: &'a U128,
    },
    LiquidityMerge {
        lpt_id: &'a String,
        merge_lpt_ids: &'a String,
        owner_id: &'a AccountId,
        pool_id: &'a String,
        left_point: &'a i32,
        right_point: &'a i32,
        added_amount: &'a U128,
        cur_amount: &'a U128,
        remove_token_x: &'a U128,
        remove_token_y: &'a U128,
        merge_token_x: &'a U128,
        merge_token_y: &'a U128,
        claim_fee_token_x: &'a U128,
        claim_fee_token_y: &'a U128,
    },
    LiquidityRemoved {
        lpt_id: &'a String,
        owner_id: &'a AccountId,
        pool_id: &'a String,
        left_point: &'a i32,
        right_point: &'a i32,
        removed_amount: &'a U128,
        cur_amount: &'a U128,
        refund_token_x: &'a U128,
        refund_token_y: &'a U128,
        claim_fee_token_x: &'a U128,
        claim_fee_token_y: &'a U128,
    },
    OrderAdded {
        order_id: &'a String,
        created_at: &'a U64,
        owner_id: &'a AccountId,
        pool_id: &'a String,
        point: &'a i32,
        sell_token: &'a AccountId,
        buy_token: &'a AccountId,
        original_amount: &'a U128,
        original_deposit_amount: &'a U128,
        swap_earn_amount: &'a U128,
    },
    OrderCancelled {
        order_id: &'a String,
        created_at: &'a U64,
        cancel_at: &'a U64,
        owner_id: &'a AccountId,
        pool_id: &'a String,
        point: &'a i32,
        sell_token: &'a AccountId,
        buy_token: &'a AccountId,
        request_cancel_amount:  &'a Option<U128>,
        actual_cancel_amount:  &'a U128,
        original_amount: &'a U128,
        cancel_amount: &'a U128,
        remain_amount: &'a U128,
        bought_amount: &'a U128,
    },
    OrderCompleted {
        order_id: &'a String,
        created_at: &'a U64,
        completed_at: &'a U64,
        owner_id: &'a AccountId,
        pool_id: &'a String,
        point: &'a i32,
        sell_token: &'a AccountId,
        buy_token: &'a AccountId,
        original_amount: &'a U128,
        original_deposit_amount: &'a U128,
        swap_earn_amount: &'a U128,
        cancel_amount: &'a U128,
        bought_amount: &'a U128,
    },
    Swap {
        swapper: &'a AccountId,
        token_in: &'a AccountId,
        token_out: &'a AccountId,
        amount_in: &'a U128,
        amount_out: &'a U128,
        pool_id: &'a PoolId,
        total_fee: &'a U128,
        protocol_fee: &'a U128,
    },
    SwapDesire {
        swapper: &'a AccountId,
        token_in: &'a AccountId,
        token_out: &'a AccountId,
        amount_in: &'a U128,
        amount_out: &'a U128,
        pool_id: &'a PoolId,
        total_fee: &'a U128,
        protocol_fee: &'a U128,
    },
    Lostfound {
        user: &'a AccountId,
        token: &'a AccountId,
        amount: &'a U128,
        // assets locked in contract
        locked: &'a bool,
    },
    ClaimChargedFee {
        user: &'a AccountId,
        pool_id: &'a String,
        amount_x: &'a U128,
        amount_y: &'a U128,
    },
    AppendUserStorage {
        operator: &'a AccountId,
        user: &'a AccountId,
        amount: &'a U128,
    },
    InitUserStorage {
        operator: &'a AccountId,
        user: &'a AccountId,
        amount: &'a U128,
    },
    WithdrawUserStorage {
        operator: &'a AccountId,
        receiver: &'a AccountId,
        amount: &'a U128,
        remain: &'a U128,
    },
    UnregisterUserStorage {
        operator: &'a AccountId,
        sponsor: &'a AccountId,
        amount: &'a U128,
    },
    HotZap {
        account_id: &'a AccountId,
        remain_assets: &'a HashMap<AccountId, U128>,
    }
}

impl Event<'_> {
    pub fn emit(&self) {
        let data = json!(self);
        let event_json = json!({
            "standard": EVENT_STANDARD,
            "version": EVENT_STANDARD_VERSION,
            "event": data["event"],
            "data": [data["data"]]
        })
        .to_string();
        log!("EVENT_JSON:{}", event_json);
    }
}
