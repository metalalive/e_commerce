use rust_decimal::Decimal;

use ecommerce_common::api::dto::CurrencyDto;

pub struct CurrencyModel {
    pub name: CurrencyDto,
    pub rate: Decimal,
}
pub struct CurrencyModelSet {
    pub base: CurrencyDto,
    pub exchange_rates: Vec<CurrencyModel>,
}
