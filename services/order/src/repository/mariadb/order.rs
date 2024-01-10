use std::sync::Arc;
use std::boxed::Box;
use std::vec::Vec;
use std::result::Result as DefaultResult;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};

use crate::api::dto::OrderLinePayDto;
use crate::api::rpc::dto::{OrderPaymentUpdateDto, OrderPaymentUpdateErrorDto};
use crate::datastore::AppMariaDbStore;
use crate::error::{AppError, AppErrorCode};
use crate::model::{
    BillingModel, ShippingModel, OrderLineModelSet, OrderLineModel, OrderLineIdentity
};
use crate::repository::{
    AbsOrderRepo, AbsOrderStockRepo, AppOrderRepoUpdateLinesUserFunc, AppOrderFetchRangeCallback
};

use super::stock::StockMariaDbRepo;

pub(crate) struct OrderMariaDbRepo
{
    _db : Arc<AppMariaDbStore>,
    _stock : Arc<Box<dyn AbsOrderStockRepo>>,
}

#[async_trait]
impl AbsOrderRepo for OrderMariaDbRepo
{
    fn stock(&self) -> Arc<Box<dyn AbsOrderStockRepo>>
    { self._stock.clone() }

    async fn create (&self, _lines:OrderLineModelSet, _bl:BillingModel, _sh:ShippingModel)
        -> DefaultResult<Vec<OrderLinePayDto>, AppError> 
    {
        Err(AppError { code: AppErrorCode::NotImplemented, detail: None })
    }
    async fn fetch_all_lines(&self, _oid:String) -> DefaultResult<Vec<OrderLineModel>, AppError>
    {
        Err(AppError { code: AppErrorCode::NotImplemented, detail: None })
    }
    async fn fetch_billing(&self, _oid:String) -> DefaultResult<BillingModel, AppError>
    {
        Err(AppError { code: AppErrorCode::NotImplemented, detail: None })
    }
    async fn fetch_shipping(&self, _oid:String) -> DefaultResult<ShippingModel, AppError>
    {
        Err(AppError { code: AppErrorCode::NotImplemented, detail: None })
    }
    async fn update_lines_payment(&self, _data:OrderPaymentUpdateDto,
                                  _cb:AppOrderRepoUpdateLinesUserFunc)
        -> DefaultResult<OrderPaymentUpdateErrorDto, AppError>
    {
        Err(AppError { code: AppErrorCode::NotImplemented, detail: None })
    }
    async fn fetch_lines_by_rsvtime(&self, _time_start: DateTime<FixedOffset>,
                                  _time_end: DateTime<FixedOffset>,
                                  _usr_cb: AppOrderFetchRangeCallback )
        -> DefaultResult<(), AppError>
    {
        Err(AppError { code: AppErrorCode::NotImplemented, detail: None })
    }
    async fn fetch_lines_by_pid(&self, _oid:&str, _pids:Vec<OrderLineIdentity>)
        -> DefaultResult<Vec<OrderLineModel>, AppError>
    {
        Err(AppError { code: AppErrorCode::NotImplemented, detail: None })
    }
    async fn fetch_ids_by_created_time(&self,  _start: DateTime<FixedOffset>,
                                       _end: DateTime<FixedOffset>)
        -> DefaultResult<Vec<String>, AppError>
    {
        Err(AppError { code: AppErrorCode::NotImplemented, detail: None })
    }
    async fn owner_id(&self, _order_id:&str) -> DefaultResult<u32, AppError>
    {
        Err(AppError { code: AppErrorCode::NotImplemented, detail: None })
    }
    async fn created_time(&self, _order_id:&str) -> DefaultResult<DateTime<FixedOffset>, AppError>
    {
        Err(AppError { code: AppErrorCode::NotImplemented, detail: None })
    }

    // TODO, rename to `cancel_unpaid_last_time()` and `cancel_unpaid_time_update()`
    async fn scheduled_job_last_time(&self) -> DateTime<FixedOffset>
    {
        DateTime::parse_from_rfc3339("1991-05-30T15:22:49.001985+09:30").unwrap()
    }
    async fn scheduled_job_time_update(&self)
    { }
} // end of trait AbsOrderRepo


impl OrderMariaDbRepo {
    pub(crate) async fn new(dbs:Vec<Arc<AppMariaDbStore>>, timenow:DateTime<FixedOffset>)
        -> DefaultResult<Self, AppError>
    {
        if dbs.is_empty() {
            Err(AppError { code: AppErrorCode::MissingDataStore,
                detail: Some(format!("mariadb"))  })
        } else {
            let _db = dbs.first().unwrap().clone();
            let stockrepo = StockMariaDbRepo::new(timenow, _db.clone());
            Ok(Self { _db, _stock: Arc::new(Box::new(stockrepo)) }) 
        }
        // TODO, consider to balance loads of order request to different database servers
        // , currently this repo selects only the first db pool
    }
}
