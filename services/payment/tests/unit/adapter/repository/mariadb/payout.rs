use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Duration, Local, Utc};
use rust_decimal::Decimal;

use ecommerce_common::api::dto::CurrencyDto;
use ecommerce_common::constant::ProductType;
use ecommerce_common::error::AppErrorCode;
use payment::adapter::repository::{AbstractChargeRepo, AppRepoErrorDetail};
use payment::hard_limit::CURRENCY_RATE_PRECISION;
use payment::model::{
    OrderCurrencySnapshot, Payout3partyModel, Payout3partyStripeModel, PayoutAmountModel,
    PayoutModel,
};

use super::super::{ut_setup_order_bill, ut_setup_orderline_set};
use super::ut_setup_db_charge_repo;
use crate::ut_setup_sharestate;

#[rustfmt::skip]
fn ut_setup_currency_snapshot() -> (Decimal, [OrderCurrencySnapshot;2]) {
    let total_amt_buyer = Decimal::new(9111, 1);
    let currency_seller = OrderCurrencySnapshot { label: CurrencyDto::TWD, rate: Decimal::new(3196, 2) };
    let currency_buyer = OrderCurrencySnapshot { label: CurrencyDto::INR, rate: Decimal::new(8778, 2) };
    (total_amt_buyer, [currency_buyer, currency_seller])
}

#[rustfmt::skip]
fn ut_setup_payout_model_stripe(
    order_id: String,
    buyer_id: u32,
    charged_ctime: DateTime<Utc>,
    merchant_id: u32,
) -> PayoutModel {
    let p3pty_m = {
        let tx_grp = "mock_charge_id_serial".to_string();
        let acct_id = "acct_no-place-is-perfect".to_string();
        let transfer_id = Some("tr_hells-you-should-avoid".to_string());
        let amount = Some(Decimal::new(1037, 2));
        let args = (tx_grp, acct_id, transfer_id, amount);
        let s = Payout3partyStripeModel::from(args);
        Payout3partyModel::Stripe(s)
    };
    let amt_m = {
        let (amt_buyer, [currency_buyer, currency_seller]) = ut_setup_currency_snapshot(); 
        let args = (amt_buyer, currency_buyer, currency_seller);
        PayoutAmountModel::try_from(args).unwrap()
    };
    let mock_storestaff_id = 904u32;
    let mock_captured_time = charged_ctime + Duration::minutes(49);
    let args = (
        merchant_id, mock_captured_time, buyer_id, charged_ctime,
        order_id, mock_storestaff_id, amt_m, p3pty_m,
    );
    PayoutModel::from(args)
} // end of fn ut_setup_payout_model_stripe

#[rustfmt::skip]
async fn ut_setup_order_replica(
    repo: Arc<Box<dyn AbstractChargeRepo>>,
    order_id: &str,
    buyer_usr_id: u32,
    charged_ctime: DateTime<Utc>,
    merchant_id: u32,
) { // to ensure currency snapshot data is ready
    let (amt_buyer, [currency_buyer, currency_seller]) = ut_setup_currency_snapshot(); 
    let mock_olines_data = vec![
        (merchant_id, ProductType::Package, 89u64, amt_buyer,
         amt_buyer, 1, Duration::minutes(219))
    ];
    let mock_currency_snapshot = {
        let list = [
            (buyer_usr_id, currency_buyer),
            (merchant_id, currency_seller),
        ];
        HashMap::from(list)
    };
    let expect_ol_set = ut_setup_orderline_set(
        buyer_usr_id, order_id, 0, charged_ctime,
        mock_currency_snapshot, mock_olines_data,
    );
    let billing = ut_setup_order_bill();
    let result = repo.create_order(&expect_ol_set, &billing).await;
    assert!(result.is_ok());
} // end of fn ut_setup_order_replica

#[actix_web::test]
async fn create_fetch_ok() {
    let shr_state = ut_setup_sharestate();
    let repo = ut_setup_db_charge_repo(shr_state).await;
    let mock_order_id = "dd20de75d019".to_string();
    let mock_buyer_id = 127u32;
    let mock_merchant_id = 6741u32;
    let mock_charged_ctime = Local::now().to_utc() - Duration::minutes(74);

    ut_setup_order_replica(
        repo.clone(),
        mock_order_id.as_str(),
        mock_buyer_id,
        mock_charged_ctime,
        mock_merchant_id,
    )
    .await;
    let payout_m = ut_setup_payout_model_stripe(
        mock_order_id,
        mock_buyer_id,
        mock_charged_ctime,
        mock_merchant_id,
    );
    let result = repo.create_payout(payout_m).await;
    assert!(result.is_ok());

    let result = repo
        .fetch_payout(mock_merchant_id, mock_buyer_id, mock_charged_ctime)
        .await;
    assert!(result.is_ok());
    let maybe_payout_m = result.unwrap();
    assert!(maybe_payout_m.is_some());
    let read_payout_m = maybe_payout_m.unwrap();
    assert_eq!(read_payout_m.merchant_id(), mock_merchant_id);
    let read_currency_base = read_payout_m.amount_base();
    assert_eq!(read_currency_base, Decimal::new(1037, 2));
    let (read_amount_merc, read_target_rate, read_currency_merc) = read_payout_m.amount_merchant();
    assert_eq!(read_currency_merc.label, CurrencyDto::TWD);
    assert_eq!(read_currency_merc.rate, Decimal::new(3196, 2));
    assert_eq!(
        read_target_rate,
        Decimal::new(036409204, CURRENCY_RATE_PRECISION)
    );
    assert_eq!(read_amount_merc, Decimal::new(33172, 2));
    match read_payout_m.thirdparty() {
        Payout3partyModel::Stripe(s) => {
            assert_eq!(s.amount().unwrap(), Decimal::new(1037, 2));
        }
    }
} // end of fn create_fetch_ok

#[actix_web::test]
async fn fetch_empty() {
    let shr_state = ut_setup_sharestate();
    let repo = ut_setup_db_charge_repo(shr_state).await;
    let mock_buyer_id = 128u32;
    let mock_merchant_id = 6741u32;
    let mock_charged_ctime = Local::now().to_utc() - Duration::minutes(999);
    let result = repo
        .fetch_payout(mock_merchant_id, mock_buyer_id, mock_charged_ctime)
        .await;
    assert!(result.is_ok());
    let maybe_payout_m = result.unwrap();
    assert!(maybe_payout_m.is_none());
}

#[actix_web::test]
async fn create_fetch_missing_currency() {
    let shr_state = ut_setup_sharestate();
    let repo = ut_setup_db_charge_repo(shr_state).await;
    let mock_order_id = "1037473a50730135".to_string();
    let mock_buyer_id = 127u32;
    let mock_merchant_id = 6741u32;
    let mock_charged_ctime = Local::now().to_utc() - Duration::minutes(88);
    let payout_m = ut_setup_payout_model_stripe(
        mock_order_id,
        mock_buyer_id,
        mock_charged_ctime,
        mock_merchant_id,
    );
    let result = repo.create_payout(payout_m).await;
    assert!(result.is_ok());

    let result = repo
        .fetch_payout(mock_merchant_id, mock_buyer_id, mock_charged_ctime)
        .await;
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(matches!(e.code, AppErrorCode::DataCorruption));
        if let AppRepoErrorDetail::DataRowParse(s) = e.detail {
            assert_eq!(s.as_str(), "missing-buyer-currency");
        } else {
            assert!(false);
        }
    }
} // end of fn create_fetch_missing_currency
