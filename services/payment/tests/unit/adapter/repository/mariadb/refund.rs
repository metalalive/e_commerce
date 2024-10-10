use chrono::{Duration, Local, SubsecRound};
use rust_decimal::Decimal;

use ecommerce_common::constant::ProductType;
use ecommerce_common::model::BaseProductIdentity;
use payment::model::{
    OLineRefundModel, OrderRefundModel, PayLineAmountModel, RefundLineQtyRejectModel,
};

use super::ut_setup_db_refund_repo;
use crate::ut_setup_sharestate;

#[rustfmt::skip]
fn ut_setup_refund_model() -> Vec<OrderRefundModel> {
    let mut lines = [
        (1063, ProductType::Package, 25, (219, 1), (1971, 1), 9, 15),
        (1063, ProductType::Package, 25, (219, 1), (438, 1), 2, 49),
        (1063, ProductType::Item, 2753, (1005, 2), (7035, 2), 7, 15),
        (1027, ProductType::Package, 902, (3040, 2), (24320, 2), 8, 20),
        (1063, ProductType::Item, 409, (2016, 2), (8064, 2), 4, 53),
        (1064, ProductType::Package, 188, (2009, 1), (4018, 1), 2, 36),
    ].into_iter()
    .map(|d| {
        let pid = BaseProductIdentity { store_id: d.0, product_type: d.1, product_id: d.2 };
        let amt_req = PayLineAmountModel {
            unit: Decimal::new(d.3.0, d.3.1),
            total: Decimal::new(d.4.0, d.4.1),
            qty: d.5
        };
        let ctime = Local::now().to_utc() - Duration::minutes(d.6);
        let amt_refunded = PayLineAmountModel::default();
        let reject = RefundLineQtyRejectModel::default();
        OLineRefundModel::from((pid, amt_req, ctime, amt_refunded, reject))
    }).collect::<Vec<_>>();
    let lines2 = lines.drain(3..).collect();
    [
        ("0238b874", lines),
        ("7e80118273b7", lines2),
    ]
        .into_iter()
        .map(|d| {
            OrderRefundModel::from((d.0.to_string(), d.1))
        }).collect::<Vec<_>>()
} // end of ut_setup_refund_model

#[actix_web::test]
async fn update_sync_time_ok() {
    let shr_state = ut_setup_sharestate();
    let repo = ut_setup_db_refund_repo(shr_state).await;

    let result = repo.last_time_synced().await;
    assert!(result.is_ok());
    let option_time = result.unwrap();
    assert!(option_time.is_none());

    let mock_time = Local::now().to_utc().trunc_subsecs(3) - Duration::hours(3);
    let result = repo.update_sycned_time(mock_time).await;
    assert!(result.is_ok());

    let result = repo.last_time_synced().await;
    let option_time = result.unwrap();
    assert!(option_time.is_some());
    let time_read = option_time.unwrap();
    assert_eq!(time_read, mock_time);

    let mock_newtime = mock_time + Duration::minutes(50);
    let result = repo.update_sycned_time(mock_newtime).await;
    assert!(result.is_ok());

    let result = repo.last_time_synced().await;
    let option_time = result.unwrap();
    assert!(option_time.is_some());
    let time_read = option_time.unwrap();
    assert_eq!(time_read, mock_newtime);
} // end of fn update_sync_time_ok

#[actix_web::test]
async fn save_refund_req_ok() {
    let shr_state = ut_setup_sharestate();
    let repo = ut_setup_db_refund_repo(shr_state).await;
    let mock_rfd_ms = ut_setup_refund_model();
    let result = repo.save_request(mock_rfd_ms).await;
    assert!(result.is_ok());
} // end of fn save_refund_req_ok
