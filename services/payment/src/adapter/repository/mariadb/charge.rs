use std::collections::HashMap;
use std::result::Result;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use mysql_async::prelude::{Query, Queryable, WithParams};
use mysql_async::{Conn, IsolationLevel, Params, TxOpts};

use ecommerce_common::api::dto::CurrencyDto;
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::logging::{app_log_event, AppLogLevel};
use ecommerce_common::model::order::BillingModel;

use crate::adapter::datastore::{AppDStoreMariaDB, AppDataStoreContext};
use crate::model::{
    ChargeBuyerMetaModel, ChargeBuyerModel, ChargeLineBuyerModel, OrderCurrencySnapshot,
    OrderLineModel, OrderLineModelSet,
};

use super::super::{AbstractChargeRepo, AppRepoError, AppRepoErrorDetail, AppRepoErrorFnLabel};
use super::charge_converter::InsertChargeArgs;
use super::order_replica::{
    FetchUnpaidOlineArgs, InsertOrderReplicaArgs, OrderCurrencyRowType, OrderlineRowType,
};

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

    async fn try_build_currency_snapshot(
        exec: &mut Conn,
        stmt: String,
        params: Params,
    ) -> Result<HashMap<u32, OrderCurrencySnapshot>, AppRepoErrorDetail> {
        let rows = stmt
            .with(params)
            .fetch::<OrderCurrencyRowType, &mut Conn>(exec)
            .await
            .map_err(|e| AppRepoErrorDetail::DatabaseQuery(e.to_string()))?;
        let iter = rows.into_iter().map(|(usr_id, label_raw, rate)| {
            let label = CurrencyDto::from(&label_raw);
            let v = OrderCurrencySnapshot { label, rate };
            (usr_id, v)
        });
        Ok(HashMap::from_iter(iter))
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
        let currency_snapshot = Self::try_build_currency_snapshot(exec, stmt, param)
            .await
            .map_err(|de| self._map_err_get_unpaid_olines(de))?;
        let mut toplvl_result = {
            let (stmt, param) = args_iter.next().unwrap();
            let result = stmt
                .with(param)
                .first::<(mysql_async::Value, u32), &mut Conn>(exec)
                .await
                .map_err(|e| {
                    self._map_err_get_unpaid_olines(AppRepoErrorDetail::DatabaseQuery(
                        e.to_string(),
                    ))
                })?
                .map(|(str_time, num_charges)| {
                    let arg = (usr_id, oid, str_time, num_charges, currency_snapshot);
                    OrderLineModelSet::try_from(arg)
                });
            if let Some(v) = result {
                let inner = v?;
                Some(inner)
            } else {
                None
            }
        };
        if let Some(v) = &mut toplvl_result {
            // --- order lines ---
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

    async fn fetch_charge_meta(
        &self,
        _usr_id: u32,
        _create_time: DateTime<Utc>,
    ) -> Result<Option<ChargeBuyerMetaModel>, AppRepoError> {
        Err(AppRepoError {
            fn_label: AppRepoErrorFnLabel::FetchChargeMeta,
            code: AppErrorCode::NotImplemented,
            detail: AppRepoErrorDetail::Unknown,
        })
    }

    async fn fetch_all_charge_lines(
        &self,
        _usr_id: u32,
        _create_time: DateTime<Utc>,
    ) -> Result<Vec<ChargeLineBuyerModel>, AppRepoError> {
        Err(AppRepoError {
            fn_label: AppRepoErrorFnLabel::FetchAllChargeLines,
            code: AppErrorCode::NotImplemented,
            detail: AppRepoErrorDetail::Unknown,
        })
    }

    async fn update_charge_progress(
        &self,
        _meta: ChargeBuyerMetaModel,
    ) -> Result<(), AppRepoError> {
        Err(AppRepoError {
            fn_label: AppRepoErrorFnLabel::UpdateChargeProgress,
            code: AppErrorCode::NotImplemented,
            detail: AppRepoErrorDetail::Unknown,
        })
    }
} // end of impl MariadbChargeRepo
