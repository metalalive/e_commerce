use std::boxed::Box;
use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use mysql_async::prelude::{Query, Queryable, WithParams};
use mysql_async::{Conn, IsolationLevel, Params, TxOpts, Value as MySqlVal};
use rust_decimal::Decimal;

use ecommerce_common::adapter::repository::OidBytes;
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::logging::{app_log_event, AppLogLevel};
use ecommerce_common::model::BaseProductIdentity;

use crate::adapter::datastore::{AppDStoreMariaDB, AppDataStoreContext};
use crate::adapter::processor::AbstractPaymentProcessor;
use crate::api::web::dto::{
    RefundCompletionOlineReqDto, RefundCompletionReqDto, RefundRejectReasonDto,
};
use crate::model::{
    ChargeBuyerModel, OLineRefundModel, OrderRefundModel, PayLineAmountModel,
    RefundLineQtyRejectModel, RefundModelError,
};

use super::super::{
    AbstractRefundRepo, AppRefundRslvReqCallback, AppRefundRslvReqOkReturn, AppRepoError,
    AppRepoErrorDetail, AppRepoErrorFnLabel,
};
use super::{inner_into_parts, raw_column_to_datetime, DATETIME_FMT_P0F, DATETIME_FMT_P3F};

const JOB_SCHE_LABEL: &str = "refund-req-sync";

type Req4RslvRowType = (
    u64,      // `product_id`
    MySqlVal, // `create_time`
    Decimal,  // `amt_req_unit`
    Decimal,  // `amt_req_total`
    u32,      // `qty_req`
    u32,      // `qty_rej_fraud`
    u32,      // `qty_rej_damage`
    Decimal,  // `amt_aprv_unit`
    Decimal,  // `amt_aprv_total`
    u32,      // `qty_aprv`
);

struct UpdateLastTimeSyncArgs(String, Params);
struct InsertRequestArgs(String, Vec<Params>);
struct FetchReqForRslvArgs(String, Params);
struct UpdateResolvedReqArgs(String, Vec<Params>);

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

    #[rustfmt::skip]
    fn try_from(value: Vec<OrderRefundModel>) -> Result<Self, Self::Error> {
        let stmt = "INSERT INTO `oline_refund_req`(`o_id`,`store_id`,`product_id`,\
                    `create_time`,`amt_req_unit`,`amt_req_total`,`qty_req`,`qty_rej_fraud`,\
                    `qty_rej_damage`,`qty_aprv`,`amt_aprv_unit`,`amt_aprv_total`) VALUES\
                    (?,?,?,?,?,?,?,?,?,?,?,?)";
        let mut errors = Vec::new();
        let final_params = value.into_iter()
            .map(Self::try_from_one_req)
            .filter_map(|r| r.map_err(|e| errors.push(e)).ok())
            .flatten().collect::<Vec<_>>();
        if errors.is_empty() {
            Ok(Self(stmt.to_string(), final_params))
        } else {
            Err(errors)
        }
    } // end of fn try-from
} // end of impl InsertRequestArgs

impl InsertRequestArgs {
    #[rustfmt::skip]
    fn try_from_one_req(
        req: OrderRefundModel,
    ) -> Result<Vec<Params>, (AppErrorCode, AppRepoErrorDetail)> {
        let (oid_hex, rlines) = req.into_parts();
        let oid_b = OidBytes::try_from(oid_hex.as_str())
            .map_err(|(code, msg)| (code, AppRepoErrorDetail::OrderIDparse(msg)))?;
        let params = rlines.into_iter()
            .map(|line| {
                let (pid, amt_req, ctime, amt_aprv, rejected) = line.into_parts();
                let BaseProductIdentity {store_id, product_id} = pid;
                let num_rej_fraud = rejected.inner_map()
                    .get(&RefundRejectReasonDto::Fraudulent)
                    .unwrap_or(&0u32).to_owned();
                let num_rej_damage = rejected.inner_map()
                    .get(&RefundRejectReasonDto::Damaged)
                    .unwrap_or(&0u32).to_owned();
                let arg = vec![
                    oid_b.as_column().into(),
                    store_id.into(),
                    product_id.into(),
                    ctime.format(DATETIME_FMT_P0F).to_string().into(),
                    amt_req.unit.into(),
                    amt_req.total.into(),
                    amt_req.qty.into(),
                    num_rej_fraud.into(),
                    num_rej_damage.into(),
                    amt_aprv.qty.into(),
                    amt_aprv.unit.into(),
                    amt_aprv.total.into(),
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

impl<'a, 'b> From<(&'a OidBytes, u32, &'b [RefundCompletionOlineReqDto])> for FetchReqForRslvArgs {
    fn from(value: (&'a OidBytes, u32, &'b [RefundCompletionOlineReqDto])) -> Self {
        let (oid_b, merchant_id, cmplt_rlines) = value;
        let stmt = Self::generate_prep_statement(cmplt_rlines.len());
        let mut args = cmplt_rlines
            .iter()
            .flat_map(|rline| {
                vec![
                    rline.product_id.into(),
                    rline
                        .time_issued
                        .format(DATETIME_FMT_P0F)
                        .to_string()
                        .into(),
                ]
            })
            .collect::<Vec<MySqlVal>>();
        args.insert(0, oid_b.as_column().into());
        args.insert(1, merchant_id.into());
        Self(stmt, Params::Positional(args))
    }
} // end of impl FetchReqForRslvArgs

inner_into_parts!(FetchReqForRslvArgs);

impl FetchReqForRslvArgs {
    #[rustfmt::skip]
    fn generate_prep_statement(num_batches : usize) -> String {
        assert_ne!(num_batches, 0);
        let cond = (0..num_batches)
            .map(|_| "(`product_id`=? AND `create_time`=?)")
            .collect::<Vec<_>>()
            .join("OR");
        format!("SELECT `product_id`,`create_time`,`amt_req_unit`,`amt_req_total`,\
        `qty_req`,`qty_rej_fraud`,`qty_rej_damage`,`amt_aprv_unit`,`amt_aprv_total`,\
        `qty_aprv` FROM `oline_refund_req` WHERE `o_id`=? AND `store_id`=? AND ({cond})")
    }
}

#[rustfmt::skip]
impl<'a> From<(&'a OidBytes, Vec<OLineRefundModel>)> for UpdateResolvedReqArgs {
    fn from(value: (&'a OidBytes, Vec<OLineRefundModel>)) -> Self {
        let (oid_b, rlines_m) = value;
        let oid = oid_b.as_column();
        let stmt = "UPDATE `oline_refund_req` SET `qty_rej_fraud`=?, `qty_rej_damage`=?,\
                    `amt_aprv_unit`=?, `amt_aprv_total`=?, `qty_aprv`=? WHERE `o_id`=? \
                    AND `store_id`=? AND `product_id`=? AND `create_time`=?";
        let params = rlines_m.into_iter()
            .map(|rline| {
                let (pid, _amt_req, ctime, amt_aprv, rejected) = rline.into_parts();
                let BaseProductIdentity {store_id, product_id} = pid;
                let num_rej_fraud = rejected.inner_map()
                    .get(&RefundRejectReasonDto::Fraudulent)
                    .unwrap_or(&0u32).to_owned();
                let num_rej_damage = rejected.inner_map()
                    .get(&RefundRejectReasonDto::Damaged)
                    .unwrap_or(&0u32).to_owned();
                let arg = vec![
                    num_rej_fraud.into(),
                    num_rej_damage.into(),
                    amt_aprv.unit.into(),
                    amt_aprv.total.into(),
                    amt_aprv.qty.into(),
                    oid.clone().into(),
                    store_id.into(),
                    product_id.into(),
                    ctime.format(DATETIME_FMT_P0F).to_string().into(),
                ];
                Params::Positional(arg)
            })
            .collect::<Vec<_>>();
        Self(stmt.to_string(), params)
    } // end of fn from
} // end of impl UpdateResolvedReqArgs

impl UpdateResolvedReqArgs {
    fn into_parts(self) -> (String, Vec<Params>) {
        let Self(stmt, params) = self;
        (stmt, params)
    }
}

#[rustfmt::skip]
impl TryFrom<(u32, Req4RslvRowType)> for OLineRefundModel {
    type Error = (AppErrorCode, AppRepoErrorDetail);
    fn try_from(value: (u32, Req4RslvRowType)) -> Result<Self, Self::Error> {
        let (
            merchant_id,
            (
                product_id, time_issued,
                amt_req_unit, amt_req_total, qty_req,
                qty_rej_fraud, qty_rej_damage,
                amt_aprv_unit, amt_aprv_total, qty_aprv,
            ),
        ) = value;
        let time_issued = raw_column_to_datetime(time_issued, 0)?;
        let pid = BaseProductIdentity {store_id: merchant_id, product_id};
        let amt_req = PayLineAmountModel {
            unit: amt_req_unit, total: amt_req_total, qty: qty_req,
        };
        let amt_aprv = PayLineAmountModel {
            unit: amt_aprv_unit, total: amt_aprv_total, qty: qty_aprv,
        };
        let rejected = {
            let list = [
                (RefundRejectReasonDto::Fraudulent, qty_rej_fraud),
                (RefundRejectReasonDto::Damaged, qty_rej_damage),
            ];
            let rejmap = HashMap::from(list);
            RefundLineQtyRejectModel::from(&rejmap)
        };
        Ok(Self::from((pid, amt_req, time_issued, amt_aprv, rejected)))
    } // end of fn try-from
} // end of impl OLineRefundModel

impl<'a> TryFrom<(&'a OidBytes, u32, Vec<Req4RslvRowType>)> for OrderRefundModel {
    type Error = (AppErrorCode, AppRepoErrorDetail);
    fn try_from(value: (&'a OidBytes, u32, Vec<Req4RslvRowType>)) -> Result<Self, Self::Error> {
        let (oid_b, merchant_id, rows) = value;
        let id = OidBytes::to_app_oid(oid_b.as_column())
            .map_err(|(code, msg)| (code, AppRepoErrorDetail::OrderIDparse(msg)))?;
        let mut errors = Vec::new();
        let lines = rows
            .into_iter()
            .filter_map(|row| {
                OLineRefundModel::try_from((merchant_id, row))
                    .map_err(|e| errors.push(e))
                    .ok()
            })
            .collect::<Vec<_>>();
        if errors.is_empty() {
            Ok(Self::from((id, lines)))
        } else {
            Err(errors.remove(0))
        }
    }
}

pub(crate) struct MariaDbRefundRepo {
    _dstore: Arc<AppDStoreMariaDB>,
}

impl MariaDbRefundRepo {
    pub(crate) fn new(ds: Arc<AppDataStoreContext>) -> Result<Self, AppRepoError> {
        ds.mariadb(Some("db-write-primary"))
            .map(|found| Self { _dstore: found })
            .ok_or(AppRepoError {
                fn_label: AppRepoErrorFnLabel::InitRefundRepo,
                code: AppErrorCode::MissingDataStore,
                detail: AppRepoErrorDetail::Unknown,
            })
    }
    fn _rslv_validate_order_id(
        charge_ms: &[ChargeBuyerModel],
    ) -> Result<OidBytes, (AppErrorCode, AppRepoErrorDetail)> {
        if charge_ms.is_empty() {
            let s = "missing-order-id".to_string();
            return Err((
                AppErrorCode::EmptyInputData,
                AppRepoErrorDetail::OrderIDparse(s),
            ));
        }
        let expect_oid = charge_ms.first().unwrap().meta.oid();
        let same = charge_ms.iter().all(|c| c.meta.oid() == expect_oid);
        if same {
            let oid_b = OidBytes::try_from(expect_oid.as_str())
                .map_err(|(code, msg)| (code, AppRepoErrorDetail::OrderIDparse(msg)))?;
            Ok(oid_b)
        } else {
            let s = "order-ids-not-consistent".to_string();
            Err((
                AppErrorCode::InvalidInput,
                AppRepoErrorDetail::OrderIDparse(s),
            ))
        }
    }

    #[rustfmt::skip]
    fn rslv_validate_order_id(&self, charge_ms: &[ChargeBuyerModel]) -> Result<OidBytes, AppRepoError> {
        Self::_rslv_validate_order_id(charge_ms)
            .map_err(|(code,detail)| {
                self._map_log_err_common(code, detail, AppRepoErrorFnLabel::ResolveRefundReq)
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
        merchant_id: u32,
        cmplt_req: RefundCompletionReqDto,
        charge_ms: Vec<ChargeBuyerModel>,
        processor: Arc<Box<dyn AbstractPaymentProcessor>>,
        usr_cb: AppRefundRslvReqCallback,
    ) -> Result<AppRefundRslvReqOkReturn, AppRepoError> {
        if cmplt_req.lines.is_empty() {
            let me = RefundModelError::EmptyResolutionRequest(merchant_id);
            return Err(AppRepoError {
                fn_label: AppRepoErrorFnLabel::ResolveRefundReq,
                code: AppErrorCode::EmptyInputData,
                detail: AppRepoErrorDetail::RefundResolution(vec![me]),
            });
        }
        let oid_b = self.rslv_validate_order_id(&charge_ms)?;
        let mut conn = self._dstore.acquire().await.map_err(|e| {
            self._map_log_err_common(
                AppErrorCode::DatabaseServerBusy,
                AppRepoErrorDetail::DataStore(e),
                AppRepoErrorFnLabel::ResolveRefundReq,
            )
        })?;
        let mut tx = {
            let options = TxOpts::new()
                .with_isolation_level(IsolationLevel::Serializable)
                .to_owned();
            conn.start_transaction(options).await.map_err(|e| {
                self._map_log_err_common(
                    AppErrorCode::RemoteDbServerFailure,
                    AppRepoErrorDetail::DatabaseTxStart(e.to_string()),
                    AppRepoErrorFnLabel::ResolveRefundReq,
                )
            })?
        };
        let (stmt, params) = {
            let arg = (&oid_b, merchant_id, cmplt_req.lines.as_slice());
            FetchReqForRslvArgs::from(arg).into_parts()
        };
        let rows = tx
            .exec::<Req4RslvRowType, String, Params>(stmt, params)
            .await
            .map_err(|e| {
                self._map_log_err_common(
                    AppErrorCode::RemoteDbServerFailure,
                    AppRepoErrorDetail::DatabaseQuery(e.to_string()),
                    AppRepoErrorFnLabel::ResolveRefundReq,
                )
            })?;
        let mut o_rfnd_m =
            OrderRefundModel::try_from((&oid_b, merchant_id, rows)).map_err(|e| {
                self._map_log_err_common(e.0, e.1, AppRepoErrorFnLabel::ResolveRefundReq)
            })?;

        let cb_res = usr_cb(&mut o_rfnd_m, cmplt_req, charge_ms, processor)
            .await
            .map_err(|detail| {
                self._map_log_err_common(
                    AppErrorCode::DataCorruption,
                    detail,
                    AppRepoErrorFnLabel::ResolveRefundReq,
                )
            })?;

        let (stmt, params) = {
            let (_oid_hex, rlines_m) = o_rfnd_m.into_parts();
            let arg = (&oid_b, rlines_m);
            UpdateResolvedReqArgs::from(arg).into_parts()
        };
        tx.exec_batch(stmt, params).await.map_err(|e| {
            self._map_log_err_common(
                AppErrorCode::RemoteDbServerFailure,
                AppRepoErrorDetail::DatabaseExec(e.to_string()),
                AppRepoErrorFnLabel::ResolveRefundReq,
            )
        })?;

        tx.commit().await.map_err(|e| {
            self._map_log_err_common(
                AppErrorCode::RemoteDbServerFailure,
                AppRepoErrorDetail::DatabaseTxCommit(e.to_string()),
                AppRepoErrorFnLabel::ResolveRefundReq,
            )
        })?;
        Ok(cb_res)
    } // end of fn resolve_request
} // end of impl MariaDbRefundRepo
