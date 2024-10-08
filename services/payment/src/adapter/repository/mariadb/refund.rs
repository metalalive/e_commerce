use std::boxed::Box;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use mysql_async::prelude::{Query, Queryable, WithParams};
use mysql_async::{Conn, IsolationLevel, Params, TxOpts, Value as MySqlVal};

use ecommerce_common::adapter::repository::OidBytes;
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::logging::{app_log_event, AppLogLevel};
use ecommerce_common::model::BaseProductIdentity;

use crate::adapter::datastore::{AppDStoreMariaDB, AppDataStoreContext};
use crate::adapter::processor::AbstractPaymentProcessor;
use crate::api::web::dto::RefundCompletionReqDto;
use crate::model::{ChargeBuyerModel, OrderRefundModel, PayLineAmountModel};

use super::super::{
    AbstractRefundRepo, AppRefundRslvReqCallback, AppRefundRslvReqOkReturn, AppRepoError,
    AppRepoErrorDetail, AppRepoErrorFnLabel,
};
use super::{inner_into_parts, raw_column_to_datetime, DATETIME_FMT_P0F, DATETIME_FMT_P3F};

const JOB_SCHE_LABEL: &str = "refund-req-sync";

struct UpdateLastTimeSyncArgs(String, Params);
struct InsertRequestArgs(String, Vec<Params>);

impl From<DateTime<Utc>> for UpdateLastTimeSyncArgs {
    fn from(value: DateTime<Utc>) -> Self {
        let t = value.format(DATETIME_FMT_P3F).to_string();
        let stmt = "INSERT INTO `job_scheduler`(`label`,`last_update`) VALUES (?,?) \
            ON DUPLICATE KEY UPDATE `last_update`=?"
            .to_string();
        let arg = vec![JOB_SCHE_LABEL.into(), t.as_str().into(), t.as_str().into()];
        let params = Params::Positional(arg);
        Self(stmt, params)
    }
}

inner_into_parts!(UpdateLastTimeSyncArgs);

impl TryFrom<Vec<OrderRefundModel>> for InsertRequestArgs {
    type Error = Vec<(AppErrorCode, AppRepoErrorDetail)>;

    fn try_from(value: Vec<OrderRefundModel>) -> Result<Self, Self::Error> {
        let stmt = "INSERT INTO `oline_refund_req`(`o_id`,`store_id`,`product_type`,`product_id`,\
                    `create_time`,`amt_unit`,`amt_total`,`qty`) VALUES(?,?,?,?,?,?,?,?)";
        let mut errors = Vec::new();
        let final_params = value
            .into_iter()
            .map(Self::try_from_one_req)
            .filter_map(|r| r.map_err(|e| errors.push(e)).ok())
            .flatten()
            .collect::<Vec<_>>();
        if errors.is_empty() {
            let o = Self(stmt.to_string(), final_params);
            Ok(o)
        } else {
            Err(errors)
        }
    } // end of fn try-from
} // end of impl InsertRequestArgs

impl InsertRequestArgs {
    fn try_from_one_req(
        req: OrderRefundModel,
    ) -> Result<Vec<Params>, (AppErrorCode, AppRepoErrorDetail)> {
        let (oid_hex, rlines) = req.into_parts();
        let oid_b = OidBytes::try_from(oid_hex.as_str())
            .map_err(|(code, msg)| (code, AppRepoErrorDetail::OrderIDparse(msg)))?;
        let params = rlines
            .into_iter()
            .map(|line| {
                let (pid, amt, ctime) = line.into_parts();
                let BaseProductIdentity {
                    store_id,
                    product_type,
                    product_id,
                } = pid;
                let PayLineAmountModel { unit, total, qty } = amt;
                let prod_typ_num: u8 = product_type.into();
                let arg = vec![
                    oid_b.as_column().into(),
                    store_id.into(),
                    prod_typ_num.to_string().into(),
                    product_id.into(),
                    ctime.format(DATETIME_FMT_P0F).to_string().into(),
                    unit.into(),
                    total.into(),
                    qty.into(),
                ];
                Params::Positional(arg)
            })
            .collect::<Vec<_>>();
        Ok(params)
    } // end of fn try_from_one_req

    fn into_parts(self) -> (String, Vec<Params>) {
        let Self(stmt, params) = self;
        (stmt, params)
    }
} // end of impl InsertRequestArgs

pub(crate) struct MariaDbRefundRepo {
    _dstore: Arc<AppDStoreMariaDB>,
}

impl MariaDbRefundRepo {
    pub(crate) fn new(ds: Arc<AppDataStoreContext>) -> Result<Self, AppRepoError> {
        ds.mariadb(None)
            .map(|found| Self { _dstore: found })
            .ok_or(AppRepoError {
                fn_label: AppRepoErrorFnLabel::InitRefundRepo,
                code: AppErrorCode::MissingDataStore,
                detail: AppRepoErrorDetail::Unknown,
            })
    }

    #[rustfmt::skip]
    fn _map_log_err_common(
        &self,
        code: AppErrorCode,
        detail: AppRepoErrorDetail,
        fn_label: AppRepoErrorFnLabel,
    ) -> AppRepoError {
        let e = AppRepoError {fn_label, code, detail};
        let logctx = self._dstore.log_context();
        app_log_event!(logctx, AppLogLevel::ERROR, "{:?}", e);
        e
    }
} // end of impl MariaDbRefundRepo

#[async_trait]
impl<'a> AbstractRefundRepo<'a> for MariaDbRefundRepo {
    async fn last_time_synced(&self) -> Result<Option<DateTime<Utc>>, AppRepoError> {
        let stmt = "SELECT `last_update` FROM `job_scheduler` WHERE `label`=?";
        let params = Params::Positional(vec![JOB_SCHE_LABEL.into()]);
        let mut conn = self._dstore.acquire().await.map_err(|e| {
            self._map_log_err_common(
                AppErrorCode::DatabaseServerBusy,
                AppRepoErrorDetail::DataStore(e),
                AppRepoErrorFnLabel::RefundGetTimeSynced,
            )
        })?;
        let result = stmt
            .with(params)
            .first::<(MySqlVal,), &mut Conn>(&mut conn)
            .await
            .map_err(|e| {
                self._map_log_err_common(
                    AppErrorCode::RemoteDbServerFailure,
                    AppRepoErrorDetail::DatabaseQuery(e.to_string()),
                    AppRepoErrorFnLabel::RefundGetTimeSynced,
                )
            })?;
        if let Some(v) = result {
            let t = raw_column_to_datetime(v.0, 3).map_err(|(code, detail)| {
                self._map_log_err_common(code, detail, AppRepoErrorFnLabel::RefundGetTimeSynced)
            })?;
            Ok(Some(t))
        } else {
            Ok(None)
        }
    } // end of fn last_time_synced

    async fn update_sycned_time(&self, t: DateTime<Utc>) -> Result<(), AppRepoError> {
        let mut conn = self._dstore.acquire().await.map_err(|e| {
            self._map_log_err_common(
                AppErrorCode::DatabaseServerBusy,
                AppRepoErrorDetail::DataStore(e),
                AppRepoErrorFnLabel::RefundUpdateTimeSynced,
            )
        })?;
        let (stmt, params) = UpdateLastTimeSyncArgs::from(t).into_parts();
        let result = conn.exec_iter(stmt, params).await.map_err(|e| {
            self._map_log_err_common(
                AppErrorCode::RemoteDbServerFailure,
                AppRepoErrorDetail::DatabaseExec(e.to_string()),
                AppRepoErrorFnLabel::RefundUpdateTimeSynced,
            )
        })?;
        let num_affected = result.affected_rows();
        let success = [1u64, 2].contains(&num_affected);
        if success {
            Ok(())
        } else {
            let msg = format!("rows-affected: {num_affected}");
            Err(self._map_log_err_common(
                AppErrorCode::DataCorruption,
                AppRepoErrorDetail::DatabaseExec(msg),
                AppRepoErrorFnLabel::RefundUpdateTimeSynced,
            ))
        }
    } // end of fn update_sycned_time

    async fn save_request(&self, reqs: Vec<OrderRefundModel>) -> Result<(), AppRepoError> {
        let (stmt, params) = InsertRequestArgs::try_from(reqs)
            .map_err(|mut es| {
                let e = es.remove(0);
                self._map_log_err_common(e.0, e.1, AppRepoErrorFnLabel::RefundSaveReq)
            })?
            .into_parts();
        let mut conn = self._dstore.acquire().await.map_err(|e| {
            self._map_log_err_common(
                AppErrorCode::DatabaseServerBusy,
                AppRepoErrorDetail::DataStore(e),
                AppRepoErrorFnLabel::RefundSaveReq,
            )
        })?;
        let mut tx = {
            let mut options = TxOpts::new();
            // Note, the level `read-committed` is applied here because this function does
            // not actually read back any saved refund request.
            options.with_isolation_level(IsolationLevel::ReadCommitted);
            conn.start_transaction(options).await.map_err(|e| {
                self._map_log_err_common(
                    AppErrorCode::RemoteDbServerFailure,
                    AppRepoErrorDetail::DatabaseTxStart(e.to_string()),
                    AppRepoErrorFnLabel::RefundSaveReq,
                )
            })?
        };

        tx.exec_batch(stmt, params).await.map_err(|e| {
            self._map_log_err_common(
                AppErrorCode::RemoteDbServerFailure,
                AppRepoErrorDetail::DatabaseExec(e.to_string()),
                AppRepoErrorFnLabel::RefundSaveReq,
            )
        })?;
        tx.commit().await.map_err(|e| {
            self._map_log_err_common(
                AppErrorCode::RemoteDbServerFailure,
                AppRepoErrorDetail::DatabaseTxCommit(e.to_string()),
                AppRepoErrorFnLabel::RefundSaveReq,
            )
        })
    } // end of fn save_request

    async fn resolve_request(
        &self,
        _new_req: RefundCompletionReqDto,
        _charge_ms: Vec<ChargeBuyerModel>,
        _processor: Arc<Box<dyn AbstractPaymentProcessor>>,
        _cb: AppRefundRslvReqCallback<'a>,
    ) -> Result<AppRefundRslvReqOkReturn, AppRepoError> {
        Err(AppRepoError {
            fn_label: AppRepoErrorFnLabel::ResolveRefundReq,
            code: AppErrorCode::NotImplemented,
            detail: AppRepoErrorDetail::Unknown,
        })
    }
} // end of impl MariaDbRefundRepo
