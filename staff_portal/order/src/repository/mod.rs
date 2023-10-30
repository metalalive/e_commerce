mod product_policy;
mod product_price;
mod order;

use std::boxed::Box;
use std::sync::Arc;
use std::vec::Vec;
use std::result::Result as DefaultResult;
use async_trait::async_trait;
use uuid::Uuid;

use crate::AppDataStoreContext;
use crate::api::dto::OrderLinePayDto;
use crate::api::rpc::dto::ProductPriceDeleteDto;
use crate::api::web::dto::OrderLineCreateErrorDto;
use crate::constant::ProductType;
use crate::error::AppError;
use crate::model::{
    ProductPolicyModelSet, ProductPriceModelSet, StockLevelModelSet, ProductStockIdentity,
    BillingModel, OrderLineModel, ShippingModel
};

// make it visible only for testing purpose
pub use self::order::OrderInMemRepo;
pub use self::product_policy::ProductPolicyInMemRepo;
pub use self::product_price::ProductPriceInMemRepo;

// the repository instance may be used across an await,
// the future created by app callers has to be able to pass to different threads
// , it is the reason to add `Send` and `Sync` as super-traits
#[async_trait]
pub trait AbstProductPolicyRepo : Sync + Send
{
    async fn new(dstore:Arc<AppDataStoreContext>) -> DefaultResult<Box<dyn AbstProductPolicyRepo>, AppError>
        where Self:Sized ;
    
    async fn fetch(&self, ids:Vec<(ProductType, u64)>) -> DefaultResult<ProductPolicyModelSet, AppError>;
    
    async fn save(&self, ppset:ProductPolicyModelSet) -> DefaultResult<(), AppError>;
    // TODO, delete operation
}

#[async_trait]
pub trait AbsProductPriceRepo : Sync + Send
{
    async fn new(dstore:Arc<AppDataStoreContext>) -> DefaultResult<Box<dyn AbsProductPriceRepo>, AppError>
        where Self:Sized ;
    async fn delete_all(&self, store_id:u32) -> DefaultResult<(), AppError>;
    async fn delete(&self, store_id:u32, ids:ProductPriceDeleteDto) -> DefaultResult<(), AppError> ;
    async fn fetch(&self, store_id:u32, ids:Vec<(ProductType,u64)>) -> DefaultResult<ProductPriceModelSet, AppError> ;
    // fetch prices of products from different sellers  at a time, the
    // first element of the `ids` tuple should be valid seller ID
    async fn fetch_many(&self, ids:Vec<(u32,ProductType,u64)>) -> DefaultResult<Vec<ProductPriceModelSet>, AppError> ;
    async fn save(&self, updated:ProductPriceModelSet) -> DefaultResult<(), AppError> ;
}


#[async_trait]
pub trait AbsOrderRepo : Sync + Send {
    async fn new(ds:Arc<AppDataStoreContext>) -> DefaultResult<Box<dyn AbsOrderRepo>, AppError>
        where Self:Sized;

    fn stock(&self) -> Arc<Box<dyn AbsOrderStockRepo>>;
    
    async fn create (&self, oid:Uuid, usr_id:u32, lines:Vec<OrderLineModel>,
                     bl:BillingModel, sh:ShippingModel)
        -> DefaultResult<(String, Vec<OrderLinePayDto>), AppError> ;

    async fn fetch_olines(&self, oid:String) -> DefaultResult<(u32, Vec<OrderLineModel>), AppError>;
}

pub type AppStockRepoReserveReturn = DefaultResult<(), DefaultResult<Vec<OrderLineCreateErrorDto>, AppError>>;
pub type AppStockRepoReserveUserFunc = fn(&mut StockLevelModelSet, &Vec<OrderLineModel>)
    -> AppStockRepoReserveReturn;

#[async_trait]
pub trait AbsOrderStockRepo : Sync +  Send {
    async fn fetch(&self, pids:Vec<ProductStockIdentity>) -> DefaultResult<StockLevelModelSet, AppError>;
    async fn save(&self, slset:StockLevelModelSet) -> DefaultResult<(), AppError>;
    async fn try_reserve(&self, cb: AppStockRepoReserveUserFunc,
                         order_req: &Vec<OrderLineModel>) -> AppStockRepoReserveReturn;
}

// TODO, consider runtime configuration for following repositories

pub async fn app_repo_product_policy (ds:Arc<AppDataStoreContext>)
    -> DefaultResult<Box<dyn AbstProductPolicyRepo>, AppError>
{
    ProductPolicyInMemRepo::new(ds).await
}
pub async fn app_repo_product_price (ds:Arc<AppDataStoreContext>)
    -> DefaultResult<Box<dyn AbsProductPriceRepo>, AppError>
{
    ProductPriceInMemRepo::new(ds).await
}
pub async fn app_repo_order (ds:Arc<AppDataStoreContext>)
    -> DefaultResult<Box<dyn AbsOrderRepo>, AppError>
{
    OrderInMemRepo::new(ds).await
}
