use std::cmp::min;
use std::sync::Arc;
use std::boxed::Box;
use std::vec::Vec;
use std::result::Result as DefaultResult;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use sqlx::{Transaction, MySql, Arguments, IntoArguments};
use sqlx::mysql::MySqlArguments;

use crate::api::rpc::dto::{OrderPaymentUpdateDto, OrderPaymentUpdateErrorDto};
use crate::constant as AppConst;
use crate::datastore::AppMariaDbStore;
use crate::error::{AppError, AppErrorCode};
use crate::model::{
    BillingModel, ShippingModel, OrderLineModelSet, OrderLineModel, OrderLineIdentity
};
use crate::repository::{
    AbsOrderRepo, AbsOrderStockRepo, AppOrderRepoUpdateLinesUserFunc, AppOrderFetchRangeCallback
};

use super::{run_query_once, hex_to_bytes};
use super::stock::StockMariaDbRepo;

struct InsertTopMetaArg<'a, 'b>(&'a Vec<u8>, u32, &'b DateTime<FixedOffset>);
struct InsertOLineArg<'a, 'b>(&'a Vec<u8>, usize, Vec<&'b OrderLineModel>);

impl<'a, 'b> Into<(String, MySqlArguments)> for InsertTopMetaArg<'a, 'b>
{
    fn into(self) -> (String, MySqlArguments) {
        let patt = "INSERT INTO `order_toplvl_meta`(`usr_id`,`o_id`,\
                    `created_time`) VALUES (?,?,?)";
        let ctime_utc = self.2.clone().naive_utc();
        let mut args = MySqlArguments::default();
        args.add(self.1);
        args.add(self.0);
        args.add(ctime_utc);
        (patt.to_string(), args)
    }
}
impl<'a, 'b> InsertOLineArg<'a, 'b>
{
    fn sql_pattern(num_batch:usize) -> String {
        let col_seq = "`o_id`,`seq`,`store_id`,`product_type`,`product_id`,`price_unit`,\
                       `price_total`,`qty_rsved`,`rsved_until`,`warranty_until`";
        let items = (0..num_batch).into_iter().map(|_| {
            "(?,?,?,?,?,?,?,?,?,?)"
        }).collect::<Vec<_>>();
        format!("INSERT INTO `order_line_detail`({}) VALUES {}",
                col_seq, items.join(","))
    }
}
impl<'a, 'b, 'q> IntoArguments<'q, MySql> for InsertOLineArg<'a, 'b> {
    fn into_arguments(self) -> <MySql as sqlx::database::HasArguments<'q>>::Arguments {
        let mut args = MySqlArguments::default();
        let (oid, mut seq, lines) = (self.0, self.1, self.2);
        lines.into_iter().map(|o| {
            let prod_typ_num : u8 = o.id_.product_type.clone().into();
            let rsved_until = o.policy.reserved_until.naive_utc();
            let warranty_until = o.policy.warranty_until.naive_utc();
            args.add(oid);
            args.add(seq as u16); // match the column type in db table
            seq += 1;
            args.add(o.id_.store_id);
            args.add(prod_typ_num.to_string());
            args.add(o.id_.product_id);
            args.add(o.price.unit);
            args.add(o.price.total);
            args.add(o.qty.reserved);
            args.add(rsved_until);
            args.add(warranty_until);
        }).count();
        args
    }
}
impl<'a, 'b> Into<(String, MySqlArguments)> for InsertOLineArg<'a, 'b>
{
    fn into(self) -> (String, MySqlArguments) {
        let num_batch = self.2.len();
        (Self::sql_pattern(num_batch), self.into_arguments())
    }
}


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

    async fn save_contact (&self, _oid:&str, _bl:BillingModel, _sh:ShippingModel)
        -> DefaultResult<(), AppError> 
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
    pub(super) async fn create_lines(
        tx:&mut Transaction<'_, MySql>, ol_set:&OrderLineModelSet,  limit:usize
        ) -> DefaultResult<(), AppError>
    {
        let (oid, usr_id, ctime, olines) = (ol_set.order_id.as_str(), ol_set.owner_id,
                                            &ol_set.create_time , &ol_set.lines);
        if olines.len() > AppConst::limit::MAX_ORDER_LINES_PER_REQUEST {
            let d = format!("actual: {}, limit:{}", olines.len(),
                    AppConst::limit::MAX_ORDER_LINES_PER_REQUEST);
            let e = AppError {code:AppErrorCode::ExceedingMaxLimit, detail:Some(d)};
            return Err(e);
        }
        let oid = hex_to_bytes(oid)?;
        let (sql_patt, args) = InsertTopMetaArg(&oid, usr_id, ctime).into();
        let _rs = run_query_once(tx, sql_patt, args, 1).await?;
        
        let mut num_processed = 0;
        let mut data = olines.iter().collect::<Vec<_>>();
        while !data.is_empty() {
            let num_batch = min(data.len(), limit);
            let items_processing = data.split_off(data.len() - num_batch);
            assert!(items_processing.len() > 0);
            assert_eq!(items_processing.len(), num_batch);
            let (sql_patt, args) = InsertOLineArg(&oid, num_processed, items_processing).into();
            let _rs = run_query_once(tx, sql_patt, args, num_batch).await?;
            num_processed += num_batch;
        } // end of loop
        Ok(())
    } // end of fn create_lines
} // end of impl OrderMariaDbRepo
