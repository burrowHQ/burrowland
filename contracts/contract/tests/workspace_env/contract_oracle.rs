use crate::*;

use contract::PriceReceiverMsg;

pub struct Oralce(pub Contract);

impl Oralce {
    pub async fn oracle_call(
        &self,
        caller: &Account,
        receiver_id: &AccountId,
        price_data: PriceData,
        msg: PriceReceiverMsg,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "oracle_call")
            .args_json(json!({
                "receiver_id": receiver_id,
                "price_data": price_data,
                "msg": near_sdk::serde_json::to_string(&msg).unwrap(),
            }))
            .max_gas()
            .deposit(NearToken::from_yoctonear(1))
            .transact()
            .await
    }
}