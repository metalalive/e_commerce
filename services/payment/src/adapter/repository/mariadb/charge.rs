use std::result::Result;
use std::sync::Arc;

use async_trait::async_trait;
use ecommerce_common::adapter::repository::OidBytes;
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::model::order::{BillingModel, ContactModel, PhyAddrModel};
use mysql_async::prelude::Queryable;
use mysql_async::{IsolationLevel, Params, TxOpts};

use super::super::{AbstractChargeRepo, AppRepoError, AppRepoErrorDetail, AppRepoErrorFnLabel};
use crate::adapter::datastore::{AppDStoreMariaDB, AppDataStoreContext};
use crate::model::{ChargeBuyerModel, OrderLineModelSet};

struct InsertOrderTopLvlArgs(String, Params);
struct InsertOrderLineArgs(String, Vec<Params>);
struct InsertBillContactArgs(String, Params);
struct InsertBillPhyAddrArgs(String, Params);
struct InsertOrderReplicaArgs(Vec<(String, Vec<Params>)>);

impl<'a, 'b> From<(&'a OrderLineModelSet, &'b OidBytes)> for InsertOrderTopLvlArgs {
    fn from(value: (&'a OrderLineModelSet, &'b OidBytes)) -> Self {
        let (ol_set, oid_b) = value;
        let arg = vec![
            ol_set.owner.into(),
            oid_b.as_column().into(),
            ol_set
                .create_time
                .format("%Y-%m-%d %H:%M:%S")
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
                        .format("%Y-%m-%d %H:%M:%S")
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
            oid_b.as_column().into(),
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
        _usr_id: u32,
        _oid: &str,
    ) -> Result<Option<OrderLineModelSet>, AppRepoError> {
        let fn_label = AppRepoErrorFnLabel::GetUnpaidOlines;
        Err(AppRepoError {
            fn_label,
            code: AppErrorCode::NotImplemented,
            detail: AppRepoErrorDetail::Unknown,
        })
    }

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
} // end of impl MariadbChargeRepo
