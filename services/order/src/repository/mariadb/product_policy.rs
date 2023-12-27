use std::sync::Arc;
use std::vec::Vec;
use std::result::Result as DefaultResult;

use async_trait::async_trait;
use sqlx::MySql;
use sqlx::pool::PoolConnection;

use crate::constant::ProductType;
use crate::datastore::AppMariaDbStore;
use crate::error::{AppError, AppErrorCode};
use crate::model::ProductPolicyModelSet;
use crate::repository::AbstProductPolicyRepo;

pub(crate) struct ProductPolicyMariaDbRepo
{
    conn: PoolConnection<MySql>
}

impl ProductPolicyMariaDbRepo
{ 
    pub async fn new(dbs: & Vec<Arc<AppMariaDbStore>>) -> DefaultResult<Self, AppError>
    {
        if dbs.is_empty() {
            let e = AppError { code: AppErrorCode::MissingDataStore,
                detail: Some(format!("mariadb")) };
            Err(e)
        } else {
            let db = dbs.first().unwrap().clone();
            let conn = db.acquire().await ?;
            Ok(Self {conn})
        } // TODO, figure out how to balance loading when the app data grows
    }
}

#[async_trait]
impl AbstProductPolicyRepo for ProductPolicyMariaDbRepo
{ 
    async fn fetch(&self, ids:Vec<(ProductType, u64)>) -> DefaultResult<ProductPolicyModelSet, AppError>
    {
        Err(AppError { code: AppErrorCode::NotImplemented, detail: None })
    }
    
    async fn save(&self, ppset:ProductPolicyModelSet) -> DefaultResult<(), AppError>
    {
        Err(AppError { code: AppErrorCode::NotImplemented, detail: None })
    }
}
