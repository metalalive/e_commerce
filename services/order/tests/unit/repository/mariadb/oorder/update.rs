use chrono::{DateTime, Duration, Local, SubsecRound};
use tokio::time::{sleep, Duration as TokioDuration};

use ecommerce_common::api::rpc::dto::{
    OrderLinePaidUpdateDto, OrderLinePayUpdateErrorDto, OrderPaymentUpdateDto,
};
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

fn ut_update_payment_repo_cb(
    saved_lines: &mut Vec<OrderLineModel>,
    data: OrderPaymentUpdateDto,
) -> Vec<OrderLinePayUpdateErrorDto> {
    let OrderPaymentUpdateDto {
        oid: _,
        lines,
        charge_time,
    } = data;
    let ctime = DateTime::parse_from_rfc3339(charge_time.as_str()).unwrap();
    OrderLineModel::update_payments(saved_lines, lines, ctime)
}

#[cfg(feature = "mariadb")]
#[tokio::test]
async fn update_payment_ok() {
    let ds = dstore_ctx_setup();
    let o_repo = app_repo_order(ds).await.unwrap();
    let mock_oid = "0e927003716b";
    let create_time = Local::now().fixed_offset();
    ut_setup_stock_product(o_repo.stock(), 1031, 9003, 25).await;
    ut_setup_stock_product(o_repo.stock(), 1032, 9010, 27).await;
    ut_setup_stock_product(o_repo.stock(), 1032, 9011, 44).await;
    ut_setup_stock_product(o_repo.stock(), 1033, 9022, 15).await;
    {
        let lines = vec![
            (1032, 9010, 13, 99, create_time),
            (1031, 9003, 10, 100, create_time),
            (1032, 9011, 15, 110, create_time),
        ];
        let ol_set = ut_oline_init_setup(mock_oid, 123, create_time, lines);
        let result = o_repo
            .stock()
            .try_reserve(mock_reserve_usr_cb_0, &ol_set)
            .await;
        assert!(result.is_ok());
    }
    let data = {
        let lines = vec![(1031u32, 9003u64, 3u32), (1032, 9010, 1), (1032, 9011, 6)]
            .into_iter()
            .map(|d| OrderLinePaidUpdateDto {
                seller_id: d.0,
                product_id: d.1,
                qty: d.2,
            })
            .collect::<Vec<_>>();
        OrderPaymentUpdateDto {
            oid: mock_oid.to_string(),
            charge_time: (create_time + Duration::seconds(6)).to_rfc3339(),
            lines,
        }
    };
    let result = o_repo
        .update_lines_payment(data, ut_update_payment_repo_cb)
        .await;
    assert!(result.is_ok());
    if let Ok(usr_err) = result {
        assert!(usr_err.lines.is_empty());
    }
    let data = {
        let lines = vec![(1032u32, 9010u64, 3u32), (1032, 9011, 5)]
            .into_iter()
            .map(|d| OrderLinePaidUpdateDto {
                seller_id: d.0,
                product_id: d.1,
                qty: d.2,
            })
            .collect::<Vec<_>>();
        OrderPaymentUpdateDto {
            oid: mock_oid.to_string(),
            charge_time: (create_time + Duration::seconds(10)).to_rfc3339(),
            lines,
        }
    };
    let result = o_repo
        .update_lines_payment(data, ut_update_payment_repo_cb)
        .await;
    assert!(result.is_ok());
    if let Ok(usr_err) = result {
        assert!(usr_err.lines.is_empty());
    }

    let pids = vec![(1031u32, 9003u64), (1032, 9011), (1032, 9010)]
        .into_iter()
        .map(|d| OrderLineIdentity {
            store_id: d.0,
            product_id: d.1,
        })
        .collect::<Vec<_>>();
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
        fn1(lines.remove(0), 9003, 3, Duration::seconds(6));
        fn1(lines.remove(0), 9010, 4, Duration::seconds(10));
        fn1(lines.remove(0), 9011, 11, Duration::seconds(10));
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
