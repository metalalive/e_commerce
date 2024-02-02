use std::cmp::min;
use std::sync::Arc;
use std::boxed::Box;
use std::vec::Vec;
use std::result::Result as DefaultResult;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset, NaiveDateTime};
use futures_util::stream::StreamExt;
use sqlx::pool::PoolConnection;
use sqlx::{Transaction, MySql, Arguments, IntoArguments, Executor, Statement, Row, Connection};
use sqlx::mysql::{MySqlArguments, MySqlRow};

use crate::api::dto::{PhoneNumberDto, CountryCode, ShippingMethod};
use crate::api::rpc::dto::{OrderPaymentUpdateDto, OrderPaymentUpdateErrorDto};
use crate::constant::{self as AppConst, ProductType};
use crate::datastore::AppMariaDbStore;
use crate::error::{AppError, AppErrorCode};
use crate::model::{
    BillingModel, ShippingModel, OrderLineModelSet, OrderLineModel, OrderLineIdentity,
    OrderLinePriceModel, OrderLineQuantityModel, OrderLineAppliedPolicyModel, ContactModel,
    PhyAddrModel, ShippingOptionModel
};
use crate::repository::{
    AbsOrderRepo, AbsOrderStockRepo, AppOrderRepoUpdateLinesUserFunc, AppOrderFetchRangeCallback
};

use super::{run_query_once, OidBytes};
use super::stock::StockMariaDbRepo;

struct InsertTopMetaArg<'a, 'b>(&'a OidBytes, u32, &'b DateTime<FixedOffset>);
struct InsertOLineArg<'a, 'b>(&'a OidBytes, usize, Vec<&'b OrderLineModel>);
struct InsertContactMeta<'a, 'b>(&'a str, &'b OidBytes, String, String);
struct InsertContactEmail<'a, 'b>(&'a str, &'b OidBytes, Vec<String>);
struct InsertContactPhone<'a, 'b>(&'a str, &'b OidBytes, Vec<PhoneNumberDto>);
struct InsertPhyAddr<'a, 'b>(&'a str, &'b OidBytes, PhyAddrModel);
struct InsertShipOption<'a>(&'a OidBytes, Vec<ShippingOptionModel>);

struct UpdateOLinePayArg<'a>(&'a OidBytes, Vec<OrderLineModel>);

struct FetchAllLinesArg(OidBytes);
struct FetchLineByIdArg<'a>(&'a OidBytes, Vec<OrderLineIdentity>);

struct OLineRow(MySqlRow);
struct EmailRow(MySqlRow);
struct PhoneRow(MySqlRow);
struct ContactMetaRow(MySqlRow);
struct PhyAddrrRow(MySqlRow);
struct ShipOptionRow(MySqlRow);

impl<'a, 'b> Into<(String, MySqlArguments)> for InsertTopMetaArg<'a, 'b>
{
    fn into(self) -> (String, MySqlArguments) {
        let patt = "INSERT INTO `order_toplvl_meta`(`usr_id`,`o_id`,\
                    `created_time`) VALUES (?,?,?)";
        let ctime_utc = self.2.clone().naive_utc();
        let mut args = MySqlArguments::default();
        let OidBytes(oid) = self.0;
        args.add(self.1);
        args.add(oid.to_vec());
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
        let (OidBytes(oid), mut seq, lines) = (self.0 , self.1, self.2);
        lines.into_iter().map(|o| {
            let prod_typ_num : u8 = o.id_.product_type.clone().into();
            let rsved_until = o.policy.reserved_until.naive_utc();
            let warranty_until = o.policy.warranty_until.naive_utc();
            args.add(oid.to_vec());
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
impl<'a, 'b> Into<(String, MySqlArguments)> for InsertContactMeta<'a, 'b>
{
    fn into(self) -> (String, MySqlArguments) {
        let (table_opt, OidBytes(oid), first_name, last_name) = (self.0, self.1, self.2, self.3);
        let patt = format!("INSERT INTO `{}_contact_meta`(`o_id`,`first_name`,`last_name`) \
                           VALUES (?,?,?)", table_opt);
        let mut args = MySqlArguments::default();
        args.add(oid.to_vec());
        args.add(first_name);
        args.add(last_name);
        (patt, args)
    }
}
impl<'a, 'b> InsertContactEmail<'a, 'b> {
    fn sql_pattern(&self) -> String {
        let (table_opt, num_batch) = (self.0, self.2.len());
        assert!(num_batch > 0);
        let items = (0..num_batch).into_iter().map(|_num| "(?,?,?)").collect::<Vec<_>>();
        format!("INSERT INTO `{}_contact_email`(`o_id`,`seq`,`mail`) VALUES {}",
                table_opt, items.join(","))
    }
}
impl <'a, 'b, 'q> IntoArguments<'q, MySql> for InsertContactEmail<'a, 'b>
{
    fn into_arguments(self) -> <MySql as sqlx::database::HasArguments<'q>>::Arguments {
        let (OidBytes(oid), mails, mut seq) = (self.1, self.2, 0u16);
        let oid = oid.to_vec();
        let mut args = MySqlArguments::default();
        mails.into_iter().map(|mail| {
            args.add(&oid);
            args.add(seq);
            args.add(mail);
            seq += 1;
        }).count();
        args
    }
}
impl<'a, 'b> Into<(String, MySqlArguments)> for InsertContactEmail<'a, 'b>
{
    fn into(self) -> (String, MySqlArguments) {
        (self.sql_pattern(), self.into_arguments())
    }
}
impl<'a, 'b> InsertContactPhone<'a, 'b> {
    fn sql_pattern(&self) -> String {
        let (table_opt, num_batch) = (self.0, self.2.len());
        assert!(num_batch > 0);
        let items = (0..num_batch).into_iter().map(|_num| "(?,?,?,?)").collect::<Vec<_>>();
        format!("INSERT INTO `{}_contact_phone`(`o_id`,`seq`,`nation`,`number`) VALUES {}",
                table_opt, items.join(","))
    }
}
impl <'a, 'b, 'q> IntoArguments<'q, MySql> for InsertContactPhone<'a, 'b>
{
    fn into_arguments(self) -> <MySql as sqlx::database::HasArguments<'q>>::Arguments {
        let (OidBytes(oid), phones, mut seq) = (self.1, self.2, 0u16);
        let oid = oid.to_vec();
        let mut args = MySqlArguments::default();
        phones.into_iter().map(|phone| {
            args.add(&oid);
            args.add(seq);
            args.add(phone.nation);
            args.add(phone.number);
            seq += 1;
        }).count();
        args
    }
}
impl<'a, 'b> Into<(String, MySqlArguments)> for InsertContactPhone<'a, 'b>
{
    fn into(self) -> (String, MySqlArguments) {
        (self.sql_pattern(), self.into_arguments())
    }
}
impl<'a, 'b> Into<(String, MySqlArguments)> for InsertPhyAddr<'a, 'b>
{
    fn into(self) -> (String, MySqlArguments) {
        let (table_opt, OidBytes(oid), addr) = (self.0, self.1, self.2);
        let patt = format!("INSERT INTO `{}_phyaddr`(`o_id`,`country`,`region`,`city`,\
                   `distinct`,`street`,`detail`) VALUES (?,?,?,?,?,?,?)", table_opt);
        let country:String = addr.country.into();
        let mut args = MySqlArguments::default();
        args.add(oid.to_vec());
        args.add(country);
        args.add(addr.region);
        args.add(addr.city);
        args.add(addr.distinct);
        args.add(addr.street_name);
        args.add(addr.detail);
        (patt, args)
    }
}
impl<'a> InsertShipOption<'a> {
    fn sql_pattern(num_batch:usize) -> String {
        let items = (0..num_batch).into_iter().map(|_num| "(?,?,?)").collect::<Vec<_>>();
        format!("INSERT INTO `ship_option`(`o_id`,`seller_id`,`method`) VALUES {}",
                items.join(","))
    }
}
impl <'a, 'q> IntoArguments<'q, MySql> for InsertShipOption<'a>
{
    fn into_arguments(self) -> <MySql as sqlx::database::HasArguments<'q>>::Arguments {
        let (OidBytes(oid), options) = (self.0, self.1);
        let oid = oid.to_vec();
        let mut args = MySqlArguments::default();
        options.into_iter().map(|so| {
            let method:String = so.method.into();
            args.add(&oid);
            args.add(so.seller_id);
            args.add(method);
        }).count();
        args
    }
}
impl<'a> Into<(String, MySqlArguments)> for InsertShipOption<'a>
{
    fn into(self) -> (String, MySqlArguments) {
        let num_batch = self.1.len();
        assert!(num_batch > 0);
        (Self::sql_pattern(num_batch), self.into_arguments())
    }
}

impl<'a> UpdateOLinePayArg<'a>
{
    fn sql_pattern(num_batch:usize) -> String {
        let condition = "(`store_id`=? AND `product_type`=? AND `product_id`=?)";
        let case_ops = (0..num_batch).into_iter().map(
            |_| ["WHEN", condition, "THEN", "?"]
        ).flatten().collect::<Vec<_>>().join(" ");
        let where_ops = (0..num_batch).into_iter().map(
            |_| condition
        ).collect::<Vec<_>>().join("OR") ;
        let portions = [
            format!("`qty_paid` = CASE {case_ops} ELSE `qty_paid` END"),
            format!("`qty_paid_last_update` = CASE {case_ops} ELSE `qty_paid_last_update` END"),
        ];
        format!("UPDATE `order_line_detail` SET {}, {} WHERE `o_id`=? AND ({})"
                , portions[0], portions[1], where_ops)
    }
}
impl<'a,'q> IntoArguments<'q, MySql> for UpdateOLinePayArg<'a>
{
    fn into_arguments(self) -> <MySql as sqlx::database::HasArguments<'q>>::Arguments
    { 
        let (OidBytes(oid), lines) = (self.0, self.1);
        let mut args = MySqlArguments::default();
        lines.iter().map(|line| {
            let prod_typ_num: u8 = line.id_.product_type.clone().into();
            args.add(line.id_.store_id);
            args.add(prod_typ_num.to_string());
            args.add(line.id_.product_id);
            args.add(line.qty.paid);
        }).count();
        lines.iter().map(|line| {
            let prod_typ_num: u8 = line.id_.product_type.clone().into();
            let time = line.qty.paid_last_update.as_ref().unwrap();
            args.add(line.id_.store_id);
            args.add(prod_typ_num.to_string());
            args.add(line.id_.product_id);
            args.add(time.naive_utc());
        }).count();
        args.add(oid.to_vec());
        lines.into_iter().map(|line| {
            let id_ = line.id_;
            let prod_typ_num: u8 = id_.product_type.into();
            args.add(id_.store_id);
            args.add(prod_typ_num.to_string());
            args.add(id_.product_id);
        }).count();
        args
    }
}
impl<'a> Into<(String, MySqlArguments)> for UpdateOLinePayArg<'a>
{
    fn into(self) -> (String, MySqlArguments) {
        let num_batch = self.1.len();
        assert!(num_batch > 0);
        (Self::sql_pattern(num_batch), self.into_arguments())
    }
}


const OLINE_SELECT_PREFIX: &'static str = "SELECT `store_id`,`product_type`,`product_id`,\
    `price_unit`,`price_total`,`qty_rsved`,`qty_paid`,`qty_paid_last_update`,`rsved_until`,\
    `warranty_until` FROM `order_line_detail`";

impl Into<(String, MySqlArguments)> for FetchAllLinesArg {
    fn into(self) -> (String, MySqlArguments) {
        let sql_patt = format!("{OLINE_SELECT_PREFIX} WHERE `o_id`=?");
        let mut args = MySqlArguments::default();
        let OidBytes(oid) = self.0;
        args.add(oid.to_vec());
        (sql_patt, args)
    }
}
impl<'a> FetchLineByIdArg<'a>
{
    fn sql_pattern(num_batch: usize) -> String
    {
        let items = (0..num_batch).into_iter().map(
            |_| "(`store_id`=? AND `product_type`=? AND `product_id`=?)"
        ).collect::<Vec<_>>();
        format!("{OLINE_SELECT_PREFIX} WHERE `o_id`=? AND ({})", items.join("OR"))
    }
}
impl<'a,'q> IntoArguments<'q, MySql> for FetchLineByIdArg<'a>
{
    fn into_arguments(self) -> <MySql as sqlx::database::HasArguments<'q>>::Arguments {
        let (OidBytes(oid_b), pids) = (self.0, self.1);
        let mut args = MySqlArguments::default();
        args.add(oid_b.to_vec());
        pids.into_iter().map(|id_| {
            let prod_typ_num: u8 = id_.product_type.into();
            args.add(id_.store_id);
            args.add(prod_typ_num.to_string());
            args.add(id_.product_id);
        }).count();
        args
    }
}
impl<'a> Into<(String, MySqlArguments)> for FetchLineByIdArg<'a>
{
    fn into(self) -> (String, MySqlArguments) {
        let num_batch = self.1.len();
        assert!(num_batch > 0);
        (Self::sql_pattern(num_batch), self.into_arguments())
    }
}


impl TryFrom<OLineRow> for OrderLineModel {
    type Error = AppError;
    fn try_from(value: OLineRow) -> DefaultResult<Self, Self::Error> {
        let row = value.0;
        let store_id = row.try_get::<u32, usize>(0)?;
        let product_type = row.try_get::<&str, usize>(1)?.parse::<ProductType>()?;
        let product_id = row.try_get::<u64, usize>(2)?;
        let unit = row.try_get::<u32, usize>(3)?;
        let total = row.try_get::<u32, usize>(4)?;
        let reserved = row.try_get::<u32, usize>(5)?;
        let paid = row.try_get::<u32, usize>(6)?;
        let result = row.try_get::<Option<NaiveDateTime>, usize>(7)?;
        let paid_last_update = if let Some(t) = result {
            Some(t.and_utc().into())
        } else { None };
        let reserved_until = row.try_get::<NaiveDateTime, usize>(8)?.and_utc().into();
        let warranty_until = row.try_get::<NaiveDateTime, usize>(9)?.and_utc().into();

        let id_ = OrderLineIdentity {store_id, product_type, product_id};
        let price = OrderLinePriceModel {unit, total};
        let qty = OrderLineQuantityModel {reserved, paid, paid_last_update};
        let policy = OrderLineAppliedPolicyModel {warranty_until, reserved_until};
        Ok(OrderLineModel { id_, price, qty, policy })
    }
} // end of impl OrderLineModel

impl TryInto<String> for EmailRow {
    type Error = AppError;
    fn try_into(self) -> DefaultResult<String, Self::Error> {
        let row = self.0;
        let mail = row.try_get::<String, usize>(0)?;
        Ok(mail)
    }
}
impl TryInto<PhoneNumberDto> for PhoneRow {
    type Error = AppError;
    fn try_into(self) -> DefaultResult<PhoneNumberDto, Self::Error> {
        let row = self.0;
        let nation = row.try_get::<u16, usize>(0)?;
        let number = row.try_get::<String, usize>(1)?;
        Ok(PhoneNumberDto {nation, number})
    }
}
impl TryInto<ContactModel> for ContactMetaRow {
    type Error = AppError;
    fn try_into(self) -> DefaultResult<ContactModel, Self::Error> {
        let row = self.0;
        let first_name = row.try_get::<String, usize>(0)?;
        let last_name  = row.try_get::<String, usize>(1)?;
        Ok(ContactModel { first_name, last_name, emails: vec![], phones: vec![] })
    }
}
impl TryInto<PhyAddrModel> for PhyAddrrRow {
    type Error = AppError;
    fn try_into(self) -> DefaultResult<PhyAddrModel, Self::Error> {
        let row = self.0;
        let country = {
            let c = row.try_get::<String, usize>(0)?;
            CountryCode::from(c)
        };
        let region = row.try_get::<String, usize>(1)?;
        let city   = row.try_get::<String, usize>(2)?;
        let distinct = row.try_get::<String, usize>(3)?;
        let street_name = row.try_get::<Option<String>, usize>(4)?;
        let detail = row.try_get::<String, usize>(5)?;
        let out = PhyAddrModel {country, region, city, distinct, street_name, detail};
        Ok(out)
    }
}
impl TryInto<ShippingOptionModel> for ShipOptionRow {
    type Error = AppError;
    fn try_into(self) -> DefaultResult<ShippingOptionModel, Self::Error> {
        let row = self.0;
        let seller_id = row.try_get::<u32, usize>(0)?;
        let method = row.try_get::<String, usize>(1)?;
        let method = ShippingMethod::from(method);
        Ok(ShippingOptionModel {seller_id, method})
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

    async fn save_contact (&self, oid:&str, bl:BillingModel, sh:ShippingModel)
        -> DefaultResult<(), AppError> 
    { // TODO, consider update case
        let oid_b = OidBytes::try_from(oid)?;
        let mut conn = self._db.acquire().await?;
        let mut tx = conn.begin().await?;
        let (bl_contact, bl_phyaddr) = (bl.contact, bl.address);
        let (sh_contact, sh_phyaddr, sh_opt) = (sh.contact, sh.address, sh.option);
        Self::_save_contact(&mut tx, &oid_b, "bill", bl_contact).await?;
        if let Some(loc) = bl_phyaddr {
            Self::_save_phyaddr(&mut tx, &oid_b, "bill", loc).await?;
        }
        Self::_save_contact(&mut tx, &oid_b, "ship", sh_contact).await?;
        if let Some(loc) = sh_phyaddr {
            Self::_save_phyaddr(&mut tx, &oid_b, "ship", loc).await?;
        }
        Self::_save_ship_opt(&mut tx, &oid_b, sh_opt).await?;
        tx.commit().await?;
        Ok(())
    }
    async fn fetch_all_lines(&self, oid:String) -> DefaultResult<Vec<OrderLineModel>, AppError>
    {
        let oid_b = OidBytes::try_from(oid.as_str())?;
        let mut conn = self._db.acquire().await?;
        let (sql_patt , args) = FetchAllLinesArg(oid_b).into();
        let stmt = conn.prepare(sql_patt.as_str()).await?;
        let query = stmt.query_with(args);
        let exec = &mut *conn;
        let mut rs_stream = exec.fetch(query);
        let mut lines = vec![];
        while let Some(result) = rs_stream.next().await {
            let row = result?;
            let item = OLineRow(row).try_into()?;
            lines.push(item)
        } // TODO, consider to return stream, let app caller determine bulk load size
        Ok(lines)
    }
    async fn fetch_billing(&self, oid:String) -> DefaultResult<BillingModel, AppError>
    {
        let OidBytes(oid_b) = OidBytes::try_from(oid.as_str())?;
        let mut conn = self._db.acquire().await?;
        let emails = Self::_fetch_mails(&mut conn, "bill", &oid_b).await?;
        let phones = Self::_fetch_phones(&mut conn, "bill", &oid_b).await?;
        let mut contact = Self::_fetch_contact_meta(&mut conn, "bill", &oid_b).await?;
        contact.emails = emails;
        contact.phones = phones;
        let address = Self::_fetch_phyaddr(&mut conn, "bill", &oid_b).await?;
        Ok(BillingModel {contact, address})
    }
    async fn fetch_shipping(&self, oid:String) -> DefaultResult<ShippingModel, AppError>
    {
        let OidBytes(oid_b) = OidBytes::try_from(oid.as_str())?;
        let mut conn = self._db.acquire().await?;
        let emails = Self::_fetch_mails(&mut conn, "ship", &oid_b).await?;
        let phones = Self::_fetch_phones(&mut conn, "ship", &oid_b).await?;
        let mut contact = Self::_fetch_contact_meta(&mut conn, "ship", &oid_b).await?;
        contact.emails = emails;
        contact.phones = phones;
        let option = Self::_fetch_ship_option(&mut conn, &oid_b).await?;
        let address = Self::_fetch_phyaddr(&mut conn, "ship", &oid_b).await?;
        Ok(ShippingModel {contact, address, option})
    }
    async fn update_lines_payment(&self, data: OrderPaymentUpdateDto,
                                  cb: AppOrderRepoUpdateLinesUserFunc)
        -> DefaultResult<OrderPaymentUpdateErrorDto, AppError>
    {
        let (oid, d_lines) = (data.oid, data.lines);
        let oid_b = OidBytes::try_from(oid.as_str())?;
        let pids = d_lines.iter().map(
            |d| OrderLineIdentity {store_id:d.seller_id, product_id:d.product_id,
                    product_type: d.product_type.clone() }
        ).collect::<Vec<_>>();
        let mut conn = self._db.acquire().await?;
        let mut tx = conn.begin().await?;
        let mut saved_lines = Self::_fetch_lines_by_pid(&mut tx, &oid_b, pids).await?;
        let errors = cb(&mut saved_lines, d_lines);
        if errors.is_empty() {
            let num_affected = saved_lines.len();
            let (sql_patt, args) = UpdateOLinePayArg(&oid_b, saved_lines).into();
            let _rs = run_query_once(&mut tx, sql_patt, args, num_affected).await?;
            tx.commit().await?;
        }
        Ok(OrderPaymentUpdateErrorDto { oid, lines: errors })
    }
    async fn fetch_lines_by_rsvtime(&self, _time_start: DateTime<FixedOffset>,
                                  _time_end: DateTime<FixedOffset>,
                                  _usr_cb: AppOrderFetchRangeCallback )
        -> DefaultResult<(), AppError>
    {
        Err(AppError { code: AppErrorCode::NotImplemented, detail: None })
    }
    async fn fetch_lines_by_pid(&self, oid:&str, pids:Vec<OrderLineIdentity>)
        -> DefaultResult<Vec<OrderLineModel>, AppError>
    {
        let oid_b = OidBytes::try_from(oid)?;
        let mut conn = self._db.acquire().await?;
        let mut tx = conn.begin().await?;
        Self::_fetch_lines_by_pid(&mut tx, &oid_b, pids).await
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
        let oid = OidBytes::try_from(oid)?;
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
        
    async fn _save_contact(tx: &mut Transaction<'_, MySql>, oid:&OidBytes,
                           table_opt:&str, data: ContactModel)
        -> DefaultResult<(), AppError>
    {
        if data.emails.is_empty() && data.phones.is_empty() {
            let d = "save-contact, num-emails:0, num-phones:0".to_string();
            let e = AppError {code:AppErrorCode::InvalidInput, detail:Some(d)};
            return Err(e);
        }
        let (f_name, l_name, emails, phones) = (data.first_name, data.last_name, data.emails, data.phones);
        let (num_mails, num_phones) = (emails.len(), phones.len());
        let (sql_patt, args) = InsertContactMeta(table_opt, oid, f_name, l_name).into();
        let _rs = run_query_once(tx, sql_patt, args, 1).await?;
        if num_mails > 0 {
            let (sql_patt, args) = InsertContactEmail(table_opt, oid, emails).into();
            let _rs = run_query_once(tx, sql_patt, args, num_mails).await?;
        }
        if num_phones > 0 {
            let (sql_patt, args) = InsertContactPhone(table_opt, oid, phones).into();
            let _rs = run_query_once(tx, sql_patt, args, num_phones).await?; 
        }
        Ok(())
    }
    async fn _save_phyaddr(tx: &mut Transaction<'_, MySql>, oid:&OidBytes,
                           table_opt:&str, data: PhyAddrModel)
        -> DefaultResult<(), AppError>
    {
        let (sql_patt, args) = InsertPhyAddr(table_opt, oid, data).into();
        let _rs = run_query_once(tx, sql_patt, args, 1).await?;
        Ok(())
    }
    async fn _save_ship_opt(tx: &mut Transaction<'_, MySql>, oid:&OidBytes,
                            data:Vec<ShippingOptionModel>)
        -> DefaultResult<(), AppError>
    {
        if data.is_empty() {
            let d = "save-ship-option, num:0".to_string();
            let e = AppError {code:AppErrorCode::InvalidInput, detail:Some(d)};
            return Err(e);
        }
        let num_sellers = data.len();
        let (sql_patt, args) = InsertShipOption(oid, data).into();
        let _rs = run_query_once(tx, sql_patt, args, num_sellers).await?;
        Ok(())
    }
    
    async fn _fetch_lines_by_pid(tx: &mut Transaction<'_, MySql>, oid:&OidBytes,
                                 pids: Vec<OrderLineIdentity> )
        -> DefaultResult<Vec<OrderLineModel>, AppError>
    {
        let (sql_patt, args) = FetchLineByIdArg(&oid, pids).into();
        let stmt = tx.prepare(sql_patt.as_str()).await?;
        let query = stmt.query_with(args);
        let exec = &mut *tx;
        let rows = exec.fetch_all(query).await?;
        let results = rows.into_iter().map(|row| {
            OLineRow(row).try_into()
        }).collect::<Vec<DefaultResult<OrderLineModel, AppError>>>();
        if let Some(Err(e)) = results.iter().find(|r| r.is_err()) {
            Err(e.to_owned())
        } else {
            let out = results.into_iter().map(|r| r.unwrap()).collect::<Vec<_>>();
            Ok(out)
        }
    }
    async fn _fetch_mails(conn:&mut PoolConnection<MySql>, table_opt:&str, oid_b:&[u8])
        -> DefaultResult<Vec<String>, AppError>
    {
        let sql_patt = format!("SELECT `mail` FROM `{}_contact_email` WHERE `o_id`=?", table_opt);
        let stmt = conn.prepare(sql_patt.as_str()).await?;
        let query = stmt.query().bind(oid_b.to_vec());
        let exec = conn.as_mut();
        let rows = exec.fetch_all(query).await?;
        let results = rows.into_iter().map(|row| {
            EmailRow(row).try_into()
        }).collect::<Vec<DefaultResult<String, AppError>>>();
        if let Some(Err(e)) = results.iter().find(|r| r.is_err()) {
            Err(e.to_owned())
        } else {
            let out = results.into_iter().map(|r| r.unwrap()).collect::<Vec<_>>();
            Ok(out)
        }
    }
    async fn _fetch_phones(conn:&mut PoolConnection<MySql>, table_opt:&str, oid_b:&[u8])
        -> DefaultResult<Vec<PhoneNumberDto>, AppError>
    {
        let sql_patt = format!("SELECT `nation`,`number` FROM `{}_contact_phone`\
                               WHERE `o_id`=?", table_opt);
        let stmt = conn.prepare(sql_patt.as_str()).await?;
        let query = stmt.query().bind(oid_b.to_vec());
        let exec = conn.as_mut();
        let rows = exec.fetch_all(query).await?;
        let results = rows.into_iter().map(|row| {
            PhoneRow(row).try_into()
        }).collect::<Vec<DefaultResult<PhoneNumberDto, AppError>>>();
        if let Some(Err(e)) = results.iter().find(|r| r.is_err()) {
            Err(e.to_owned())
        } else {
            let out = results.into_iter().map(|r| r.unwrap()).collect::<Vec<_>>();
            Ok(out)
        }
    }
    async fn _fetch_contact_meta(conn:&mut PoolConnection<MySql>, table_opt:&str, oid_b:&[u8])
        -> DefaultResult<ContactModel, AppError>
    {
        let sql_patt = format!("SELECT `first_name`,`last_name` FROM `{}_contact_meta`\
                               WHERE `o_id`=?", table_opt);
        let stmt = conn.prepare(sql_patt.as_str()).await?;
        let query = stmt.query().bind(oid_b.to_vec());
        let exec = conn.as_mut();
        let row  = exec.fetch_one(query).await?;
        let out = ContactMetaRow(row).try_into()?;
        Ok(out)
    }
    async fn _fetch_phyaddr(conn:&mut PoolConnection<MySql>, table_opt:&str, oid_b:&[u8])
        -> DefaultResult<Option<PhyAddrModel>, AppError>
    {
        let sql_patt = format!("SELECT `country`,`region`,`city`,`distinct`,`street`,\
                               `detail` FROM `{}_phyaddr` WHERE `o_id`=?", table_opt);
        let stmt = conn.prepare(sql_patt.as_str()).await?;
        let query = stmt.query().bind(oid_b.to_vec());
        let exec = conn.as_mut();
        let result = exec.fetch_optional(query).await?;
        if let Some(row) = result {
            let out = PhyAddrrRow(row).try_into()?;
            Ok(Some(out))
        } else { Ok(None) }
    }
    async fn _fetch_ship_option(conn:&mut PoolConnection<MySql>, oid_b:&[u8])
        -> DefaultResult<Vec<ShippingOptionModel>, AppError>
    {
        let sql_patt = "SELECT `seller_id`,`method` FROM `ship_option` WHERE `o_id`=?";
        let stmt = conn.prepare(sql_patt).await?;
        let query = stmt.query().bind(oid_b.to_vec());
        let exec = conn.as_mut();
        let rows = exec.fetch_all(query).await?;
        let results = rows.into_iter().map(|row| {
            ShipOptionRow(row).try_into()
        }).collect::<Vec<DefaultResult<ShippingOptionModel, AppError>>>();
        if let Some(Err(e)) = results.iter().find(|r| r.is_err()) {
            Err(e.to_owned())
        } else {
            let out = results.into_iter().map(|r| r.unwrap()).collect::<Vec<_>>();
            Ok(out)
        }
    }
} // end of impl OrderMariaDbRepo

