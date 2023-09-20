mod product_policy;
mod product_price;

use std::boxed::Box;
use std::sync::Arc;
use std::result::Result;
use std::vec::Vec;
use async_trait::async_trait;

use crate::AppDataStoreContext;
use crate::api::rpc::dto::ProductPriceDeleteDto;
use crate::error::AppError;
use crate::model::{ProductPolicyModelSet, ProductPriceModelSet};

// make it visible only for testing purpose
pub use self::product_policy::ProductPolicyInMemRepo;
pub use self::product_price::ProductPriceInMemRepo;

// the repository instance may be used across an await,
// the future created by app callers has to be able to pass to different threads
// , it is the reason to add `Send` and `Sync` as super-traits
#[async_trait]
pub trait AbstProductPolicyRepo : Sync + Send
{
    fn new(dstore:Arc<AppDataStoreContext>) -> Result<Box<dyn AbstProductPolicyRepo>, AppError>
        where Self:Sized ;
    
    async fn fetch(&self, usr_id:u32, ids:Vec<u64>) -> Result<ProductPolicyModelSet, AppError>;
    
    async fn save(&self, ppset:ProductPolicyModelSet) -> Result<(), AppError>;
    // TODO, delete operation
}
#[async_trait]

pub trait AbsProductPriceRepo : Sync + Send
{
    fn new(dstore:Arc<AppDataStoreContext>) -> Result<Box<dyn AbsProductPriceRepo>, AppError>
        where Self:Sized ;
    async fn delete_all(&self, store_id:u32) -> Result<(), AppError>;
    async fn delete(&self, store_id:u32, ids:ProductPriceDeleteDto) -> Result<(), AppError> ;
    async fn fetch(&self, store_id:u32, ids:Vec<(u8,u64)>) -> Result<ProductPriceModelSet, AppError> ;
    async fn save(&self, updated:ProductPriceModelSet) -> Result<(), AppError> ;
}

// TODO, consider runtime configuration for following repositories

pub fn app_repo_product_policy (ds:Arc<AppDataStoreContext>)
    -> Result<Box<dyn AbstProductPolicyRepo>, AppError>
{
    ProductPolicyInMemRepo::new(ds)
}

pub fn app_repo_product_price (ds:Arc<AppDataStoreContext>)
    -> Result<Box<dyn AbsProductPriceRepo>, AppError>
{
    ProductPriceInMemRepo::new(ds)
}
