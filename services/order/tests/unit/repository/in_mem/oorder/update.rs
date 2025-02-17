use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::result::Result as DefaultResult;

use chrono::{DateTime, Duration as ChronoDuration};
use rust_decimal::Decimal;
use tokio::time::{sleep, Duration as TokioDuration};

use ecommerce_common::api::dto::CurrencyDto;
use ecommerce_common::api::rpc::dto::{
    OrderLinePaidUpdateDto, OrderLinePayUpdateErrorDto, OrderLinePayUpdateErrorReason,
    OrderPaymentUpdateDto,
};
use ecommerce_common::error::AppErrorCode;

use order::datastore::AppInMemoryDStore;
use order::error::AppError;
use order::model::{CurrencyModel, OrderCurrencyModel, OrderLineModel, OrderLineModelSet};
use order::repository::{AbsOrderRepo, OrderInMemRepo};

use super::create::{ut_setup_save_stock, ut_setup_stock_rsv_cb};
use super::{in_mem_repo_ds_setup, ut_setup_billing, ut_setup_orderlines, ut_setup_shipping};

fn ut_setup_order_currency(mock_seller_ids: [u32; 2]) -> OrderCurrencyModel {
    let buyer = CurrencyModel {
        name: CurrencyDto::TWD,
        rate: Decimal::new(32118, 3),
    };
    let seller_c = CurrencyModel {
        name: CurrencyDto::IDR,
        rate: Decimal::new(139040043, 4),
    };
    let iter = mock_seller_ids
        .into_iter()
        .map(|seller_id| (seller_id, seller_c.clone()));
    OrderCurrencyModel {
        buyer,
        sellers: HashMap::from_iter(iter),
    }
}

async fn ut_setup_saved_order(
    o_repo: &OrderInMemRepo,
    mock_oid: &str,
    mock_usr_id: u32,
    lines: Vec<OrderLineModel>,
    mock_seller_ids: [u32; 2],
) {
    let mut billings = ut_setup_billing();
    let mut shippings = ut_setup_shipping(&mock_seller_ids);
    assert!(lines.len() >= 3);
    assert!(!billings.is_empty());
    assert!(!shippings.is_empty());
    let ol_set = OrderLineModelSet {
        order_id: mock_oid.to_string(),
        lines,
        owner_id: mock_usr_id,
        currency: ut_setup_order_currency(mock_seller_ids),
        create_time: DateTime::parse_from_rfc3339("2022-11-07T04:00:00.519-01:00").unwrap(),
    };
    let stockrepo = o_repo.stock();
    let result = stockrepo.try_reserve(ut_setup_stock_rsv_cb, &ol_set).await;
    assert!(result.is_ok());
    let result = o_repo
        .save_contact(
            ol_set.order_id.as_str(),
            billings.remove(0),
            shippings.remove(0),
        )
        .await;
    assert!(result.is_ok());
} // end of fn ut_setup_saved_order

fn ut_setup_oline_new_payment(sellers_id: [u32; 2]) -> Vec<OrderLinePaidUpdateDto> {
    vec![
        OrderLinePaidUpdateDto {
            seller_id: sellers_id[1],
            product_id: 192,
            qty: 1,
        },
        OrderLinePaidUpdateDto {
            seller_id: sellers_id[0],
            product_id: 193,
            qty: 1,
        },
        OrderLinePaidUpdateDto {
            seller_id: sellers_id[0],
            product_id: 190,
            qty: 2,
        },
    ]
}

fn ut_usr_cb_ok_1(
    models: &mut Vec<OrderLineModel>,
    data: OrderPaymentUpdateDto,
) -> Vec<OrderLinePayUpdateErrorDto> {
    assert_eq!(models.len(), 3);
    assert_eq!(data.lines.len(), 3);
    let OrderPaymentUpdateDto {
        oid: _,
        lines,
        charge_time,
    } = data;
    let dt_charge_time = DateTime::parse_from_rfc3339(charge_time.as_str()).unwrap();
    lines
        .into_iter()
        .map(|d| {
            let result = models
                .iter_mut()
                .find(|m| m.id().store_id == d.seller_id && m.id().product_id == d.product_id);
            assert!(result.is_some());
            let saved = result.unwrap();
            assert_eq!(saved.qty.paid, 0);
            assert!(saved.qty.paid_last_update.is_none());
            saved.qty.paid = d.qty;
            saved.qty.paid_last_update = Some(dt_charge_time);
        })
        .count();
    vec![]
}
fn ut_usr_cb_ok_2(
    models: &mut Vec<OrderLineModel>,
    data: OrderPaymentUpdateDto,
) -> Vec<OrderLinePayUpdateErrorDto> {
    assert_eq!(models.len(), 3);
    let OrderPaymentUpdateDto {
        oid: _,
        lines,
        charge_time,
    } = data;
    let dt_charge_time = DateTime::parse_from_rfc3339(charge_time.as_str()).unwrap();
    lines
        .into_iter()
        .map(|d| {
            let result = models
                .iter()
                .find(|m| m.id().store_id == d.seller_id && m.id().product_id == d.product_id);
            assert!(result.is_some());
            let saved = result.unwrap();
            assert_eq!(saved.qty.paid, d.qty);
            assert!(saved.qty.paid_last_update.is_some());
            if let Some(t) = saved.qty.paid_last_update.as_ref() {
                assert_eq!(t, &dt_charge_time);
            }
        })
        .count();
    vec![]
}

#[tokio::test]
async fn in_mem_update_lines_payment_ok() {
    let mock_seller_ids = [19u32, 43];
    let oid = OrderLineModel::generate_order_id(7);
    let mock_repo_time = DateTime::parse_from_rfc3339("2023-12-24T14:30:41+02:00").unwrap();
    let mock_charge_time = "2023-12-24T15:57:41+02:00".to_string();
    let o_repo = in_mem_repo_ds_setup::<AppInMemoryDStore>(60, Some(mock_repo_time)).await;
    let lines = ut_setup_orderlines(&mock_seller_ids);
    ut_setup_save_stock(o_repo.stock(), mock_repo_time, &lines).await;
    ut_setup_saved_order(&o_repo, oid.as_str(), 124, lines, mock_seller_ids).await;
    let data = OrderPaymentUpdateDto {
        oid: oid.clone(),
        charge_time: mock_charge_time.clone(),
        lines: ut_setup_oline_new_payment(mock_seller_ids),
    };
    let result = o_repo.update_lines_payment(data, ut_usr_cb_ok_1).await;
    assert!(result.is_ok());
    if let Ok(v) = result {
        assert_eq!(v.oid, oid);
        assert_eq!(v.lines.len(), 0);
    }
    for _ in 0..2 {
        // examine saved order lines
        let data = OrderPaymentUpdateDto {
            oid: oid.clone(),
            charge_time: mock_charge_time.clone(),
            lines: ut_setup_oline_new_payment(mock_seller_ids),
        };
        let result = o_repo.update_lines_payment(data, ut_usr_cb_ok_2).await;
        assert!(result.is_ok());
    }
} // end of fn in_mem_update_lines_payment_ok

fn ut_usr_cb_err_1(
    models: &mut Vec<OrderLineModel>,
    data: OrderPaymentUpdateDto,
) -> Vec<OrderLinePayUpdateErrorDto> {
    assert_eq!(models.len(), 3);
    assert_eq!(data.lines.len(), 3);
    let dt_charge_time = DateTime::parse_from_rfc3339(data.charge_time.as_str()).unwrap();
    let mut err_reasons = vec![
        OrderLinePayUpdateErrorReason::InvalidQuantity,
        OrderLinePayUpdateErrorReason::InvalidQuantity,
        OrderLinePayUpdateErrorReason::Omitted,
    ];
    models
        .iter_mut()
        .map(|m| {
            assert_eq!(m.qty.paid, 0);
            assert!(m.qty.paid_last_update.is_none());
            let d = data.lines.get(0).unwrap();
            m.qty.paid += d.qty;
            m.qty.paid_last_update = Some(dt_charge_time);
        })
        .count();
    data.lines
        .into_iter()
        .map(|d| OrderLinePayUpdateErrorDto {
            seller_id: d.seller_id,
            product_id: d.product_id,
            reason: err_reasons.remove(0),
        })
        .collect()
}

fn ut_usr_cb_err_2(
    models: &mut Vec<OrderLineModel>,
    data: OrderPaymentUpdateDto,
) -> Vec<OrderLinePayUpdateErrorDto> {
    assert_eq!(models.len(), 3);
    data.lines
        .into_iter()
        .map(|d| {
            let result = models
                .iter()
                .find(|m| m.id().store_id == d.seller_id && m.id().product_id == d.product_id);
            assert!(result.is_some());
            let saved = result.unwrap();
            assert_eq!(saved.qty.paid, 0);
            assert!(saved.qty.paid_last_update.is_none());
        })
        .count();
    vec![]
}

#[tokio::test]
async fn in_mem_update_lines_payment_usr_cb_err() {
    let mock_seller_ids = [18u32, 41];
    let oid = OrderLineModel::generate_order_id(7);
    let mock_repo_time = DateTime::parse_from_rfc3339("2023-12-24T14:30:41+02:00").unwrap();
    let o_repo = in_mem_repo_ds_setup::<AppInMemoryDStore>(30, Some(mock_repo_time)).await;
    let lines = ut_setup_orderlines(&mock_seller_ids);
    ut_setup_save_stock(o_repo.stock(), mock_repo_time, &lines).await;
    ut_setup_saved_order(&o_repo, oid.as_str(), 124, lines, mock_seller_ids).await;
    let mut lines = ut_setup_oline_new_payment(mock_seller_ids);
    lines[0].qty = 9998;
    lines[1].qty = 9999;
    let data = OrderPaymentUpdateDto {
        oid: oid.clone(),
        charge_time: "1999-07-31T23:59:59+09:00".to_string(),
        lines,
    };
    let result = o_repo.update_lines_payment(data, ut_usr_cb_err_1).await;
    assert!(result.is_ok());
    if let Ok(v) = result {
        assert_eq!(v.oid, oid);
        assert_eq!(v.lines.len(), 3);
    } // examine the order lines, payment status should not be modified
    let data = OrderPaymentUpdateDto {
        oid,
        charge_time: "1999-07-31T23:59:59+09:00".to_string(),
        lines: ut_setup_oline_new_payment(mock_seller_ids),
    };
    let result = o_repo.update_lines_payment(data, ut_usr_cb_err_2).await;
    assert!(result.is_ok());
}

fn ut_rd_oline_set_usr_cb<'a>(
    _repo: &'a dyn AbsOrderRepo,
    ol_set: OrderLineModelSet,
) -> Pin<Box<dyn Future<Output = DefaultResult<(), AppError>> + Send + 'a>> {
    let fut = async move {
        let (owner_id, product_ids) = match ol_set.order_id.as_str() {
            "OrderIDone" => {
                assert_eq!(ol_set.lines.len(), 3);
                (
                    126u32,
                    vec![(576u32, 190u64), (576u32, 192u64), (117u32, 193u64)],
                )
            }
            "OrderIDtwo" => {
                assert_eq!(ol_set.lines.len(), 1);
                (127u32, vec![(117, 1190)])
            }
            "OrderIDthree" => {
                assert_eq!(ol_set.lines.len(), 2);
                (128u32, vec![(576, 1190), (576, 194)])
            }
            "OrderIDfive" => {
                return Err(AppError {
                    code: AppErrorCode::Unknown,
                    detail: Some(format!("unit-test")),
                });
            }
            _others => {
                assert!(false);
                (0u32, vec![])
            }
        };
        assert_eq!(ol_set.owner_id, owner_id);
        let mut product_id_set: HashSet<(u32, u64)> = HashSet::from_iter(product_ids.into_iter());
        let all_items_found = ol_set.lines.iter().all(|m| {
            let key = (m.id().store_id, m.id().product_id);
            product_id_set.remove(&key)
        });
        assert!(all_items_found);
        Ok(())
    };
    Box::pin(fut)
} // end of ut_rd_oline_set_usr_cb

#[tokio::test]
async fn in_mem_fetch_lines_rsvtime_ok() {
    let mock_seller_ids = [117u32, 576];
    let mock_repo_time = DateTime::parse_from_rfc3339("2023-12-24T14:30:41+02:00").unwrap();
    let o_repo = in_mem_repo_ds_setup::<AppInMemoryDStore>(60, Some(mock_repo_time)).await;
    let mut lines = (
        ut_setup_orderlines(&mock_seller_ids),
        ut_setup_orderlines(&mock_seller_ids),
        ut_setup_orderlines(&mock_seller_ids),
        ut_setup_orderlines(&mock_seller_ids),
    );
    {
        // ensure there are enough stock for all the order requests
        let mut _lines = ut_setup_orderlines(&mock_seller_ids);
        _lines
            .iter_mut()
            .map(|o| {
                o.qty.reserved *= 4;
            })
            .count();
        ut_setup_save_stock(o_repo.stock(), mock_repo_time, &_lines).await;
    }
    let start_time = DateTime::parse_from_rfc3339("2023-01-15T09:23:50+05:00").unwrap();
    let end_time = DateTime::parse_from_rfc3339("2023-01-16T09:23:50+05:00").unwrap();
    {
        lines.0[1].policy.reserved_until = start_time + ChronoDuration::minutes(1);
        lines.1[2].policy.reserved_until = start_time + ChronoDuration::minutes(5);
        lines.2[3].policy.reserved_until = start_time + ChronoDuration::minutes(7);
        lines.0[4].policy.reserved_until = start_time + ChronoDuration::minutes(11);
        lines.0[5].policy.reserved_until = start_time + ChronoDuration::minutes(13);
        lines.2[6].policy.reserved_until = start_time + ChronoDuration::minutes(17);
    }
    ut_setup_saved_order(&o_repo, "OrderIDone", 126, lines.0, mock_seller_ids).await;
    ut_setup_saved_order(&o_repo, "OrderIDtwo", 127, lines.1, mock_seller_ids).await;
    ut_setup_saved_order(&o_repo, "OrderIDthree", 128, lines.2, mock_seller_ids).await;
    ut_setup_saved_order(&o_repo, "OrderIDfour", 129, lines.3, mock_seller_ids).await;
    let result = o_repo
        .fetch_lines_by_rsvtime(start_time, end_time, ut_rd_oline_set_usr_cb)
        .await;
    assert!(result.is_ok());
} // end of fn in_mem_fetch_lines_rsvtime_ok

#[tokio::test]
async fn in_mem_fetch_lines_rsvtime_usrcb_err() {
    let mock_seller_ids = [117u32, 576];
    let mock_repo_time = DateTime::parse_from_rfc3339("2023-12-24T14:30:41+02:00").unwrap();
    let o_repo = in_mem_repo_ds_setup::<AppInMemoryDStore>(30, Some(mock_repo_time)).await;
    let mut lines = (
        ut_setup_orderlines(&mock_seller_ids),
        ut_setup_orderlines(&mock_seller_ids),
    );
    {
        // ensure there are enough stock for all the order requests
        let mut _lines = ut_setup_orderlines(&mock_seller_ids);
        _lines
            .iter_mut()
            .map(|o| {
                o.qty.reserved *= 2;
            })
            .count();
        ut_setup_save_stock(o_repo.stock(), mock_repo_time, &_lines).await;
    }
    let start_time = DateTime::parse_from_rfc3339("2023-01-15T09:23:50+05:00").unwrap();
    let end_time = DateTime::parse_from_rfc3339("2023-01-16T09:23:50+05:00").unwrap();
    {
        lines.0[5].policy.reserved_until = start_time + ChronoDuration::minutes(45);
        lines.1[2].policy.reserved_until = start_time + ChronoDuration::minutes(18);
    }
    ut_setup_saved_order(&o_repo, "OrderIDfive", 130, lines.0, mock_seller_ids).await;
    ut_setup_saved_order(&o_repo, "OrderIDtwo", 127, lines.1, mock_seller_ids).await;
    let result = o_repo
        .fetch_lines_by_rsvtime(start_time, end_time, ut_rd_oline_set_usr_cb)
        .await;
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.code, AppErrorCode::Unknown);
        assert_eq!(e.detail.as_ref().unwrap().as_str(), "unit-test");
    }
}

#[tokio::test]
async fn in_mem_scheduled_job_time_ok() {
    let o_repo = in_mem_repo_ds_setup::<AppInMemoryDStore>(30, None).await;
    let time0 = o_repo.cancel_unpaid_last_time().await.unwrap();
    let time1 = o_repo.cancel_unpaid_last_time().await.unwrap();
    let _ = sleep(TokioDuration::from_secs(1));
    o_repo.cancel_unpaid_time_update().await.unwrap();
    let time2 = o_repo.cancel_unpaid_last_time().await.unwrap();
    let _ = sleep(TokioDuration::from_secs(1));
    o_repo.cancel_unpaid_time_update().await.unwrap();
    let time3 = o_repo.cancel_unpaid_last_time().await.unwrap();
    let time4 = o_repo.cancel_unpaid_last_time().await.unwrap();
    assert_eq!(time0, time1);
    assert!(time2 > time1);
    assert!(time3 > time2);
    assert_eq!(time3, time4);
}
