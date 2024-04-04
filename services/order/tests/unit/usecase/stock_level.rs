use chrono::DateTime;
use std::vec::Vec;

use order::api::rpc::dto::InventoryEditStockLevelDto;
use order::constant::ProductType;
use order::error::{AppError, AppErrorCode};
use order::model::{ProductStockModel, StockLevelModelSet, StockQuantityModel, StoreStockModel};
use order::usecase::StockLevelUseCase;

use super::MockOrderRepo;
use crate::{ut_setup_share_state, MockConfidential};

fn ut_setup_data() -> Vec<InventoryEditStockLevelDto> {
    vec![
        InventoryEditStockLevelDto {
            qty_add: 13,
            store_id: 91,
            product_type: ProductType::Item,
            product_id: 210094,
            expiry: DateTime::parse_from_rfc3339("2023-01-19T06:05:39.001+05:00").unwrap(),
        },
        InventoryEditStockLevelDto {
            qty_add: 2,
            store_id: 91,
            product_type: ProductType::Package,
            product_id: 210095,
            expiry: DateTime::parse_from_rfc3339("2023-01-19T06:05:39.002+05:00").unwrap(),
        },
        InventoryEditStockLevelDto {
            qty_add: -9,
            store_id: 99,
            product_type: ProductType::Item,
            product_id: 210096,
            expiry: DateTime::parse_from_rfc3339("2023-01-19T06:05:40.003+05:00").unwrap(),
        },
        InventoryEditStockLevelDto {
            qty_add: 5,
            store_id: 101,
            product_type: ProductType::Package,
            product_id: 210097,
            expiry: DateTime::parse_from_rfc3339("2023-01-29T06:05:47.001+05:00").unwrap(),
        },
    ]
}

#[tokio::test]
async fn edit_ok() {
    let app_state = ut_setup_share_state("config_ok.json", Box::new(MockConfidential {}));
    let init_data = ut_setup_data();
    let expect_fetch_res = Ok(StockLevelModelSet {
        stores: vec![StoreStockModel {
            store_id: init_data[2].store_id,
            products: vec![ProductStockModel {
                type_: init_data[2].product_type.clone(),
                id_: init_data[2].product_id,
                is_create: false,
                expiry: init_data[2].expiry.into(),
                quantity: StockQuantityModel::new(2, 0, 0, None),
            }],
        }],
    });
    let expect_save_res = Ok(());
    let repo = MockOrderRepo::build(
        expect_save_res,
        expect_fetch_res,
        vec![],
        vec![],
        vec![],
        vec![],
        None,
        None,
    );
    let result =
        StockLevelUseCase::try_edit(init_data, Box::new(repo), app_state.log_context().clone())
            .await;
    assert!(result.is_ok());
    let _stock_lvl_rd = result.unwrap();
    // TODO, verify present data from model set
}

#[tokio::test]
async fn edit_fetch_error() {
    let app_state = ut_setup_share_state("config_ok.json", Box::new(MockConfidential {}));
    let init_data = ut_setup_data();
    let expect_fetch_res = Err(AppError {
        code: AppErrorCode::DataCorruption,
        detail: Some("unit-test".to_string()),
    });
    let expect_save_res = Ok(());
    let repo = MockOrderRepo::build(
        expect_save_res,
        expect_fetch_res,
        vec![],
        vec![],
        vec![],
        vec![],
        None,
        None,
    );
    let result =
        StockLevelUseCase::try_edit(init_data, Box::new(repo), app_state.log_context().clone())
            .await;
    assert!(result.is_err());
    if let Err(error) = result {
        assert_eq!(error.code, AppErrorCode::DataCorruption);
        if let Some(msg) = error.detail {
            assert_eq!(msg.as_str(), "unit-test");
        }
    }
}

#[tokio::test]
async fn edit_save_error() {
    let app_state = ut_setup_share_state("config_ok.json", Box::new(MockConfidential {}));
    let init_data = ut_setup_data();
    let expect_fetch_res = Ok(StockLevelModelSet {
        stores: vec![StoreStockModel {
            store_id: init_data[2].store_id,
            products: vec![ProductStockModel {
                type_: init_data[2].product_type.clone(),
                id_: init_data[2].product_id,
                expiry: init_data[2].expiry.into(),
                is_create: false,
                quantity: StockQuantityModel::new(2, 0, 0, None),
            }],
        }],
    });
    let expect_save_res = Err(AppError {
        code: AppErrorCode::DataTableNotExist,
        detail: Some("unit-test".to_string()),
    });
    let repo = MockOrderRepo::build(
        expect_save_res,
        expect_fetch_res,
        vec![],
        vec![],
        vec![],
        vec![],
        None,
        None,
    );
    let result =
        StockLevelUseCase::try_edit(init_data, Box::new(repo), app_state.log_context().clone())
            .await;
    assert!(result.is_err());
    if let Err(error) = result {
        assert_eq!(error.code, AppErrorCode::DataTableNotExist);
        if let Some(msg) = error.detail {
            assert_eq!(msg.as_str(), "unit-test");
        }
    }
}
