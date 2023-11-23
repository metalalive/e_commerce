use std::result::Result as DefaultResult;

use chrono::DateTime;
use order::api::rpc::dto::StockReturnErrorDto;
use order::api::web::dto::OrderLineReqDto;
use order::constant::ProductType;
use order::error::{AppError, AppErrorCode};
use order::model::{
    ProductPolicyModelSet, ProductPolicyModel, ProductPriceModelSet, ProductPriceModel,
    OrderLineModelSet, OrderLinePriceModel, OrderLineQuantityModel, OrderLineAppliedPolicyModel,
    OrderLineModel, OrderReturnModel, OrderLineIdentity
};
use order::repository::{AbsOrderRepo, AbsOrderReturnRepo};
use order::usecase::{
    CreateOrderUseCase, CreateOrderUsKsErr, OrderDiscardUnpaidItemsUseCase,
    ReturnLinesReqUseCase, ReturnLinesReqUcOutput
};

use super::{ut_setup_share_state, MockOrderRepo, MockOrderReturnRepo};

fn ut_setup_prod_policies () -> ProductPolicyModelSet
{
    ProductPolicyModelSet {policies: vec![
        ProductPolicyModel {product_type:ProductType::Package, product_id:168,
            warranty_hours:127, auto_cancel_secs:1008, is_create:false },
        ProductPolicyModel {product_type:ProductType::Item, product_id:168,
            warranty_hours:20000, auto_cancel_secs:1250, is_create:false },
        ProductPolicyModel {product_type:ProductType::Package, product_id:174,
            warranty_hours:30000, auto_cancel_secs:2255, is_create:false },
    ]}
}

fn ut_setup_prod_prices () -> Vec<ProductPriceModelSet>
{
    vec![
        ProductPriceModelSet {store_id:51, items:vec![
            ProductPriceModel {product_type:ProductType::Item, product_id:168,
                start_after:DateTime::parse_from_rfc3339("2023-07-31T10:16:54+05:00").unwrap().into(),
                end_before:DateTime::parse_from_rfc3339("2023-10-10T09:01:31+02:00").unwrap().into(),
                is_create:false, price: 510  },
            ProductPriceModel {product_type:ProductType::Package, product_id:168,
                start_after:DateTime::parse_from_rfc3339("2023-07-31T10:16:54+05:00").unwrap().into(),
                end_before:DateTime::parse_from_rfc3339("2023-10-10T09:01:31+02:00").unwrap().into(),
                is_create:false, price: 1130  },
        ]},
        ProductPriceModelSet {store_id:52, items:vec![
            ProductPriceModel {product_type:ProductType::Item, product_id:168,
                start_after:DateTime::parse_from_rfc3339("2023-07-31T11:29:04+02:00").unwrap().into(),
                end_before:DateTime::parse_from_rfc3339("2023-08-30T09:01:31-08:00").unwrap().into(),
                is_create:false, price: 480  },
            ProductPriceModel {product_type:ProductType::Package, product_id:900,
                start_after:DateTime::parse_from_rfc3339("2023-05-01T21:49:04+02:00").unwrap().into(),
                end_before:DateTime::parse_from_rfc3339("2023-07-31T09:01:55-10:00").unwrap().into(),
                is_create:false, price: 490  },
            ProductPriceModel {product_type:ProductType::Item, product_id:901,
                start_after:DateTime::parse_from_rfc3339("2023-05-01T21:49:04+02:00").unwrap().into(),
                end_before:DateTime::parse_from_rfc3339("2023-07-31T09:01:55-10:00").unwrap().into(),
                is_create:false, price: 399  },
        ]},
    ]
}

#[test]
fn validate_orderline_ok ()
{
    let ms_policy = ut_setup_prod_policies();
    let ms_price = ut_setup_prod_prices();
    let data = vec![
        OrderLineReqDto {seller_id:52, product_type:ProductType::Item,
            product_id:168, quantity:6 },
        OrderLineReqDto {seller_id:51, product_type:ProductType::Package,
            product_id:168, quantity:1 },
        OrderLineReqDto {seller_id:51, product_type:ProductType::Item,
            product_id:168, quantity:10 },
    ];
    let result = CreateOrderUseCase::validate_orderline(ms_policy, ms_price, data);
    assert!(result.is_ok());
    if let Ok(v) = result {
        assert_eq!(v.len(), 3);
        let found = v.iter().any(|m| {
            m.id_.store_id==52 && m.id_.product_id==168
                && m.id_.product_type==ProductType::Item
        });
        assert!(found);
        let found = v.iter().any(|m| {
            m.id_.store_id==51 && m.id_.product_id==168
                && m.id_.product_type==ProductType::Item
        });
        assert!(found);
        let found = v.iter().any(|m| {
            m.id_.store_id==51 && m.id_.product_id==168
                && m.id_.product_type==ProductType::Package
        });
        assert!(found);
    }
} // end of fn validate_orderline_ok

#[test]
fn validate_orderline_missing_properties ()
{
    let ms_policy = ut_setup_prod_policies();
    let ms_price = ut_setup_prod_prices();
    let data = vec![
        OrderLineReqDto {seller_id:52, product_type:ProductType::Package,
            product_id:174, quantity:4 },
        OrderLineReqDto {seller_id:52, product_type:ProductType::Package,
            product_id:900, quantity:2 },
        OrderLineReqDto {seller_id:51, product_type:ProductType::Package,
            product_id:168, quantity:11 },
        OrderLineReqDto {seller_id:52, product_type:ProductType::Item,
            product_id:901, quantity:9 },
    ];
    let result = CreateOrderUseCase::validate_orderline(ms_policy, ms_price, data);
    assert!(result.is_err());
    if let Err(CreateOrderUsKsErr::Client(v)) = result {
        let errs = v.order_lines.unwrap();
        assert_eq!(errs.len(), 3);
        let found = errs.iter().find(|e| {
            e.seller_id==52 && e.product_type==ProductType::Package && e.product_id==900
        }).unwrap();
        if let Some(v) = found.nonexist.as_ref() {
            assert!(v.product_policy);
            assert!(!v.product_price);
        }
        let found = errs.iter().find(|e| {
            e.seller_id==52 && e.product_type==ProductType::Item && e.product_id==901
        }).unwrap();
        if let Some(v) = found.nonexist.as_ref() {
            assert!(v.product_policy);
            assert!(!v.product_price);
        }
        let found = errs.iter().find(|e| {
            e.seller_id==52 && e.product_type==ProductType::Package && e.product_id==174
        }).unwrap();
        if let Some(v) = found.nonexist.as_ref() {
            assert!(!v.product_policy);
            assert!(v.product_price);
        }
    }
} // end of validate_orderline_missing_properties


fn ut_setup_orderlines () -> Vec<OrderLineModel>
{
    let paid_last_update = Some(DateTime::parse_from_rfc3339("2023-11-15T09:23:49+02:00").unwrap());
    let reserved_until = DateTime::parse_from_rfc3339("2023-11-15T09:23:50+02:00").unwrap();
    let warranty_until = DateTime::parse_from_rfc3339("2023-12-24T13:39:41+02:00").unwrap();
    vec![
        OrderLineModel {id_: OrderLineIdentity {store_id:108 , product_type:ProductType::Item,
            product_id: 190}, price:OrderLinePriceModel { unit:10, total: 39 },
            qty: OrderLineQuantityModel {reserved: 4, paid: 3, paid_last_update},
            policy: OrderLineAppliedPolicyModel { reserved_until, warranty_until }
        },
        OrderLineModel {id_: OrderLineIdentity {store_id:800, product_type:ProductType::Item,
            product_id: 191}, price:OrderLinePriceModel { unit:12, total: 60 },
            qty: OrderLineQuantityModel {reserved: 5, paid: 5, paid_last_update},
            policy: OrderLineAppliedPolicyModel { reserved_until, warranty_until }
        },
        OrderLineModel {id_: OrderLineIdentity {store_id:426, product_type:ProductType::Package,
            product_id: 192}, price:OrderLinePriceModel { unit:12, total: 60 },
            qty: OrderLineQuantityModel {reserved: 8, paid: 5, paid_last_update},
            policy: OrderLineAppliedPolicyModel { reserved_until, warranty_until }
        },
    ]
}

fn ut_setup_olines_returns () -> Vec<OrderReturnModel>
{
    let return_time = DateTime::parse_from_rfc3339("2023-11-18T02:39:04+02:00").unwrap();
    vec![
        OrderReturnModel {
            id_:OrderLineIdentity {store_id:108, product_id:190, product_type:ProductType::Item},
            qty:vec![(1, return_time.clone())],  price:OrderLinePriceModel { unit:10, total: 10 }
        },
        OrderReturnModel {
            id_:OrderLineIdentity {store_id:800, product_id:191, product_type:ProductType::Item},
            qty:vec![(1, return_time.clone())],  price:OrderLinePriceModel { unit:12, total: 12 }
        },
        OrderReturnModel {
            id_:OrderLineIdentity {store_id:426, product_id:192, product_type:ProductType::Package},
            qty:vec![(2, return_time.clone())],  price:OrderLinePriceModel { unit:12, total: 24 }
        },
    ]
}


async fn discard_unpaid_items_common(
        stock_return_results: Vec<DefaultResult<Vec<StockReturnErrorDto>, AppError>>,
        fetched_ol_sets:Vec<OrderLineModelSet>
    )
    -> DefaultResult<(), AppError>
{
    let shr_state = ut_setup_share_state();
    let logctx = shr_state.log_context().clone();
    let not_impl_err = AppError{detail:None, code:AppErrorCode::NotImplemented};
    let repo = MockOrderRepo::build(
        Err(not_impl_err.clone()),
        Err(not_impl_err.clone()),
        stock_return_results,
        fetched_ol_sets, vec![], None
    );
    let repo:Box<dyn AbsOrderRepo> = Box::new(repo);
    let uc = OrderDiscardUnpaidItemsUseCase::new(repo, logctx);
    uc.execute().await
}

#[tokio::test]
async fn discard_unpaid_items_ok()
{
    let mut mocked_olines = ut_setup_orderlines();
    let stock_return_results = vec![ Ok(vec![]), Ok(vec![]) ];
    let fetched_ol_sets = vec![
        OrderLineModelSet {order_id:"xx1".to_string(), lines:mocked_olines.drain(0..2).collect()},
        OrderLineModelSet {order_id:"xx2".to_string(), lines:mocked_olines},
    ];
    let result = discard_unpaid_items_common(stock_return_results, fetched_ol_sets).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn discard_unpaid_items_err_stocklvl()
{
    let mut mocked_olines = ut_setup_orderlines();
    let data_corrupt = AppError{detail:Some(format!("unit-test")), code:AppErrorCode::DataCorruption};
    let stock_return_results = vec![ Ok(vec![]), Err(data_corrupt) ];
    let fetched_ol_sets = vec![
        OrderLineModelSet {order_id:"xx1".to_string(), lines:mocked_olines.drain(0..1).collect()},
        OrderLineModelSet {order_id:"xx2".to_string(), lines:mocked_olines},
    ];
    let result = discard_unpaid_items_common(stock_return_results, fetched_ol_sets).await;
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.code, AppErrorCode::DataCorruption);
        assert_eq!(e.detail.as_ref().unwrap(), "unit-test");
    }
}


async fn request_lines_request_common(
        fetched_olines: Vec<OrderLineModel>,
        fetched_returns: DefaultResult<Vec<OrderReturnModel>, AppError>,
        save_result: DefaultResult<usize, AppError>, 
        req_usr_id: u32, 
        owner_usr_id: u32
    )
    -> DefaultResult<ReturnLinesReqUcOutput, AppError>
{
    let shr_state = ut_setup_share_state();
    let logctx = shr_state.log_context().clone();
    let not_impl_err = AppError{detail:None, code:AppErrorCode::NotImplemented};
    let o_repo:Box<dyn AbsOrderRepo> = {
        let repo = MockOrderRepo::build(
            Err(not_impl_err.clone()),
            Err(not_impl_err.clone()),
            vec![], vec![], fetched_olines, Some(owner_usr_id)
        );
        Box::new(repo)
    };
    let or_repo:Box<dyn AbsOrderReturnRepo> = {
        let repo = MockOrderReturnRepo::build(fetched_returns, save_result);
        Box::new(repo)
    };
    let mock_order_id = "SomebodyOrderedThis".to_string();
    let mock_return_req = vec![
        OrderLineReqDto {seller_id:145, product_type:ProductType::Package,
            product_id:599, quantity:2 }
    ];
    let uc = ReturnLinesReqUseCase {logctx, usr_prof_id:req_usr_id, o_repo, or_repo };
    uc.execute(mock_order_id, mock_return_req).await
} // end of fnf request_lines_request_common

#[tokio::test]
async fn return_lines_request_ok()
{
    let fetched_olines = ut_setup_orderlines();
    let fetched_returns = Ok(ut_setup_olines_returns());
    let save_result = Ok(fetched_olines.len()); 
    let owner_usr_id = 1710u32;
    let result = request_lines_request_common(
        fetched_olines, fetched_returns,  save_result, 
        owner_usr_id , owner_usr_id ).await ;
    assert!(result.is_ok());
}

#[tokio::test]
async fn return_lines_request_fetch_error()
{
    let fetched_olines = ut_setup_orderlines();
    let fetched_returns = Err(AppError{code: AppErrorCode::DataCorruption, detail: Some(format!("unit-test")) });
    let save_result = Ok(fetched_olines.len()); 
    let owner_usr_id = 1710u32;
    let result = request_lines_request_common(
        fetched_olines, fetched_returns,  save_result, 
        owner_usr_id , owner_usr_id ).await ;
    assert!(result.is_err());
    if let Err(e) = result.as_ref() {
        assert_eq!(e.code, AppErrorCode::DataCorruption);
        assert_eq!(e.detail.as_ref().unwrap(), "unit-test");
    }
}

#[tokio::test]
async fn return_lines_request_save_error()
{
    let fetched_olines = ut_setup_orderlines();
    let fetched_returns = Ok(ut_setup_olines_returns());
    let save_result = Err(AppError{code: AppErrorCode::DataTableNotExist, detail: Some(format!("unit-test")) });
    let owner_usr_id = 1710u32;
    let result = request_lines_request_common(
        fetched_olines, fetched_returns,  save_result, 
        owner_usr_id , owner_usr_id ).await ;
    assert!(result.is_err());
    if let Err(e) = result.as_ref() {
        assert_eq!(e.code, AppErrorCode::DataTableNotExist);
        assert_eq!(e.detail.as_ref().unwrap(), "unit-test");
    }
}
