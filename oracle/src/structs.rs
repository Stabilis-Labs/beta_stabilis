use scrypto::prelude::*;

#[derive(ScryptoSbor, PartialEq, Eq, PartialOrd, Ord, Debug, Copy, Clone)]
pub struct PriceData {
    pub price: Decimal,
    pub timestamp: i64,
}
