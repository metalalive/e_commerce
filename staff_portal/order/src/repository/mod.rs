mod product_policy;
mod product_price;
mod order;

use std::boxed::Box;
use std::sync::Arc;
use std::result::Result as DefaultResult;
use std::vec::Vec;
use async_trait::async_trait;

use crate::AppDataStoreContext;
use crate::api::rpc::dto::ProductPriceDeleteDto;
use crate::error::AppError;
use crate::model::{ProductPolicyModelSet, ProductPriceModelSet};

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
    fn new(dstore:Arc<AppDataStoreContext>) -> DefaultResult<Box<dyn AbstProductPolicyRepo>, AppError>
        where Self:Sized ;
    
    async fn fetch(&self, usr_id:u32, ids:Vec<u64>) -> DefaultResult<ProductPolicyModelSet, AppError>;
    
    async fn save(&self, ppset:ProductPolicyModelSet) -> DefaultResult<(), AppError>;
    // TODO, delete operation
}

#[async_trait]
pub trait AbsProductPriceRepo : Sync + Send
{
    fn new(dstore:Arc<AppDataStoreContext>) -> DefaultResult<Box<dyn AbsProductPriceRepo>, AppError>
        where Self:Sized ;
    async fn delete_all(&self, store_id:u32) -> DefaultResult<(), AppError>;
    async fn delete(&self, store_id:u32, ids:ProductPriceDeleteDto) -> DefaultResult<(), AppError> ;
    async fn fetch(&self, store_id:u32, ids:Vec<(u8,u64)>) -> DefaultResult<ProductPriceModelSet, AppError> ;
    async fn save(&self, updated:ProductPriceModelSet) -> DefaultResult<(), AppError> ;
}

pub trait AbsOrderRepo : Send + Sync {
    fn new(ds:Arc<AppDataStoreContext>) -> DefaultResult<Box<dyn AbsOrderRepo>, AppError>
        where Self:Sized;
}

// TODO, consider runtime configuration for following repositories

pub fn app_repo_product_policy (ds:Arc<AppDataStoreContext>)
    -> DefaultResult<Box<dyn AbstProductPolicyRepo>, AppError>
{
    ProductPolicyInMemRepo::new(ds)
}
pub fn app_repo_product_price (ds:Arc<AppDataStoreContext>)
    -> DefaultResult<Box<dyn AbsProductPriceRepo>, AppError>
{
    ProductPriceInMemRepo::new(ds)
}
pub fn app_repo_order(ds:Arc<AppDataStoreContext>)
    -> DefaultResult<Box<dyn AbsOrderRepo>, AppError>
{
    OrderInMemRepo::new(ds)
}
