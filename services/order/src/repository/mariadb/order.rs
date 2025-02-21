use std::boxed::Box;
use std::cmp::min;
use std::collections::HashMap;
use std::result::Result as DefaultResult;
use std::sync::Arc;
use std::vec::Vec;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset, Local, NaiveDateTime};
use futures_util::stream::StreamExt;
use rust_decimal::Decimal;
use sqlx::database::Database as AbstractDatabase;
use sqlx::mysql::{MySqlArguments, MySqlConnection, MySqlRow};
use sqlx::{Arguments, Connection, Executor, IntoArguments, MySql, Row, Statement, Transaction};

use ecommerce_common::adapter::repository::OidBytes;
use ecommerce_common::api::dto::{CountryCode, CurrencyDto, PhoneNumberDto};
use ecommerce_common::api::rpc::dto::{OrderPaymentUpdateDto, OrderPaymentUpdateErrorDto};
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::model::order::{BillingModel, ContactModel, PhyAddrModel};

use crate::api::dto::ShippingMethod;
use crate::constant::hard_limit;
use crate::datastore::AppMariaDbStore;
use crate::error::AppError;
use crate::model::{
    CurrencyModel, OrderCurrencyModel, OrderLineAppliedPolicyModel, OrderLineIdentity,
    OrderLineModel, OrderLineModelSet, OrderLinePriceModel, OrderLineQuantityModel,
    ProdAttriPriceModel, ShippingModel, ShippingOptionModel,
};
use crate::repository::{
    AbsOrderRepo, AbsOrderStockRepo, AppOrderFetchRangeCallback, AppOrderRepoUpdateLinesUserFunc,
};

use super::stock::StockMariaDbRepo;
use super::{run_query_once, to_app_oid};

struct InsertTopMetaArg<'a, 'b, 'c>(
    &'a OidBytes,
    u32,
    &'b DateTime<FixedOffset>,
    &'c CurrencyModel,
);
struct InsertSellerCurrencyArg<'a, 'b>(&'a OidBytes, &'b HashMap<u32, CurrencyModel>);
struct InsertOLineArg<'a, 'b>(&'a OidBytes, usize, Vec<&'b OrderLineModel>);
struct InsertContactMeta<'a, 'b>(&'a str, &'b OidBytes, String, String);
struct InsertContactEmail<'a, 'b>(&'a str, &'b OidBytes, Vec<String>);
struct InsertContactPhone<'a, 'b>(&'a str, &'b OidBytes, Vec<PhoneNumberDto>);
struct InsertPhyAddr<'a, 'b>(&'a str, &'b OidBytes, PhyAddrModel);
struct InsertShipOption<'a>(&'a OidBytes, Vec<ShippingOptionModel>);

struct UpdateOLinePayArg<'a>(&'a OidBytes, Vec<OrderLineModel>);

struct FetchAllLinesArg(OidBytes);
struct FetchLineByIdArg<'a>(&'a OidBytes, Vec<OrderLineIdentity>);

struct TopLvlMetaRow(MySqlRow, HashMap<u32, CurrencyModel>);
struct BuyerCurrencyRow<'a>(&'a MySqlRow, usize);
struct SellerCurrencyRow(MySqlRow);
struct OLineRow(MySqlRow);
struct EmailRow(MySqlRow);
struct PhoneRow(MySqlRow);
struct ContactMetaRow(MySqlRow);
struct PhyAddrrRow(MySqlRow);
struct ShipOptionRow(MySqlRow);

impl<'a, 'b, 'c> From<InsertTopMetaArg<'a, 'b, 'c>> for (String, MySqlArguments) {
    fn from(value: InsertTopMetaArg<'a, 'b, 'c>) -> (String, MySqlArguments) {
        let patt = "INSERT INTO `order_toplvl_meta`(`usr_id`,`o_id`,`created_time`,\
                    `buyer_currency`,`buyer_ex_rate`) VALUES (?,?,?,?,?)";
        let ctime_utc = value.2.clone().naive_utc();
        let mut args = MySqlArguments::default();
        args.add(value.1).unwrap();
        args.add(value.0.as_column()).unwrap();
        args.add(ctime_utc).unwrap();
        args.add(value.3.name.to_string()).unwrap();
        args.add(value.3.rate).unwrap(); // copy trait implemented in Decimal type
        (patt.to_string(), args)
    }
}

impl<'a, 'b> InsertSellerCurrencyArg<'a, 'b> {
    fn sql_pattern(num_batch: usize) -> String {
        let items = (0..num_batch)
            .map(|_| "(?,?,?,?)")
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "INSERT INTO `oseller_currency_snapshot`(`seller_id`,`o_id`,\
                `label`,`ex_rate`) VALUES {items}"
        )
    }
}
impl<'a, 'b, 'q> IntoArguments<'q, MySql> for InsertSellerCurrencyArg<'a, 'b> {
    fn into_arguments(self) -> <MySql as AbstractDatabase>::Arguments<'q> {
        let mut args = MySqlArguments::default();
        self.1
            .iter()
            .map(|(seller_id, v)| {
                args.add(seller_id).unwrap();
                args.add(self.0.as_column()).unwrap();
                args.add(v.name.to_string()).unwrap();
                args.add(v.rate).unwrap();
            })
            .count();
        args
    }
}
impl<'a, 'b> From<InsertSellerCurrencyArg<'a, 'b>> for (String, MySqlArguments) {
    fn from(value: InsertSellerCurrencyArg<'a, 'b>) -> (String, MySqlArguments) {
        let num_batch = value.1.len();
        (
            InsertSellerCurrencyArg::sql_pattern(num_batch),
            value.into_arguments(),
        )
    }
}

impl<'a, 'b> InsertOLineArg<'a, 'b> {
    fn sql_pattern(num_batch: usize) -> String {
        let col_seq = "`o_id`,`seq`,`store_id`,`product_id`,`price_unit`,`price_total`,\
                       `qty_rsved`,`rsved_until`,`warranty_until`,`attr_lastupdate`,\
                       `attr_price`,`attr_seq`";
        let items = (0..num_batch)
            .map(|_| "(?,?,?,?,?,?,?,?,?,?,?,?)")
            .collect::<Vec<_>>();
        format!(
            "INSERT INTO `order_line_detail`({}) VALUES {}",
            col_seq,
            items.join(",")
        )
    }
}
impl<'a, 'b, 'q> IntoArguments<'q, MySql> for InsertOLineArg<'a, 'b> {
    fn into_arguments(self) -> <MySql as AbstractDatabase>::Arguments<'q> {
        let mut args = MySqlArguments::default();
        let (oid, mut seq, lines) = (self.0, self.1, self.2);
        lines
            .into_iter()
            .map(|o| {
                let rsved_until = o.policy.reserved_until.naive_utc();
                let warranty_until = o.policy.warranty_until.naive_utc();
                let attr_lupdate = o.attrs_charge().lastupdate().naive_utc();
                let attr_pricemap = o.attrs_charge().serialize_map().unwrap();
                args.add(oid.as_column()).unwrap();
                args.add(seq as u16).unwrap(); // match the column type in db table
                seq += 1;
                args.add(o.id().store_id()).unwrap();
                args.add(o.id().product_id()).unwrap();
                args.add(o.price().unit()).unwrap();
                args.add(o.price().total()).unwrap();
                args.add(o.qty.reserved).unwrap();
                args.add(rsved_until).unwrap();
                args.add(warranty_until).unwrap();
                args.add(attr_lupdate).unwrap();
                args.add(attr_pricemap).unwrap();
                args.add(o.id().attrs_seq_num()).unwrap();
            })
            .count();
        args
    }
}
impl<'a, 'b> From<InsertOLineArg<'a, 'b>> for (String, MySqlArguments) {
    fn from(value: InsertOLineArg<'a, 'b>) -> (String, MySqlArguments) {
        let num_batch = value.2.len();
        (
            InsertOLineArg::sql_pattern(num_batch),
            value.into_arguments(),
        )
    }
}
impl<'a, 'b> From<InsertContactMeta<'a, 'b>> for (String, MySqlArguments) {
    fn from(value: InsertContactMeta<'a, 'b>) -> (String, MySqlArguments) {
        let (table_opt, oid, first_name, last_name) = (value.0, value.1, value.2, value.3);
        let patt = format!(
            "INSERT INTO `{}_contact_meta`(`o_id`,`first_name`,`last_name`) \
                           VALUES (?,?,?)",
            table_opt
        );
        let mut args = MySqlArguments::default();
        args.add(oid.as_column()).unwrap();
        args.add(first_name).unwrap();
        args.add(last_name).unwrap();
        (patt, args)
    }
}
impl<'a, 'b> InsertContactEmail<'a, 'b> {
    fn sql_pattern(&self) -> String {
        let (table_opt, num_batch) = (self.0, self.2.len());
        assert!(num_batch > 0);
        let items = (0..num_batch).map(|_num| "(?,?,?)").collect::<Vec<_>>();
        format!(
            "INSERT INTO `{}_contact_email`(`o_id`,`seq`,`mail`) VALUES {}",
            table_opt,
            items.join(",")
        )
    }
}
impl<'a, 'b, 'q> IntoArguments<'q, MySql> for InsertContactEmail<'a, 'b> {
    fn into_arguments(self) -> <MySql as AbstractDatabase>::Arguments<'q> {
        let (oid, mails, mut seq) = (self.1, self.2, 0u16);
        let oid = oid.as_column();
        let mut args = MySqlArguments::default();
        mails
            .into_iter()
            .map(|mail| {
                args.add(&oid).unwrap();
                args.add(seq).unwrap();
                args.add(mail).unwrap();
                seq += 1;
            })
            .count();
        args
    }
}
impl<'a, 'b> From<InsertContactEmail<'a, 'b>> for (String, MySqlArguments) {
    fn from(value: InsertContactEmail<'a, 'b>) -> (String, MySqlArguments) {
        (value.sql_pattern(), value.into_arguments())
    }
}
impl<'a, 'b> InsertContactPhone<'a, 'b> {
    fn sql_pattern(&self) -> String {
        let (table_opt, num_batch) = (self.0, self.2.len());
        assert!(num_batch > 0);
        let items = (0..num_batch).map(|_num| "(?,?,?,?)").collect::<Vec<_>>();
        format!(
            "INSERT INTO `{}_contact_phone`(`o_id`,`seq`,`nation`,`number`) VALUES {}",
            table_opt,
            items.join(",")
        )
    }
}
impl<'a, 'b, 'q> IntoArguments<'q, MySql> for InsertContactPhone<'a, 'b> {
    fn into_arguments(self) -> <MySql as AbstractDatabase>::Arguments<'q> {
        let (oid, phones, mut seq) = (self.1, self.2, 0u16);
        let oid = oid.as_column();
        let mut args = MySqlArguments::default();
        phones
            .into_iter()
            .map(|phone| {
                args.add(&oid).unwrap();
                args.add(seq).unwrap();
                args.add(phone.nation).unwrap();
                args.add(phone.number).unwrap();
                seq += 1;
            })
            .count();
        args
    }
}
impl<'a, 'b> From<InsertContactPhone<'a, 'b>> for (String, MySqlArguments) {
    fn from(value: InsertContactPhone<'a, 'b>) -> (String, MySqlArguments) {
        (value.sql_pattern(), value.into_arguments())
    }
}
impl<'a, 'b> From<InsertPhyAddr<'a, 'b>> for (String, MySqlArguments) {
    fn from(value: InsertPhyAddr<'a, 'b>) -> (String, MySqlArguments) {
        let (table_opt, oid, addr) = (value.0, value.1, value.2);
        let patt = format!(
            "INSERT INTO `{}_phyaddr`(`o_id`,`country`,`region`,`city`,\
                   `distinct`,`street`,`detail`) VALUES (?,?,?,?,?,?,?)",
            table_opt
        );
        let country: String = addr.country.into();
        let mut args = MySqlArguments::default();
        args.add(oid.as_column()).unwrap();
        args.add(country).unwrap();
        args.add(addr.region).unwrap();
        args.add(addr.city).unwrap();
        args.add(addr.distinct).unwrap();
        args.add(addr.street_name).unwrap();
        args.add(addr.detail).unwrap();
        (patt, args)
    }
}
impl<'a> InsertShipOption<'a> {
    fn sql_pattern(num_batch: usize) -> String {
        let items = (0..num_batch).map(|_num| "(?,?,?)").collect::<Vec<_>>();
        format!(
            "INSERT INTO `ship_option`(`o_id`,`seller_id`,`method`) VALUES {}",
            items.join(",")
        )
    }
}
impl<'a, 'q> IntoArguments<'q, MySql> for InsertShipOption<'a> {
    fn into_arguments(self) -> <MySql as AbstractDatabase>::Arguments<'q> {
        let (oid, options) = (self.0, self.1);
        let oid = oid.as_column();
        let mut args = MySqlArguments::default();
        options
            .into_iter()
            .map(|so| {
                let method: String = so.method.into();
                args.add(&oid).unwrap();
                args.add(so.seller_id).unwrap();
                args.add(method).unwrap();
            })
            .count();
        args
    }
}
impl<'a> From<InsertShipOption<'a>> for (String, MySqlArguments) {
    fn from(value: InsertShipOption<'a>) -> (String, MySqlArguments) {
        let num_batch = value.1.len();
        assert!(num_batch > 0);
        (
            InsertShipOption::sql_pattern(num_batch),
            value.into_arguments(),
        )
    }
}

impl<'a> UpdateOLinePayArg<'a> {
    fn sql_pattern(num_batch: usize) -> String {
        let condition = "(`store_id`=? AND `product_id`=? AND `attr_seq`=?)";
        let case_ops = (0..num_batch)
            .flat_map(|_| ["WHEN", condition, "THEN", "?"])
            .collect::<Vec<_>>()
            .join(" ");
        let where_ops = (0..num_batch)
            .map(|_| condition)
            .collect::<Vec<_>>()
            .join("OR");
        let portions = [
            format!("`qty_paid` = CASE {case_ops} ELSE `qty_paid` END"),
            format!("`qty_paid_last_update` = CASE {case_ops} ELSE `qty_paid_last_update` END"),
        ];
        format!(
            "UPDATE `order_line_detail` SET {}, {} WHERE `o_id`=? AND ({})",
            portions[0], portions[1], where_ops
        )
    }
}
impl<'a, 'q> IntoArguments<'q, MySql> for UpdateOLinePayArg<'a> {
    fn into_arguments(self) -> <MySql as AbstractDatabase>::Arguments<'q> {
        let (oid, lines) = (self.0, self.1);
        let mut args = MySqlArguments::default();
        lines
            .iter()
            .map(|line| {
                args.add(line.id().store_id()).unwrap();
                args.add(line.id().product_id()).unwrap();
                args.add(line.id().attrs_seq_num()).unwrap();
                args.add(line.qty.paid).unwrap();
            })
            .count();
        lines
            .iter()
            .map(|line| {
                let time = line.qty.paid_last_update.as_ref().unwrap();
                args.add(line.id().store_id()).unwrap();
                args.add(line.id().product_id()).unwrap();
                args.add(line.id().attrs_seq_num()).unwrap();
                args.add(time.naive_utc()).unwrap();
            })
            .count();
        args.add(oid.as_column()).unwrap();
        lines
            .into_iter()
            .map(|line| {
                args.add(line.id().store_id()).unwrap();
                args.add(line.id().product_id()).unwrap();
                args.add(line.id().attrs_seq_num()).unwrap();
            })
            .count();
        args
    }
}
impl<'a> From<UpdateOLinePayArg<'a>> for (String, MySqlArguments) {
    fn from(value: UpdateOLinePayArg<'a>) -> (String, MySqlArguments) {
        let num_batch = value.1.len();
        assert!(num_batch > 0);
        (
            UpdateOLinePayArg::sql_pattern(num_batch),
            value.into_arguments(),
        )
    }
}

const OLINE_SELECT_PREFIX: &str = "SELECT `store_id`,`product_id`,`attr_seq`,`price_unit`,\
   `price_total`,`qty_rsved`,`qty_paid`,`qty_paid_last_update`,`rsved_until`,\
    `warranty_until`,`attr_lastupdate`,`attr_price` FROM `order_line_detail`";

impl From<FetchAllLinesArg> for (String, MySqlArguments) {
    fn from(value: FetchAllLinesArg) -> (String, MySqlArguments) {
        let sql_patt = format!("{OLINE_SELECT_PREFIX} WHERE `o_id`=?");
        let mut args = MySqlArguments::default();
        let oid = value.0;
        args.add(oid.as_column()).unwrap();
        (sql_patt, args)
    }
}
impl<'a> FetchLineByIdArg<'a> {
    fn sql_pattern(num_batch: usize) -> String {
        let items = (0..num_batch)
            .map(|_| "(`store_id`=? AND `product_id`=? AND `attr_seq`=?)")
            .collect::<Vec<_>>();
        format!(
            "{OLINE_SELECT_PREFIX} WHERE `o_id`=? AND ({})",
            items.join("OR")
        )
    }
}
impl<'a, 'q> IntoArguments<'q, MySql> for FetchLineByIdArg<'a> {
    fn into_arguments(self) -> <MySql as AbstractDatabase>::Arguments<'q> {
        let (oid_b, pids) = (self.0, self.1);
        let mut args = MySqlArguments::default();
        args.add(oid_b.as_column()).unwrap();
        pids.into_iter()
            .map(|pid| {
                args.add(pid.store_id()).unwrap();
                args.add(pid.product_id()).unwrap();
                args.add(pid.attrs_seq_num()).unwrap();
            })
            .count();
        args
    }
}
impl<'a> From<FetchLineByIdArg<'a>> for (String, MySqlArguments) {
    fn from(value: FetchLineByIdArg<'a>) -> (String, MySqlArguments) {
        let num_batch = value.1.len();
        assert!(num_batch > 0);
        (
            FetchLineByIdArg::sql_pattern(num_batch),
            value.into_arguments(),
        )
    }
}

#[rustfmt::skip]
impl<'a> TryInto<CurrencyModel> for BuyerCurrencyRow<'a> {
    type Error = AppError;
    fn try_into(self) -> DefaultResult<CurrencyModel, Self::Error> {
        let Self(row, start_idx) = self;
        let name_raw = row.try_get::<&[u8], usize>(start_idx)?;
        let name_raw = std::str::from_utf8(name_raw)
            .map_err(|e| AppError {
                code: AppErrorCode::DataCorruption,
                detail: Some(e.to_string()),
            })? .to_string();
        let name = CurrencyDto::from(&name_raw);
        if matches!(name, CurrencyDto::Unknown) {
            let msg = format!("buyer-currency-label, raw-saved:{name_raw}");
            return Err(AppError {
                code: AppErrorCode::DataCorruption, detail: Some(msg)
            });
        }
        let rate = row.try_get::<Decimal, usize>(start_idx + 1)?;
        Ok(CurrencyModel {name, rate})
    }
}
impl TryInto<(u32, CurrencyModel)> for SellerCurrencyRow {
    type Error = AppError;
    fn try_into(self) -> DefaultResult<(u32, CurrencyModel), Self::Error> {
        let row = self.0;
        let seller_id = row.try_get::<u32, usize>(0)?;
        // reuse the code, the order of the columns `currency-label` and `exchange-rate`
        // is consistent in every function of this module
        let m = BuyerCurrencyRow(&row, 1).try_into()?;
        Ok((seller_id, m))
    }
}

impl TryInto<OrderLineModelSet> for TopLvlMetaRow {
    type Error = AppError;
    fn try_into(self) -> DefaultResult<OrderLineModelSet, Self::Error> {
        let Self(row, sellers_currency) = self;
        let order_id = to_app_oid(&row, 0)?;
        let owner_id = row.try_get::<u32, usize>(1)?;
        let create_time = row.try_get::<NaiveDateTime, usize>(2)?.and_utc().into();
        let buyer = BuyerCurrencyRow(&row, 3).try_into()?;
        let currency = OrderCurrencyModel {
            buyer,
            sellers: sellers_currency,
        };
        let args = (order_id, owner_id, create_time, currency, Vec::new());
        OrderLineModelSet::try_from_repo(args).map_err(|mut es| {
            let e = es.remove(0);
            AppError {
                code: AppErrorCode::DataCorruption,
                detail: Some(e.to_string()),
            }
        })
    }
} // end of impl TopLvlMetaRow

#[rustfmt::skip]
impl TryFrom<OLineRow> for OrderLineModel {
    type Error = AppError;
    fn try_from(value: OLineRow) -> DefaultResult<Self, Self::Error> {
        let row = value.0;
        let store_id = row.try_get::<u32, usize>(0)?;
        let product_id = row.try_get::<u64, usize>(1)?;
        let attr_seq = row.try_get::<u16, usize>(2)?;
        let unit = row.try_get::<u32, usize>(3)?;
        let total = row.try_get::<u32, usize>(4)?;
        let reserved = row.try_get::<u32, usize>(5)?;
        let paid = row.try_get::<u32, usize>(6)?;
        let paid_last_update = {
            let r = row.try_get::<Option<NaiveDateTime>, usize>(7)?;
            r.map(|t| t.and_utc().into())
        };
        let reserved_until = row.try_get::<NaiveDateTime, usize>(8)?.and_utc().into();
        let warranty_until = row.try_get::<NaiveDateTime, usize>(9)?.and_utc().into();
        let attr_lupdate = row.try_get::<NaiveDateTime, usize>(10)?.and_utc().into();
        let attrprice = {
            let raw = row.try_get::<&[u8], usize>(11)?;
            let serial = std::str::from_utf8(raw).map_err(|e| AppError {
                code: AppErrorCode::DataCorruption,
                detail: Some(format!("cvt-prod-attr-price: {}", e)),
            })?;
            ProdAttriPriceModel::deserialize_map(serial)?
        };
        let id_ = OrderLineIdentity::from((store_id, product_id, attr_seq));
        let price = OrderLinePriceModel::from((unit, total));
        let qty = OrderLineQuantityModel {reserved, paid, paid_last_update};
        let policy = OrderLineAppliedPolicyModel {warranty_until, reserved_until};
        let attr_chg = ProdAttriPriceModel::from((attr_lupdate, attrprice));
        Ok(OrderLineModel::from((id_, price, policy, qty, attr_chg)))
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
        Ok(PhoneNumberDto { nation, number })
    }
}
#[rustfmt::skip]
impl TryInto<ContactModel> for ContactMetaRow {
    type Error = AppError;
    fn try_into(self) -> DefaultResult<ContactModel, Self::Error> {
        let row = self.0;
        let first_name = row.try_get::<String, usize>(0)?;
        let last_name = row.try_get::<String, usize>(1)?;
        Ok(ContactModel {
            first_name, last_name,
            emails: vec![], phones: vec![],
        })
    }
}

#[rustfmt::skip]
impl TryInto<PhyAddrModel> for PhyAddrrRow {
    type Error = AppError;
    fn try_into(self) -> DefaultResult<PhyAddrModel, Self::Error> {
        let row = self.0;
        let country = {
            let c_raw = row.try_get::<&[u8], usize>(0)?;
            let c = std::str::from_utf8(c_raw)
                .map_err(|e| AppError {
                    code: AppErrorCode::DataCorruption,
                    detail: Some(e.to_string()),
                }) ?;
            CountryCode::from(c.to_string())
        };
        let region = row.try_get::<String, usize>(1)?;
        let city = row.try_get::<String, usize>(2)?;
        let distinct = row.try_get::<String, usize>(3)?;
        let street_name = row.try_get::<Option<String>, usize>(4)?;
        let detail = row.try_get::<String, usize>(5)?;
        Ok(PhyAddrModel {
            country, region, city, distinct, street_name, detail,
        })
    }
}
impl TryInto<ShippingOptionModel> for ShipOptionRow {
    type Error = AppError;
    fn try_into(self) -> DefaultResult<ShippingOptionModel, Self::Error> {
        let row = self.0;
        let seller_id = row.try_get::<u32, usize>(0)?;
        let mthd_raw = row.try_get::<&[u8], usize>(1)?;
        let mthd_raw = std::str::from_utf8(mthd_raw).map_err(|e| AppError {
            code: AppErrorCode::DataCorruption,
            detail: Some(e.to_string()),
        })?;
        let method = ShippingMethod::from(mthd_raw.to_string());
        Ok(ShippingOptionModel { seller_id, method })
    }
}

pub(crate) struct OrderMariaDbRepo {
    _db: Arc<AppMariaDbStore>,
    _stock: Arc<Box<dyn AbsOrderStockRepo>>,
}

#[async_trait]
impl AbsOrderRepo for OrderMariaDbRepo {
    fn stock(&self) -> Arc<Box<dyn AbsOrderStockRepo>> {
        self._stock.clone()
    }

    async fn save_contact(
        &self,
        oid: &str,
        bl: BillingModel,
        sh: ShippingModel,
    ) -> DefaultResult<(), AppError> {
        // TODO, consider update case
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
    async fn fetch_all_lines(&self, oid: String) -> DefaultResult<Vec<OrderLineModel>, AppError> {
        let oid_b = OidBytes::try_from(oid.as_str())?;
        let mut conn = self._db.acquire().await?;
        let (sql_patt, args) = FetchAllLinesArg(oid_b).into();
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
    async fn fetch_billing(&self, oid: String) -> DefaultResult<BillingModel, AppError> {
        let oid_b = OidBytes::try_from(oid.as_str())?;
        let mut conn = self._db.acquire().await?;
        let emails = Self::_fetch_mails(conn.as_mut(), "bill", &oid_b).await?;
        let phones = Self::_fetch_phones(conn.as_mut(), "bill", &oid_b).await?;
        let mut contact = Self::_fetch_contact_meta(conn.as_mut(), "bill", &oid_b).await?;
        contact.emails = emails;
        contact.phones = phones;
        let address = Self::_fetch_phyaddr(conn.as_mut(), "bill", &oid_b).await?;
        Ok(BillingModel { contact, address })
    }
    async fn fetch_shipping(&self, oid: String) -> DefaultResult<ShippingModel, AppError> {
        let oid_b = OidBytes::try_from(oid.as_str())?;
        let mut conn = self._db.acquire().await?;
        let emails = Self::_fetch_mails(conn.as_mut(), "ship", &oid_b).await?;
        let phones = Self::_fetch_phones(conn.as_mut(), "ship", &oid_b).await?;
        let mut contact = Self::_fetch_contact_meta(conn.as_mut(), "ship", &oid_b).await?;
        contact.emails = emails;
        contact.phones = phones;
        let option = Self::_fetch_ship_option(conn.as_mut(), &oid_b).await?;
        let address = Self::_fetch_phyaddr(conn.as_mut(), "ship", &oid_b).await?;
        Ok(ShippingModel {
            contact,
            address,
            option,
        })
    }
    async fn update_lines_payment(
        &self,
        data: OrderPaymentUpdateDto,
        cb: AppOrderRepoUpdateLinesUserFunc,
    ) -> DefaultResult<OrderPaymentUpdateErrorDto, AppError> {
        let oid = data.oid.clone();
        let oid_b = OidBytes::try_from(oid.as_str())?;
        let attr_seq_dummy = 0u16; // TODO, finish implementation
        let pids = data
            .lines
            .iter()
            .map(|d| OrderLineIdentity::from((d.seller_id, d.product_id, attr_seq_dummy)))
            .collect::<Vec<_>>();
        let mut conn = self._db.acquire().await?;
        let mut tx = conn.begin().await?;
        let mut saved_lines = Self::_fetch_lines_by_pid(&mut tx, &oid_b, pids).await?;
        let errors = cb(&mut saved_lines, data);
        if errors.is_empty() {
            let num_affected = saved_lines.len();
            let (sql_patt, args) = UpdateOLinePayArg(&oid_b, saved_lines).into();
            let _rs = run_query_once(&mut tx, sql_patt, args, Some(num_affected)).await?;
            tx.commit().await?;
        }
        Ok(OrderPaymentUpdateErrorDto {
            oid,
            charge_time: None,
            lines: errors,
        })
    }
    async fn fetch_lines_by_rsvtime(
        &self,
        time_start: DateTime<FixedOffset>,
        time_end: DateTime<FixedOffset>,
        usr_cb: AppOrderFetchRangeCallback,
    ) -> DefaultResult<(), AppError> {
        // current approach will lead to full-table scan and requires 2 conncetions,
        // TODO, improve query time when the table grows to large amount of data
        let mut conn0 = self._db.acquire().await?;
        let mut conn1 = self._db.acquire().await?;
        let (time_start, time_end) = (time_start.naive_utc(), time_end.naive_utc());
        let sql_patt = "SELECT `a`.`o_id`,`a`.`usr_id`,`a`.`created_time`, `a`.`buyer_currency`, \
                        `a`.`buyer_ex_rate` FROM `order_toplvl_meta` AS `a` INNER JOIN \
                        `order_line_detail` AS `b` ON `a`.`o_id` = `b`.`o_id` WHERE \
                        `b`.`rsved_until` > ? AND `b`.`rsved_until` < ? GROUP BY `a`.`o_id`";
        let stmt = conn0.prepare(sql_patt).await?;
        let mut stream = {
            let query = stmt.query().bind(time_start).bind(time_end);
            let exec = &mut *conn0;
            exec.fetch(query)
        };
        while let Some(result) = stream.next().await {
            let row = result?;
            let oid_raw = row.try_get::<Vec<u8>, usize>(0)?;
            let sellers_currency =
                Self::_fetch_seller_exrates(conn1.as_mut(), oid_raw.clone()).await?;
            let mut ol_set: OrderLineModelSet = TopLvlMetaRow(row, sellers_currency).try_into()?;
            let sql_patt = format!(
                "{OLINE_SELECT_PREFIX} WHERE `o_id`=? AND \
                    (? < `rsved_until` AND `rsved_until` < ?)"
            );
            let stmt = conn1.prepare(sql_patt.as_str()).await?;
            let query = stmt.query().bind(oid_raw).bind(time_start).bind(time_end);
            let exec = &mut *conn1;
            let rows = exec.fetch_all(query).await?;
            let results = rows
                .into_iter()
                .map(|row| OLineRow(row).try_into())
                .collect::<Vec<DefaultResult<OrderLineModel, AppError>>>();
            let newlines = if let Some(Err(e)) = results.iter().find(|r| r.is_err()) {
                return Err(e.to_owned());
            } else {
                results.into_iter().map(|r| r.unwrap()).collect::<Vec<_>>()
            };
            ol_set.append_lines(newlines);
            usr_cb(self, ol_set).await?;
        } // end of loop
        Ok(())
    } // end of fn fetch_lines_by_rsvtime

    async fn fetch_lines_by_pid(
        &self,
        oid: &str,
        pids: Vec<OrderLineIdentity>,
    ) -> DefaultResult<Vec<OrderLineModel>, AppError> {
        let oid_b = OidBytes::try_from(oid)?;
        let mut conn = self._db.acquire().await?;
        let mut tx = conn.begin().await?;
        Self::_fetch_lines_by_pid(&mut tx, &oid_b, pids).await
    }
    // TODO, cache the metadata `owner-id` and `create-time` , these records can be shared
    // among the functions : `fetch_ids_by_created_time()`, `owner_id()`, `created_time()`
    async fn fetch_ids_by_created_time(
        &self,
        start: DateTime<FixedOffset>,
        end: DateTime<FixedOffset>,
    ) -> DefaultResult<Vec<String>, AppError> {
        // TODO, to enhance performance, build extra index for the column `create-time`
        let mut conn = self._db.acquire().await?;
        let sql_patt = "SELECT `o_id` FROM `order_toplvl_meta` WHERE \
                        `created_time` >= ? AND `created_time` <= ?";
        let (start, end) = (start.naive_utc(), end.naive_utc());
        let stmt = conn.prepare(sql_patt).await?;
        let query = stmt.query().bind(start).bind(end);
        let exec = conn.as_mut();
        let rows = exec.fetch_all(query).await?;
        let results = rows
            .into_iter()
            .map(|row| to_app_oid(&row, 0))
            .collect::<Vec<DefaultResult<String, AppError>>>();
        let o_meta = if let Some(Err(e)) = results.iter().find(|r| r.is_err()) {
            return Err(e.to_owned());
        } else {
            results.into_iter().map(|r| r.unwrap()).collect::<Vec<_>>()
        };
        Ok(o_meta)
    }
    async fn owner_id(&self, oid: &str) -> DefaultResult<u32, AppError> {
        let OidBytes(oid_b) = OidBytes::try_from(oid)?;
        let sql_patt = "SELECT `usr_id` FROM `order_toplvl_meta` WHERE `o_id`=?";
        let mut conn = self._db.acquire().await?;
        let stmt = conn.prepare(sql_patt).await?;
        let query = stmt.query().bind(oid_b.to_vec());
        let exec = conn.as_mut();
        let row = exec.fetch_one(query).await?;
        let owner_id = row.try_get::<u32, usize>(0)?;
        Ok(owner_id)
    }
    async fn created_time(&self, oid: &str) -> DefaultResult<DateTime<FixedOffset>, AppError> {
        let OidBytes(oid_b) = OidBytes::try_from(oid)?;
        let sql_patt = "SELECT `created_time` FROM `order_toplvl_meta` WHERE `o_id`=?";
        let mut conn = self._db.acquire().await?;
        let stmt = conn.prepare(sql_patt).await?;
        let query = stmt.query().bind(oid_b.to_vec());
        let exec = conn.as_mut();
        let row = exec.fetch_one(query).await?;
        let ctime = row.try_get::<NaiveDateTime, usize>(0)?.and_utc().into();
        Ok(ctime)
    }

    async fn currency_exrates(&self, oid: &str) -> DefaultResult<OrderCurrencyModel, AppError> {
        let oid_b = OidBytes::try_from(oid)?;
        let sql_patt = "SELECT `buyer_currency`, `buyer_ex_rate` FROM `order_toplvl_meta` \
                        WHERE `o_id`=?";
        let mut conn = self._db.acquire().await?;
        let buyer = {
            let stmt = conn.prepare(sql_patt).await?;
            let query = stmt.query().bind(oid_b.as_column());
            let exec = &mut *conn;
            let row = exec.fetch_one(query).await?;
            BuyerCurrencyRow(&row, 0).try_into()?
        };
        let sellers = Self::_fetch_seller_exrates(conn.as_mut(), oid_b.as_column()).await?;
        Ok(OrderCurrencyModel { buyer, sellers })
    }

    async fn cancel_unpaid_last_time(&self) -> DefaultResult<DateTime<FixedOffset>, AppError> {
        let sql_patt = "SELECT `last_update` FROM `schedule_job`";
        let mut conn = self._db.acquire().await?;
        let stmt = conn.prepare(sql_patt).await?;
        let query = stmt.query();
        let exec = conn.as_mut();
        let row = exec.fetch_one(query).await?;
        let utime = row.try_get::<NaiveDateTime, usize>(0)?;
        let t = utime.and_utc().fixed_offset();
        Ok(t)
    }
    async fn cancel_unpaid_time_update(&self) -> DefaultResult<(), AppError> {
        let mut conn = self._db.acquire().await?;
        let sql_patt = "UPDATE `schedule_job` SET `last_update`=?";
        let t = Local::now().naive_utc();
        let stmt = conn.prepare(sql_patt).await?;
        let query = stmt.query().bind(t);
        let exec = &mut *conn;
        let resultset = query.execute(exec).await?;
        let _num_affected = resultset.rows_affected();
        Ok(())
    }
} // end of trait AbsOrderRepo

impl OrderMariaDbRepo {
    pub(crate) async fn new(
        dbs: Vec<Arc<AppMariaDbStore>>,
        timenow: DateTime<FixedOffset>,
    ) -> DefaultResult<Self, AppError> {
        if dbs.is_empty() {
            Err(AppError {
                code: AppErrorCode::MissingDataStore,
                detail: Some("mariadb".to_string()),
            })
        } else {
            let _db = dbs.first().unwrap().clone();
            let stockrepo = StockMariaDbRepo::new(timenow, _db.clone());
            Ok(Self {
                _db,
                _stock: Arc::new(Box::new(stockrepo)),
            })
        }
        // TODO, consider to balance loads of order request to different database servers
        // , currently this repo selects only the first db pool
    }
    pub(super) async fn create_lines(
        tx: &mut Transaction<'_, MySql>,
        ol_set: &OrderLineModelSet,
        limit: usize,
    ) -> DefaultResult<(), AppError> {
        let (oid, usr_id, ctime, olines) = (
            ol_set.id().as_str(),
            ol_set.owner(),
            ol_set.create_time(),
            ol_set.lines(),
        );
        if olines.len() > hard_limit::MAX_ORDER_LINES_PER_REQUEST {
            let d = format!(
                "actual: {}, limit:{}",
                olines.len(),
                hard_limit::MAX_ORDER_LINES_PER_REQUEST
            );
            let e = AppError {
                code: AppErrorCode::ExceedingMaxLimit,
                detail: Some(d),
            };
            return Err(e);
        }
        let oid = OidBytes::try_from(oid)?;
        {
            // check precision of currency rates, should not exceed the limit
            ol_set.currency().buyer.check_rate_range()?;
            let ms = ol_set.currency().sellers.values().collect::<Vec<_>>();
            CurrencyModel::check_rate_range_multi(ms)?;
        }
        let (sql_patt, args) =
            InsertTopMetaArg(&oid, usr_id, &ctime, &ol_set.currency().buyer).into();
        let _rs = run_query_once(tx, sql_patt, args, Some(1)).await?;

        let (sql_patt, args) = InsertSellerCurrencyArg(&oid, &ol_set.currency().sellers).into();
        let _rs = run_query_once(tx, sql_patt, args, Some(ol_set.currency().sellers.len())).await?;

        let mut num_processed = 0;
        let mut data = olines.iter().collect::<Vec<_>>();
        while !data.is_empty() {
            let num_batch = min(data.len(), limit);
            let items_processing = data.split_off(data.len() - num_batch);
            assert!(!items_processing.is_empty());
            assert_eq!(items_processing.len(), num_batch);
            let (sql_patt, args) = InsertOLineArg(&oid, num_processed, items_processing).into();
            let _rs = run_query_once(tx, sql_patt, args, Some(num_batch)).await?;
            num_processed += num_batch;
        } // end of loop
        Ok(())
    } // end of fn create_lines

    async fn _save_contact(
        tx: &mut Transaction<'_, MySql>,
        oid: &OidBytes,
        table_opt: &str,
        data: ContactModel,
    ) -> DefaultResult<(), AppError> {
        if data.emails.is_empty() && data.phones.is_empty() {
            let d = "save-contact, num-emails:0, num-phones:0".to_string();
            let e = AppError {
                code: AppErrorCode::InvalidInput,
                detail: Some(d),
            };
            return Err(e);
        }
        let (f_name, l_name, emails, phones) =
            (data.first_name, data.last_name, data.emails, data.phones);
        let (num_mails, num_phones) = (emails.len(), phones.len());
        let (sql_patt, args) = InsertContactMeta(table_opt, oid, f_name, l_name).into();
        let _rs = run_query_once(tx, sql_patt, args, Some(1)).await?;
        if num_mails > 0 {
            let (sql_patt, args) = InsertContactEmail(table_opt, oid, emails).into();
            let _rs = run_query_once(tx, sql_patt, args, Some(num_mails)).await?;
        }
        if num_phones > 0 {
            let (sql_patt, args) = InsertContactPhone(table_opt, oid, phones).into();
            let _rs = run_query_once(tx, sql_patt, args, Some(num_phones)).await?;
        }
        Ok(())
    }
    async fn _save_phyaddr(
        tx: &mut Transaction<'_, MySql>,
        oid: &OidBytes,
        table_opt: &str,
        data: PhyAddrModel,
    ) -> DefaultResult<(), AppError> {
        let (sql_patt, args) = InsertPhyAddr(table_opt, oid, data).into();
        let _rs = run_query_once(tx, sql_patt, args, Some(1)).await?;
        Ok(())
    }
    async fn _save_ship_opt(
        tx: &mut Transaction<'_, MySql>,
        oid: &OidBytes,
        data: Vec<ShippingOptionModel>,
    ) -> DefaultResult<(), AppError> {
        if data.is_empty() {
            let d = "save-ship-option, num:0".to_string();
            let e = AppError {
                code: AppErrorCode::InvalidInput,
                detail: Some(d),
            };
            return Err(e);
        }
        let num_sellers = data.len();
        let (sql_patt, args) = InsertShipOption(oid, data).into();
        let _rs = run_query_once(tx, sql_patt, args, Some(num_sellers)).await?;
        Ok(())
    }

    async fn _fetch_lines_by_pid(
        tx: &mut Transaction<'_, MySql>,
        oid: &OidBytes,
        pids: Vec<OrderLineIdentity>,
    ) -> DefaultResult<Vec<OrderLineModel>, AppError> {
        let (sql_patt, args) = FetchLineByIdArg(oid, pids).into();
        let stmt = tx.prepare(sql_patt.as_str()).await?;
        let query = stmt.query_with(args);
        let exec = &mut *tx;
        let rows = exec.fetch_all(query).await?;
        let results = rows
            .into_iter()
            .map(|row| OLineRow(row).try_into())
            .collect::<Vec<DefaultResult<OrderLineModel, AppError>>>();
        if let Some(Err(e)) = results.iter().find(|r| r.is_err()) {
            Err(e.to_owned())
        } else {
            let out = results.into_iter().map(|r| r.unwrap()).collect::<Vec<_>>();
            Ok(out)
        }
    }
    async fn _fetch_mails(
        conn: &mut MySqlConnection,
        table_opt: &str,
        oid_b: &OidBytes,
    ) -> DefaultResult<Vec<String>, AppError> {
        let sql_patt = format!(
            "SELECT `mail` FROM `{}_contact_email` WHERE `o_id`=?",
            table_opt
        );
        let stmt = conn.prepare(sql_patt.as_str()).await?;
        let query = stmt.query().bind(oid_b.as_column());
        let rows = conn.fetch_all(query).await?;
        let results = rows
            .into_iter()
            .map(|row| EmailRow(row).try_into())
            .collect::<Vec<DefaultResult<String, AppError>>>();
        if let Some(Err(e)) = results.iter().find(|r| r.is_err()) {
            Err(e.to_owned())
        } else {
            let out = results.into_iter().map(|r| r.unwrap()).collect::<Vec<_>>();
            Ok(out)
        }
    }
    async fn _fetch_phones(
        conn: &mut MySqlConnection,
        table_opt: &str,
        oid_b: &OidBytes,
    ) -> DefaultResult<Vec<PhoneNumberDto>, AppError> {
        let sql_patt = format!(
            "SELECT `nation`,`number` FROM `{}_contact_phone` WHERE `o_id`=?",
            table_opt
        );
        let stmt = conn.prepare(sql_patt.as_str()).await?;
        let query = stmt.query().bind(oid_b.as_column());
        let rows = conn.fetch_all(query).await?;
        let results = rows
            .into_iter()
            .map(|row| PhoneRow(row).try_into())
            .collect::<Vec<DefaultResult<PhoneNumberDto, AppError>>>();
        if let Some(Err(e)) = results.iter().find(|r| r.is_err()) {
            Err(e.to_owned())
        } else {
            let out = results.into_iter().map(|r| r.unwrap()).collect::<Vec<_>>();
            Ok(out)
        }
    }
    async fn _fetch_contact_meta(
        conn: &mut MySqlConnection,
        table_opt: &str,
        oid_b: &OidBytes,
    ) -> DefaultResult<ContactModel, AppError> {
        let sql_patt = format!(
            "SELECT `first_name`,`last_name` FROM `{}_contact_meta` WHERE `o_id`=?",
            table_opt
        );
        let stmt = conn.prepare(sql_patt.as_str()).await?;
        let query = stmt.query().bind(oid_b.as_column());
        let row = conn.fetch_one(query).await?;
        let out = ContactMetaRow(row).try_into()?;
        Ok(out)
    }
    async fn _fetch_phyaddr(
        conn: &mut MySqlConnection,
        table_opt: &str,
        oid_b: &OidBytes,
    ) -> DefaultResult<Option<PhyAddrModel>, AppError> {
        let sql_patt = format!(
            "SELECT `country`,`region`,`city`,`distinct`,`street`,\
                `detail` FROM `{}_phyaddr` WHERE `o_id`=?",
            table_opt
        );
        let stmt = conn.prepare(sql_patt.as_str()).await?;
        let query = stmt.query().bind(oid_b.as_column());
        let result = conn.fetch_optional(query).await?;
        if let Some(row) = result {
            let out = PhyAddrrRow(row).try_into()?;
            Ok(Some(out))
        } else {
            Ok(None)
        }
    }
    async fn _fetch_ship_option(
        conn: &mut MySqlConnection,
        oid_b: &OidBytes,
    ) -> DefaultResult<Vec<ShippingOptionModel>, AppError> {
        let sql_patt = "SELECT `seller_id`,`method` FROM `ship_option` WHERE `o_id`=?";
        let stmt = conn.prepare(sql_patt).await?;
        let query = stmt.query().bind(oid_b.as_column());
        let rows = conn.fetch_all(query).await?;
        let results = rows
            .into_iter()
            .map(|row| ShipOptionRow(row).try_into())
            .collect::<Vec<DefaultResult<ShippingOptionModel, AppError>>>();
        if let Some(Err(e)) = results.iter().find(|r| r.is_err()) {
            Err(e.to_owned())
        } else {
            let out = results.into_iter().map(|r| r.unwrap()).collect::<Vec<_>>();
            Ok(out)
        }
    }

    async fn _fetch_seller_exrates(
        conn: &mut MySqlConnection,
        oid_raw: Vec<u8>,
    ) -> DefaultResult<HashMap<u32, CurrencyModel>, AppError> {
        let sql_patt =
            "SELECT `seller_id`,`label`,`ex_rate` FROM `oseller_currency_snapshot` WHERE `o_id`=?";
        let stmt = conn.prepare(sql_patt).await?;
        let query = stmt.query().bind(oid_raw);
        let rows = conn.fetch_all(query).await?;
        let mut errors = Vec::new();
        let iter = rows.into_iter().filter_map(|row| {
            SellerCurrencyRow(row)
                .try_into()
                .map_err(|e| errors.push(e))
                .ok()
        });
        let map = HashMap::from_iter(iter);
        if errors.is_empty() {
            Ok(map)
        } else {
            Err(errors.remove(0))
        }
    }
} // end of impl OrderMariaDbRepo
