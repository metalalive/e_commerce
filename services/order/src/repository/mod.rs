use async_trait::async_trait;
use chrono::{DateTime, FixedOffset, Local as LocalTime};
use std::boxed::Box;
use std::future::Future;
use std::pin::Pin;
use std::result::Result as DefaultResult;
use std::sync::Arc;
use std::vec::Vec;

use ecommerce_common::api::dto::CurrencyDto;
use ecommerce_common::api::rpc::dto::{
    OrderLinePayUpdateErrorDto, OrderPaymentUpdateDto, OrderPaymentUpdateErrorDto,
};
use ecommerce_common::constant::ProductType;
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::model::order::BillingModel;
use ecommerce_common::model::BaseProductIdentity;

use crate::api::rpc::dto::{ProductPriceDeleteDto, StockLevelReturnDto, StockReturnErrorDto};
use crate::api::web::dto::OrderLineCreateErrorDto;
use crate::error::AppError;
use crate::model::{
    CartModel, CurrencyModelSet, OrderCurrencyModel, OrderLineIdentity, OrderLineModel,
    OrderLineModelSet, OrderReturnModel, ProductPolicyModelSet, ProductPriceModelSet,
    ProductStockIdentity, ShippingModel, StockLevelModelSet,
};
use crate::AppDataStoreContext;

mod in_mem;
// make in-memory repo visible only for testing purpose
pub use in_mem::cart::CartInMemRepo;
pub use in_mem::currency::CurrencyInMemRepo;
pub use in_mem::oline_return::OrderReturnInMemRepo;
pub use in_mem::order::OrderInMemRepo;
pub use in_mem::product_policy::ProductPolicyInMemRepo;
pub use in_mem::product_price::ProductPriceInMemRepo;

#[cfg(feature = "mariadb")]
mod mariadb;

#[cfg(feature = "mariadb")]
use mariadb::product_policy::ProductPolicyMariaDbRepo;

#[cfg(feature = "mariadb")]
use mariadb::product_price::ProductPriceMariaDbRepo;

#[cfg(feature = "mariadb")]
use mariadb::currency::CurrencyMariaDbRepo;

#[cfg(feature = "mariadb")]
use mariadb::order::OrderMariaDbRepo;

#[cfg(feature = "mariadb")]
use mariadb::oline_return::OrderReturnMariaDbRepo;

#[cfg(feature = "mariadb")]
use mariadb::cart::CartMariaDbRepo;

// the repository instance may be used across an await,
// the future created by app callers has to be able to pass to different threads
// , it is the reason to add `Send` and `Sync` as super-traits
#[async_trait]
pub trait AbstProductPolicyRepo: Sync + Send {
    async fn fetch(
        &self,
        ids: Vec<(ProductType, u64)>,
    ) -> DefaultResult<ProductPolicyModelSet, AppError>;
    async fn save(&self, ppset: ProductPolicyModelSet) -> DefaultResult<(), AppError>;
    // TODO, delete operation
}

#[async_trait]
pub trait AbsProductPriceRepo: Sync + Send {
    async fn delete_all(&self, store_id: u32) -> DefaultResult<(), AppError>;
    async fn delete(
        &self,
        store_id: u32,
        ids: ProductPriceDeleteDto,
    ) -> DefaultResult<(), AppError>;
    async fn fetch(
        &self,
        store_id: u32,
        ids: Vec<(ProductType, u64)>,
    ) -> DefaultResult<ProductPriceModelSet, AppError>;
    // fetch prices of products from different sellers  at a time, the
    // first element of the `ids` tuple should be valid seller ID
    // TODO, switch argumen type to `crate::model::BaseProductIdentity`
    async fn fetch_many(
        &self,
        ids: Vec<(u32, ProductType, u64)>,
    ) -> DefaultResult<Vec<ProductPriceModelSet>, AppError>;
    async fn save(&self, updated: ProductPriceModelSet) -> DefaultResult<(), AppError>;
}

/// Note:
/// in this project the base currency is always USD due to the constraint of 3rd party exchange
/// rate service I apply, it is free plan and not allow to change base currency, this should not
/// be huge problem since the application can convert the rate between different specific
/// currencies.
#[async_trait]
pub trait AbsCurrencyRepo: Sync + Send {
    async fn fetch(&self, chosen: Vec<CurrencyDto>) -> DefaultResult<CurrencyModelSet, AppError>;

    async fn save(&self, ms: CurrencyModelSet) -> DefaultResult<(), AppError>;
}

#[async_trait]
pub trait AbsOrderRepo: Sync + Send {
    fn stock(&self) -> Arc<Box<dyn AbsOrderStockRepo>>;

    async fn save_contact(
        &self,
        oid: &str,
        bl: BillingModel,
        sh: ShippingModel,
    ) -> DefaultResult<(), AppError>;

    async fn fetch_all_lines(&self, oid: String) -> DefaultResult<Vec<OrderLineModel>, AppError>;

    async fn fetch_billing(&self, oid: String) -> DefaultResult<BillingModel, AppError>;

    async fn fetch_shipping(&self, oid: String) -> DefaultResult<ShippingModel, AppError>;

    async fn update_lines_payment(
        &self,
        data: OrderPaymentUpdateDto,
        cb: AppOrderRepoUpdateLinesUserFunc,
    ) -> DefaultResult<OrderPaymentUpdateErrorDto, AppError>;

    async fn fetch_lines_by_rsvtime(
        &self,
        time_start: DateTime<FixedOffset>,
        time_end: DateTime<FixedOffset>,
        usr_cb: AppOrderFetchRangeCallback,
    ) -> DefaultResult<(), AppError>;

    async fn fetch_lines_by_pid(
        &self,
        oid: &str,
        pids: Vec<OrderLineIdentity>,
    ) -> DefaultResult<Vec<OrderLineModel>, AppError>;

    async fn fetch_ids_by_created_time(
        &self,
        start: DateTime<FixedOffset>,
        end: DateTime<FixedOffset>,
    ) -> DefaultResult<Vec<String>, AppError>;

    async fn owner_id(&self, order_id: &str) -> DefaultResult<u32, AppError>;
    async fn created_time(&self, order_id: &str) -> DefaultResult<DateTime<FixedOffset>, AppError>;

    async fn currency_exrates(&self, oid: &str) -> DefaultResult<OrderCurrencyModel, AppError>;

    async fn cancel_unpaid_last_time(&self) -> DefaultResult<DateTime<FixedOffset>, AppError>;
    async fn cancel_unpaid_time_update(&self) -> DefaultResult<(), AppError>;
} // end of trait AbsOrderRepo

pub type AppOrderRepoUpdateLinesUserFunc =
    fn(&mut Vec<OrderLineModel>, OrderPaymentUpdateDto) -> Vec<OrderLinePayUpdateErrorDto>;

// declare a callback function type which can easily be passed,
// - I made the return type to be `Future` trait object wrapped in `Pin` type
//   because `Future` (generated by async block expression) does not implement `Unpin` trait,
//   that means the `Future`  bobject cannot be moved to different memory locations once
//   generated.
// - the placeholder lifetime `'_` specified in the `Future` trait object will elide
//   lifetime check in this module, not sure how Rust compiler processes this under the
//   hood, but it looks like the lifetime check will be done in given / external callback
//   function signature
pub type AppOrderFetchRangeCallback =
    fn(
        &dyn AbsOrderRepo,
        OrderLineModelSet,
    ) -> Pin<Box<dyn Future<Output = DefaultResult<(), AppError>> + Send + '_>>;

pub type AppStockRepoReserveReturn =
    DefaultResult<(), DefaultResult<Vec<OrderLineCreateErrorDto>, AppError>>;

pub type AppStockRepoReserveUserFunc =
    fn(&mut StockLevelModelSet, &OrderLineModelSet) -> AppStockRepoReserveReturn;

// if the function pointer type is declared directly in function signature of a
// trait method, the function pointer will be viewed as closure block
pub type AppStockRepoReturnUserFunc =
    fn(&mut StockLevelModelSet, StockLevelReturnDto) -> Vec<StockReturnErrorDto>;

#[async_trait]
pub trait AbsOrderStockRepo: Sync + Send {
    async fn fetch(
        &self,
        pids: Vec<ProductStockIdentity>,
    ) -> DefaultResult<StockLevelModelSet, AppError>;
    async fn save(&self, slset: StockLevelModelSet) -> DefaultResult<(), AppError>;
    async fn try_reserve(
        &self,
        cb: AppStockRepoReserveUserFunc,
        order_req: &OrderLineModelSet,
    ) -> AppStockRepoReserveReturn;
    async fn try_return(
        &self,
        cb: AppStockRepoReturnUserFunc,
        data: StockLevelReturnDto,
    ) -> DefaultResult<Vec<StockReturnErrorDto>, AppError>;
}

#[async_trait]
pub trait AbsOrderReturnRepo: Sync + Send {
    async fn fetch_by_pid(
        &self,
        oid: &str,
        pids: Vec<OrderLineIdentity>,
    ) -> DefaultResult<Vec<OrderReturnModel>, AppError>;

    /// return list of tuples that contain order-id string and corresponding return model
    async fn fetch_by_created_time(
        &self,
        start: DateTime<FixedOffset>,
        end: DateTime<FixedOffset>,
    ) -> DefaultResult<Vec<(String, OrderReturnModel)>, AppError>;

    // TODO, no production code refers to this function, consider to remove
    async fn fetch_by_oid_ctime(
        &self,
        oid: &str,
        start: DateTime<FixedOffset>,
        end: DateTime<FixedOffset>,
    ) -> DefaultResult<Vec<OrderReturnModel>, AppError>;

    async fn create(
        &self,
        oid: &str,
        reqs: Vec<OrderReturnModel>,
    ) -> DefaultResult<usize, AppError>;
}

#[async_trait]
pub trait AbsCartRepo: Sync + Send {
    async fn update(&self, obj: CartModel) -> DefaultResult<usize, AppError>;

    async fn discard(&self, owner: u32, seq: u8) -> DefaultResult<(), AppError>;

    async fn num_lines_saved(&self, owner: u32, seq: u8) -> DefaultResult<usize, AppError>;

    async fn fetch_cart(&self, owner: u32, seq: u8) -> DefaultResult<CartModel, AppError>;

    async fn fetch_lines_by_pid(
        &self,
        owner: u32,
        seq: u8,
        pids: Vec<BaseProductIdentity>,
    ) -> DefaultResult<CartModel, AppError>;
}

pub async fn app_repo_product_policy(
    ds: Arc<AppDataStoreContext>,
) -> DefaultResult<Box<dyn AbstProductPolicyRepo>, AppError> {
    #[cfg(feature = "mariadb")]
    if let Some(dbs) = ds.sql_dbs.as_ref() {
        let obj = ProductPolicyMariaDbRepo::new(dbs).await?;
        Ok(Box::new(obj))
    } else {
        Err(AppError {
            code: AppErrorCode::FeatureDisabled,
            detail: Some("mariadb".to_string()),
        })
    }
    #[cfg(not(feature = "mariadb"))]
    if let Some(m) = ds.in_mem.as_ref() {
        let obj = ProductPolicyInMemRepo::new(m.clone()).await?;
        Ok(Box::new(obj))
    } else {
        Err(AppError {
            code: AppErrorCode::MissingDataStore,
            detail: Some("unknwon-type".to_string()),
        })
    }
}

pub async fn app_repo_product_price(
    ds: Arc<AppDataStoreContext>,
) -> DefaultResult<Box<dyn AbsProductPriceRepo>, AppError> {
    #[cfg(feature = "mariadb")]
    if let Some(dbs) = ds.sql_dbs.as_ref() {
        let obj = ProductPriceMariaDbRepo::new(dbs)?;
        Ok(Box::new(obj))
    } else {
        Err(AppError {
            code: AppErrorCode::FeatureDisabled,
            detail: Some("mariadb".to_string()),
        })
    }
    #[cfg(not(feature = "mariadb"))]
    if let Some(m) = ds.in_mem.as_ref() {
        let obj = ProductPriceInMemRepo::new(m.clone()).await?;
        Ok(Box::new(obj))
    } else {
        Err(AppError {
            code: AppErrorCode::MissingDataStore,
            detail: Some("unknwon-type".to_string()),
        })
    }
}

pub async fn app_repo_currency(
    ds: Arc<AppDataStoreContext>,
) -> DefaultResult<Box<dyn AbsCurrencyRepo>, AppError> {
    #[cfg(feature = "mariadb")]
    if let Some(dbs) = ds.sql_dbs.as_ref() {
        let obj = CurrencyMariaDbRepo::try_build(dbs)?;
        Ok(Box::new(obj))
    } else {
        Err(AppError {
            code: AppErrorCode::FeatureDisabled,
            detail: Some("mariadb".to_string()),
        })
    }
    #[cfg(not(feature = "mariadb"))]
    if let Some(m) = ds.in_mem.as_ref() {
        let obj = CurrencyInMemRepo::new(m.clone()).await?;
        Ok(Box::new(obj))
    } else {
        Err(AppError {
            code: AppErrorCode::MissingDataStore,
            detail: Some("unknwon-type".to_string()),
        })
    }
} // end of fn app_repo_currency

pub async fn app_repo_order(
    ds: Arc<AppDataStoreContext>,
) -> DefaultResult<Box<dyn AbsOrderRepo>, AppError> {
    let timenow = LocalTime::now().fixed_offset();
    #[cfg(feature = "mariadb")]
    if let Some(dbs) = ds.sql_dbs.as_ref() {
        let obj = OrderMariaDbRepo::new(dbs.clone(), timenow).await?;
        Ok(Box::new(obj))
    } else {
        Err(AppError {
            code: AppErrorCode::FeatureDisabled,
            detail: Some("mariadb".to_string()),
        })
    }
    #[cfg(not(feature = "mariadb"))]
    if let Some(m) = &ds.in_mem {
        let obj = OrderInMemRepo::new(m.clone(), timenow).await?;
        Ok(Box::new(obj))
    } else {
        Err(AppError {
            code: AppErrorCode::MissingDataStore,
            detail: Some("unknown-type".to_string()),
        })
    }
}
pub async fn app_repo_order_return(
    ds: Arc<AppDataStoreContext>,
) -> DefaultResult<Box<dyn AbsOrderReturnRepo>, AppError> {
    #[cfg(feature = "mariadb")]
    if let Some(dbs) = ds.sql_dbs.as_ref() {
        let obj = OrderReturnMariaDbRepo::new(dbs.clone()).await?;
        Ok(Box::new(obj))
    } else {
        Err(AppError {
            code: AppErrorCode::FeatureDisabled,
            detail: Some("mariadb".to_string()),
        })
    }
    #[cfg(not(feature = "mariadb"))]
    if let Some(m) = &ds.in_mem {
        let obj = OrderReturnInMemRepo::new(m.clone()).await?;
        Ok(Box::new(obj))
    } else {
        Err(AppError {
            code: AppErrorCode::MissingDataStore,
            detail: Some("unknown-type".to_string()),
        })
    }
}
pub async fn app_repo_cart(
    ds: Arc<AppDataStoreContext>,
) -> DefaultResult<Box<dyn AbsCartRepo>, AppError> {
    #[cfg(feature = "mariadb")]
    if let Some(dbs) = ds.sql_dbs.as_ref() {
        let obj = CartMariaDbRepo::new(dbs.clone()).await?;
        Ok(Box::new(obj))
    } else {
        Err(AppError {
            code: AppErrorCode::FeatureDisabled,
            detail: Some("mariadb".to_string()),
        })
    }
    #[cfg(not(feature = "mariadb"))]
    if let Some(m) = &ds.in_mem {
        let obj = CartInMemRepo::new(m.clone()).await?;
        Ok(Box::new(obj))
    } else {
        Err(AppError {
            code: AppErrorCode::MissingDataStore,
            detail: Some("unknown-type".to_string()),
        })
    }
}
