use chrono::Utc;
use rust_decimal::Decimal;

use ecommerce_common::constant::ProductType;
use ecommerce_common::model::BaseProductIdentity;
use payment::api::web::dto::{
    PaymentMethodReqDto, PaymentMethodRespDto, StripeCheckoutSessionReqDto, StripeCheckoutUImodeDto,
};
use payment::model::{
    BuyerPayInState, ChargeBuyerModel, ChargeLineBuyerModel, ChargeToken, PayLineAmountModel,
};

use crate::model::ut_default_currency_snapshot;
use crate::ut_setup_sharestate;

fn ut_setup_chargebuyer_stripe(
    owner: u32,
    order_id: &str,
    mock_lines: Vec<(u32, ProductType, u64, Decimal, Decimal, u32)>,
) -> ChargeBuyerModel {
    let ctime = Utc::now();
    let token = ChargeToken::encode(owner, ctime.clone());
    let mut usr_ids = mock_lines.iter().map(|dl| dl.0).collect::<Vec<_>>();
    usr_ids.push(owner);
    let currency_snapshot = ut_default_currency_snapshot(usr_ids);
    let method_inner = StripeCheckoutSessionReqDto {
        customer_id: None,
        return_url: None,
        success_url: Some("https://docs.rs/tokio".to_string()),
        cancel_url: Some("https://resources.nvidia.com/en-us-grace-cpu".to_string()),
        ui_mode: StripeCheckoutUImodeDto::RedirectPage,
    };
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
    ChargeBuyerModel {
        owner,
        token,
        create_time: ctime,
        oid: order_id.to_string(),
        lines,
        currency_snapshot,
        state: BuyerPayInState::Initialized,
        method: PaymentMethodReqDto::Stripe(method_inner),
    }
} // end of fn ut_setup_chargebuyer_stripe

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
    let cline_set = ut_setup_chargebuyer_stripe(mock_usr_id, mock_order_id, mock_lines);
    let result = proc_ctx.pay_in_start(&cline_set).await;
    if let Err(e) = &result {
        println!("unit test error : {:?}", e)
    }
    assert!(result.is_ok());
    if let Ok(pay_in_res) = result {
        let cond = matches!(pay_in_res.state, BuyerPayInState::ProcessorAccepted(_));
        assert!(cond);
        match &pay_in_res.method {
            PaymentMethodRespDto::Stripe(s) => {
                assert!(s.redirect_url.is_some());
                assert!(s.client_session.is_none());
            }
        }
    }
    // TODO, automatically fill form on Stripe-hosted page using web scraping tools
} // end of fn pay_in_ok
