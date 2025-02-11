mod charge;
mod merchant;
mod order_replica;
pub(super) mod payout;
pub(super) mod refund;
mod reporting;

use chrono::{DateTime, Duration, Local, Utc};
use rust_decimal::Decimal;
use std::collections::HashMap;

use ecommerce_common::api::dto::{CountryCode, CurrencyDto};
use ecommerce_common::model::BaseProductIdentity;
use payment::api::web::dto::ChargeStatusDto;
use payment::model::{
    BuyerPayInState, Charge3partyModel, Charge3partyStripeModel, ChargeBuyerMetaModel,
    ChargeBuyerModel, ChargeLineBuyerModel, Merchant3partyStripeModel, OrderCurrencySnapshot,
    PayLineAmountModel, StripeAccountCapabilityModel, StripeAccountCapableState,
    StripeAccountLinkModel, StripeAccountSettingModel, StripeCheckoutPaymentStatusModel,
    StripeSessionStatusModel,
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
        transfer_group: "mock-transfer-group".to_string(),
        payment_state: StripeCheckoutPaymentStatusModel::no_payment_required,
        session_state: StripeSessionStatusModel::complete,
        expiry: *t0 + Duration::minutes(5),
    };
    Charge3partyModel::Stripe(sess)
}

#[rustfmt::skip]
pub(crate) type UTestChargeLineRawData = (
    u32, u64, (i64, u32), (i64, u32), u32, (i64, u32), (i64, u32), u32, u32
);

#[rustfmt::skip]
pub(crate) fn ut_setup_buyer_charge(
    owner: u32,
    create_time: DateTime<Utc>,
    oid: String,
    state: BuyerPayInState,
    method: Charge3partyModel,
    d_lines: Vec<UTestChargeLineRawData>,
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
    d_lines: Vec<UTestChargeLineRawData>,
) -> Vec<ChargeLineBuyerModel> {
    d_lines
        .into_iter()
        .map(|dl| {
            let pid = BaseProductIdentity {
                store_id: dl.0,  product_id: dl.1,
            };
            let amount_orig = PayLineAmountModel {
                unit: Decimal::new(dl.2.0, dl.2.1),
                total: Decimal::new(dl.3.0, dl.3.1),
                qty: dl.4,
            };
            let amount_refunded = PayLineAmountModel {
                unit: Decimal::new(dl.5.0, dl.5.1),
                total: Decimal::new(dl.6.0, dl.6.1),
                qty: dl.7,
            };
            let num_rejected = dl.8;
            let arg = (pid, amount_orig, amount_refunded, num_rejected);
            ChargeLineBuyerModel::from(arg)
        })
        .collect()
}

#[rustfmt::skip]
fn ut_partial_eq_charge_status_dto(a :&ChargeStatusDto, b :&ChargeStatusDto) -> bool {
    match (a, b) {
        (ChargeStatusDto::Initialized, ChargeStatusDto::Initialized) |
        (ChargeStatusDto::InternalSyncing, ChargeStatusDto::InternalSyncing) |
        (ChargeStatusDto::PspRefused, ChargeStatusDto::PspRefused) |
        (ChargeStatusDto::SessionExpired, ChargeStatusDto::SessionExpired) |
        (ChargeStatusDto::Completed, ChargeStatusDto::Completed) |
        (ChargeStatusDto::PspProcessing, ChargeStatusDto::PspProcessing) => true,
        _others => false,
    }
}

pub(super) fn ut_default_merchant_3party_stripe() -> Merchant3partyStripeModel {
    let t_now = Local::now().to_utc();
    let capabilities = StripeAccountCapabilityModel {
        transfers: StripeAccountCapableState::inactive,
    };
    let settings = StripeAccountSettingModel {
        payout_delay_days: 7,
        payout_interval: "daily".to_string(),
        debit_negative_balances: false,
    };
    let update_link = Some(StripeAccountLinkModel {
        url: "https://docs.python.org/3/library".to_string(),
        expiry: t_now - Duration::minutes(3),
    });
    Merchant3partyStripeModel {
        id: "acct_1oi3gwtiy832yt".to_string(),
        country: CountryCode::ID,
        email: Some("hayley@wo0dberry.org".to_string()),
        capabilities,
        tos_accepted: Some(t_now),
        charges_enabled: false,
        payouts_enabled: false,
        details_submitted: false,
        created: t_now,
        settings,
        update_link,
    }
}
