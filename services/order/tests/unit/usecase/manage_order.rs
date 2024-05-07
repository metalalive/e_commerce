use chrono::{DateTime, Duration, FixedOffset, Local};
use std::collections::hash_map::RandomState;
use std::collections::{HashMap, HashSet};
use std::result::Result as DefaultResult;
use std::sync::Arc;

use ecommerce_common::constant::ProductType;
use ecommerce_common::error::AppErrorCode;

use order::api::rpc::dto::{
    OrderReplicaInventoryDto, OrderReplicaInventoryReqDto, OrderReplicaRefundReqDto,
    StockReturnErrorDto,
};
use order::api::web::dto::OrderLineReqDto;
use order::constant::app_meta;
use order::error::AppError;
use order::logging::AppLogContext;
use order::model::{
    OrderLineAppliedPolicyModel, OrderLineIdentity, OrderLineModel, OrderLineModelSet,
    OrderLinePriceModel, OrderLineQuantityModel, OrderReturnModel, ProductPolicyModel,
    ProductPolicyModelSet, ProductPriceModel, ProductPriceModelSet,
};
use order::repository::{AbsOrderRepo, AbsOrderReturnRepo};
use order::usecase::{
    CreateOrderUsKsErr, CreateOrderUseCase, OrderDiscardUnpaidItemsUseCase,
    OrderReplicaInventoryUseCase, OrderReplicaRefundUseCase, ReturnLinesReqUcOutput,
    ReturnLinesReqUseCase,
};
use order::{AppAuthClaimPermission, AppAuthPermissionCode, AppAuthedClaim};

use super::{MockOrderRepo, MockOrderReturnRepo};
use crate::{ut_setup_share_state, MockConfidential};

fn ut_setup_prod_policies() -> ProductPolicyModelSet {
    ProductPolicyModelSet {
        policies: vec![
            ProductPolicyModel {
                product_type: ProductType::Package,
                product_id: 168,
                min_num_rsv: 0,
                warranty_hours: 127,
                auto_cancel_secs: 1008,
                is_create: false,
                max_num_rsv: 0,
            },
            ProductPolicyModel {
                product_type: ProductType::Item,
                product_id: 168,
                min_num_rsv: 0,
                warranty_hours: 20000,
                auto_cancel_secs: 1250,
                is_create: false,
                max_num_rsv: 0,
            },
            ProductPolicyModel {
                product_type: ProductType::Package,
                product_id: 174,
                min_num_rsv: 0,
                warranty_hours: 30000,
                auto_cancel_secs: 2255,
                is_create: false,
                max_num_rsv: 0,
            },
            ProductPolicyModel {
                product_type: ProductType::Item,
                product_id: 169,
                min_num_rsv: 1,
                warranty_hours: 21000,
                auto_cancel_secs: 150,
                is_create: false,
                max_num_rsv: 5,
            },
        ],
    }
}

fn ut_setup_prod_prices() -> Vec<ProductPriceModelSet> {
    vec![
        ProductPriceModelSet {
            store_id: 51,
            items: vec![
                ProductPriceModel {
                    product_type: ProductType::Item,
                    product_id: 168,
                    start_after: DateTime::parse_from_rfc3339("2023-07-31T10:16:54+05:00")
                        .unwrap()
                        .into(),
                    end_before: DateTime::parse_from_rfc3339("2023-10-10T09:01:31+02:00")
                        .unwrap()
                        .into(),
                    is_create: false,
                    price: 510,
                },
                ProductPriceModel {
                    product_type: ProductType::Package,
                    product_id: 168,
                    start_after: DateTime::parse_from_rfc3339("2023-07-31T10:16:54+05:00")
                        .unwrap()
                        .into(),
                    end_before: DateTime::parse_from_rfc3339("2023-10-10T09:01:31+02:00")
                        .unwrap()
                        .into(),
                    is_create: false,
                    price: 1130,
                },
                ProductPriceModel {
                    product_type: ProductType::Item,
                    product_id: 169,
                    start_after: DateTime::parse_from_rfc3339("2022-12-02T14:29:54+05:00")
                        .unwrap()
                        .into(),
                    end_before: DateTime::parse_from_rfc3339("2023-01-15T19:01:31+02:00")
                        .unwrap()
                        .into(),
                    is_create: false,
                    price: 190,
                },
            ],
        },
        ProductPriceModelSet {
            store_id: 52,
            items: vec![
                ProductPriceModel {
                    product_type: ProductType::Item,
                    product_id: 168,
                    start_after: DateTime::parse_from_rfc3339("2023-07-31T11:29:04+02:00")
                        .unwrap()
                        .into(),
                    end_before: DateTime::parse_from_rfc3339("2023-08-30T09:01:31-08:00")
                        .unwrap()
                        .into(),
                    is_create: false,
                    price: 480,
                },
                ProductPriceModel {
                    product_type: ProductType::Package,
                    product_id: 900,
                    start_after: DateTime::parse_from_rfc3339("2023-05-01T21:49:04+02:00")
                        .unwrap()
                        .into(),
                    end_before: DateTime::parse_from_rfc3339("2023-07-31T09:01:55-10:00")
                        .unwrap()
                        .into(),
                    is_create: false,
                    price: 490,
                },
                ProductPriceModel {
                    product_type: ProductType::Item,
                    product_id: 901,
                    start_after: DateTime::parse_from_rfc3339("2023-05-01T21:49:04+02:00")
                        .unwrap()
                        .into(),
                    end_before: DateTime::parse_from_rfc3339("2023-07-31T09:01:55-10:00")
                        .unwrap()
                        .into(),
                    is_create: false,
                    price: 399,
                },
            ],
        },
    ]
}

#[test]
fn validate_orderline_ok() {
    let ms_policy = ut_setup_prod_policies();
    let ms_price = ut_setup_prod_prices();
    let data = vec![
        OrderLineReqDto {
            seller_id: 52,
            product_type: ProductType::Item,
            product_id: 168,
            quantity: 6,
        },
        OrderLineReqDto {
            seller_id: 51,
            product_type: ProductType::Package,
            product_id: 168,
            quantity: 1,
        },
        OrderLineReqDto {
            seller_id: 51,
            product_type: ProductType::Item,
            product_id: 168,
            quantity: 10,
        },
    ];
    let result = CreateOrderUseCase::validate_orderline(ms_policy, ms_price, data);
    assert!(result.is_ok());
    if let Ok(v) = result {
        assert_eq!(v.len(), 3);
        let found = v.iter().any(|m| {
            m.id_.store_id == 52
                && m.id_.product_id == 168
                && m.id_.product_type == ProductType::Item
        });
        assert!(found);
        let found = v.iter().any(|m| {
            m.id_.store_id == 51
                && m.id_.product_id == 168
                && m.id_.product_type == ProductType::Item
        });
        assert!(found);
        let found = v.iter().any(|m| {
            m.id_.store_id == 51
                && m.id_.product_id == 168
                && m.id_.product_type == ProductType::Package
        });
        assert!(found);
    }
} // end of fn validate_orderline_ok

#[test]
fn validate_orderline_client_errors() {
    let ms_policy = ut_setup_prod_policies();
    let ms_price = ut_setup_prod_prices();
    let data = vec![
        OrderLineReqDto {
            seller_id: 51,
            product_type: ProductType::Item,
            product_id: 169,
            quantity: 6,
        },
        OrderLineReqDto {
            seller_id: 52,
            product_type: ProductType::Package,
            product_id: 174,
            quantity: 4,
        },
        OrderLineReqDto {
            seller_id: 52,
            product_type: ProductType::Package,
            product_id: 900,
            quantity: 2,
        },
        OrderLineReqDto {
            seller_id: 51,
            product_type: ProductType::Package,
            product_id: 168,
            quantity: 11,
        },
        OrderLineReqDto {
            seller_id: 52,
            product_type: ProductType::Item,
            product_id: 901,
            quantity: 9,
        },
    ];
    let result = CreateOrderUseCase::validate_orderline(ms_policy, ms_price, data);
    assert!(result.is_err());
    if let Err(CreateOrderUsKsErr::ReqContent(v)) = result {
        let errs = v.order_lines.unwrap();
        assert_eq!(errs.len(), 4);
        let found = errs
            .iter()
            .find(|e| {
                e.seller_id == 52 && e.product_type == ProductType::Package && e.product_id == 900
            })
            .unwrap();
        if let Some(v) = found.nonexist.as_ref() {
            assert!(v.product_policy);
            assert!(!v.product_price);
        }
        let found = errs
            .iter()
            .find(|e| {
                e.seller_id == 52 && e.product_type == ProductType::Item && e.product_id == 901
            })
            .unwrap();
        if let Some(v) = found.nonexist.as_ref() {
            assert!(v.product_policy);
            assert!(!v.product_price);
        }
        let found = errs
            .iter()
            .find(|e| {
                e.seller_id == 52 && e.product_type == ProductType::Package && e.product_id == 174
            })
            .unwrap();
        if let Some(v) = found.nonexist.as_ref() {
            assert!(!v.product_policy);
            assert!(v.product_price);
        }
        let found = errs
            .iter()
            .find(|e| {
                e.seller_id == 51 && e.product_type == ProductType::Item && e.product_id == 169
            })
            .unwrap();
        if let Some(v) = found.rsv_limit.as_ref() {
            assert_eq!(v.max_, 5);
            assert_eq!(v.given, 6);
        }
    }
} // end of validate_orderline_client_errors

fn ut_setup_orderlines() -> Vec<OrderLineModel> {
    let base_time = Local::now().fixed_offset();
    let paid_last_update = Some(base_time + Duration::minutes(4));
    let reserved_until = base_time + Duration::minutes(5);
    let warranty_until = base_time + Duration::days(14);
    vec![
        OrderLineModel {
            id_: OrderLineIdentity {
                store_id: 108,
                product_type: ProductType::Item,
                product_id: 190,
            },
            price: OrderLinePriceModel {
                unit: 10,
                total: 139,
            },
            qty: OrderLineQuantityModel {
                reserved: 14,
                paid: 13,
                paid_last_update,
            },
            policy: OrderLineAppliedPolicyModel {
                reserved_until,
                warranty_until,
            },
        },
        OrderLineModel {
            id_: OrderLineIdentity {
                store_id: 800,
                product_type: ProductType::Item,
                product_id: 191,
            },
            price: OrderLinePriceModel {
                unit: 12,
                total: 180,
            },
            qty: OrderLineQuantityModel {
                reserved: 15,
                paid: 15,
                paid_last_update,
            },
            policy: OrderLineAppliedPolicyModel {
                reserved_until,
                warranty_until,
            },
        },
        OrderLineModel {
            id_: OrderLineIdentity {
                store_id: 426,
                product_type: ProductType::Package,
                product_id: 192,
            },
            price: OrderLinePriceModel {
                unit: 12,
                total: 216,
            },
            qty: OrderLineQuantityModel {
                reserved: 18,
                paid: 15,
                paid_last_update,
            },
            policy: OrderLineAppliedPolicyModel {
                reserved_until,
                warranty_until,
            },
        },
    ]
}

fn ut_setup_olines_returns() -> Vec<OrderReturnModel> {
    let return_time = DateTime::parse_from_rfc3339("2023-11-18T02:39:04+02:00").unwrap();
    vec![
        OrderReturnModel {
            id_: OrderLineIdentity {
                store_id: 108,
                product_id: 190,
                product_type: ProductType::Item,
            },
            qty: HashMap::from([
                (
                    return_time + Duration::seconds(11),
                    (
                        1,
                        OrderLinePriceModel {
                            unit: 10,
                            total: 10,
                        },
                    ),
                ),
                (
                    return_time + Duration::seconds(30),
                    (
                        5,
                        OrderLinePriceModel {
                            unit: 13,
                            total: 65,
                        },
                    ),
                ),
            ]),
        },
        OrderReturnModel {
            id_: OrderLineIdentity {
                store_id: 800,
                product_id: 191,
                product_type: ProductType::Item,
            },
            qty: HashMap::from([
                (
                    return_time + Duration::seconds(6),
                    (
                        1,
                        OrderLinePriceModel {
                            unit: 12,
                            total: 12,
                        },
                    ),
                ),
                (
                    return_time + Duration::seconds(28),
                    (
                        1,
                        OrderLinePriceModel {
                            unit: 12,
                            total: 12,
                        },
                    ),
                ),
                (
                    return_time + Duration::seconds(65),
                    (
                        2,
                        OrderLinePriceModel {
                            unit: 12,
                            total: 24,
                        },
                    ),
                ),
                (
                    return_time + Duration::seconds(99),
                    (
                        1,
                        OrderLinePriceModel {
                            unit: 12,
                            total: 12,
                        },
                    ),
                ),
            ]),
        },
        OrderReturnModel {
            id_: OrderLineIdentity {
                store_id: 426,
                product_id: 192,
                product_type: ProductType::Package,
            },
            qty: HashMap::from([
                (
                    return_time + Duration::seconds(12),
                    (
                        2,
                        OrderLinePriceModel {
                            unit: 11,
                            total: 22,
                        },
                    ),
                ),
                (
                    return_time + Duration::seconds(73),
                    (
                        3,
                        OrderLinePriceModel {
                            unit: 11,
                            total: 33,
                        },
                    ),
                ),
                (
                    return_time + Duration::seconds(94),
                    (
                        1,
                        OrderLinePriceModel {
                            unit: 11,
                            total: 11,
                        },
                    ),
                ),
            ]),
        },
        OrderReturnModel {
            id_: OrderLineIdentity {
                store_id: 426,
                product_id: 8964,
                product_type: ProductType::Item,
            },
            qty: HashMap::from([
                (
                    return_time + Duration::seconds(10),
                    (
                        3,
                        OrderLinePriceModel {
                            unit: 15,
                            total: 45,
                        },
                    ),
                ),
                (
                    return_time + Duration::seconds(19),
                    (
                        4,
                        OrderLinePriceModel {
                            unit: 15,
                            total: 60,
                        },
                    ),
                ),
            ]),
        },
    ]
}

async fn discard_unpaid_items_common(
    stock_return_results: Vec<DefaultResult<Vec<StockReturnErrorDto>, AppError>>,
    fetched_ol_sets: Vec<OrderLineModelSet>,
) -> DefaultResult<(), AppError> {
    let shr_state = ut_setup_share_state("config_ok.json", Box::new(MockConfidential {}));
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
    let fetched_ol_sets = vec![
        OrderLineModelSet {
            order_id: "xx1".to_string(),
            owner_id: 123,
            create_time: create_time.clone(),
            lines: mocked_olines.drain(0..2).collect(),
        },
        OrderLineModelSet {
            order_id: "xx2".to_string(),
            owner_id: 124,
            create_time,
            lines: mocked_olines,
        },
    ];
    let result = discard_unpaid_items_common(stock_return_results, fetched_ol_sets).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn discard_unpaid_items_err_stocklvl() {
    let mut mocked_olines = ut_setup_orderlines();
    let data_corrupt = AppError {
        detail: Some(format!("unit-test")),
        code: AppErrorCode::DataCorruption,
    };
    let stock_return_results = vec![Ok(vec![]), Err(data_corrupt)];
    let create_time = DateTime::parse_from_rfc3339("2022-11-07T04:00:00.519-01:00").unwrap();
    let fetched_ol_sets = vec![
        OrderLineModelSet {
            order_id: "xx1".to_string(),
            owner_id: 500,
            create_time: create_time.clone(),
            lines: mocked_olines.drain(0..1).collect(),
        },
        OrderLineModelSet {
            order_id: "xx2".to_string(),
            owner_id: 510,
            create_time,
            lines: mocked_olines,
        },
    ];
    let result = discard_unpaid_items_common(stock_return_results, fetched_ol_sets).await;
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.code, AppErrorCode::DataCorruption);
        assert_eq!(e.detail.as_ref().unwrap(), "unit-test");
    }
}

fn ut_oreturn_setup_repository_1(
    fetched_olines: Vec<OrderLineModel>,
    fetched_oids_ctime: Vec<String>,
    owner_usr_id: u32,
    order_ctime: Option<DateTime<FixedOffset>>,
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

async fn request_lines_request_common(
    fetched_olines: Vec<OrderLineModel>,
    fetched_returns: DefaultResult<Vec<OrderReturnModel>, AppError>,
    save_result: DefaultResult<usize, AppError>,
    req_usr_id: u32,
    owner_usr_id: u32,
) -> DefaultResult<ReturnLinesReqUcOutput, AppError> {
    let shr_state = ut_setup_share_state("config_ok.json", Box::new(MockConfidential {}));
    let logctx = shr_state.log_context().clone();
    let o_repo = ut_oreturn_setup_repository_1(fetched_olines, vec![], owner_usr_id, None);
    let or_repo = ut_oreturn_setup_repository_2(fetched_returns, Ok(vec![]), save_result);
    let mock_order_id = "SomebodyOrderedThis".to_string();
    let mock_return_req = vec![OrderLineReqDto {
        seller_id: 800,
        product_type: ProductType::Item,
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
} // end of fnf request_lines_request_common

#[tokio::test]
async fn return_lines_request_ok() {
    let fetched_olines = ut_setup_orderlines();
    let fetched_returns = Ok(ut_setup_olines_returns());
    let save_result = Ok(fetched_olines.len());
    let owner_usr_id = 1710u32;
    let result = request_lines_request_common(
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
    let result = request_lines_request_common(
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
    let result = request_lines_request_common(
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
}

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
    let shr_state = ut_setup_share_state("config_ok.json", Box::new(MockConfidential {}));
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
    let shr_state = ut_setup_share_state("config_ok.json", Box::new(MockConfidential {}));
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
    let fetched_returns = ut_setup_olines_returns();
    let expect_num_returns = fetched_returns
        .iter()
        .map(|ret| ret.qty.len())
        .sum::<usize>();
    let expect_refunds: HashSet<(u32, u64, DateTime<FixedOffset>, u32), RandomState> = {
        let iter = fetched_returns.iter().flat_map(|ret| {
            ret.qty.iter().map(|(t, (_q, refund))| {
                (
                    ret.id_.store_id,
                    ret.id_.product_id,
                    t.clone(),
                    refund.total,
                )
            })
        });
        HashSet::from_iter(iter)
    };
    let repo = ut_oreturn_setup_repository_2(Ok(fetched_returns), Ok(vec![]), Err(unknown_err));
    let req = OrderReplicaRefundReqDto {
        order_id: "My391004".to_string(),
        start: DateTime::parse_from_rfc3339("2023-11-17T12:00:04+02:00").unwrap(),
        end: DateTime::parse_from_rfc3339("2023-11-19T12:00:04+02:00").unwrap(),
    };
    let uc = OrderReplicaRefundUseCase { repo };
    let result = uc.execute(req).await;
    assert!(result.is_ok());
    if let Ok(v) = result {
        assert_eq!(v.len(), expect_num_returns);
        let iter = v.into_iter().map(|item| {
            (
                item.seller_id,
                item.product_id,
                item.create_time,
                item.amount.total,
            )
        });
        let actual_refunds = HashSet::from_iter(iter);
        let diff_cnt = expect_refunds.difference(&actual_refunds).count();
        assert_eq!(diff_cnt, 0);
    }
} // end of fn replica_refund_ok
