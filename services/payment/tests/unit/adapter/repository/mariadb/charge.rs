use std::boxed::Box;
use std::sync::Arc;

use chrono::Duration;
use ecommerce_common::constant::ProductType;
use payment::adapter::repository::{app_repo_charge, AbstractChargeRepo};

use crate::adapter::repository::{ut_setup_order_bill, ut_setup_orderline_set};
use crate::ut_setup_sharestate;

async fn ut_setup_db_repo() -> Arc<Box<dyn AbstractChargeRepo>> {
    let shr_state = ut_setup_sharestate("config_ok.json");
    let dstore = shr_state.datastore();
    let result = app_repo_charge(dstore).await;
    let repo = result.unwrap();
    Arc::new(repo)
}

#[actix_web::test]
async fn create_order_ok() {
    let repo = ut_setup_db_repo().await;
    let ol_set = ut_setup_orderline_set(
        123,
        "9d73ba76d5",
        0,
        vec![
            (
                2603,
                ProductType::Item,
                180,
                [34, 340, 10, 68, 2],
                Duration::minutes(2),
            ),
            (
                2603,
                ProductType::Package,
                211,
                [29, 261, 9, 58, 2],
                Duration::minutes(3),
            ),
            (
                2379,
                ProductType::Item,
                449,
                [35, 420, 12, 35, 1],
                Duration::minutes(4),
            ),
        ],
    );
    let billing = ut_setup_order_bill();
    let result = repo.create_order(&ol_set, &billing).await;
    if let Err(e) = &result {
        println!("[debug] DB error {:?}", e)
    }
    assert!(result.is_ok());
} // end of fn create_order_ok
