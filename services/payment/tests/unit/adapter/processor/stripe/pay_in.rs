use chrono::Utc;
use rust_decimal::Decimal;

use ecommerce_common::constant::ProductType;
use ecommerce_common::model::BaseProductIdentity;
use payment::api::web::dto::{
    PaymentMethodReqDto, PaymentMethodRespDto, StripeCheckoutSessionReqDto, StripeCheckoutUImodeDto,
};
use payment::model::{
    BuyerPayInState, ChargeBuyerMetaModel, ChargeBuyerModel, ChargeLineBuyerModel,
    ChargeMethodModel, PayLineAmountModel, StripeCheckoutPaymentStatusModel,
    StripeSessionStatusModel,
};

use crate::model::ut_default_currency_snapshot;
use crate::ut_setup_sharestate;

fn ut_setup_chargebuyer_stripe(
    owner: u32,
    order_id: &str,
    mock_lines: Vec<(u32, ProductType, u64, Decimal, Decimal, u32)>,
) -> ChargeBuyerModel {
    let ctime = Utc::now();
    let mut usr_ids = mock_lines.iter().map(|dl| dl.0).collect::<Vec<_>>();
    usr_ids.push(owner);
    let currency_snapshot = ut_default_currency_snapshot(usr_ids);
    let lines = mock_lines
        .into_iter()
        .map(|d| ChargeLineBuyerModel {
            pid: BaseProductIdentity {
                store_id: d.0,
                product_type: d.1,
                product_id: d.2,
            },
            amount: PayLineAmountModel {
                unit: d.3,
                total: d.4,
                qty: d.5,
            },
        })
        .collect();
    let meta = ChargeBuyerMetaModel {
        owner,
        create_time: ctime,
        oid: order_id.to_string(),
        state: BuyerPayInState::Initialized,
        method: ChargeMethodModel::Unknown,
    };
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

#[actix_web::test]
async fn pay_in_ok() {
    let (mock_usr_id, mock_order_id) = (195, "a0b46792f11c");
    let shr_state = ut_setup_sharestate();
    let proc_ctx = shr_state.processor_context();
    // for TWD, Stripe allows only 2 decimal places in the fractional part of amount
    let mock_lines = vec![
        (
            26u32,
            ProductType::Package,
            2603u64,
            Decimal::new(791, 1),
            Decimal::new(3955, 1),
            5u32,
        ),
        (
            12,
            ProductType::Item,
            1227,
            Decimal::new(48, 0),
            Decimal::new(432, 0),
            9,
        ),
    ];
    let pay_mthd_req = ut_default_method_stripe_request();
    let charge_buyer = ut_setup_chargebuyer_stripe(mock_usr_id, mock_order_id, mock_lines);
    let result = proc_ctx.pay_in_start(&charge_buyer, pay_mthd_req).await;
    if let Err(e) = &result {
        println!("unit test error : {:?}", e)
    }
    assert!(result.is_ok());
    let (pay_in_res, charge_mthd_m) = result.unwrap();
    let cond = matches!(pay_in_res.state, BuyerPayInState::ProcessorAccepted(_));
    assert!(cond);
    match &pay_in_res.method {
        PaymentMethodRespDto::Stripe(s) => {
            assert!(s.redirect_url.is_some());
            assert!(s.client_session.is_none());
        }
    }
    match &charge_mthd_m {
        ChargeMethodModel::Stripe(m) => {
            assert!(!m.checkout_session_id.is_empty());
            assert!(!m.payment_intent_id.is_empty());
            let cond = matches!(m.session_state, StripeSessionStatusModel::open);
            assert!(cond);
            let cond = matches!(m.payment_state, StripeCheckoutPaymentStatusModel::unpaid);
            assert!(cond);
        }
        ChargeMethodModel::Unknown => {
            assert!(false);
        }
    }
    // TODO, automatically fill form on Stripe-hosted page using web scraping tools
} // end of fn pay_in_ok
