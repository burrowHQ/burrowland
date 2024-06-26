use crate::*;
use std::convert::TryFrom;

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Prices {
    pub prices: HashMap<TokenId, Price>,
}

impl Prices {
    pub fn new() -> Self {
        Self {
            prices: HashMap::new(),
        }
    }

    pub fn from_prices(prices: HashMap<TokenId, Price>) -> Self {
        Self {
            prices,
        }
    }

    pub fn get_unwrap(&self, token_id: &TokenId) -> &Price {
        self.prices.get(token_id).expect(format!("Asset {} price is missing", token_id).as_str())
    }
}

impl From<PriceData> for Prices {
    fn from(data: PriceData) -> Self {
        Self {
            prices: data
                .prices
                .into_iter()
                .filter_map(|AssetOptionalPrice { asset_id, price }| {
                    let token_id =
                        AccountId::try_from(asset_id).expect("Asset is not a valid token ID");
                    price.map(|price| (token_id, price))
                })
                .collect(),
        }
    }
}

impl Contract {
    /// Updates last prices in the contract.
    /// The prices will only be stored if the old price for the token is already present or the
    /// asset with this token ID exists.
    pub fn internal_set_prices(&mut self, prices: &Prices) {
        for (token_id, price) in prices.prices.iter() {
            if self.last_prices.contains_key(&token_id) || self.assets.contains_key(&token_id) {
                self.last_prices.insert(token_id.clone(), price.clone());
            }
        }
    }
}
