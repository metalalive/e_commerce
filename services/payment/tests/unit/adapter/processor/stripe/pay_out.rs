use std::boxed::Box;
use std::fs::File;
use std::sync::Arc;

use chrono::Local;
use ecommerce_common::api::dto::CurrencyDto;
use ecommerce_common::constant::ProductType;
use rust_decimal::Decimal;
use serde_json::Value as JsnVal;

use ecommerce_common::config::AppConfig;
use payment::adapter::processor::{
    AbstractPaymentProcessor, AppProcessorErrorReason, AppProcessorFnLabel, BaseClientErrorReason,
};
use payment::model::{
    BuyerPayInState, Charge3partyModel, ChargeBuyerModel, Payout3partyModel,
    Payout3partyStripeModel, PayoutAmountModel, PayoutModel,
};

use crate::model::{
    ut_default_charge_method_stripe, ut_default_currency_snapshot, ut_setup_buyer_charge,
};
use crate::{ut_setup_sharestate, EXAMPLE_REL_PATH};

#[rustfmt::skip]
fn ut_setup_merchant_stripe_account(
    acfg: Arc<AppConfig>,
    filename: &str,
    merchant_id: u32,
) -> String {
    let fullpath = acfg.basepath.service.clone() + EXAMPLE_REL_PATH + filename;
    let f = File::open(fullpath).unwrap();
    let map = serde_json::from_reader::<File, JsnVal>(f).unwrap();
    let key = merchant_id.to_string();
    let found_account = map.as_object().unwrap()
        .get(&key).unwrap().as_str().unwrap();
    found_account.to_string()
}

#[rustfmt::skip]
fn ut_setup_payout_model(
    mock_merchant_id: u32,
    mock_staff_id: u32,
    acfg: Arc<AppConfig>,
    charge_buyer: &ChargeBuyerModel,
) -> PayoutModel {
    let stripe_dst_account = ut_setup_merchant_stripe_account(
        acfg, "app-usr-id-to-stripe-account.json", mock_merchant_id,
    );
    let stripe_tx_grp =
        if let Charge3partyModel::Stripe(pay_mthd) = charge_buyer.meta.method_3party() {
            pay_mthd.transfer_group.clone()
        } else {
            panic!("");
        };
    let arg = (stripe_tx_grp, stripe_dst_account, None, None);
    let mock_3pty = Payout3partyModel::Stripe(Payout3partyStripeModel::from(arg));
    let buyer_usr_id = charge_buyer.meta.owner();
    let arg = (
        // Decimal::ONE, Decimal::new(3222, 2),
        Decimal::new(10344, 1), 
        charge_buyer.currency_snapshot.get(&buyer_usr_id).unwrap().clone(),
        charge_buyer.currency_snapshot.get(&mock_merchant_id).unwrap().clone(),
    );
    let mock_amount = PayoutAmountModel::try_from(arg).unwrap();
    let mock_order_id = "ouwa-a-A-ha".to_string();
    let arg = (
        mock_merchant_id, Local::now().to_utc(), buyer_usr_id,  *charge_buyer.meta.create_time(),
        mock_order_id, mock_staff_id, mock_amount, mock_3pty,
    );
    PayoutModel::from(arg)
}

// this function is invoked as part of happy-path test case in pay-in module
// TODO, better design for testing
#[rustfmt::skip]
pub(super) async fn ok_exact_once(
    acfg: Arc<AppConfig>,
    proc_ctx : Arc<Box<dyn AbstractPaymentProcessor>>,
    charge_buyer : ChargeBuyerModel,
) {
    let (mock_merchant_id, mock_staff_id) = (12u32, 5566u32);
    let mock_payout_m = ut_setup_payout_model(
        mock_merchant_id, mock_staff_id, acfg, &charge_buyer
    );
    let result = proc_ctx.pay_out(mock_payout_m).await;
    assert!(result.is_ok());
    if let Ok(v) = result {
        let (payout_dto, payout_m) = v.into_parts();
        assert_eq!(payout_dto.currency, CurrencyDto::TWD);
        assert_eq!(payout_dto.amount, "1034.40");
        match payout_m.thirdparty() {
            Payout3partyModel::Stripe(s) => {
                let amt_serial = s.amount().unwrap();
                assert_eq!(amt_serial, Decimal::new(3222, 2));
            }
        }
    }
} // end of fn ok_exact_once

#[rustfmt::skip]
#[actix_web::test]
async fn err_invalid_account() {
    let shr_state = ut_setup_sharestate();
    let proc_ctx = shr_state.processor_context();
    let (mock_buyer_id, mock_merchant_id, mock_staff_id) = (95u32, 999u32, 5566u32);
    let charge_buyer = {
        let usr_ids = vec![mock_buyer_id, mock_merchant_id];
        let currency_snapshot = ut_default_currency_snapshot(usr_ids);
        let time_now = Local::now().to_utc();
        let charge_lines = vec![
            (mock_merchant_id, ProductType::Item, 91038u64,
             (300i64, 0u32), (900i64, 0u32), 3u32,
             (0i64, 0u32), (0i64, 0u32), 0u32, 0u32),
        ];
        let paymethod = ut_default_charge_method_stripe(&time_now);
        ut_setup_buyer_charge(
            mock_buyer_id, time_now, "unit-test-mock-order-id".to_string(),
            BuyerPayInState::OrderAppSynced(time_now),
            paymethod,
            charge_lines, currency_snapshot
        )
    };
    let mock_payout_m = ut_setup_payout_model(
        mock_merchant_id, mock_staff_id, shr_state.config(), &charge_buyer
    );
    let result = proc_ctx.pay_out(mock_payout_m).await;
    assert!(result.is_err());
    if let Err(e) = result {
        let cond = matches!(e.fn_label, AppProcessorFnLabel::PayOut);
        assert!(cond);
        if let AppProcessorErrorReason::LowLvlNet(ce) = e.reason {
            let cond = matches!(ce.reason, BaseClientErrorReason::DeserialiseFailure(_, _));
            assert!(cond);
        } else {
            assert!(false);
        }
    }
} // end of fn err_invalid_account
