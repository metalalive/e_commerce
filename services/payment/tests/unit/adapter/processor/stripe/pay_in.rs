use std::thread::sleep;
use std::time::Duration;

use chrono::Utc;
use fantoccini::{Client as PageCtrler, ClientBuilder, Locator};
use rust_decimal::Decimal;
use serde_json::{Map as JsnMap, Value as JsnVal};

use ecommerce_common::model::BaseProductIdentity;
use payment::adapter::processor::{AppProcessorError, AppProcessorPayInResult};
use payment::api::web::dto::{
    PaymentMethodReqDto, PaymentMethodRespDto, StripeCheckoutSessionReqDto, StripeCheckoutUImodeDto,
};
use payment::hard_limit::CREATE_CHARGE_SECONDS_INTERVAL;
use payment::model::{
    BuyerPayInState, Charge3partyModel, ChargeBuyerMetaModel, ChargeBuyerModel,
    ChargeLineBuyerModel, PayLineAmountModel, StripeCheckoutPaymentStatusModel,
    StripeSessionStatusModel,
};

use super::pay_out;
use crate::model::ut_default_currency_snapshot;
use crate::ut_setup_sharestate;

struct UtestWebForm<'a> {
    ctrler: &'a PageCtrler,
}
impl<'a> UtestWebForm<'a> {
    async fn write_input_field<'b, 'c>(&self, id: &'b str, wr_value: &'c str) {
        let result = self.ctrler.find(Locator::Id(id)).await;
        let elm = result.unwrap();
        let actual_tag = elm.tag_name().await.unwrap();
        assert_eq!(actual_tag.as_str(), "input");
        assert!(elm.is_enabled().await.unwrap());
        let result = elm.send_keys(wr_value).await;
        assert!(result.is_ok());
    }
    async fn click_button_by_css(&self, cls: &str) {
        let result = self.ctrler.find(Locator::Css(cls)).await;
        let elm = result.unwrap();
        let actual_tag = elm.tag_name().await.unwrap();
        assert_eq!(actual_tag.as_str(), "button");
        assert!(elm.is_enabled().await.unwrap());
        let result = elm.click().await;
        assert!(result.is_ok());
    } // TODO, improve
}

fn ut_setup_chargebuyer_stripe(
    owner: u32,
    order_id: &str,
    mock_lines: Vec<(u32, u64, (i64, u32), (i64, u32), u32)>,
) -> ChargeBuyerModel {
    let ctime = Utc::now();
    let mut usr_ids = mock_lines.iter().map(|dl| dl.0).collect::<Vec<_>>();
    usr_ids.push(owner);
    let currency_snapshot = ut_default_currency_snapshot(usr_ids);
    let lines = mock_lines
        .into_iter()
        .map(|d| {
            let pid = BaseProductIdentity {
                store_id: d.0,
                product_id: d.1,
            };
            let amt_orig = PayLineAmountModel {
                unit: Decimal::new(d.2 .0, d.2 .1),
                total: Decimal::new(d.3 .0, d.3 .1),
                qty: d.4,
            };
            let amt_refuned = PayLineAmountModel::default();
            let num_rejected = 0u32;
            let attr_set_seq_dummy = 0;
            let args = (pid, attr_set_seq_dummy, amt_orig, amt_refuned, num_rejected);
            ChargeLineBuyerModel::from(args)
        })
        .collect();
    let arg = (order_id.to_string(), owner, ctime);
    let meta = ChargeBuyerMetaModel::from(arg);
    ChargeBuyerModel {
        lines,
        meta,
        currency_snapshot,
    }
} // end of fn ut_setup_chargebuyer_stripe

fn ut_default_method_stripe_request() -> PaymentMethodReqDto {
    let inner = StripeCheckoutSessionReqDto {
        customer_id: None,
        return_url: None,
        success_url: Some("https://docs.rs/tokio".to_string()),
        cancel_url: Some("https://resources.nvidia.com/en-us-grace-cpu".to_string()),
        ui_mode: StripeCheckoutUImodeDto::RedirectPage,
    };
    PaymentMethodReqDto::Stripe(inner)
}

macro_rules! ut_verify_charge_stripe_model {
    (
        $charge_mthd_m: expr,
        $expect_sess_state: path,
        $expect_pay_state: path,
    ) => {
        match $charge_mthd_m {
            Charge3partyModel::Stripe(m) => {
                assert!(!m.checkout_session_id.is_empty());
                assert!(!m.payment_intent_id.is_empty());
                assert!(!m.transfer_group.is_empty());
                let cond = matches!(&m.session_state, $expect_sess_state);
                assert!(cond);
                let cond = matches!(&m.payment_state, $expect_pay_state);
                assert!(cond);
            }
            Charge3partyModel::Unknown => {
                assert!(false);
            }
        }
    };
}

fn ut_verify_stripe_session_ok(
    result: Result<(AppProcessorPayInResult, Charge3partyModel), AppProcessorError>,
) -> (AppProcessorPayInResult, Charge3partyModel) {
    if let Err(e) = &result {
        println!("unit test error : {:?}", e)
    }
    assert!(result.is_ok());
    let (pay_in_res, charge_3pty_m) = result.unwrap();
    let cond = matches!(pay_in_res.state, BuyerPayInState::ProcessorAccepted(_));
    assert!(cond);
    match &pay_in_res.method {
        PaymentMethodRespDto::Stripe(s) => {
            assert!(s.redirect_url.is_some());
            assert!(s.client_session.is_none());
        }
    }
    ut_verify_charge_stripe_model!(
        &charge_3pty_m,
        StripeSessionStatusModel::open,
        StripeCheckoutPaymentStatusModel::unpaid,
    );
    (pay_in_res, charge_3pty_m)
} // end of fn ut_verify_stripe_session_ok

async fn ut_autofill_stripe_hosted_webform(
    pay_in_res: AppProcessorPayInResult,
    data: [&str; 5],
    // number of seconds to wait after clicking the button to submit payment form
    nsecs_wait_after_submit: usize,
) {
    let cap = {
        let opts_serial = br#"{"args": ["-headless"], "log": {"level": "info"}}"#;
        let moz_opts = serde_json::from_slice::<JsnVal>(opts_serial).unwrap();
        let kv = [
            (
                "browserName".to_string(),
                JsnVal::String("firefox".to_string()),
            ),
            ("moz:firefoxOptions".to_string(), moz_opts),
        ];
        JsnMap::from_iter(kv.into_iter())
    };
    let controller = ClientBuilder::native()
        .capabilities(cap)
        .connect("http://localhost:4444")
        .await
        .unwrap();
    let url = match &pay_in_res.method {
        PaymentMethodRespDto::Stripe(s) => s.redirect_url.clone().unwrap(),
    };
    let result = controller.goto(url.as_str()).await;
    assert!(result.is_ok());

    let form = UtestWebForm {
        ctrler: &controller,
    };
    form.write_input_field("email", data[0]).await;
    form.write_input_field("billingName", data[1]).await;
    form.write_input_field("cardNumber", data[2]).await;
    form.write_input_field("cardExpiry", data[3]).await;
    form.write_input_field("cardCvc", data[4]).await;
    form.click_button_by_css(".SubmitButton").await;

    for _ in 0..nsecs_wait_after_submit {
        let result = controller.current_url().await;
        let curr_url = result.unwrap();
        if curr_url.as_str() == url.as_str() {
            sleep(Duration::from_secs(1));
        } else {
            println!("[debug] redirect back to your app website");
            break;
        }
    }
    controller.close().await.unwrap();
    sleep(Duration::from_secs(CREATE_CHARGE_SECONDS_INTERVAL as u64));
} // end of fn ut_autofill_stripe_hosted_webform

#[actix_web::test]
async fn charge_flow_completed() {
    let (mock_usr_id, mock_order_id) = (195, "a0b46792f11c");
    let shr_state = ut_setup_sharestate();
    let proc_ctx = shr_state.processor_context();
    // for TWD, Stripe allows only 2 decimal places in the fractional part of amount
    #[rustfmt::skip]
    let mock_lines = vec![
        (26u32, 2603u64, (791i64, 1u32), (3955i64, 1u32), 5u32),
        (12, 1227, (48, 0), (432, 0), 9),
        (12, 8454, (502, 1), (6024, 1), 12),
    ];
    let pay_mthd_req = ut_default_method_stripe_request();
    let mut charge_buyer = ut_setup_chargebuyer_stripe(mock_usr_id, mock_order_id, mock_lines);
    let result = proc_ctx.pay_in_start(&charge_buyer, pay_mthd_req).await;
    let (pay_in_res, charge_3pty_m) = ut_verify_stripe_session_ok(result);

    // automatically fill form on Stripe-hosted page using web scraping tools
    let test_data = [
        "whatever@unittest-data.io",
        "Toxic Poisoneous",
        "3566002020360505",
        "1125",
        "689",
    ];
    ut_autofill_stripe_hosted_webform(pay_in_res, test_data, 45).await;

    // refresh charge status
    charge_buyer.meta.update_3party(charge_3pty_m);
    let result = proc_ctx.pay_in_progress(&charge_buyer.meta).await;
    assert!(result.is_ok());
    let charge_3pty_m = result.unwrap();
    ut_verify_charge_stripe_model!(
        &charge_3pty_m,
        StripeSessionStatusModel::complete,
        StripeCheckoutPaymentStatusModel::paid,
    );

    charge_buyer.meta.update_3party(charge_3pty_m);
    pay_out::ok_exact_once(shr_state.config(), proc_ctx, charge_buyer).await;
} // end of fn charge_flow_completed

#[actix_web::test]
async fn charge_declined_invalid_card() {
    let (mock_usr_id, mock_order_id) = (196, "0b46792f11c5");
    let shr_state = ut_setup_sharestate();
    let proc_ctx = shr_state.processor_context();
    let mock_lines = vec![
        (26u32, 23u64, (96i64, 0u32), (480i64, 0u32), 5u32),
        (12, 12721, (48, 0), (432, 0), 9),
    ];
    let pay_mthd_req = ut_default_method_stripe_request();
    let mut charge_buyer = ut_setup_chargebuyer_stripe(mock_usr_id, mock_order_id, mock_lines);
    let result = proc_ctx.pay_in_start(&charge_buyer, pay_mthd_req).await;
    let (pay_in_res, charge_3pty_m) = ut_verify_stripe_session_ok(result);

    let test_data = [
        "washing@money-laundry.org",
        "Liar Cheater",
        "4000000000004954", // assume high risk card in Stripe fraud detection
        "0128",
        "037",
    ]; // Stripe will report error on the checkout form and never redirect
       // to user-specific URL
    ut_autofill_stripe_hosted_webform(pay_in_res, test_data, 20).await;

    // --- refresh charge status
    charge_buyer.meta.update_3party(charge_3pty_m);
    let result = proc_ctx.pay_in_progress(&charge_buyer.meta).await;
    assert!(result.is_ok());
    let charge_3pty_m = result.unwrap();
    ut_verify_charge_stripe_model!(
        &charge_3pty_m,
        StripeSessionStatusModel::open,
        StripeCheckoutPaymentStatusModel::unpaid,
    );
} // end of fn charge_declined_invalid_card
