mod charge;

use rust_decimal::Decimal;
use std::collections::HashMap;

use ecommerce_common::api::dto::CurrencyDto;
use payment::model::OrderCurrencySnapshot;

pub(crate) fn ut_default_currency_snapshot(
    usr_ids: Vec<u32>,
) -> HashMap<u32, OrderCurrencySnapshot> {
    let iter = usr_ids.into_iter().map(|usr_id| {
        (
            usr_id,
            OrderCurrencySnapshot {
                label: CurrencyDto::TWD,
                rate: Decimal::new(321, 1),
            },
        )
    });
    HashMap::from_iter(iter)
}
