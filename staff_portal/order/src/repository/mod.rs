mod product_policy;

use std::boxed::Box;
use std::sync::Arc;
use async_trait::async_trait;

use crate::AppDataStoreContext;
use crate::error::AppError;
use crate::model::ProductPolicyModelSet;
// make it visible only for testing purpose
pub use self::product_policy::ProductPolicyInMemRepo;

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

pub fn app_repo_product_policy (ds:Arc<AppDataStoreContext>)
    -> Result<Box<dyn AbstProductPolicyRepo>, AppError>
{ // TODO, consider runtime configuration
    ProductPolicyInMemRepo::new(ds)
}
