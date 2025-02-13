use std::collections::hash_map::RandomState;
use std::collections::{HashMap, HashSet};
use std::result::Result as DefaultResult;
use std::sync::Arc;

use chrono::{DateTime, Duration, FixedOffset, Local};
use rust_decimal::Decimal;

use ecommerce_common::api::dto::CurrencyDto;
use ecommerce_common::api::rpc::dto::OrderReplicaRefundReqDto;
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::logging::AppLogContext;

use order::api::rpc::dto::{
    OrderReplicaInventoryDto, OrderReplicaInventoryReqDto, StockReturnErrorDto,
};
use order::api::web::dto::OrderLineReqDto;
use order::constant::app_meta;
use order::error::AppError;
use order::model::{
    CurrencyModel, CurrencyModelSet, OrderCurrencyModel, OrderLineAppliedPolicyModel,
    OrderLineIdentity, OrderLineModel, OrderLineModelSet, OrderLinePriceModel,
    OrderLineQuantityModel, OrderReturnModel, ProductPolicyModel, ProductPolicyModelSet,
    ProductPriceModel, ProductPriceModelSet,
};
use order::repository::{AbsOrderRepo, AbsOrderReturnRepo};
use order::usecase::{
    CreateOrderUsKsErr, CreateOrderUseCase, OrderDiscardUnpaidItemsUseCase,
    OrderReplicaInventoryUseCase, OrderReplicaRefundUseCase, ReturnLinesReqUcOutput,
    ReturnLinesReqUseCase,
};
use order::{AppAuthClaimPermission, AppAuthPermissionCode, AppAuthedClaim};

use super::{MockCurrencyRepo, MockOrderRepo, MockOrderReturnRepo};
use crate::{ut_setup_share_state, MockConfidential};

fn ut_setup_prod_policies() -> ProductPolicyModelSet {
    let policies = [
        #[cfg_attr(rustfmt, rustfmt_skip)]
        (1168u64, 0u16, 127u32, 1008u32, false, 0u16),
        (168, 0, 20000, 1250, false, 0),
        (174, 0, 30000, 2255, false, 0),
        (169, 1, 21000, 150, false, 5),
    ]
    .into_iter()
    .map(|d| ProductPolicyModel {
        product_id: d.0,
        min_num_rsv: d.1,
        warranty_hours: d.2,
        auto_cancel_secs: d.3,
        is_create: d.4,
        max_num_rsv: d.5,
    })
    .collect::<Vec<_>>();
    ProductPolicyModelSet { policies }
}

#[cfg_attr(rustfmt, rustfmt_skip)]
fn ut_setup_prod_prices() -> Vec<ProductPriceModelSet> {
    let raw2obj = |d: (u64, &str, &str, u32)| -> ProductPriceModel {
        let start_after = DateTime::parse_from_rfc3339(d.1).unwrap();
        let end_before =  DateTime::parse_from_rfc3339(d.2).unwrap();
        let args = (d.0, d.3, [start_after , end_before]);
        ProductPriceModel::from(args)
    };
    vec![
        ProductPriceModelSet {
            store_id: 51,
            currency: CurrencyDto::THB,
            items: [
                (168u64, "2023-07-31T10:16:54+05:00", "2023-10-10T09:01:31+02:00", 510u32),
                (1168, "2023-07-31T10:16:54+05:00", "2023-10-10T09:01:31+02:00", 1130),
                (169, "2022-12-02T14:29:54+05:00", "2023-01-15T19:01:31+02:00", 190),
            ]
            .into_iter().map(raw2obj).collect::<Vec<_>>(),
        },
        ProductPriceModelSet {
            store_id: 52,
            currency: CurrencyDto::TWD,
            items: [
                (168u64,"2023-07-31T11:29:04+02:00", "2023-08-30T09:01:31-08:00", 480u32),
                (900, "2023-05-01T21:49:04+02:00", "2023-07-31T09:01:55-10:00", 490),
                (901,"2023-05-01T21:49:04+02:00", "2023-07-31T09:01:55-10:00", 399),
            ]
            .into_iter().map(raw2obj).collect::<Vec<_>>(),
        },
    ]
} // end of fn ut_setup_prod_prices

#[cfg_attr(rustfmt, rustfmt_skip)]
fn ut_setup_currency_mset(data : Vec<(CurrencyDto, i64, u32)>) -> CurrencyModelSet
{ // note base currency in this project is always USD
    let base = CurrencyDto::USD;
    let exchange_rates = data.into_iter()
        .map(|d| CurrencyModel {
            name: d.0,
            rate: Decimal::new(d.1, d.2)
        })
        .collect::<Vec<_>>();
    CurrencyModelSet { base, exchange_rates }
}

fn ut_setup_order_currency(seller_ids: Vec<u32>) -> OrderCurrencyModel {
    let buyer = CurrencyModel {
        name: CurrencyDto::TWD,
        rate: Decimal::new(32041, 3),
    };
    let seller_c = buyer.clone();
    // in this test module, I assume buyer and all the sellers use the same currency
    let kv_pairs = seller_ids
        .into_iter()
        .map(|seller_id| (seller_id, seller_c.clone()));
    OrderCurrencyModel {
        buyer,
        sellers: HashMap::from_iter(kv_pairs),
    }
}

#[test]
fn validate_orderline_ok() {
    let ms_policy = ut_setup_prod_policies();
    let ms_price = ut_setup_prod_prices();
    let data = [(52u32, 168u64, 6u32), (51, 1168, 1), (51, 168, 10)]
        .into_iter()
        .map(|d| OrderLineReqDto {
            seller_id: d.0,
            product_id: d.1,
            quantity: d.2,
        })
        .collect::<Vec<_>>();
    let result = CreateOrderUseCase::validate_orderline(ms_policy, ms_price, data);
    assert!(result.is_ok());
    if let Ok(v) = result {
        assert_eq!(v.len(), 3);
        v.into_iter()
            .map(|m| {
                let id = m.id_;
                let search_key = (id.store_id, id.product_id);
                let found = match search_key {
                    (52, 168) => true,
                    (51, 168) => true,
                    (51, 1168) => true,
                    _others => false,
                };
                assert!(found);
            })
            .count();
    }
} // end of fn validate_orderline_ok

#[test]
fn validate_orderline_client_errors() {
    let ms_policy = ut_setup_prod_policies();
    let ms_price = ut_setup_prod_prices();
    let data = [
        (51u32, 169u64, 6u32),
        (52, 174, 4),
        (52, 900, 2),
        (51, 1168, 11),
        (52, 901, 9),
    ]
    .into_iter()
    .map(|d| OrderLineReqDto {
        seller_id: d.0,
        product_id: d.1,
        quantity: d.2,
    })
    .collect::<Vec<_>>();
    let result = CreateOrderUseCase::validate_orderline(ms_policy, ms_price, data);
    assert!(result.is_err());
    if let Err(CreateOrderUsKsErr::ReqContent(v)) = result {
        let errs = v.order_lines.unwrap();
        assert_eq!(errs.len(), 4);
        let found = errs
            .iter()
            .find(|e| e.seller_id == 52 && e.product_id == 900)
            .unwrap();
        if let Some(v) = found.nonexist.as_ref() {
            assert!(v.product_policy);
            assert!(!v.product_price);
        }
        let found = errs
            .iter()
            .find(|e| e.seller_id == 52 && e.product_id == 901)
            .unwrap();
        if let Some(v) = found.nonexist.as_ref() {
            assert!(v.product_policy);
            assert!(!v.product_price);
        }
        let found = errs
            .iter()
            .find(|e| e.seller_id == 52 && e.product_id == 174)
            .unwrap();
        if let Some(v) = found.nonexist.as_ref() {
            assert!(!v.product_policy);
            assert!(v.product_price);
        }
        let found = errs
            .iter()
            .find(|e| e.seller_id == 51 && e.product_id == 169)
            .unwrap();
        if let Some(v) = found.rsv_limit.as_ref() {
            assert_eq!(v.max_, 5);
            assert_eq!(v.given, 6);
        }
    }
} // end of validate_orderline_client_errors

#[tokio::test]
async fn create_order_snapshot_currency_ok() {
    let mock_repo = {
        let mock_data = vec![
            (CurrencyDto::TWD, 31998i64, 3u32),
            (CurrencyDto::INR, 81780780, 6),
            (CurrencyDto::THB, 3509, 2),
        ];
        let mock_fetched_mset = ut_setup_currency_mset(mock_data);
        let r = MockCurrencyRepo::build(Some(mock_fetched_mset));
        Box::new(r)
    };
    let mock_label_buyer = CurrencyDto::INR;
    let mock_seller_ms_price = ut_setup_prod_prices();
    let result = CreateOrderUseCase::snapshot_currencies(
        mock_repo.as_ref(),
        mock_label_buyer,
        &mock_seller_ms_price,
    )
    .await;
    assert!(result.is_ok());
    if let Ok(v) = result {
        assert_eq!(v.buyer.name, CurrencyDto::INR);
        assert_eq!(v.buyer.rate.to_string().as_str(), "81.780780");
        v.sellers
            .into_iter()
            .map(|(seller_id, cm)| {
                let expect = match seller_id {
                    51 => (CurrencyDto::THB, "35.09"),
                    52 => (CurrencyDto::TWD, "31.998"),
                    _others => (CurrencyDto::Unknown, ""),
                };
                assert_eq!(cm.name, expect.0);
                assert_eq!(cm.rate.to_string().as_str(), expect.1);
            })
            .count();
    }
} // end of fn create_order_snapshot_currency_ok

#[tokio::test]
async fn create_order_snapshot_currency_err() {
    let mock_repo = {
        let mock_data = vec![
            (CurrencyDto::USD, 1000i64, 3u32),
            (CurrencyDto::INR, 8178078, 5),
        ];
        let mock_fetched_mset = ut_setup_currency_mset(mock_data);
        let r = MockCurrencyRepo::build(Some(mock_fetched_mset));
        Box::new(r)
    };
    let mock_label_buyer = CurrencyDto::INR;
    let mock_seller_ms_price = ut_setup_prod_prices();
    let result = CreateOrderUseCase::snapshot_currencies(
        mock_repo.as_ref(),
        mock_label_buyer,
        &mock_seller_ms_price,
    )
    .await;
    assert!(result.is_err());
    if let Err(es) = result {
        assert_eq!(es.len(), 2);
        es.into_iter()
            .map(|e| {
                assert_eq!(e.code, AppErrorCode::InvalidInput);
                let detail_check = e.detail.as_ref().map(|s| s.contains("fail-load-ex-rate"));
                assert_eq!(detail_check, Some(true));
            })
            .count();
    } else {
        assert!(false);
    }
} // end of fn create_order_snapshot_currency_err

#[rustfmt::skip]
fn ut_setup_orderlines() -> Vec<OrderLineModel> {
    let base_time = Local::now().fixed_offset();
    let paid_last_update = Some(base_time + Duration::minutes(4));
    let reserved_until = base_time + Duration::minutes(5);
    let warranty_until = base_time + Duration::days(14);
    [
        (108u32, 190u64, 10u32, 139u32, 14u32, 13u32),
        (800, 191, 12, 180, 15, 15),
        (426, 192, 12, 216, 18, 15),
    ]
    .into_iter()
    .map(|d| OrderLineModel {
        id_: OrderLineIdentity {store_id: d.0, product_id: d.1},
        price: OrderLinePriceModel {unit: d.2, total: d.3},
        qty: OrderLineQuantityModel {reserved: d.4, paid: d.5, paid_last_update},
        policy: OrderLineAppliedPolicyModel {reserved_until, warranty_until},
    })
    .collect::<Vec<_>>()
}

#[rustfmt::skip]
fn ut_setup_olines_returns() -> Vec<OrderReturnModel> {
    let return_time = DateTime::parse_from_rfc3339("2023-11-18T02:39:04+02:00").unwrap();
    vec![
        OrderReturnModel {
            id_: OrderLineIdentity { store_id: 108, product_id: 190 },
            qty: HashMap::from([
                (
                    return_time + Duration::seconds(11),
                    (1, OrderLinePriceModel {unit: 10, total: 10}),
                ),
                (
                    return_time + Duration::seconds(30),
                    (5, OrderLinePriceModel {unit: 13, total: 65}),
                ),
            ]),
        },
        OrderReturnModel {
            id_: OrderLineIdentity { store_id: 800, product_id: 191 },
            qty: HashMap::from([
                (
                    return_time + Duration::seconds(6),
                    (1, OrderLinePriceModel {unit: 12, total: 12}),
                ),
                (
                    return_time + Duration::seconds(28),
                    (1, OrderLinePriceModel {unit: 12, total: 12}),
                ),
                (
                    return_time + Duration::seconds(65),
                    (2, OrderLinePriceModel {unit: 12, total: 24}),
                ),
                (
                    return_time + Duration::seconds(99),
                    (1, OrderLinePriceModel {unit: 12, total: 12}),
                ),
            ]),
        },
        OrderReturnModel {
            id_: OrderLineIdentity { store_id: 426, product_id: 192 },
            qty: HashMap::from([
                (
                    return_time + Duration::seconds(12),
                    (2, OrderLinePriceModel {unit: 11, total: 22}),
                ),
                (
                    return_time + Duration::seconds(73),
                    (3, OrderLinePriceModel {unit: 11, total: 33}),
                ),
                (
                    return_time + Duration::seconds(94),
                    (1, OrderLinePriceModel {unit: 11, total: 11}),
                ),
            ]),
        },
        OrderReturnModel {
            id_: OrderLineIdentity { store_id: 426, product_id: 8964 },
            qty: HashMap::from([
                (
                    return_time + Duration::seconds(10),
                    (3, OrderLinePriceModel {unit: 15, total: 45}),
                ),
                (
                    return_time + Duration::seconds(19),
                    (4, OrderLinePriceModel {unit: 15, total: 60}),
                ),
            ]),
        },
    ]
}

async fn discard_unpaid_items_common(
    stock_return_results: Vec<DefaultResult<Vec<StockReturnErrorDto>, AppError>>,
    fetched_ol_sets: Vec<OrderLineModelSet>,
) -> DefaultResult<(), AppError> {
    let shr_state = ut_setup_share_state("config_ok_no_sqldb.json", Box::new(MockConfidential {}));
    let logctx = shr_state.log_context().clone();
    let not_impl_err = AppError {
        detail: None,
        code: AppErrorCode::NotImplemented,
    };
    let repo = MockOrderRepo::build(
        Err(not_impl_err.clone()),
        Err(not_impl_err.clone()),
        stock_return_results,
        fetched_ol_sets,
        vec![],
        vec![],
        None,
        None,
        None, // note this use case does not require to examine exchange rate
    );
    let repo: Box<dyn AbsOrderRepo> = Box::new(repo);
    let uc = OrderDiscardUnpaidItemsUseCase::new(repo, logctx);
    uc.execute().await
}

#[tokio::test]
async fn discard_unpaid_items_ok() {
    let mut mocked_olines = ut_setup_orderlines();
    let stock_return_results = vec![Ok(vec![]), Ok(vec![])];
    let create_time = DateTime::parse_from_rfc3339("2022-11-07T04:00:00.519-01:00").unwrap();
    let mocked_seller_ids = mocked_olines
        .iter()
        .map(|v| v.id_.store_id)
        .collect::<Vec<_>>();
    let fetched_ol_sets = vec![
        OrderLineModelSet {
            order_id: "xx1".to_string(),
            owner_id: 123,
            create_time: create_time.clone(),
            lines: mocked_olines.drain(0..2).collect(),
            currency: ut_setup_order_currency(mocked_seller_ids.clone()),
        },
        OrderLineModelSet {
            order_id: "xx2".to_string(),
            owner_id: 124,
            create_time,
            lines: mocked_olines,
            currency: ut_setup_order_currency(mocked_seller_ids),
        },
    ];
    let result = discard_unpaid_items_common(stock_return_results, fetched_ol_sets).await;
    assert!(result.is_ok());
} // end of fn discard_unpaid_items_ok

#[tokio::test]
async fn discard_unpaid_items_err_stocklvl() {
    let mut mocked_olines = ut_setup_orderlines();
    let data_corrupt = AppError {
        detail: Some(format!("unit-test")),
        code: AppErrorCode::DataCorruption,
    };
    let stock_return_results = vec![Ok(vec![]), Err(data_corrupt)];
    let create_time = DateTime::parse_from_rfc3339("2022-11-07T04:00:00.519-01:00").unwrap();
    let mocked_seller_ids = mocked_olines
        .iter()
        .map(|v| v.id_.store_id)
        .collect::<Vec<_>>();
    let fetched_ol_sets = vec![
        OrderLineModelSet {
            order_id: "xx1".to_string(),
            owner_id: 500,
            create_time: create_time.clone(),
            lines: mocked_olines.drain(0..1).collect(),
            currency: ut_setup_order_currency(mocked_seller_ids.clone()),
        },
        OrderLineModelSet {
            order_id: "xx2".to_string(),
            owner_id: 510,
            create_time,
            lines: mocked_olines,
            currency: ut_setup_order_currency(mocked_seller_ids),
        },
    ];
    let result = discard_unpaid_items_common(stock_return_results, fetched_ol_sets).await;
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.code, AppErrorCode::DataCorruption);
        assert_eq!(e.detail.as_ref().unwrap(), "unit-test");
    }
} // end of fn discard_unpaid_items_err_stocklvl

fn ut_oreturn_setup_repository_1(
    fetched_olines: Vec<OrderLineModel>,
    fetched_oids_ctime: Vec<String>,
    owner_usr_id: u32,
    order_ctime: Option<DateTime<FixedOffset>>,
    currency_rate: Option<OrderCurrencyModel>,
) -> Box<dyn AbsOrderRepo> {
    let not_impl_err = AppError {
        detail: None,
        code: AppErrorCode::NotImplemented,
    };
    let repo = MockOrderRepo::build(
        Err(not_impl_err.clone()),
        Err(not_impl_err.clone()),
        vec![],
        vec![],
        fetched_olines,
        fetched_oids_ctime,
        Some(owner_usr_id),
        order_ctime,
        currency_rate,
    );
    Box::new(repo)
}

fn ut_oreturn_setup_repository_2(
    fetched_returns: DefaultResult<Vec<OrderReturnModel>, AppError>,
    fetched_oid_returns: DefaultResult<Vec<(String, OrderReturnModel)>, AppError>,
    save_result: DefaultResult<usize, AppError>,
) -> Box<dyn AbsOrderReturnRepo> {
    let repo = MockOrderReturnRepo::build(fetched_returns, fetched_oid_returns, save_result);
    Box::new(repo)
}

async fn return_lines_request_common(
    fetched_olines: Vec<OrderLineModel>,
    fetched_returns: DefaultResult<Vec<OrderReturnModel>, AppError>,
    save_result: DefaultResult<usize, AppError>,
    req_usr_id: u32,
    owner_usr_id: u32,
) -> DefaultResult<ReturnLinesReqUcOutput, AppError> {
    let shr_state = ut_setup_share_state("config_ok_no_sqldb.json", Box::new(MockConfidential {}));
    let logctx = shr_state.log_context().clone();
    let mocked_seller_ids = fetched_olines
        .iter()
        .map(|v| v.id_.store_id)
        .collect::<Vec<_>>();
    let currency_rate = ut_setup_order_currency(mocked_seller_ids);
    let o_repo = ut_oreturn_setup_repository_1(
        fetched_olines,
        vec![],
        owner_usr_id,
        None,
        Some(currency_rate),
    );
    let or_repo = ut_oreturn_setup_repository_2(fetched_returns, Ok(vec![]), save_result);
    let mock_order_id = "SomebodyOrderedThis".to_string();
    let mock_return_req = vec![OrderLineReqDto {
        seller_id: 800,
        product_id: 191,
        quantity: 2,
    }];
    let authed_claim = AppAuthedClaim {
        profile: req_usr_id,
        iat: 0,
        exp: 0,
        aud: Vec::new(),
        quota: vec![],
        perms: vec![AppAuthClaimPermission {
            app_code: app_meta::RESOURCE_QUOTA_AP_CODE,
            codename: AppAuthPermissionCode::can_create_return_req,
        }],
    };
    let uc = ReturnLinesReqUseCase {
        logctx,
        authed_claim,
        o_repo,
        or_repo,
    };
    uc.execute(mock_order_id, mock_return_req).await
} // end of fnf return_lines_request_common

#[tokio::test]
async fn return_lines_request_ok() {
    let fetched_olines = ut_setup_orderlines();
    let fetched_returns = Ok(ut_setup_olines_returns());
    let save_result = Ok(fetched_olines.len());
    let owner_usr_id = 1710u32;
    let result = return_lines_request_common(
        fetched_olines,
        fetched_returns,
        save_result,
        owner_usr_id,
        owner_usr_id,
    )
    .await;
    assert!(result.is_ok());
    if let Ok(out) = result {
        assert!(matches!(out, ReturnLinesReqUcOutput::Success));
    }
}

#[tokio::test]
async fn return_lines_request_fetch_error() {
    let fetched_olines = ut_setup_orderlines();
    let fetched_returns = Err(AppError {
        code: AppErrorCode::DataCorruption,
        detail: Some(format!("unit-test")),
    });
    let save_result = Ok(fetched_olines.len());
    let owner_usr_id = 1710u32;
    let result = return_lines_request_common(
        fetched_olines,
        fetched_returns,
        save_result,
        owner_usr_id,
        owner_usr_id,
    )
    .await;
    assert!(result.is_err());
    if let Err(e) = result.as_ref() {
        assert_eq!(e.code, AppErrorCode::DataCorruption);
        assert_eq!(e.detail.as_ref().unwrap(), "unit-test");
    }
}

#[tokio::test]
async fn return_lines_request_save_error() {
    let fetched_olines = ut_setup_orderlines();
    let fetched_returns = Ok(ut_setup_olines_returns());
    let save_result = Err(AppError {
        code: AppErrorCode::DataTableNotExist,
        detail: Some(format!("unit-test")),
    });
    let owner_usr_id = 1710u32;
    let result = return_lines_request_common(
        fetched_olines,
        fetched_returns,
        save_result,
        owner_usr_id,
        owner_usr_id,
    )
    .await;
    assert!(result.is_err());
    if let Err(e) = result.as_ref() {
        assert_eq!(e.code, AppErrorCode::DataTableNotExist);
        assert_eq!(e.detail.as_ref().unwrap(), "unit-test");
    }
} // end of fn return_lines_request_save_error

async fn replica_inventory_common(
    // --- order repo
    fetched_olines: Vec<OrderLineModel>,
    fetched_oids_ctime: Vec<String>,
    owner_usr_id: u32,
    order_ctime: Option<DateTime<FixedOffset>>,
    // --- order-return repo
    fetched_oid_returns: DefaultResult<Vec<(String, OrderReturnModel)>, AppError>,
    logctx: Arc<AppLogContext>,
) -> DefaultResult<OrderReplicaInventoryDto, AppError> {
    let unknown_err = AppError {
        detail: None,
        code: AppErrorCode::Unknown,
    };
    let o_repo = ut_oreturn_setup_repository_1(
        fetched_olines,
        fetched_oids_ctime,
        owner_usr_id,
        order_ctime,
        None,
    );
    let ret_repo = ut_oreturn_setup_repository_2(Ok(vec![]), fetched_oid_returns, Err(unknown_err));
    let mock_req = OrderReplicaInventoryReqDto {
        start: DateTime::parse_from_rfc3339("2022-11-06T02:33:00.519-09:00").unwrap(),
        end: DateTime::parse_from_rfc3339("2022-11-07T02:30:00.770-09:00").unwrap(),
    };
    let uc = OrderReplicaInventoryUseCase {
        logctx,
        ret_repo,
        o_repo,
    };
    uc.execute(mock_req).await
}

#[tokio::test]
async fn replica_inventory_ok() {
    let shr_state = ut_setup_share_state("config_ok_no_sqldb.json", Box::new(MockConfidential {}));
    let logctx = shr_state.log_context().clone();
    let fetched_olines = ut_setup_orderlines();
    let fetched_oids_ctime = vec!["order739".to_string()];
    let owner_usr_id = 1710u32;
    let order_ctime = Some(Local::now().fixed_offset());
    let fetched_oid_returns = ["order446", "order701", "order880", "order701"]
        .into_iter()
        .map(|s| s.to_string())
        .zip(ut_setup_olines_returns().into_iter())
        .collect();
    let result = replica_inventory_common(
        fetched_olines,
        fetched_oids_ctime,
        owner_usr_id,
        order_ctime,
        Ok(fetched_oid_returns),
        logctx,
    )
    .await;
    assert!(result.is_ok());
    if let Ok(v) = result {
        assert_eq!(v.reservations.len(), 1);
        assert_eq!(v.reservations[0].oid.as_str(), "order739");
        assert_eq!(v.reservations[0].lines.len(), 3);
        assert_eq!(v.returns.len(), 3);
        v.returns
            .into_iter()
            .map(|d| {
                let actual_num_lines = d.lines.len();
                let actual_num_returns = d.lines.iter().map(|r| r.qty).sum::<u32>();
                let (expect_num_lines, expect_num_returns) = match d.oid.as_str() {
                    "order446" => (2, 6),
                    "order701" => (6, 12),
                    "order880" => (3, 6),
                    _others => (0, 0),
                };
                assert_eq!(actual_num_lines, expect_num_lines);
                assert_eq!(actual_num_returns, expect_num_returns);
            })
            .count();
    }
} // end of fn replica_inventory_ok

#[tokio::test]
async fn replica_inventory_err() {
    let shr_state = ut_setup_share_state("config_ok_no_sqldb.json", Box::new(MockConfidential {}));
    let logctx = shr_state.log_context().clone();
    let fetched_olines = ut_setup_orderlines();
    let fetched_oids_ctime = vec!["order739".to_string()];
    let owner_usr_id = 1710u32;
    let order_ctime = Some(Local::now().fixed_offset());
    let fetched_oid_ret_err = AppError {
        code: AppErrorCode::DataCorruption,
        detail: Some(format!("unit-test")),
    };
    let result = replica_inventory_common(
        fetched_olines,
        fetched_oids_ctime,
        owner_usr_id,
        order_ctime,
        Err(fetched_oid_ret_err),
        logctx,
    )
    .await;
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.code, AppErrorCode::DataCorruption);
        assert_eq!(e.detail.as_ref().unwrap().as_str(), "unit-test");
    }
}

#[tokio::test]
async fn replica_refund_ok() {
    let unknown_err = AppError {
        detail: None,
        code: AppErrorCode::Unknown,
    };
    let mock_order_id = "My391004".to_string();
    let fetched_oid_returns = ut_setup_olines_returns()
        .into_iter()
        .map(|rm| (mock_order_id.clone(), rm))
        .collect::<Vec<_>>();
    let mock_buyer_id = 789u32;
    let mocked_seller_ids = fetched_oid_returns
        .iter()
        .map(|v| v.1.id_.store_id)
        .collect::<Vec<_>>();
    let mocked_currency_rate = ut_setup_order_currency(mocked_seller_ids);
    let expect_num_returns = fetched_oid_returns
        .iter()
        .map(|v| v.1.qty.len())
        .sum::<usize>();
    let expect_refunds: HashSet<(u32, u64, String, String, u32), RandomState> = {
        let iter = fetched_oid_returns.iter().flat_map(|(_, ret)| {
            ret.qty.iter().map(|(t, (qty, refund))| {
                let scale_limit = mocked_currency_rate.buyer.name.amount_fraction_scale();
                let mantissa = (refund.total as i64) * 10i64.pow(scale_limit);
                (
                    ret.id_.store_id,
                    ret.id_.product_id,
                    t.to_rfc3339(),
                    Decimal::new(mantissa, scale_limit).to_string(),
                    *qty,
                )
            })
        });
        HashSet::from_iter(iter)
    };
    let o_repo = ut_oreturn_setup_repository_1(
        Vec::new(),
        Vec::new(),
        mock_buyer_id,
        None,
        Some(mocked_currency_rate),
    );
    let ret_repo =
        ut_oreturn_setup_repository_2(Ok(vec![]), Ok(fetched_oid_returns), Err(unknown_err));
    let req = OrderReplicaRefundReqDto {
        start: "2023-11-17T12:00:04+02:00".to_string(),
        end: "2023-11-19T12:00:04+02:00".to_string(),
    };
    let uc = OrderReplicaRefundUseCase { ret_repo, o_repo };
    let result = uc.execute(req).await;
    assert!(result.is_ok());
    if let Ok(mut v) = result {
        assert_eq!(v.len(), 1);
        let read_refunds = v.remove(&mock_order_id).unwrap();
        assert_eq!(read_refunds.len(), expect_num_returns);
        let iter = read_refunds.into_iter().map(|item| {
            (
                item.seller_id,
                item.product_id,
                item.create_time,
                item.amount.total,
                item.qty,
            )
        });
        let actual_refunds = HashSet::from_iter(iter);
        let diff_cnt = expect_refunds.difference(&actual_refunds).count();
        assert_eq!(diff_cnt, 0);
    }
} // end of fn replica_refund_ok
