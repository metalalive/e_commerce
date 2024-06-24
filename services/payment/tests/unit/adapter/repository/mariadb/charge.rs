use std::boxed::Box;
use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Duration, FixedOffset, Local, SubsecRound};
use ecommerce_common::constant::ProductType;
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::model::BaseProductIdentity;
use payment::adapter::repository::{app_repo_charge, AbstractChargeRepo, AppRepoErrorDetail};
use payment::api::web::dto::{
    PaymentMethodReqDto, StripeCheckoutSessionReqDto, StripeCheckoutUImodeDto,
};
use payment::model::{BuyerPayInState, ChargeBuyerModel, OrderLineModel, OrderLineModelSet};
use payment::AppSharedState;

use crate::adapter::repository::{
    ut_setup_buyer_charge, ut_setup_order_bill, ut_setup_orderline_set,
};
use crate::ut_setup_sharestate;

async fn ut_setup_db_repo(shr_state: AppSharedState) -> Arc<Box<dyn AbstractChargeRepo>> {
    let dstore = shr_state.datastore();
    let result = app_repo_charge(dstore).await;
    let repo = result.unwrap();
    Arc::new(repo)
}

fn ut_verify_fetched_order(
    actual: OrderLineModelSet,
    expect_order_toplvl: (u32, &str, u32, DateTime<FixedOffset>),
    expect_olines: Vec<(u32, ProductType, u64, [u32; 5], Duration)>,
) {
    assert!(!expect_olines.is_empty());
    let (expect_usr_id, expect_order_id, expect_num_charges, expect_ctime) = expect_order_toplvl;
    assert_eq!(actual.owner, expect_usr_id);
    assert_eq!(actual.id, expect_order_id);
    assert_eq!(actual.num_charges, expect_num_charges);
    assert_eq!(actual.create_time, expect_ctime.trunc_subsecs(0));
    let mut expect_line_map = {
        let mut hm = HashMap::new();
        expect_olines
            .into_iter()
            .map(|c| {
                let ctime = expect_ctime.trunc_subsecs(0);
                let (store_id, prod_typ, prod_id, rsv_detail, rsv_until) = c;
                let key = (store_id, prod_typ, prod_id);
                let value = (rsv_detail, ctime + rsv_until);
                let _empty = hm.insert(key, value);
            })
            .count();
        assert!(hm.len() > 0);
        hm
    };
    actual
        .lines
        .into_iter()
        .map(|line| {
            let OrderLineModel {
                pid,
                rsv_total,
                paid_total,
                reserved_until,
            } = line;
            let BaseProductIdentity {
                store_id,
                product_type,
                product_id,
            } = pid;
            let key = (store_id, product_type, product_id);
            let actual_val = (
                [
                    rsv_total.unit,
                    rsv_total.total,
                    rsv_total.qty,
                    paid_total.total,
                    paid_total.qty,
                ],
                reserved_until,
            );
            let expect_val = expect_line_map.remove(&key).unwrap();
            assert_eq!(actual_val, expect_val);
        })
        .count();
    assert!(expect_line_map.is_empty());
} // end of fn ut_verify_fetched_order

#[actix_web::test]
async fn create_order_replica_ok() {
    let mock_order_toplvl_data = (123, "9d73ba76d5", 0, Local::now().fixed_offset());
    let mock_olines_data = vec![
        (
            2603,
            ProductType::Item,
            180,
            [34, 340, 10, 0, 0],
            Duration::minutes(2),
        ),
        (
            2603,
            ProductType::Package,
            211,
            [29, 261, 9, 0, 0],
            Duration::minutes(5),
        ),
        (
            2379,
            ProductType::Item,
            449,
            [35, 420, 12, 0, 0],
            Duration::minutes(11),
        ),
    ];
    let shr_state = ut_setup_sharestate();
    let repo = ut_setup_db_repo(shr_state).await;
    let expect_ol_set = ut_setup_orderline_set(
        mock_order_toplvl_data.0,
        mock_order_toplvl_data.1,
        mock_order_toplvl_data.2,
        mock_order_toplvl_data.3.clone(),
        mock_olines_data.clone(),
    );
    let billing = ut_setup_order_bill();
    let result = repo.create_order(&expect_ol_set, &billing).await;
    assert!(result.is_ok());
    let result = repo
        .get_unpaid_olines(mock_order_toplvl_data.0, mock_order_toplvl_data.1)
        .await;
    if let Ok(Some(v)) = result {
        ut_verify_fetched_order(v, mock_order_toplvl_data, mock_olines_data);
    } else {
        assert!(false);
    }
} // end of fn create_order_replica_ok

#[actix_web::test]
async fn read_unpaid_orderline_ok() {
    // This test case assumes few charges were already made against a
    // valid order, this application does not use repository this way,
    // the test code here is simply for verification of the database
    // repository
    let mock_order_toplvl_data = (124, "c071ce550de1", 2, Local::now().fixed_offset());
    let mock_olines_data = vec![
        (
            8299,
            ProductType::Package,
            37,
            [31, 310, 10, 0, 0],
            Duration::minutes(15),
        ),
        (
            8299,
            ProductType::Item,
            219,
            [45, 180, 4, 45, 1],
            Duration::minutes(14),
        ),
        (
            3034,
            ProductType::Package,
            602,
            [90, 450, 5, 360, 4],
            Duration::minutes(13),
        ),
        (
            3034,
            ProductType::Item,
            595,
            [112, 336, 3, 336, 3],
            Duration::minutes(12),
        ),
        (
            8299,
            ProductType::Item,
            253,
            [48, 480, 10, 96, 2],
            Duration::minutes(10),
        ),
    ];
    let shr_state = ut_setup_sharestate();
    let repo = ut_setup_db_repo(shr_state).await;
    let expect_ol_set = ut_setup_orderline_set(
        mock_order_toplvl_data.0,
        mock_order_toplvl_data.1,
        mock_order_toplvl_data.2,
        mock_order_toplvl_data.3.clone(),
        mock_olines_data.clone(),
    );
    let billing = ut_setup_order_bill();
    let result = repo.create_order(&expect_ol_set, &billing).await;
    // if let Err(e) = &result {
    //     println!("[debug] DB error {:?}", e)
    // }
    assert!(result.is_ok());
    let result = repo
        .get_unpaid_olines(mock_order_toplvl_data.0, mock_order_toplvl_data.1)
        .await;
    if let Ok(Some(v)) = result {
        let mock_olines_data = mock_olines_data
            .into_iter()
            .filter(|c| c.3[2] > c.3[4])
            .collect(); // filter out the lines if all items are paid
        ut_verify_fetched_order(v, mock_order_toplvl_data, mock_olines_data);
    } else {
        assert!(false);
    }
} // end of fn get_unpaid_orderline_ok

#[actix_web::test]
async fn read_order_replica_nonexist() {
    let shr_state = ut_setup_sharestate();
    let repo = ut_setup_db_repo(shr_state).await;
    let result = repo.get_unpaid_olines(125, "beef01").await;
    assert!(result.is_ok());
    if let Ok(v) = result {
        assert!(v.is_none());
    }
} // end of fn read_order_replica_nonexist

fn _ut_setup_buyer_charge() -> ChargeBuyerModel {
    let mock_owner = 126;
    let mock_create_time = Local::now().fixed_offset().to_utc() - Duration::minutes(4);
    let mock_oid = "dee50de6".to_string();
    let mock_state = BuyerPayInState::ProcessorAccepted(mock_create_time + Duration::seconds(95));
    let mock_method = {
        let sess = StripeCheckoutSessionReqDto {
            customer_id: None,
            return_url: None,
            success_url: None,
            cancel_url: None,
            ui_mode: StripeCheckoutUImodeDto::EmbeddedJs,
        };
        PaymentMethodReqDto::Stripe(sess)
    };
    let mock_data_lines = vec![
        (3034, ProductType::Package, 602, 90, 360, 4),
        (8299, ProductType::Item, 351, 55, 110, 2),
    ];
    ut_setup_buyer_charge(
        mock_owner,
        mock_create_time,
        mock_oid,
        mock_state,
        mock_method,
        mock_data_lines,
    )
}

#[actix_web::test]
async fn buyer_create_stripe_charge_ok() {
    let shr_state = ut_setup_sharestate();
    let repo = ut_setup_db_repo(shr_state).await;
    let cline_set = _ut_setup_buyer_charge();
    let result = repo.create_charge(cline_set).await;
    assert!(result.is_ok());
}

#[actix_web::test]
async fn buyer_create_charge_invalid_state() {
    let shr_state = ut_setup_sharestate();
    let repo = ut_setup_db_repo(shr_state).await;
    let mut cline_set = _ut_setup_buyer_charge();
    cline_set.state = BuyerPayInState::Initialized;
    let result = repo.create_charge(cline_set).await;
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.code, AppErrorCode::InvalidInput);
        if let AppRepoErrorDetail::ChargeStatus(s) = e.detail {
            let cond = matches!(s, BuyerPayInState::Initialized);
            assert!(cond);
        } else {
            assert!(false);
        }
    }
}
