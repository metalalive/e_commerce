use std::collections::{HashMap, HashSet};
use std::result::Result;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use mysql_async::prelude::{Query, WithParams};
use mysql_async::{Conn, Params, Value as MySqlVal};
use rust_decimal::Decimal;

use ecommerce_common::adapter::repository::OidBytes;
use ecommerce_common::api::dto::CurrencyDto;
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::logging::{app_log_event, AppLogLevel};

use super::{inner_into_parts, raw_column_to_datetime, DATETIME_FMT_P0F};
use crate::adapter::datastore::{AppDStoreMariaDB, AppDataStoreContext};
use crate::adapter::repository::{
    AbstractReportingRepo, AppRepoError, AppRepoErrorDetail, AppRepoErrorFnLabel,
};
use crate::api::web::dto::ReportTimeRangeDto;
use crate::model::{
    ChargeBuyerMetaModel, ChargeBuyerModel, ChargeLineBuyerModel, OrderCurrencySnapshot,
};

#[rustfmt::skip]
type ChargeLineRowType = (
    u32,      // `buyer-usr-id`
    MySqlVal, // `create-time`
    u64,      // `product-id`
    u16,      // `attr-set-seq`
    Decimal, Decimal, u32, // `amount-paid-original`
    Decimal, Decimal, u32, // `amount-refunded`
    u32,      // `qty-rejected-for-refund`
);
type ChargeMetaRowType = (
    u32,              // buyer-usr-id
    MySqlVal,         // create-time
    Vec<u8>,          // order-id hex
    String,           // `state`
    Option<MySqlVal>, // `processor_accepted_time`
    Option<MySqlVal>, // `processor_completed_time`
    Option<MySqlVal>, // `orderapp_synced_time`
    String,           // `pay_method`
    String,           // `detail_3rdparty`, serialised json
);

#[rustfmt::skip]
type OrderCurrencyRowType = (Vec<u8>,u32, String, Decimal);

type InnerChargeLineMap = HashMap<(u32, DateTime<Utc>), Vec<ChargeLineBuyerModel>>;
type InnerOrderCurrencyMap = HashMap<String, HashMap<u32, OrderCurrencySnapshot>>;

struct FetchChargeLineArgs(String, Params);
struct FetchChargeMetaArgs(String, Params);
struct FetchCurrencySnapshotArgs(String, Params);

#[rustfmt::skip]
impl<'a> From<(u32, &'a ReportTimeRangeDto)> for FetchChargeLineArgs {
    fn from(value: (u32, &'a ReportTimeRangeDto)) -> Self {
        let (store_id, t_range) = value;
        let stmt = "SELECT `buyer_id`,`create_time`,`product_id`,`attr_seq`,`amt_orig_unit`,\
                    `amt_orig_total`,`qty_orig`,`amt_rfnd_unit`,`amt_rfnd_total`,`qty_rfnd`,\
                    `qty_rej` FROM `charge_line` WHERE `store_id`=?  AND `create_time` >= ? \
                    AND `create_time` <= ?"
            .to_string();
        let args = vec![
            store_id.into(),
            t_range.start_after.format(DATETIME_FMT_P0F).to_string().into(),
            t_range.end_before.format(DATETIME_FMT_P0F).to_string().into(),
        ];
        let params = Params::Positional(args);
        Self(stmt, params)
    }
}

inner_into_parts!(FetchChargeLineArgs);

impl<'a> From<Vec<&'a (u32, DateTime<Utc>)>> for FetchChargeMetaArgs {
    fn from(value: Vec<&'a (u32, DateTime<Utc>)>) -> Self {
        let stmt = Self::sql_prep_stmt(value.len());
        let args = value
            .into_iter()
            .flat_map(|(buyer_id, ctime)| {
                vec![
                    (*buyer_id).into(),
                    ctime.format(DATETIME_FMT_P0F).to_string().into(),
                ]
            })
            .collect::<Vec<_>>();
        Self(stmt, Params::Positional(args))
    }
}
impl FetchChargeMetaArgs {
    fn sql_prep_stmt(num_batch: usize) -> String {
        assert_ne!(num_batch, 0);
        let cond = (0..num_batch)
            .map(|_| "(`usr_id`=? AND `create_time`=?)")
            .collect::<Vec<_>>()
            .join("OR");
        format!(
            "SELECT `usr_id`,`create_time`,`order_id`,`state`,`processor_accepted_time`,\
                 `processor_completed_time`,`orderapp_synced_time`,`pay_method`,`detail_3rdparty`\
                 FROM `charge_buyer_toplvl` WHERE {cond}"
        )
    }
}

inner_into_parts!(FetchChargeMetaArgs);

impl<'a> From<(u32, Vec<(&'a Vec<u8>, u32)>)> for FetchCurrencySnapshotArgs {
    fn from(value: (u32, Vec<(&'a Vec<u8>, u32)>)) -> Self {
        let (store_id, oid_buyer_pairs) = value;
        assert!(!oid_buyer_pairs.is_empty());
        let stmt = Self::sql_prep_stmt(oid_buyer_pairs.len());
        let args = oid_buyer_pairs
            .into_iter()
            .flat_map(|(oid_raw, buyer_id)| {
                vec![oid_raw.as_slice().into(), buyer_id.into(), store_id.into()]
            })
            .collect::<Vec<_>>();
        Self(stmt, Params::Positional(args))
    }
}
impl FetchCurrencySnapshotArgs {
    fn sql_prep_stmt(num_batch: usize) -> String {
        let cond = (0..num_batch)
            .map(|_| "(`o_id`=? AND `usr_id` IN (?,?))")
            .collect::<Vec<_>>()
            .join("OR");
        format!(
            "SELECT `o_id`,`usr_id`,`label`,`ex_rate` FROM \
              `order_currency_snapshot` WHERE {cond}"
        )
    }
}

inner_into_parts!(FetchCurrencySnapshotArgs);

pub struct MariadbReportingRepo {
    dstore_pri: Arc<AppDStoreMariaDB>,
    dstore_rep: Arc<AppDStoreMariaDB>,
}

impl MariadbReportingRepo {
    pub(crate) fn new(ds: Arc<AppDataStoreContext>) -> Result<Self, AppRepoError> {
        let dstore_pri = ds.mariadb(Some("db-write-primary")).ok_or(AppRepoError {
            fn_label: AppRepoErrorFnLabel::InitReportingRepo,
            code: AppErrorCode::MissingDataStore,
            detail: AppRepoErrorDetail::Unknown,
        })?;
        let dstore_rep = ds.mariadb(Some("db-read-replica")).ok_or(AppRepoError {
            fn_label: AppRepoErrorFnLabel::InitReportingRepo,
            code: AppErrorCode::MissingDataStore,
            detail: AppRepoErrorDetail::Unknown,
        })?;
        Ok(Self {
            dstore_pri,
            dstore_rep,
        })
    }

    #[rustfmt::skip]
    fn parse_charge_lines(
        store_id: u32,
        rows: Vec<ChargeLineRowType>,
    ) -> Result<InnerChargeLineMap, (AppErrorCode, AppRepoErrorDetail)> {
        let mut errors = Vec::new();
        let kv_pairs = rows.into_iter()
            .filter_map(|row| {
                let (
                    buyer_usr_id, ctime_raw, product_id, attr_set_seq,
                    amt_orig_unit, amt_orig_total, qty_orig,
                    amt_rfnd_unit, amt_rfnd_total, qty_rfnd, num_rejected,
                ) = row;
                let d = (
                    store_id, product_id, attr_set_seq,
                    amt_orig_unit, amt_orig_total, qty_orig,
                    amt_rfnd_unit, amt_rfnd_total, qty_rfnd, num_rejected,
                );
                let result0 = raw_column_to_datetime(ctime_raw, 0);
                let result1 = ChargeLineBuyerModel::try_from(d);
                match (result0, result1) {
                    (Ok(ctime), Ok(cline)) => Some(((buyer_usr_id, ctime), cline)),
                    (Ok(_), Err(detail)) => {
                        let code = AppErrorCode::DataCorruption;
                        errors.push((code, detail));
                        None
                    }
                    (Err(reason), _) => {
                        errors.push(reason);
                        None
                    }
                }
            })
            .collect::<Vec<_>>();
        if !errors.is_empty() {
            return Err(errors.remove(0));
        }
        let mut out = HashMap::new();
        kv_pairs
            .into_iter()
            .map(|(k, v)| {
                let entry = out.entry(k).or_insert(Vec::new());
                entry.push(v);
            })
            .count();
        Ok(out)
    } // end of parse_charge_lines

    #[rustfmt::skip]
    fn parse_charge_meta(
        row: ChargeMetaRowType,
    ) -> Result<ChargeBuyerMetaModel, (AppErrorCode, AppRepoErrorDetail)> {
        let (
            buyer_usr_id, ctime_raw, oid_raw, buyin_state, accepted_time_3pty,
            completed_time_3pty, orderapp_synced_time, mthd_3pty_label,
            detail_3pty_serial,
        ) = row;
        let create_time = raw_column_to_datetime(ctime_raw, 0)?;
        let d = (
            buyer_usr_id, create_time,
            (
                oid_raw, buyin_state, accepted_time_3pty, completed_time_3pty,
                orderapp_synced_time, mthd_3pty_label, detail_3pty_serial,
            ),
        );
        ChargeBuyerMetaModel::try_from(d)
    }
    fn parse_charge_metas(
        rows: Vec<ChargeMetaRowType>,
    ) -> Result<Vec<ChargeBuyerMetaModel>, (AppErrorCode, AppRepoErrorDetail)> {
        let mut errors = Vec::new();
        let charge_ms = rows
            .into_iter()
            .filter_map(|row| {
                Self::parse_charge_meta(row)
                    .map_err(|reason| errors.push(reason))
                    .ok()
            })
            .collect::<Vec<_>>();
        if errors.is_empty() {
            Ok(charge_ms)
        } else {
            Err(errors.remove(0))
        }
    } // end of fn parse_charge_metas

    fn gen_sql_currency_snapshot(store_id: u32, rows: &[ChargeMetaRowType]) -> (String, Params) {
        let iter = rows.iter().map(|row| {
            let oid = &row.2;
            let buyer_id = row.0;
            (oid, buyer_id)
        });
        let hset = HashSet::<(&Vec<u8>, u32)>::from_iter(iter);
        let oid_buyer_pairs = hset.into_iter().collect::<Vec<_>>();
        FetchCurrencySnapshotArgs::from((store_id, oid_buyer_pairs)).into_parts()
    }

    fn parse_currency_snapshots(
        rows: Vec<OrderCurrencyRowType>,
    ) -> Result<InnerOrderCurrencyMap, (AppErrorCode, AppRepoErrorDetail)> {
        let mut errors = Vec::new();
        let mut outer = HashMap::new();
        rows.into_iter()
            .map(|row| {
                let (oid_raw, actor_id, label_raw, rate) = row;
                let result = OidBytes::to_app_oid(oid_raw).map_err(|(code, msg)| {
                    let reason = (code, AppRepoErrorDetail::DataRowParse(msg));
                    errors.push(reason)
                });
                let oid = match result {
                    Ok(v) => v,
                    Err(_) => {
                        return;
                    }
                };
                let entry_h = outer
                    .entry(oid)
                    .or_insert(HashMap::<u32, OrderCurrencySnapshot>::new());
                let label = CurrencyDto::from(&label_raw);
                let sc = OrderCurrencySnapshot { label, rate };
                let _old = entry_h.insert(actor_id, sc);
            })
            .count();
        if errors.is_empty() {
            Ok(outer)
        } else {
            Err(errors.remove(0))
        }
    } // end of parse_currency_snapshots

    fn fetch_charges_merge_parts(
        c_metas: Vec<ChargeBuyerMetaModel>,
        mut cline_map: InnerChargeLineMap,
        currency_snapshots: InnerOrderCurrencyMap,
    ) -> Result<Vec<ChargeBuyerModel>, AppRepoErrorDetail> {
        let mut errors = Vec::new();
        let charge_ms = c_metas
            .into_iter()
            .filter_map(|meta| {
                let k = (meta.owner(), *meta.create_time());
                let lines = if let Some(v) = cline_map.remove(&k) {
                    v
                } else {
                    let msg = format!("missing-lines, charge-id: {:?}", k);
                    errors.push(AppRepoErrorDetail::ConstructChargeFailure(msg));
                    return None;
                };
                let k = meta.oid();
                let sc = if let Some(v) = currency_snapshots.get(k) {
                    // in this project, multiple charges may share the same currency snapshot
                    v.clone()
                } else {
                    let msg = format!("missing-currency-snapshot, oid: {:?}", k);
                    errors.push(AppRepoErrorDetail::ConstructChargeFailure(msg));
                    return None;
                };
                let charge_m = ChargeBuyerModel {
                    meta,
                    lines,
                    currency_snapshot: sc,
                };
                Some(charge_m)
            })
            .collect::<Vec<_>>();
        assert!(cline_map.is_empty()); // TODO, error handling
        if errors.is_empty() {
            Ok(charge_ms)
        } else {
            Err(errors.remove(0))
        }
    }

    #[rustfmt::skip]
    fn map_log_err(
        &self,
        reason : (AppErrorCode, AppRepoErrorDetail),
        fn_label: AppRepoErrorFnLabel,
    ) -> AppRepoError {
        let (code, detail) = reason;
        let e = AppRepoError { fn_label, code, detail };
        let logctx = self.dstore_pri.log_context();
        app_log_event!(logctx, AppLogLevel::ERROR, "{:?}", e);
        e
    }
} // end of impl MariadbReportingRepo

#[async_trait]
impl AbstractReportingRepo for MariadbReportingRepo {
    async fn fetch_charges_by_merchant(
        &self,
        store_id: u32,
        t_range: ReportTimeRangeDto,
    ) -> Result<Vec<ChargeBuyerModel>, AppRepoError> {
        let mut conn_rep = self.dstore_rep.acquire().await.map_err(|e| {
            let code = AppErrorCode::DatabaseServerBusy;
            let detail = AppRepoErrorDetail::DataStore(e);
            let fn_label = AppRepoErrorFnLabel::ReportChargeByMerchant;
            self.map_log_err((code, detail), fn_label)
        })?;
        let (stmt, params) = FetchChargeLineArgs::from((store_id, &t_range)).into_parts();

        // TODO, consider to stream the fetch operation row by row, in case
        // the number of lines to fetch is very large
        let rows = stmt
            .with(params)
            .fetch::<ChargeLineRowType, &mut Conn>(&mut conn_rep)
            .await
            .map_err(|e| {
                let code = AppErrorCode::RemoteDbServerFailure;
                let detail = AppRepoErrorDetail::DatabaseQuery(e.to_string());
                let fn_label = AppRepoErrorFnLabel::ReportChargeByMerchant;
                self.map_log_err((code, detail), fn_label)
            })?;
        drop(conn_rep);

        let cline_map = Self::parse_charge_lines(store_id, rows).map_err(|reason| {
            let fn_label = AppRepoErrorFnLabel::ReportChargeByMerchant;
            self.map_log_err(reason, fn_label)
        })?;
        if cline_map.is_empty() {
            return Ok(Vec::new());
        }

        let mut conn_pri = self.dstore_pri.acquire().await.map_err(|e| {
            let code = AppErrorCode::DatabaseServerBusy;
            let detail = AppRepoErrorDetail::DataStore(e);
            let fn_label = AppRepoErrorFnLabel::ReportChargeByMerchant;
            self.map_log_err((code, detail), fn_label)
        })?;
        let charge_ids = cline_map.keys().collect::<Vec<_>>();
        let (stmt, params) = FetchChargeMetaArgs::from(charge_ids).into_parts();
        let rows_meta = stmt
            .with(params)
            .fetch::<ChargeMetaRowType, &mut Conn>(&mut conn_pri)
            .await
            .map_err(|e| {
                let code = AppErrorCode::RemoteDbServerFailure;
                let detail = AppRepoErrorDetail::DatabaseQuery(e.to_string());
                let fn_label = AppRepoErrorFnLabel::ReportChargeByMerchant;
                self.map_log_err((code, detail), fn_label)
            })?;

        let (stmt, params) = Self::gen_sql_currency_snapshot(store_id, &rows_meta);

        let c_metas = Self::parse_charge_metas(rows_meta).map_err(|reason| {
            let fn_label = AppRepoErrorFnLabel::ReportChargeByMerchant;
            self.map_log_err(reason, fn_label)
        })?;

        let rows = stmt
            .with(params)
            .fetch::<OrderCurrencyRowType, &mut Conn>(&mut conn_pri)
            .await
            .map_err(|e| {
                let code = AppErrorCode::RemoteDbServerFailure;
                let detail = AppRepoErrorDetail::DatabaseQuery(e.to_string());
                let fn_label = AppRepoErrorFnLabel::ReportChargeByMerchant;
                self.map_log_err((code, detail), fn_label)
            })?;

        let currency_snapshots = Self::parse_currency_snapshots(rows).map_err(|reason| {
            let fn_label = AppRepoErrorFnLabel::ReportChargeByMerchant;
            self.map_log_err(reason, fn_label)
        })?;

        Self::fetch_charges_merge_parts(c_metas, cline_map, currency_snapshots).map_err(|detail| {
            let code = AppErrorCode::DataCorruption;
            let fn_label = AppRepoErrorFnLabel::ReportChargeByMerchant;
            self.map_log_err((code, detail), fn_label)
        })
    } // end of fn fetch_charges_by_merchant
} // end of impl MariadbReportingRepo
