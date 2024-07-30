mod charge;

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use std::collections::HashMap;

use ecommerce_common::api::dto::CurrencyDto;
use payment::model::{
    ChargeMethodModel, ChargeMethodStripeModel, OrderCurrencySnapshot,
    StripeCheckoutPaymentStatusModel, StripeSessionStatusModel,
};

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

pub(crate) fn ut_default_charge_method_stripe(t0: &DateTime<Utc>) -> ChargeMethodModel {
    let sess = ChargeMethodStripeModel {
        checkout_session_id: "mock-session-id".to_string(),
        payment_intent_id: "mock-payment-intent-id".to_string(),
        payment_state: StripeCheckoutPaymentStatusModel::no_payment_required,
        session_state: StripeSessionStatusModel::complete,
        expiry: *t0 + Duration::minutes(5),
    };
    ChargeMethodModel::Stripe(sess)
}
