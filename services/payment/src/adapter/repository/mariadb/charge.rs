use std::result::Result;
use std::sync::Arc;

use async_trait::async_trait;
use futures_util::StreamExt;
use mysql_async::prelude::{Queryable, Query, WithParams};
use mysql_async::{IsolationLevel, Params, TxOpts, Conn};

use ecommerce_common::adapter::repository::OidBytes;
use ecommerce_common::constant::ProductType;
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::model::BaseProductIdentity;
use ecommerce_common::model::order::{BillingModel, ContactModel, PhyAddrModel};

use super::super::{AbstractChargeRepo, AppRepoError, AppRepoErrorDetail, AppRepoErrorFnLabel};
use super::raw_column_to_datetime;
use crate::adapter::datastore::{AppDStoreMariaDB, AppDataStoreContext};
use crate::model::{ChargeBuyerModel, OrderLineModelSet, OrderLineModel, PayLineAmountModel};

struct InsertOrderTopLvlArgs(String, Params);
struct InsertOrderLineArgs(String, Vec<Params>);
struct InsertBillContactArgs(String, Params);
struct InsertBillPhyAddrArgs(String, Params);
struct InsertOrderReplicaArgs(Vec<(String, Vec<Params>)>);
struct FetchUnpaidOlineArgs([(String, Params); 2]);

const DATETIME_FMT_P0F:&'static str = "%Y-%m-%d %H:%M:%S";
type OrderlineRowType = (u32,String,u64,u32,u32,u32,u32,u32,mysql_async::Value);

impl<'a, 'b> From<(&'a OrderLineModelSet, &'b OidBytes)> for InsertOrderTopLvlArgs {
    fn from(value: (&'a OrderLineModelSet, &'b OidBytes)) -> Self {
        let (ol_set, oid_b) = value;
        let arg = vec![
            ol_set.owner.into(),
            oid_b.as_column().into(),
            ol_set
                .create_time
                .format(DATETIME_FMT_P0F)
                .to_string()
                .into(),
            ol_set.num_charges.into(),
        ];
        let params = Params::Positional(arg);
        let stmt = "INSERT INTO `order_toplvl_meta`(`usr_id`, `o_id`, \
            `create_time`, `num_charges`) VALUES (?,?,?,?)";
        Self(stmt.to_string(), params)
    }
}
impl<'a, 'b> From<(&'a OrderLineModelSet, &'b OidBytes)> for InsertOrderLineArgs {
    fn from(value: (&'a OrderLineModelSet, &'b OidBytes)) -> Self {
        let (ol_set, oid_b) = value;
        let params = ol_set
            .lines
            .iter()
            .map(|line| {
                let prod_type_num: u8 = line.pid.product_type.clone().into();
                let arg = vec![
                    oid_b.as_column().into(),
                    line.pid.store_id.into(),
                    prod_type_num.to_string().into(),
                    line.pid.product_id.into(),
                    line.rsv_total.unit.into(),
                    line.rsv_total.total.into(),
                    line.paid_total.total.into(),
                    line.rsv_total.qty.into(),
                    line.paid_total.qty.into(),
                    line.reserved_until
                        .to_utc()
                        .format(DATETIME_FMT_P0F)
                        .to_string()
                        .into(),
                ];
                Params::Positional(arg)
            })
            .collect::<Vec<_>>();
        let stmt = "INSERT INTO `order_line_detail`(`o_id`,`store_id`, \
            `product_type`,`product_id`,`price_unit`,`price_total_rsved`, \
            `price_total_paid`,`qty_rsved`,`qty_paid`,`rsved_until`) \
            VALUES (?,?,?,?,?, ?,?,?,?,?)";
        Self(stmt.to_string(), params)
    }
} // end of impl InsertOrderLineArgs

impl<'a, 'b> TryFrom<(&'a ContactModel, &'b OidBytes)> for InsertBillContactArgs {
    type Error = AppRepoError;
    fn try_from(value: (&'a ContactModel, &'b OidBytes)) -> Result<Self, Self::Error> {
        let (contact, oid_b) = value;
        let serial_mails =
            serde_json::to_string(&contact.emails).map_err(Self::map_contact_error)?;
        let serial_phones =
            serde_json::to_string(&contact.phones).map_err(Self::map_contact_error)?;
        let arg = vec![
            oid_b.0.into(),
            contact.first_name.as_str().into(),
            contact.last_name.as_str().into(),
            serial_mails.into(),
            serial_phones.into(),
        ];
        let params = Params::Positional(arg);
        let stmt = "INSERT INTO `orderbill_contact`(`o_id`,`first_name`,`last_name`, \
                    `emails`,`phones`) VALUES (?,?,?,?,?)";
        Ok(Self(stmt.to_string(), params))
    }
}
impl InsertBillContactArgs {
    fn map_contact_error(e: serde_json::Error) -> AppRepoError {
        AppRepoError {
            code: AppErrorCode::InvalidInput,
            fn_label: AppRepoErrorFnLabel::CreateOrder,
            detail: AppRepoErrorDetail::OrderContactInfo(e.to_string()),
        }
    }
}

impl<'a, 'b> From<(&'a PhyAddrModel, &'b OidBytes)> for InsertBillPhyAddrArgs {
    fn from(value: (&'a PhyAddrModel, &'b OidBytes)) -> Self {
        let (addr, oid_b) = value;
        let arg = vec![
            oid_b.as_column().into(),
            String::from(addr.country.clone()).into(),
            addr.region.as_str().into(),
            addr.city.as_str().into(),
            addr.distinct.as_str().into(),
            addr.street_name.as_deref().into(),
            addr.detail.as_str().into(),
        ];
        let params = Params::Positional(arg);
        let stmt = "INSERT INTO `orderbill_phyaddr`(`o_id`,`country`,`region`,`city`, \
                    `distinct`,`street`,`detail`) VALUES (?,?,?,?,?,?,?)";
        Self(stmt.to_string(), params)
    }
}

impl<'a, 'b> TryFrom<(&'a OrderLineModelSet, &'b BillingModel)> for InsertOrderReplicaArgs {
    type Error = AppRepoError;
    fn try_from(value: (&'a OrderLineModelSet, &'b BillingModel)) -> Result<Self, Self::Error> {
        let (ol_set, billing) = value;
        let oid_b = OidBytes::try_from(ol_set.id.as_str()).map_err(|(code, msg)| AppRepoError {
            code,
            fn_label: AppRepoErrorFnLabel::CreateOrder,
            detail: AppRepoErrorDetail::OrderIDparse(msg),
        })?;
        let toplvl_arg = InsertOrderTopLvlArgs::from((ol_set, &oid_b));
        let olines_arg = InsertOrderLineArgs::from((ol_set, &oid_b));
        let contact_arg = InsertBillContactArgs::try_from((&billing.contact, &oid_b))?;
        let mut inner = vec![
            (toplvl_arg.0, vec![toplvl_arg.1]),
            (olines_arg.0, olines_arg.1),
            (contact_arg.0, vec![contact_arg.1]),
        ];
        if let Some(a) = &billing.address {
            let phyaddr_arg = InsertBillPhyAddrArgs::from((a, &oid_b));
            inner.push((phyaddr_arg.0, vec![phyaddr_arg.1]));
        }
        Ok(Self(inner))
    }
} // end of impl InsertOrderReplicaArgs

impl<'a> TryFrom<(u32, &'a str)> for FetchUnpaidOlineArgs {
    type Error = AppRepoError;
    fn try_from(value: (u32, &'a str)) -> Result<Self, Self::Error> {
        let (usr_id, oid_hex) = value;
        let oid_b = OidBytes::try_from(oid_hex).map_err(|(code, msg)| 
            AppRepoError {
                code, fn_label: AppRepoErrorFnLabel::GetUnpaidOlines,
                detail: AppRepoErrorDetail::OrderIDparse(msg),
            }
        )?;
        let params = [
            vec![usr_id.into(), oid_b.as_column().into()],
            vec![oid_b.0.into()]
        ].into_iter().map(Params::Positional).collect::<Vec<_>>();
        let stmts = [
            "SELECT `create_time`,`num_charges` FROM `order_toplvl_meta` \
             WHERE `usr_id`=? AND `o_id`=?",
            "SELECT `store_id`,`product_type`,`product_id`,`price_unit`,`price_total_rsved`,\
             `price_total_paid`,`qty_rsved`,`qty_paid`,`rsved_until` FROM \
             `order_line_detail` WHERE `o_id`=?  AND `qty_rsved` > `qty_paid`",
        ].into_iter().map(ToString::to_string).collect::<Vec<_>>();
        let zipped = stmts.into_iter().zip(params).collect::<Vec<_>>();
        let inner = zipped.try_into().unwrap();
        Ok(Self(inner))
    }
} // end of impl FetchUnpaidOlineArgs

impl<'a> TryFrom<(u32, &'a str, mysql_async::Value, u32)> for OrderLineModelSet {
    type Error = AppRepoError;
    fn try_from(value: (u32, &'a str, mysql_async::Value, u32)) -> Result<Self, Self::Error> {
        let (usr_id, oid, ctime, num_charges) = value;
        let create_time = raw_column_to_datetime(ctime, 0)?;
        Ok(OrderLineModelSet {owner:usr_id, id:oid.to_string(),
             create_time, lines:vec![], num_charges})
    }
} // end of impl OrderLineModelSet

impl TryFrom<OrderlineRowType> for OrderLineModel {
    type Error = AppRepoError;
    fn try_from(value: OrderlineRowType) -> Result<Self, Self::Error> {
        let (store_id, prod_typ_str, product_id, price_unit, price_total_rsved,
             price_total_paid, qty_rsved, qty_paid, rsved_until ) = value;
        let product_type = prod_typ_str.parse::<ProductType>().map_err(
            |e| AppRepoError {
                fn_label: AppRepoErrorFnLabel::GetUnpaidOlines,
                code: AppErrorCode::DataCorruption,
                detail: AppRepoErrorDetail::DataRowParse(e.0.to_string()),
            }
        )?;
        let reserved_until = raw_column_to_datetime(rsved_until, 0)?.into();
        let rsv_total = PayLineAmountModel { unit: price_unit, total: price_total_rsved, qty: qty_rsved};
        let paid_total = PayLineAmountModel { unit: price_unit, total: price_total_paid, qty: qty_paid };
        let pid = BaseProductIdentity { store_id, product_type, product_id };
        Ok(Self { pid, rsv_total, paid_total, reserved_until })
    } // end of fn try-from
} // end of impl OrderLineModel


pub(crate) struct MariadbChargeRepo {
    _dstore: Arc<AppDStoreMariaDB>,
}

impl MariadbChargeRepo {
    pub(crate) async fn new(ds: Arc<AppDataStoreContext>) -> Result<Self, AppRepoError> {
        ds.mariadb(None)
            .map(|found| Self { _dstore: found })
            .ok_or(AppRepoError {
                fn_label: AppRepoErrorFnLabel::InitRepo,
                code: AppErrorCode::MissingDataStore,
                detail: AppRepoErrorDetail::Unknown,
            })
    }
}

#[async_trait]
impl AbstractChargeRepo for MariadbChargeRepo {
    async fn get_unpaid_olines(
        &self,
        usr_id: u32,
        oid: &str,
    ) -> Result<Option<OrderLineModelSet>, AppRepoError> {
        let mut args_iter = FetchUnpaidOlineArgs::try_from((usr_id, oid))?.0.into_iter();
        let mut conn = self
            ._dstore
            .acquire()
            .await
            .map_err(|e| Self::_map_err_get_unpaid_olines(AppRepoErrorDetail::DataStore(e)))?;
        let exec = &mut conn;
        let (stmt, param) = args_iter.next().unwrap();
        let result = stmt.with(param)
            .first::<(mysql_async::Value, u32), &mut Conn>(exec)
            .await
            .map_err(|e| Self::_map_err_get_unpaid_olines(
                AppRepoErrorDetail::DatabaseQuery(e.to_string()))
            )?
            .map(|(str_time, num_charges)| {
                OrderLineModelSet::try_from((usr_id, oid, str_time, num_charges))
            });
        let mut toplvl_result = if let Some(v) = result {
            let inner = v?;
            Some(inner)
        } else { None };
        if let Some(v) = &mut toplvl_result {
            let (stmt, param) = args_iter.next().unwrap();
            let mut line_stream = stmt.with(param) 
                .stream::<OrderlineRowType, &mut Conn>(exec)
                .await
                .map_err(|e| Self::_map_err_get_unpaid_olines(
                    AppRepoErrorDetail::DatabaseQuery(e.to_string()))
                )?;
            while let Some(result) = line_stream.next().await {
                let row = result.map_err(|e| Self::_map_err_get_unpaid_olines(
                        AppRepoErrorDetail::DatabaseQuery(e.to_string()))
                    )?;
                let oline = OrderLineModel::try_from(row)?;
                v.lines.push(oline);
            }
        }
        Ok(toplvl_result)
    } // end of fn get-unpaid-olines

    async fn create_order(
        &self,
        ol_set: &OrderLineModelSet,
        billing: &BillingModel,
    ) -> Result<(), AppRepoError> {
        let args = InsertOrderReplicaArgs::try_from((ol_set, billing))?;
        let mut conn = self
            ._dstore
            .acquire()
            .await
            .map_err(|e| Self::_map_err_create_order(AppRepoErrorDetail::DataStore(e)))?;
        let mut options = TxOpts::default();
        options.with_isolation_level(IsolationLevel::RepeatableRead);
        let mut tx = conn.start_transaction(options).await.map_err(|e| {
            Self::_map_err_create_order(AppRepoErrorDetail::DatabaseTxStart(e.to_string()))
        })?;
        for (stmt, params_iter) in args.0 {
            tx.exec_batch(stmt, params_iter).await.map_err(|e| {
                Self::_map_err_create_order(AppRepoErrorDetail::DatabaseExec(e.to_string()))
            })?;
        }
        tx.commit().await.map_err(|e| {
            Self::_map_err_create_order(AppRepoErrorDetail::DatabaseTxCommit(e.to_string()))
        })
    } // end of fn create_order

    async fn create_charge(&self, _cline_set: ChargeBuyerModel) -> Result<(), AppRepoError> {
        let fn_label = AppRepoErrorFnLabel::CreateCharge;
        Err(AppRepoError {
            fn_label,
            code: AppErrorCode::NotImplemented,
            detail: AppRepoErrorDetail::Unknown,
        })
    }
} // end of impl MariadbChargeRepo

impl MariadbChargeRepo {
    fn _map_err_create_order(detail: AppRepoErrorDetail) -> AppRepoError {
        AppRepoError {
            fn_label: AppRepoErrorFnLabel::CreateOrder,
            code: AppErrorCode::Unknown,
            detail,
        }
    }
    fn _map_err_get_unpaid_olines(detail: AppRepoErrorDetail) -> AppRepoError {
        AppRepoError {
            fn_label: AppRepoErrorFnLabel::GetUnpaidOlines,
            code: AppErrorCode::Unknown,
            detail,
        }
    }
} // end of impl MariadbChargeRepo
