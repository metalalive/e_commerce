use std::result::Result;
use std::sync::Arc;

use async_trait::async_trait;
use futures_util::StreamExt;
use mysql_async::prelude::{Query, Queryable, WithParams};
use mysql_async::{Conn, IsolationLevel, TxOpts};

use ecommerce_common::error::AppErrorCode;
use ecommerce_common::logging::{app_log_event, AppLogLevel};
use ecommerce_common::model::order::BillingModel;

use super::super::{AbstractChargeRepo, AppRepoError, AppRepoErrorDetail, AppRepoErrorFnLabel};
use super::charge_converter::{
    FetchUnpaidOlineArgs, InsertChargeArgs, InsertOrderReplicaArgs, OrderlineRowType,
};
use crate::adapter::datastore::{AppDStoreMariaDB, AppDataStoreContext};
use crate::model::{ChargeBuyerModel, OrderLineModel, OrderLineModelSet};

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
    fn _map_err_create_order(&self, detail: AppRepoErrorDetail) -> AppRepoError {
        let e = AppRepoError {
            fn_label: AppRepoErrorFnLabel::CreateOrder,
            code: AppErrorCode::Unknown,
            detail,
        };
        let logctx = self._dstore.log_context();
        app_log_event!(logctx, AppLogLevel::ERROR, "{:?}", e);
        e
    }
    fn _map_err_get_unpaid_olines(&self, detail: AppRepoErrorDetail) -> AppRepoError {
        let e = AppRepoError {
            fn_label: AppRepoErrorFnLabel::GetUnpaidOlines,
            code: AppErrorCode::Unknown,
            detail,
        };
        let logctx = self._dstore.log_context();
        app_log_event!(logctx, AppLogLevel::ERROR, "{:?}", e);
        e
    }
    fn _map_err_create_charge(&self, detail: AppRepoErrorDetail) -> AppRepoError {
        let e = AppRepoError {
            fn_label: AppRepoErrorFnLabel::CreateCharge,
            code: AppErrorCode::Unknown,
            detail,
        };
        let logctx = self._dstore.log_context();
        app_log_event!(logctx, AppLogLevel::ERROR, "{:?}", e);
        e
    }
} // end of impl MariadbChargeRepo

#[async_trait]
impl AbstractChargeRepo for MariadbChargeRepo {
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
            .map_err(|e| self._map_err_create_order(AppRepoErrorDetail::DataStore(e)))?;
        let mut options = TxOpts::default();
        options.with_isolation_level(IsolationLevel::RepeatableRead);
        let mut tx = conn.start_transaction(options).await.map_err(|e| {
            self._map_err_create_order(AppRepoErrorDetail::DatabaseTxStart(e.to_string()))
        })?;
        for (stmt, params_iter) in args.0 {
            tx.exec_batch(stmt, params_iter).await.map_err(|e| {
                self._map_err_create_order(AppRepoErrorDetail::DatabaseExec(e.to_string()))
            })?;
        }
        tx.commit().await.map_err(|e| {
            self._map_err_create_order(AppRepoErrorDetail::DatabaseTxCommit(e.to_string()))
        })
    } // end of fn create_order

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
            .map_err(|e| self._map_err_get_unpaid_olines(AppRepoErrorDetail::DataStore(e)))?;
        let exec = &mut conn;
        let (stmt, param) = args_iter.next().unwrap();
        let result = stmt
            .with(param)
            .first::<(mysql_async::Value, u32), &mut Conn>(exec)
            .await
            .map_err(|e| {
                self._map_err_get_unpaid_olines(AppRepoErrorDetail::DatabaseQuery(e.to_string()))
            })?
            .map(|(str_time, num_charges)| {
                OrderLineModelSet::try_from((usr_id, oid, str_time, num_charges))
            });
        let mut toplvl_result = if let Some(v) = result {
            let inner = v?;
            Some(inner)
        } else {
            None
        };
        if let Some(v) = &mut toplvl_result {
            let (stmt, param) = args_iter.next().unwrap();
            let mut line_stream = stmt
                .with(param)
                .stream::<OrderlineRowType, &mut Conn>(exec)
                .await
                .map_err(|e| {
                    self._map_err_get_unpaid_olines(AppRepoErrorDetail::DatabaseQuery(
                        e.to_string(),
                    ))
                })?;
            while let Some(result) = line_stream.next().await {
                let row = result.map_err(|e| {
                    self._map_err_get_unpaid_olines(AppRepoErrorDetail::DatabaseQuery(
                        e.to_string(),
                    ))
                })?;
                let oline = OrderLineModel::try_from(row)?;
                v.lines.push(oline);
            }
        }
        Ok(toplvl_result)
    } // end of fn get-unpaid-olines

    async fn create_charge(&self, cline_set: ChargeBuyerModel) -> Result<(), AppRepoError> {
        let args = InsertChargeArgs::try_from(cline_set)?;
        let mut conn = self
            ._dstore
            .acquire()
            .await
            .map_err(|e| self._map_err_create_charge(AppRepoErrorDetail::DataStore(e)))?;
        let mut options = TxOpts::new();
        options.with_isolation_level(IsolationLevel::RepeatableRead);
        let mut tx = conn.start_transaction(options).await.map_err(|e| {
            self._map_err_create_charge(AppRepoErrorDetail::DatabaseTxStart(e.to_string()))
        })?;
        for (stmt, params_iter) in args.0 {
            tx.exec_batch(stmt, params_iter).await.map_err(|e| {
                self._map_err_create_charge(AppRepoErrorDetail::DatabaseExec(e.to_string()))
            })?;
        }
        tx.commit().await.map_err(|e| {
            self._map_err_create_order(AppRepoErrorDetail::DatabaseTxCommit(e.to_string()))
        })
    }
} // end of impl MariadbChargeRepo
