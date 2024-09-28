use chrono::{Duration, Local};
use ecommerce_common::api::dto::CurrencyDto;
use rust_decimal::Decimal;

use payment::model::{
    OrderCurrencySnapshot, Payout3partyModel, Payout3partyStripeModel, PayoutAmountModel,
    PayoutModel,
};

use super::ut_setup_db_charge_repo;
use crate::ut_setup_sharestate;

#[rustfmt::skip]
fn ut_setup_payout_model_stripe() -> PayoutModel {
    let p3pty_m = {
        let tx_grp = "mock_charge_id_serial".to_string();
        let acct_id = "acct_no-place-is-perfect".to_string();
        let transfer_id = Some("tr_hells-you-should-avoid".to_string());
        let amount = Some(Decimal::new(9105, 2));
        let args = (tx_grp, acct_id, transfer_id, amount);
        let s = Payout3partyStripeModel::from(args);
        Payout3partyModel::Stripe(s)
    };
    let amt_m = {
        let (target_rate, total_bs, total_mc) = (Decimal::new(3196, 2), Decimal::new(911, 1), Decimal::new(275042, 1));
        let currency_seller = OrderCurrencySnapshot { label: CurrencyDto::TWD, rate: target_rate };
        let currency_buyer = OrderCurrencySnapshot { label: CurrencyDto::USD, rate: Decimal::ONE };
        let args = (target_rate, total_bs, total_mc, currency_seller, currency_buyer);
        PayoutAmountModel::from(args)
    };
    let (mock_merchant_id, mock_buyer_id) = (6741u32, 3990u32);
    let mock_storestaff_id = 904u32;
    let mock_charged_ctime = Local::now().to_utc() - Duration::minutes(74);
    let mock_captured_time = mock_charged_ctime + Duration::minutes(49);
    let order_id = "de75d019".to_string();
    let args = (
        mock_merchant_id, mock_captured_time, mock_buyer_id, mock_charged_ctime,
        order_id, mock_storestaff_id, amt_m, p3pty_m,
    );
    PayoutModel::from(args)
}

#[actix_web::test]
async fn create_ok() {
    let shr_state = ut_setup_sharestate();
    let repo = ut_setup_db_charge_repo(shr_state).await;
    let payout_m = ut_setup_payout_model_stripe();
    let result = repo.create_payout(payout_m).await;
    assert!(result.is_ok());
}
