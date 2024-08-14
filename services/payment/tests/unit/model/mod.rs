mod charge;
mod order_replica;

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use std::collections::HashMap;

use ecommerce_common::api::dto::CurrencyDto;
use ecommerce_common::constant::ProductType;
use ecommerce_common::model::BaseProductIdentity;
use payment::model::{
    BuyerPayInState, Charge3partyModel, Charge3partyStripeModel, ChargeBuyerMetaModel,
    ChargeBuyerModel, ChargeLineBuyerModel, OrderCurrencySnapshot, PayLineAmountModel,
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

pub(crate) fn ut_default_charge_method_stripe(t0: &DateTime<Utc>) -> Charge3partyModel {
    let sess = Charge3partyStripeModel {
        checkout_session_id: "mock-session-id".to_string(),
        payment_intent_id: "mock-payment-intent-id".to_string(),
        payment_state: StripeCheckoutPaymentStatusModel::no_payment_required,
        session_state: StripeSessionStatusModel::complete,
        expiry: *t0 + Duration::minutes(5),
    };
    Charge3partyModel::Stripe(sess)
}

#[rustfmt::skip]
pub(crate) fn ut_setup_buyer_charge(
    owner: u32,
    create_time: DateTime<Utc>,
    oid: String,
    state: BuyerPayInState,
    method: Charge3partyModel,
    d_lines: Vec<(u32, ProductType, u64, (i64, u32), (i64, u32), u32)>,
    currency_snapshot: HashMap<u32, OrderCurrencySnapshot>,
) -> ChargeBuyerModel {
    let lines = ut_setup_buyer_charge_lines(d_lines);
    let mut meta = ChargeBuyerMetaModel::from((oid, owner, create_time));
    meta.update_progress(&state);
    meta.update_3party(method);
    ChargeBuyerModel {meta, lines, currency_snapshot}
}

#[rustfmt::skip]
pub(crate) fn ut_setup_buyer_charge_lines(
    d_lines: Vec<(u32, ProductType, u64, (i64, u32), (i64, u32), u32)>,
) -> Vec<ChargeLineBuyerModel> {
    d_lines
        .into_iter()
        .map(|dl| ChargeLineBuyerModel {
            pid: BaseProductIdentity {
                store_id: dl.0, product_type: dl.1, product_id: dl.2,
            },
            amount: PayLineAmountModel {
                unit: Decimal::new(dl.3.0, dl.3.1),
                total: Decimal::new(dl.4.0, dl.4.1),
                qty: dl.5,
            },
        })
        .collect()
}
