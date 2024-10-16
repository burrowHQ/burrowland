use crate::*;

pub struct PythContract(pub Contract);


impl PythContract {
    pub async fn set_price(
        &self,
        price_identifier: &str, 
        pyth_price: PythPrice
    ) -> Result<ExecutionFinalResult> {
        self.0
            .call("set_price")
            .args_json(json!({
                "price_identifier": price_identifier,
                "pyth_price": pyth_price,
            }))
            .gas(Gas::from_tgas(20))
            .transact()
            .await
    }
}

impl PythContract {
    pub async fn get_price(
        &self,
        price_identifier: &str,
    ) -> Result<Option<PythPrice>> {
        self.0
            .call("get_price")
            .args_json(json!({
                "price_identifier": price_identifier
            }))
            .view()
            .await?
            .json::<Option<PythPrice>>()
    }
}