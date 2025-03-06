use std::collections::HashMap;
use std::fs::File;
use std::str::FromStr;
use std::sync::Arc;

use chrono::{DateTime, Duration, Local, Utc};
use payment::adapter::processor::{AppProcessorErrorReason, BaseClientErrorReason};
use rust_decimal::Decimal;
use serde::Deserialize;

use ecommerce_common::api::dto::{CurrencyDto, PayAmountDto};
use ecommerce_common::config::AppConfig;
use payment::model::{
    BuyerPayInState, Charge3partyModel, ChargeBuyerModel, OrderCurrencySnapshot,
    RefundReqResolutionModel, StripeCheckoutPaymentStatusModel,
};

use crate::model::refund::{ut_setup_refund_cmplt_dto, UTestRefundCmpltDtoRawData};
use crate::model::{
    ut_default_charge_method_stripe, ut_setup_buyer_charge, UTestChargeLineRawData,
};
use crate::{ut_setup_sharestate, EXAMPLE_REL_PATH};

#[derive(Deserialize)]
struct UTestDataStripeBuyerCurrency {
    label: CurrencyDto,
    rate: String,
}
#[derive(Deserialize)]
struct UTestDataStripeChargeLine {
    prod_id: u64,
    attr_seq: u16,
    qty_orig: u32,
    amt_orig: PayAmountDto,
}
#[derive(Deserialize)]
struct UTestDataStripeRefundLine {
    prod_id: u64,
    attr_seq: u16,
    qty: u32,
    amount_total: String,
}
#[derive(Deserialize)]
struct UTestDataStripeTopLvl {
    merchant_id: u32,
    payment_intent: String,
    currency_buyer: UTestDataStripeBuyerCurrency,
    charge_lines: Vec<UTestDataStripeChargeLine>,
    refund_lines: Vec<UTestDataStripeRefundLine>,
}

#[rustfmt::skip]
impl UTestDataStripeBuyerCurrency {
    fn test_data(&self) -> (CurrencyDto, i64, u32) {
        let d_rate = Decimal::from_str(self.rate.as_str()).unwrap();
        (self.label.clone(), d_rate.mantissa() as i64, d_rate.scale())
    }
}
#[rustfmt::skip]
impl UTestDataStripeChargeLine {
    fn test_data(&self, merchant_id: u32) -> UTestChargeLineRawData {
        let d_unit = Decimal::from_str(self.amt_orig.unit.as_str()).unwrap();
        let d_total = Decimal::from_str(self.amt_orig.total.as_str()).unwrap();
        (
            (merchant_id, self.prod_id, self.attr_seq),
            ((d_unit.mantissa() as i64, d_unit.scale()),
            (d_total.mantissa() as i64, d_total.scale()),
            self.qty_orig), ((0,0), (0,0), 0), 0,
        )
    }
}
#[rustfmt::skip]
impl UTestDataStripeRefundLine {
    fn test_data(&self, time_bias_req: i64) -> UTestRefundCmpltDtoRawData {
        let d_total = Decimal::from_str(self.amount_total.as_str()).unwrap();
        assert_eq!(d_total.scale(), 1);
        (
            (self.prod_id, self.attr_seq), time_bias_req,
            d_total.mantissa() as i64, self.qty, 0, 0,
        )
    }
}
#[rustfmt::skip]
impl UTestDataStripeTopLvl {
    fn charge_lines_raw_data(&self) -> Vec<UTestChargeLineRawData> {
        self.charge_lines.iter()
            .map(|cl| cl.test_data(self.merchant_id))
            .collect::<Vec<_>>()
    }
    fn refund_lines_raw_data(&self) -> Vec<UTestRefundCmpltDtoRawData> {
        let mut num_rlines = 0..self.refund_lines.len();
        self.refund_lines.iter()
            .map(|rl| {
                let t = num_rlines.next().unwrap() as i64;
                rl.test_data(t)
            })
            .collect::<Vec<_>>()
    }
}

#[rustfmt::skip]
fn ut_load_mockdata_from_file(acfg: Arc<AppConfig>, filename:&str) -> UTestDataStripeTopLvl
{
    let fullpath = acfg.basepath.service.clone() + EXAMPLE_REL_PATH + filename;
    let f = File::open(fullpath.as_str()).unwrap();
    let result = serde_json::from_reader::<File, UTestDataStripeTopLvl>(f);
    result.unwrap()
}

#[rustfmt::skip]
fn ut_setup_buyer_charge_inner(
    time_base: DateTime<Utc>,
    merchant_id: u32,
    currency_buyer_d: (CurrencyDto, i64, u32),
    mock_stripe_payment_intent: String,
    ch_dlines: Vec<UTestChargeLineRawData>,
) -> ChargeBuyerModel {
    let mock_oid = "d1e5390dd2".to_string();
    let buyer_usr_id = 925u32;
    let charge_ctime = time_base - Duration::minutes(86);
    let paymethod = {
        let mut mthd = ut_default_charge_method_stripe(&charge_ctime);
        if let Charge3partyModel::Stripe(s) = &mut mthd {
            s.payment_state = StripeCheckoutPaymentStatusModel::paid;
            s.payment_intent_id = mock_stripe_payment_intent;
        }
        mthd
    };
    let currency_snapshot = {
        let iter = [
            (buyer_usr_id, currency_buyer_d.0, (currency_buyer_d.1, currency_buyer_d.2)),
            (merchant_id, CurrencyDto::INR, (8635, 2)),
        ]
        .map(|(usr_id, label, ratescalar)| {
            let rate = Decimal::new(ratescalar.0, ratescalar.1);
            let obj = OrderCurrencySnapshot { label, rate };
            (usr_id, obj)
        });
        HashMap::from_iter(iter)
    };
    ut_setup_buyer_charge(
        buyer_usr_id, charge_ctime, mock_oid,
        BuyerPayInState::OrderAppSynced(time_base),
        paymethod, ch_dlines, currency_snapshot,
    )
} // end of fn ut_setup_buyer_charge_inner

#[actix_web::test]
async fn refund_ok() {
    let time_now = Local::now().to_utc();
    let shr_state = ut_setup_sharestate();
    let mock_data =
        ut_load_mockdata_from_file(shr_state.config(), "processor-stripe-refund-ok.json");

    let mock_charge_m = ut_setup_buyer_charge_inner(
        time_now,
        mock_data.merchant_id,
        mock_data.currency_buyer.test_data(),
        mock_data.payment_intent.clone(),
        mock_data.charge_lines_raw_data(),
    );
    let mock_cmplt_req = ut_setup_refund_cmplt_dto(time_now, mock_data.refund_lines_raw_data());
    let arg = (mock_data.merchant_id, &mock_charge_m, &mock_cmplt_req);
    let rfd_rslv_m = RefundReqResolutionModel::try_from(arg).unwrap();

    let proc_ctx = shr_state.processor_context();
    let result = proc_ctx.refund(rfd_rslv_m).await;
    assert!(result.is_ok());
} // end of fn refund_ok

#[actix_web::test]
async fn err_invalid_payment_intent() {
    let time_now = Local::now().to_utc();
    let shr_state = ut_setup_sharestate();
    let mock_data = {
        let mut md =
            ut_load_mockdata_from_file(shr_state.config(), "processor-stripe-refund-ok.json");
        md.payment_intent = "overwrite-with-invalid-payment-intent".to_string();
        md
    };
    let mock_charge_m = ut_setup_buyer_charge_inner(
        time_now,
        mock_data.merchant_id,
        mock_data.currency_buyer.test_data(),
        mock_data.payment_intent.clone(),
        mock_data.charge_lines_raw_data(),
    );
    let mock_cmplt_req = ut_setup_refund_cmplt_dto(time_now, mock_data.refund_lines_raw_data());
    let arg = (mock_data.merchant_id, &mock_charge_m, &mock_cmplt_req);
    let rfd_rslv_m = RefundReqResolutionModel::try_from(arg).unwrap();

    let proc_ctx = shr_state.processor_context();
    let result = proc_ctx.refund(rfd_rslv_m).await;
    assert!(result.is_err());
    if let Err(e) = result {
        if let AppProcessorErrorReason::LowLvlNet(be) = e.reason {
            if let BaseClientErrorReason::DeserialiseFailure(raw_resp_body, http_status) = be.reason
            {
                assert_eq!(http_status, 404);
                assert!(raw_resp_body.contains("resource_missing"));
            } else {
                assert!(false);
            }
        } else {
            assert!(false);
        }
    } else {
        assert!(false);
    }
} // end of fn err_invalid_payment_intent
