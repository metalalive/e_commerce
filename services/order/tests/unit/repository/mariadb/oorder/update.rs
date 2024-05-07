use chrono::{Duration, Local, SubsecRound};
use tokio::time::{sleep, Duration as TokioDuration};

use ecommerce_common::constant::ProductType;
use order::api::rpc::dto::{OrderLinePaidUpdateDto, OrderPaymentUpdateDto};
use order::model::{OrderLineIdentity, OrderLineModel, OrderLineModelSet, StockLevelModelSet};
use order::repository::{app_repo_order, AppStockRepoReserveReturn};

use super::super::dstore_ctx_setup;
use super::{ut_oline_init_setup, ut_setup_stock_product};

fn mock_reserve_usr_cb_0(
    ms: &mut StockLevelModelSet,
    req: &OrderLineModelSet,
) -> AppStockRepoReserveReturn {
    let errors = ms.try_reserve(req);
    assert!(errors.is_empty());
    Ok(())
}

#[cfg(feature = "mariadb")]
#[tokio::test]
async fn update_payment_ok() {
    let ds = dstore_ctx_setup();
    let o_repo = app_repo_order(ds).await.unwrap();
    let mock_oid = "0e927003716b";
    let create_time = Local::now().fixed_offset();
    ut_setup_stock_product(o_repo.stock(), 1031, ProductType::Item, 9003, 25).await;
    ut_setup_stock_product(o_repo.stock(), 1032, ProductType::Package, 9010, 27).await;
    ut_setup_stock_product(o_repo.stock(), 1032, ProductType::Item, 9011, 44).await;
    ut_setup_stock_product(o_repo.stock(), 1033, ProductType::Item, 9012, 32).await;
    {
        let lines = vec![
            (
                1032,
                ProductType::Package,
                9010,
                13,
                99,
                create_time.clone(),
            ),
            (1031, ProductType::Item, 9003, 10, 100, create_time.clone()),
            (1032, ProductType::Item, 9011, 10, 110, create_time.clone()),
        ];
        let ol_set = ut_oline_init_setup(mock_oid, 123, create_time, lines);
        let result = o_repo
            .stock()
            .try_reserve(mock_reserve_usr_cb_0, &ol_set)
            .await;
        assert!(result.is_ok());
    }
    let data = {
        let lines = vec![
            OrderLinePaidUpdateDto {
                seller_id: 1031,
                product_id: 9003,
                qty: 4,
                product_type: ProductType::Item,
                time: create_time + Duration::seconds(5),
            },
            OrderLinePaidUpdateDto {
                seller_id: 1032,
                product_id: 9010,
                qty: 1,
                product_type: ProductType::Package,
                time: create_time + Duration::seconds(6),
            },
        ];
        OrderPaymentUpdateDto {
            oid: mock_oid.to_string(),
            lines,
        }
    };
    let result = o_repo
        .update_lines_payment(data, OrderLineModel::update_payments)
        .await;
    assert!(result.is_ok());
    if let Ok(usr_err) = result {
        assert!(usr_err.lines.is_empty());
    }
    let data = {
        let lines = vec![
            OrderLinePaidUpdateDto {
                seller_id: 1032,
                product_id: 9010,
                qty: 3,
                product_type: ProductType::Package,
                time: create_time + Duration::seconds(7),
            },
            OrderLinePaidUpdateDto {
                seller_id: 1032,
                product_id: 9011,
                qty: 5,
                product_type: ProductType::Item,
                time: create_time + Duration::seconds(10),
            },
        ];
        OrderPaymentUpdateDto {
            oid: mock_oid.to_string(),
            lines,
        }
    };
    let result = o_repo
        .update_lines_payment(data, OrderLineModel::update_payments)
        .await;
    assert!(result.is_ok());
    if let Ok(usr_err) = result {
        assert!(usr_err.lines.is_empty());
    }

    let pids = vec![
        OrderLineIdentity {
            store_id: 1031,
            product_id: 9003,
            product_type: ProductType::Item,
        },
        OrderLineIdentity {
            store_id: 1032,
            product_id: 9011,
            product_type: ProductType::Item,
        },
        OrderLineIdentity {
            store_id: 1032,
            product_id: 9010,
            product_type: ProductType::Package,
        },
    ];
    let result = o_repo.fetch_lines_by_pid(mock_oid, pids).await;
    assert!(result.is_ok());
    if let Ok(mut lines) = result {
        assert_eq!(lines.len(), 3);
        lines.sort_by(|a, b| a.id_.product_id.cmp(&b.id_.product_id));
        let fn1 =
            |line: OrderLineModel, exp_product_id: u64, exp_paid: u32, exp_duration: Duration| {
                assert_eq!(line.id_.product_id, exp_product_id);
                assert_eq!(line.qty.paid, exp_paid);
                let expect = create_time.round_subsecs(1) + exp_duration;
                let actual = line.qty.paid_last_update.unwrap().round_subsecs(1);
                assert_eq!(actual, expect);
            };
        fn1(lines.remove(0), 9003, 4, Duration::seconds(5));
        fn1(lines.remove(0), 9010, 3, Duration::seconds(7));
        fn1(lines.remove(0), 9011, 5, Duration::seconds(10));
    }
} // end of fn update_payment_ok

#[cfg(feature = "mariadb")]
#[tokio::test]
async fn cancel_unpaid_job_time_ok() {
    let ds = dstore_ctx_setup();
    let o_repo = app_repo_order(ds).await.unwrap();
    let time0 = o_repo.cancel_unpaid_last_time().await.unwrap();
    let time1 = o_repo.cancel_unpaid_last_time().await.unwrap();
    let _ = sleep(TokioDuration::from_secs(1)).await;
    o_repo.cancel_unpaid_time_update().await.unwrap();
    let time2 = o_repo.cancel_unpaid_last_time().await.unwrap();
    let _ = sleep(TokioDuration::from_secs(1)).await;
    o_repo.cancel_unpaid_time_update().await.unwrap();
    let time3 = o_repo.cancel_unpaid_last_time().await.unwrap();
    let time4 = o_repo.cancel_unpaid_last_time().await.unwrap();
    // println!("[debug] time {:?} {:?} ", time2, time3);
    assert_eq!(time0, time1);
    assert!(time2 > time1);
    assert!(time3 > time2);
    assert_eq!(time3, time4);
}
