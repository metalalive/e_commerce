use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use mysql_async::prelude::{Query, Queryable, WithParams};
use mysql_async::{Conn, Params, Value as MySqlVal};

use ecommerce_common::error::AppErrorCode;
use ecommerce_common::logging::{app_log_event, AppLogLevel};

use super::super::{AbstractRefundRepo, AppRepoError, AppRepoErrorDetail, AppRepoErrorFnLabel};
use super::{inner_into_parts, raw_column_to_datetime, DATETIME_FMT_P3F};
use crate::adapter::datastore::{AppDStoreMariaDB, AppDataStoreContext};
use crate::model::OrderRefundModel;

const JOB_SCHE_LABEL: &str = "refund-req-sync";

struct UpdateLastTimeSyncArgs(String, Params);

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
impl AbstractRefundRepo for MariaDbRefundRepo {
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
    }

    async fn save_request(&self, req: Vec<OrderRefundModel>) -> Result<(), AppRepoError> {
        Err(AppRepoError {
            fn_label: AppRepoErrorFnLabel::RefundSaveReq,
            code: AppErrorCode::NotImplemented,
            detail: AppRepoErrorDetail::Unknown,
        })
    }
} // end of impl MariaDbRefundRepo
