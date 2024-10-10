use std::collections::HashMap;
use std::result::Result;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, SubsecRound, Utc};
use futures_util::StreamExt;
use mysql_async::prelude::{Query, Queryable, WithParams};
use mysql_async::{Conn, IsolationLevel, Params, TxOpts};

use ecommerce_common::adapter::repository::OidBytes;
use ecommerce_common::api::dto::CurrencyDto;
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::logging::{app_log_event, AppLogLevel};
use ecommerce_common::model::order::BillingModel;

use crate::adapter::datastore::{AppDStoreMariaDB, AppDataStoreContext};
use crate::model::{
    ChargeBuyerMetaModel, ChargeBuyerModel, ChargeLineBuyerMap, ChargeLineBuyerModel, Label3party,
    OrderCurrencySnapshot, OrderLineModel, OrderLineModelSet, PayoutAmountModel, PayoutModel,
};

use super::super::{AbstractChargeRepo, AppRepoError, AppRepoErrorDetail, AppRepoErrorFnLabel};
use super::charge_converter::{
    ChargeLineRowType, ChargeMetaRowType, FetchChargeLineArgs, FetchChargeMetaArgs,
    InsertChargeArgs, UpdateChargeMetaArgs,
};
use super::order_replica::{
    FetchCurrencySnapshotArgs, FetchUnpaidOlineArgs, InsertOrderReplicaArgs, OrderCurrencyRowType,
    OrderlineRowType,
};
use super::payout::{
    FetchPayout3partyArgs, FetchPayoutMetaArgs, InsertPayout3partyArgs, InsertPayoutMetaArgs,
    PayoutMetaRowType,
};
use super::raw_column_to_datetime;

pub(crate) struct MariadbChargeRepo {
    _dstore: Arc<AppDStoreMariaDB>,
}

impl MariadbChargeRepo {
    pub(crate) async fn new(ds: Arc<AppDataStoreContext>) -> Result<Self, AppRepoError> {
        ds.mariadb(None)
            .map(|found| Self { _dstore: found })
            .ok_or(AppRepoError {
                fn_label: AppRepoErrorFnLabel::InitChargeRepo,
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

    async fn _fetch_charge_meta(
        exec: &mut Conn,
        usr_id: u32,
        create_time: DateTime<Utc>,
    ) -> Result<Option<ChargeBuyerMetaModel>, (AppErrorCode, AppRepoErrorDetail)> {
        let create_time = create_time.trunc_subsecs(0);
        let (stmt, params) = FetchChargeMetaArgs::from((usr_id, create_time)).into_parts();
        let raw = stmt
            .with(params)
            .first::<ChargeMetaRowType, &mut Conn>(exec)
            .await
            .map_err(|e| {
                let code = AppErrorCode::RemoteDbServerFailure;
                let detail = AppRepoErrorDetail::DatabaseQuery(e.to_string());
                (code, detail)
            })?;
        if let Some(v) = raw {
            let args = (usr_id, create_time, v);
            let obj = ChargeBuyerMetaModel::try_from(args)?;
            Ok(Some(obj))
        } else {
            Ok(None)
        }
    } // end of fn _fetch_charge_meta

    async fn fetch_charge_lines(
        exec: &mut Conn,
        usr_id: u32, // buyer id
        create_time: DateTime<Utc>,
        maybe_store_id: Option<u32>,
    ) -> Result<Vec<ChargeLineBuyerModel>, (AppErrorCode, AppRepoErrorDetail)> {
        let create_time = create_time.trunc_subsecs(0);
        let (stmt, params) =
            FetchChargeLineArgs::from((usr_id, create_time, maybe_store_id)).into_parts();
        let raw = stmt
            .with(params)
            .fetch::<ChargeLineRowType, &mut Conn>(exec)
            .await
            .map_err(|e| {
                let code = AppErrorCode::RemoteDbServerFailure;
                let detail = AppRepoErrorDetail::DatabaseQuery(e.to_string());
                (code, detail)
            })?;
        let mut errors = Vec::new();
        let lines = raw
            .into_iter()
            .filter_map(|v| {
                ChargeLineBuyerModel::try_from(v)
                    .map_err(|e| errors.push(e))
                    .ok()
            })
            .collect::<Vec<_>>();
        if errors.is_empty() {
            Ok(lines)
        } else {
            let detail = errors.remove(0);
            let code = AppErrorCode::DataCorruption;
            Err((code, detail))
        }
    } // end of fn fetch_charge_lines

    #[rustfmt::skip]
    fn _map_log_err_common(
        &self,
        reason : (AppErrorCode, AppRepoErrorDetail),
        fn_label: AppRepoErrorFnLabel,
    ) -> AppRepoError {
        let (code, detail) = reason;
        let e = AppRepoError { fn_label, code, detail };
        let logctx = self._dstore.log_context();
        app_log_event!(logctx, AppLogLevel::ERROR, "{:?}", e);
        e
    }

    fn _map_err_create_order(&self, detail: AppRepoErrorDetail) -> AppRepoError {
        self._map_log_err_common(
            (AppErrorCode::Unknown, detail),
            AppRepoErrorFnLabel::CreateOrder,
        )
    }
    fn _map_err_get_unpaid_olines(&self, detail: AppRepoErrorDetail) -> AppRepoError {
        self._map_log_err_common(
            (AppErrorCode::Unknown, detail),
            AppRepoErrorFnLabel::GetUnpaidOlines,
        )
    }
    fn _map_err_create_charge(&self, detail: AppRepoErrorDetail) -> AppRepoError {
        self._map_log_err_common(
            (AppErrorCode::Unknown, detail),
            AppRepoErrorFnLabel::CreateCharge,
        )
    }
    fn _map_err_update_charge_progress(
        &self,
        code: AppErrorCode,
        detail: AppRepoErrorDetail,
    ) -> AppRepoError {
        self._map_log_err_common((code, detail), AppRepoErrorFnLabel::UpdateChargeProgress)
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

    async fn fetch_charge_ids(
        &self,
        _oid: &str,
    ) -> Result<Option<(u32, Vec<DateTime<Utc>>)>, AppRepoError> {
        Err(AppRepoError {
            fn_label: AppRepoErrorFnLabel::FetchChargeIds,
            code: AppErrorCode::NotImplemented,
            detail: AppRepoErrorDetail::Unknown,
        })
    }

    async fn fetch_charge_meta(
        &self,
        usr_id: u32,
        create_time: DateTime<Utc>,
    ) -> Result<Option<ChargeBuyerMetaModel>, AppRepoError> {
        let mut conn = self._dstore.acquire().await.map_err(|e| {
            self._map_log_err_common(
                (
                    AppErrorCode::DatabaseServerBusy,
                    AppRepoErrorDetail::DataStore(e),
                ),
                AppRepoErrorFnLabel::FetchChargeMeta,
            )
        })?;
        Self::_fetch_charge_meta(&mut conn, usr_id, create_time)
            .await
            .map_err(|reason| {
                self._map_log_err_common(reason, AppRepoErrorFnLabel::FetchChargeMeta)
            })
    }

    async fn fetch_all_charge_lines(
        &self,
        usr_id: u32,
        create_time: DateTime<Utc>,
    ) -> Result<Vec<ChargeLineBuyerModel>, AppRepoError> {
        let mut conn = self._dstore.acquire().await.map_err(|e| {
            self._map_log_err_common(
                (
                    AppErrorCode::DatabaseServerBusy,
                    AppRepoErrorDetail::DataStore(e),
                ),
                AppRepoErrorFnLabel::FetchChargeLines,
            )
        })?;
        Self::fetch_charge_lines(&mut conn, usr_id, create_time, None)
            .await
            .map_err(|reason| {
                self._map_log_err_common(reason, AppRepoErrorFnLabel::FetchChargeLines)
            })
    }

    async fn update_charge_progress(&self, meta: ChargeBuyerMetaModel) -> Result<(), AppRepoError> {
        let arg = UpdateChargeMetaArgs::try_from(meta)
            .map_err(|(code, detail)| self._map_err_update_charge_progress(code, detail))?;
        let (stmt, params) = arg.into_parts();
        let mut conn = self._dstore.acquire().await.map_err(|e| {
            let code = AppErrorCode::DatabaseServerBusy;
            let detail = AppRepoErrorDetail::DataStore(e);
            self._map_err_update_charge_progress(code, detail)
        })?;
        let result = stmt
            .with(params)
            .run::<&mut Conn>(&mut conn)
            .await
            .map_err(|e| {
                let code = AppErrorCode::RemoteDbServerFailure;
                let detail = AppRepoErrorDetail::DatabaseExec(e.to_string());
                self._map_err_update_charge_progress(code, detail)
            })?;
        let num_affected = result.affected_rows();
        if num_affected == 1u64 {
            Ok(())
        } else {
            let code = AppErrorCode::Unknown;
            let msg = format!("num-affected-rows : {num_affected}");
            let detail = AppRepoErrorDetail::DatabaseExec(msg);
            Err(self._map_err_update_charge_progress(code, detail))
        }
    } // end of fn update_charge_progress

    async fn update_lines_refund(&self, _cl_map: ChargeLineBuyerMap) -> Result<(), AppRepoError> {
        Err(AppRepoError {
            fn_label: AppRepoErrorFnLabel::UpdateChargeLinesRefund,
            code: AppErrorCode::NotImplemented,
            detail: AppRepoErrorDetail::Unknown,
        })
    }

    async fn fetch_charge_by_merchant(
        &self,
        buyer_id: u32,
        create_time: DateTime<Utc>,
        store_id: u32,
    ) -> Result<Option<ChargeBuyerModel>, AppRepoError> {
        let mut conn = self._dstore.acquire().await.map_err(|e| {
            self._map_log_err_common(
                (
                    AppErrorCode::DatabaseServerBusy,
                    AppRepoErrorDetail::DataStore(e),
                ),
                AppRepoErrorFnLabel::FetchChargeByMerchant,
            )
        })?;
        let option_meta = Self::_fetch_charge_meta(&mut conn, buyer_id, create_time)
            .await
            .map_err(|reason| {
                self._map_log_err_common(reason, AppRepoErrorFnLabel::FetchChargeByMerchant)
            })?;
        let meta = if let Some(v) = option_meta {
            v
        } else {
            return Ok(None);
        };

        let lines = Self::fetch_charge_lines(&mut conn, buyer_id, create_time, Some(store_id))
            .await
            .map_err(|reason| {
                self._map_log_err_common(reason, AppRepoErrorFnLabel::FetchChargeByMerchant)
            })?;

        let currency_snapshot = {
            let oid_ref = meta.oid().as_str();
            let args = (oid_ref, Some([buyer_id, store_id]));
            let (stmt, values) = FetchCurrencySnapshotArgs::try_from(args)
                .map_err(|reason| {
                    self._map_log_err_common(reason, AppRepoErrorFnLabel::FetchChargeByMerchant)
                })?
                .into_parts();
            let params = Params::Positional(values);
            Self::try_build_currency_snapshot(&mut conn, stmt, params)
                .await
                .map_err(|detail| {
                    self._map_log_err_common(
                        (AppErrorCode::Unknown, detail),
                        AppRepoErrorFnLabel::FetchChargeByMerchant,
                    )
                })?
        };
        Ok(Some(ChargeBuyerModel {
            meta,
            lines,
            currency_snapshot,
        }))
    } // end of fn fetch_charge_by_merchant

    async fn fetch_payout(
        &self,
        store_id: u32,
        buyer_usr_id: u32,
        charged_ctime: DateTime<Utc>,
    ) -> Result<Option<PayoutModel>, AppRepoError> {
        let mut conn = self._dstore.acquire().await.map_err(|e| {
            self._map_log_err_common(
                (
                    AppErrorCode::DatabaseServerBusy,
                    AppRepoErrorDetail::DataStore(e),
                ),
                AppRepoErrorFnLabel::FetchPayout,
            )
        })?;
        let (stmt, params) = {
            let arg = (buyer_usr_id, charged_ctime, store_id);
            FetchPayoutMetaArgs::from(arg).into_parts()
        };
        let maybe_row = stmt
            .with(params)
            .first::<PayoutMetaRowType, &mut Conn>(&mut conn)
            .await
            .map_err(|e| {
                let code = AppErrorCode::RemoteDbServerFailure;
                let detail = AppRepoErrorDetail::DatabaseQuery(e.to_string());
                self._map_log_err_common((code, detail), AppRepoErrorFnLabel::FetchPayout)
            })?;
        let row_meta = if let Some(v) = maybe_row {
            v
        } else {
            return Ok(None);
        };
        let p3pty_m = {
            let label3pt = Label3party::try_from(row_meta.4.as_str()).map_err(|s| {
                let code = AppErrorCode::DataCorruption;
                let detail = AppRepoErrorDetail::PayMethodUnsupport(s.to_string());
                self._map_log_err_common((code, detail), AppRepoErrorFnLabel::FetchPayout)
            })?;
            let arg = (buyer_usr_id, charged_ctime, store_id, label3pt);
            FetchPayout3partyArgs::from(arg)
                .fetch(&mut conn)
                .await
                .map_err(|reason| {
                    self._map_log_err_common(reason, AppRepoErrorFnLabel::FetchPayout)
                })?
        };

        let oid_ref = OidBytes::to_app_oid(row_meta.2.clone()).map_err(|(code, msg)| {
            let detail = AppRepoErrorDetail::OrderIDparse(msg);
            self._map_log_err_common((code, detail), AppRepoErrorFnLabel::FetchPayout)
        })?;

        let mut currency_snapshot = {
            let args = (oid_ref.as_str(), Some([buyer_usr_id, store_id]));
            let (stmt, values) = FetchCurrencySnapshotArgs::try_from(args)
                .map_err(|reason| {
                    self._map_log_err_common(reason, AppRepoErrorFnLabel::FetchPayout)
                })?
                .into_parts();
            let params = Params::Positional(values);
            Self::try_build_currency_snapshot(&mut conn, stmt, params)
                .await
                .map_err(|detail| {
                    self._map_log_err_common(
                        (AppErrorCode::Unknown, detail),
                        AppRepoErrorFnLabel::FetchPayout,
                    )
                })?
        };

        let currency_buyer = currency_snapshot
            .remove(&buyer_usr_id)
            .ok_or(AppRepoErrorDetail::DataRowParse(
                "missing-buyer-currency".to_string(),
            ))
            .map_err(|detail| {
                self._map_log_err_common(
                    (AppErrorCode::DataCorruption, detail),
                    AppRepoErrorFnLabel::FetchPayout,
                )
            })?;
        let currency_seller = currency_snapshot
            .remove(&store_id)
            .ok_or(AppRepoErrorDetail::DataRowParse(
                "missing-seller-currency".to_string(),
            ))
            .map_err(|detail| {
                self._map_log_err_common(
                    (AppErrorCode::DataCorruption, detail),
                    AppRepoErrorFnLabel::FetchPayout,
                )
            })?;

        let arg = (row_meta.3, currency_seller, currency_buyer);
        let amount_m = PayoutAmountModel::try_from(arg).map_err(|e| {
            let msg = format!("payout-model: {:?}", e);
            let detail = AppRepoErrorDetail::DataRowParse(msg);
            self._map_log_err_common(
                (AppErrorCode::DataCorruption, detail),
                AppRepoErrorFnLabel::FetchPayout,
            )
        })?;

        let capture_create_time = raw_column_to_datetime(row_meta.0, 0)
            .map_err(|reason| self._map_log_err_common(reason, AppRepoErrorFnLabel::FetchPayout))?;

        let arg = (
            store_id,
            capture_create_time,
            buyer_usr_id,
            charged_ctime,
            oid_ref,
            row_meta.1,
            amount_m,
            p3pty_m,
        );
        Ok(Some(PayoutModel::from(arg)))
    } // end of fn fetch_payout

    async fn create_payout(&self, payout_m: PayoutModel) -> Result<(), AppRepoError> {
        let (p_inner, p3pty) = payout_m.into_parts();
        let label3pt = Label3party::from(&p3pty);
        let (buyer_usr_id, charged_ctime) = p_inner.referenced_charge();
        let merchant_id = p_inner.merchant_id();

        let (stmt_3pt, params_3pt) = {
            let arg = (buyer_usr_id, charged_ctime, merchant_id, p3pty);
            InsertPayout3partyArgs::try_from(arg)
                .map_err(|reason| {
                    self._map_log_err_common(reason, AppRepoErrorFnLabel::CreatePayout)
                })?
                .into_parts()
        };
        let (stmt_meta, params_meta) = InsertPayoutMetaArgs::try_from((p_inner, label3pt))
            .map_err(|reason| self._map_log_err_common(reason, AppRepoErrorFnLabel::CreatePayout))?
            .into_parts();
        let pairs = vec![(stmt_3pt, params_3pt), (stmt_meta, params_meta)];
        let mut conn = self._dstore.acquire().await.map_err(|e| {
            let code = AppErrorCode::DatabaseServerBusy;
            let detail = AppRepoErrorDetail::DataStore(e);
            self._map_log_err_common((code, detail), AppRepoErrorFnLabel::FetchChargeByMerchant)
        })?;

        let mut options = TxOpts::new();
        options.with_isolation_level(IsolationLevel::Serializable);
        let mut tx = conn.start_transaction(options).await.map_err(|e| {
            let code = AppErrorCode::RemoteDbServerFailure;
            let detail = AppRepoErrorDetail::DatabaseTxStart(e.to_string());
            self._map_log_err_common((code, detail), AppRepoErrorFnLabel::FetchChargeByMerchant)
        })?;

        for (stmt, params) in pairs {
            let qresult = tx.exec_iter(stmt, params).await.map_err(|e| {
                let code = AppErrorCode::RemoteDbServerFailure;
                let detail = AppRepoErrorDetail::DatabaseExec(e.to_string());
                self._map_log_err_common((code, detail), AppRepoErrorFnLabel::FetchChargeByMerchant)
            })?;
            let num_inserted = qresult.affected_rows();
            if num_inserted != 1u64 {
                let code = AppErrorCode::DataCorruption;
                let msg = format!("insertion-failure, actual:{num_inserted}");
                let detail = AppRepoErrorDetail::DatabaseExec(msg);
                return Err(self._map_log_err_common(
                    (code, detail),
                    AppRepoErrorFnLabel::FetchChargeByMerchant,
                ));
            }
        } // end of for loop

        tx.commit().await.map_err(|e| {
            let code = AppErrorCode::RemoteDbServerFailure;
            let detail = AppRepoErrorDetail::DatabaseTxCommit(e.to_string());
            self._map_log_err_common((code, detail), AppRepoErrorFnLabel::FetchChargeByMerchant)
        })
    } // end of fn create-payout
} // end of impl MariadbChargeRepo
