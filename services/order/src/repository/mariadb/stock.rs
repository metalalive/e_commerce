use std::sync::Arc;
use std::vec::Vec;
use std::result::Result as DefaultResult;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};

use crate::api::rpc::dto::{StockLevelReturnDto, StockReturnErrorDto};
use crate::datastore::AppMariaDbStore;
use crate::error::{AppError, AppErrorCode};
use crate::model::{ProductStockIdentity, StockLevelModelSet, OrderLineModelSet};
use crate::repository::{
    AbsOrderStockRepo, AppStockRepoReserveUserFunc, AppStockRepoReserveReturn, AppStockRepoReturnUserFunc
};

pub(super) struct StockMariaDbRepo
{
    _time_now : DateTime<FixedOffset>,
    _db : Arc<AppMariaDbStore>,
}

#[async_trait]
impl AbsOrderStockRepo for StockMariaDbRepo
{
    async fn fetch(&self, _pids:Vec<ProductStockIdentity>) -> DefaultResult<StockLevelModelSet, AppError>
    {
        Err(AppError { code: AppErrorCode::NotImplemented, detail: None })
    }
    async fn save(&self, slset:StockLevelModelSet) -> DefaultResult<(), AppError>
    {
        Err(AppError { code: AppErrorCode::NotImplemented, detail: None })
    }
    async fn try_reserve(&self, _cb: AppStockRepoReserveUserFunc,
                         _order_req: &OrderLineModelSet) -> AppStockRepoReserveReturn
    { // TODO, figure out how to send `sqlx` transaction object between tasks
        let e = AppError { code: AppErrorCode::NotImplemented, detail: None };
        Err(Err(e))
    }
    async fn try_return(&self,  _cb: AppStockRepoReturnUserFunc,
                        _data: StockLevelReturnDto )
        -> DefaultResult<Vec<StockReturnErrorDto>, AppError>
    {
        Err(AppError { code: AppErrorCode::NotImplemented, detail: None })
    }
} // end of impl AbsOrderStockRepo for StockMariaDbRepo

impl StockMariaDbRepo {
    pub(crate) fn new (time_now: DateTime<FixedOffset>, _db: Arc<AppMariaDbStore>)
        -> Self
    {
        Self { _time_now: time_now, _db }
    }
}
