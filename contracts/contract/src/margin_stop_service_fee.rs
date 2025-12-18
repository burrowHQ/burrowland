use crate::*;

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
#[serde(crate = "near_sdk::serde")]
pub struct MarginStopServiceFee {
    pub token_id: AccountId,
    pub amount: U128,
}

pub fn read_mssf_from_storage(
) -> Option<MarginStopServiceFee> {
    env::storage_read(MARGIN_STOP_SERVICE_FEE.as_bytes()).map(|v| {
        MarginStopServiceFee::try_from_slice(&v).expect("deserialize margin stop service fee failed.")
    })
}

pub fn write_mssf_to_storage(
    data: MarginStopServiceFee,
) {
    env::storage_write(
        MARGIN_STOP_SERVICE_FEE.as_bytes(),
        &data.try_to_vec().unwrap(),
    );
}

#[near_bindgen]
impl Contract {
    #[payable]
    pub fn set_mssf(&mut self, mssf: MarginStopServiceFee) {
        assert_one_yocto();
        self.assert_owner();
        write_mssf_to_storage(mssf);
    }

    pub fn get_mssf(&self) -> Option<MarginStopServiceFee> {
        read_mssf_from_storage()
    }
}