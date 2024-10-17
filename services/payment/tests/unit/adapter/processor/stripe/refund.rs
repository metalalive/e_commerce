
use crate::model::refund::ut_setup_refund_cmplt_dto;

fn ut_setup_buyer_charge_inner(
    time_base: DateTime<Utc>,
    merchant_id: u32,
    buyer_usr_id: u32,
    charge_dlines: Vec<UTestChargeLineRawData>,
) -> ChargeBuyerModel {
    let mock_oid = "d1e5390dd2".to_string();
    let charge_ctime = time_base - Duration::minutes(86);
    let paymethod = {
        let mut mthd = ut_default_charge_method_stripe(&charge_ctime);
        if let Charge3partyModel::Stripe(s) = &mut mthd {
            s.payment_state = StripeCheckoutPaymentStatusModel::paid;
        }
        mthd
    };
    let currency_snapshot = {
        let iter = [
            (buyer_usr_id, CurrencyDto::TWD, (3185i64, 2u32)),
            (merchant_id, CurrencyDto::IDR, (123451, 1)),
        ]
        .map(|(usr_id, label, ratescalar)| {
            let rate = Decimal::new(ratescalar.0, ratescalar.1);
            let obj = OrderCurrencySnapshot { label, rate };
            (usr_id, obj)
        });
        HashMap::from_iter(iter)
    };
    ut_setup_buyer_charge(
        buyer_usr_id,
        charge_ctime,
        mock_oid.clone(),
        BuyerPayInState::OrderAppSynced(time_base),
        paymethod,
        charge_dlines,
        currency_snapshot,
    )
} // end of fn ut_setup_buyer_charge_inner

#[actix_web::test]
async fn refund_ok() {
    let arg = (mock_merchant_id, &mock_charge_ms[0], &mock_cmplt_req0);
    let resolve_m0 = RefundReqResolutionModel::try_from(arg).unwrap();
}

#[actix_web::test]
async fn err_invalid_rslv_model() {
}

#[actix_web::test]
async fn err_invalid_payment_intent() {
}

