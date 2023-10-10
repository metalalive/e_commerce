use std::sync::Arc;
use std::boxed::Box;
use std::vec::Vec;
use std::result::Result as DefaultResult;

use async_trait::async_trait;
use chrono::DateTime;

use order::AppDataStoreContext;
use order::api::rpc::dto::InventoryEditStockLevelDto;
use order::constant::ProductType;
use order::error::{AppError, AppErrorCode};
use order::model::{StockLevelModelSet, ProductStockIdentity, StoreStockModel, ProductStockModel, StockQuantityModel};
use order::repository::{AbsOrderRepo, AbsOrderStockRepo};
use order::usecase::StockLevelUseCase;

struct MockStockRepo {
    _mocked_save_r:  DefaultResult<(), AppError>,
    _mocked_fetch_r: DefaultResult<StockLevelModelSet, AppError>,
}
struct MockOrderRepo {
    _mocked_save:  DefaultResult<(), AppError>,
    _mocked_fetch: DefaultResult<StockLevelModelSet, AppError>,
}

#[async_trait]
impl AbsOrderStockRepo for MockStockRepo {
    async fn fetch(&self, _pids:Vec<ProductStockIdentity>) -> DefaultResult<StockLevelModelSet, AppError>
    { self._mocked_fetch_r.clone() }
    async fn save(&self, _slset:StockLevelModelSet) -> DefaultResult<(), AppError>
    { self._mocked_save_r.clone() }
}

impl AbsOrderRepo for MockOrderRepo {
    fn new(_ds:Arc<AppDataStoreContext>) -> DefaultResult<Box<dyn AbsOrderRepo>, AppError>
        where Self:Sized
    { Err(AppError {code:AppErrorCode::NotImplemented, detail:None}) }
    
    fn stock(&self) -> Arc<Box<dyn AbsOrderStockRepo>> {
        let obj = MockStockRepo {
            _mocked_save_r:  self._mocked_save.clone(),
            _mocked_fetch_r: self._mocked_fetch.clone(),
        };
        Arc::new(Box::new(obj))
    }
}

impl MockOrderRepo {
    fn build(save_r:DefaultResult<(), AppError>,
             fetch_r:DefaultResult<StockLevelModelSet, AppError> ) -> Self
    {
        Self{_mocked_save:save_r, _mocked_fetch:fetch_r}
    }
}

fn ut_setup_data() -> Vec<InventoryEditStockLevelDto>
{
    vec![
        InventoryEditStockLevelDto {qty_add:13, store_id:91, product_type:ProductType::Item, product_id: 210094,
            expiry: DateTime::parse_from_rfc3339("2023-01-19T06:05:39.001+05:00").unwrap().into()  },
        InventoryEditStockLevelDto {qty_add:2,  store_id:91, product_type:ProductType::Package, product_id: 210095,
            expiry: DateTime::parse_from_rfc3339("2023-01-19T06:05:39.002+05:00").unwrap().into()  },
        InventoryEditStockLevelDto {qty_add:-9, store_id:99, product_type:ProductType::Item, product_id: 210096,
            expiry: DateTime::parse_from_rfc3339("2023-01-19T06:05:40.003+05:00").unwrap().into()  },
        InventoryEditStockLevelDto {qty_add:5, store_id:101, product_type:ProductType::Package, product_id: 210097,
            expiry: DateTime::parse_from_rfc3339("2023-01-29T06:05:47.001+05:00").unwrap().into()  },
    ]
}

#[tokio::test]
async fn edit_ok ()
{
    let init_data = ut_setup_data();
    let expect_fetch_res = Ok(StockLevelModelSet{stores:vec![
        StoreStockModel {store_id:init_data[2].store_id, products:vec![
            ProductStockModel {type_:init_data[2].product_type.clone(),
                id_:init_data[2].product_id, expiry: init_data[2].expiry, is_create:false,
                quantity: StockQuantityModel {total: 2, booked: 0, cancelled: 0}}
        ]}
    ]}) ; 
    let expect_save_res  = Ok(());
    let repo = MockOrderRepo::build(expect_save_res, expect_fetch_res);
    let result = StockLevelUseCase::try_edit(init_data, Box::new(repo)).await;
    assert!(result.is_ok());
    let _stock_lvl_rd = result.unwrap();
    // TODO, verify present data from model set
}

#[tokio::test]
async fn edit_fetch_error ()
{
    let init_data = ut_setup_data();
    let expect_fetch_res = Err(AppError{code:AppErrorCode::DataCorruption,
            detail:Some("unit-test".to_string())}); 
    let expect_save_res = Ok(()); 
    let repo = MockOrderRepo::build(expect_save_res, expect_fetch_res);
    let result = StockLevelUseCase::try_edit(init_data, Box::new(repo)).await;
    assert!(result.is_err());
    if let Err(error) = result {
        assert_eq!(error.code, AppErrorCode::DataCorruption);
        if let Some(msg) = error.detail {
            assert_eq!(msg.as_str(), "unit-test");
        }
    }
}

#[tokio::test]
async fn edit_save_error ()
{
    let init_data = ut_setup_data();
    let expect_fetch_res = Ok(StockLevelModelSet{stores:vec![
        StoreStockModel {store_id:init_data[2].store_id, products:vec![
            ProductStockModel {type_:init_data[2].product_type.clone(),
                id_:init_data[2].product_id, expiry: init_data[2].expiry, is_create:false,
                quantity: StockQuantityModel {total: 2, booked: 0, cancelled: 0}}
        ]}
    ]}) ; 
    let expect_save_res = Err(AppError{code:AppErrorCode::DataTableNotExist,
            detail:Some("unit-test".to_string())});
    let repo = MockOrderRepo::build(expect_save_res, expect_fetch_res);
    let result = StockLevelUseCase::try_edit(init_data, Box::new(repo)).await;
    assert!(result.is_err());
    if let Err(error) = result {
        assert_eq!(error.code, AppErrorCode::DataTableNotExist);
        if let Some(msg) = error.detail {
            assert_eq!(msg.as_str(), "unit-test");
        }
    }
}

