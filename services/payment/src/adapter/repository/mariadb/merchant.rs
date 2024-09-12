use std::result::Result;
use std::sync::Arc;

use async_trait::async_trait;
use mysql_async::prelude::{Query, Queryable, WithParams};
use mysql_async::{Conn, IsolationLevel, Params, TxOpts};

use ecommerce_common::error::AppErrorCode;
use ecommerce_common::logging::{app_log_event, AppLogLevel};

use crate::adapter::datastore::{AppDStoreMariaDB, AppDataStoreContext};
use crate::api::web::dto::StoreOnboardReqDto;
use crate::model::{Merchant3partyModel, Merchant3partyStripeModel, MerchantProfileModel};

use super::super::{AbstractMerchantRepo, AppRepoError, AppRepoErrorDetail, AppRepoErrorFnLabel};
use super::{raw_column_to_datetime, DATETIME_FMT_P0F};

struct InsertUpdateProfileArgs(String, Params);
struct Insert3partyArgs(String, Params);
struct FetchProfileArgs(String, Params);
struct Fetch3partyArgs(String, Params, String);

type MercProfRowType = (
    String,             // `name`
    u32,                // `supervisor_id`
    String,             // `staff_ids`
    mysql_async::Value, // `last_update`
);
type Merc3ptyRowType = (Vec<u8>,);

impl From<MerchantProfileModel> for InsertUpdateProfileArgs {
    fn from(value: MerchantProfileModel) -> Self {
        let MerchantProfileModel {
            id,
            name,
            supervisor_id,
            staff_ids,
            last_update,
        } = value;
        let staff_ids = staff_ids
            .into_iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let last_update = last_update.format(DATETIME_FMT_P0F).to_string();
        let arg = vec![
            id.into(),
            name.into(),
            supervisor_id.into(),
            staff_ids.into(),
            last_update.into(),
        ];
        let params = Params::Positional(arg);
        let stmt = "INSERT INTO `merchant_profile`(`id`,`name`,`supervisor_id`,`staff_ids`,`last_update`) \
                    VALUES (?,?,?,?,?) ON DUPLICATE KEY UPDATE `name`=VALUE(`name`), `staff_ids`=VALUE(`staff_ids`), \
                    `supervisor_id`=VALUE(`supervisor_id`), `last_update`=VALUE(`last_update`)";
        Self(stmt.to_string(), params)
    }
} // end of impl InsertUpdateProfileArgs

impl TryFrom<(u32, Merchant3partyModel)> for Insert3partyArgs {
    type Error = (AppErrorCode, AppRepoErrorDetail);
    fn try_from(value: (u32, Merchant3partyModel)) -> Result<Self, Self::Error> {
        let (store_id, m3pty) = value;
        let (method, detail) = match m3pty {
            Merchant3partyModel::Stripe(sm) => {
                let label = "Stripe".to_string();
                let d = serde_json::to_string(&sm).map_err(|e| {
                    (
                        AppErrorCode::DataCorruption,
                        AppRepoErrorDetail::PayDetail(label.clone(), e.to_string()),
                    )
                })?;
                (label, d)
            }
            Merchant3partyModel::Unknown => {
                return Err((
                    AppErrorCode::InvalidInput,
                    AppRepoErrorDetail::PayMethodUnsupport("unknown".to_string()),
                ));
            }
        };
        let arg = vec![store_id.into(), method.into(), detail.into()];
        let params = Params::Positional(arg);
        let stmt = "INSERT INTO `merchant_3party`(`sid`,`method`,`detail`) VALUES (?,?,?)";
        Ok(Self(stmt.to_string(), params))
    } // end of try-from
} // end of impl Insert3partyArgs

impl From<u32> for FetchProfileArgs {
    fn from(value: u32) -> Self {
        let stmt = "SELECT `name`,`supervisor_id`,`staff_ids`,`last_update` FROM \
                    `merchant_profile` WHERE `id`=?";
        let arg = vec![value.into()];
        let params = Params::Positional(arg);
        Self(stmt.to_string(), params)
    }
}

impl<'a> From<(u32, &'a StoreOnboardReqDto)> for Fetch3partyArgs {
    fn from(value: (u32, &'a StoreOnboardReqDto)) -> Self {
        let stmt = "SELECT `detail` FROM `merchant_3party` WHERE `sid`=? AND `method`=?";
        let (store_id, req3pty) = value;
        let method = match req3pty {
            StoreOnboardReqDto::Stripe(_) => "Stripe",
        };
        let paymethod = method.to_string();
        let arg = vec![store_id.into(), method.into()];
        let params = Params::Positional(arg);
        Self(stmt.to_string(), params, paymethod)
    }
}

impl TryFrom<(u32, MercProfRowType)> for MerchantProfileModel {
    type Error = (AppErrorCode, AppRepoErrorDetail);
    #[rustfmt::skip]
    fn try_from(value: (u32, MercProfRowType)) -> Result<Self, Self::Error> {
        let (id, (name, supervisor_id, staff_ids_raw, last_update_raw)) = value;
        let last_update = raw_column_to_datetime(last_update_raw, 0)?;
        let mut errors = Vec::new();
        let staff_ids = staff_ids_raw
            .split(',')
            .filter_map(|v| {
                v.parse::<u32>()
                    .map_err(|e| {
                        let msg = format!("invalid-staff-id: {v} : {:?}", e);
                        errors.push(msg)
                    })
                    .ok()
            })
            .collect::<Vec<_>>();
        if errors.is_empty() {
            Ok(Self { id, name, supervisor_id, staff_ids, last_update })
        } else {
            let code = AppErrorCode::DataCorruption;
            let detail = AppRepoErrorDetail::DataRowParse(errors.remove(0));
            Err((code, detail))
        }
    } // end of fn try-from
} // end of impl MerchantProfileModel

impl<'a> TryFrom<(&'a StoreOnboardReqDto, Merc3ptyRowType)> for Merchant3partyModel {
    type Error = (AppErrorCode, AppRepoErrorDetail);
    fn try_from(value: (&'a StoreOnboardReqDto, Merc3ptyRowType)) -> Result<Self, Self::Error> {
        let (label, (detail_raw,)) = value;
        let out = match label {
            StoreOnboardReqDto::Stripe(_) => {
                let s = serde_json::from_slice::<Merchant3partyStripeModel>(&detail_raw).map_err(
                    |e| {
                        (
                            AppErrorCode::DataCorruption,
                            AppRepoErrorDetail::DataRowParse(e.to_string()),
                        )
                    },
                )?;
                Self::Stripe(s)
            }
        };
        Ok(out)
    }
} // end of impl MerchantProfileModel

pub(crate) struct MariadbMerchantRepo {
    _dstore: Arc<AppDStoreMariaDB>,
}

impl MariadbMerchantRepo {
    pub(crate) fn new(ds: Arc<AppDataStoreContext>) -> Result<Self, AppRepoError> {
        ds.mariadb(None)
            .map(|found| Self { _dstore: found })
            .ok_or(AppRepoError {
                fn_label: AppRepoErrorFnLabel::InitMerchantRepo,
                code: AppErrorCode::MissingDataStore,
                detail: AppRepoErrorDetail::Unknown,
            })
    }

    fn _map_err_create(&self, detail: AppRepoErrorDetail) -> AppRepoError {
        let e = AppRepoError {
            fn_label: AppRepoErrorFnLabel::CreateMerchant,
            code: AppErrorCode::RemoteDbServerFailure,
            detail,
        };
        let logctx = self._dstore.log_context();
        app_log_event!(logctx, AppLogLevel::ERROR, "{:?}", e);
        e
    }
    fn _map_err_fetch(&self, code: AppErrorCode, detail: AppRepoErrorDetail) -> AppRepoError {
        let e = AppRepoError {
            fn_label: AppRepoErrorFnLabel::FetchMerchant,
            code,
            detail,
        };
        let logctx = self._dstore.log_context();
        app_log_event!(logctx, AppLogLevel::ERROR, "{:?}", e);
        e
    }
} // end of impl MariadbMerchantRepo

#[async_trait]
impl AbstractMerchantRepo for MariadbMerchantRepo {
    async fn create(
        &self,
        mprof: MerchantProfileModel,
        m3pty: Merchant3partyModel,
    ) -> Result<(), AppRepoError> {
        let q_arg_3pty =
            Insert3partyArgs::try_from((mprof.id, m3pty)).map_err(|(code, detail)| {
                let mut e = self._map_err_create(detail);
                e.code = code;
                e
            })?;
        let q_arg_prof = InsertUpdateProfileArgs::from(mprof);

        let mut conn = self
            ._dstore
            .acquire()
            .await
            .map_err(|e| self._map_err_create(AppRepoErrorDetail::DataStore(e)))?;
        let mut options = TxOpts::new();
        // in the application, only the owner or initial supervisor of a shop can invoke
        // this function during onboarding process, non-repeatable read should not happen
        // in such scenario, that is why iso-level is set to `read-committed`
        // TODO: recheck this explanation
        options.with_isolation_level(IsolationLevel::ReadCommitted);
        let mut tx = conn.start_transaction(options).await.map_err(|e| {
            self._map_err_create(AppRepoErrorDetail::DatabaseTxStart(e.to_string()))
        })?;
        let resultset = tx
            .exec_iter(q_arg_prof.0, q_arg_prof.1)
            .await
            .map_err(|e| self._map_err_create(AppRepoErrorDetail::DatabaseExec(e.to_string())))?;
        let cond = [1u64, 2].contains(&resultset.affected_rows());
        if !cond {
            let msg = format!("num-rows-affected: {}", resultset.affected_rows());
            let e = self._map_err_create(AppRepoErrorDetail::DatabaseExec(msg));
            return Err(e);
        }
        let resultset = tx
            .exec_iter(q_arg_3pty.0, q_arg_3pty.1)
            .await
            .map_err(|e| self._map_err_create(AppRepoErrorDetail::DatabaseExec(e.to_string())))?;
        if resultset.affected_rows() != 1u64 {
            let msg = format!("num-rows-affected: {}", resultset.affected_rows());
            let e = self._map_err_create(AppRepoErrorDetail::DatabaseExec(msg));
            return Err(e);
        }

        tx.commit().await.map_err(|e| {
            self._map_err_create(AppRepoErrorDetail::DatabaseTxCommit(e.to_string()))
        })?;
        Ok(())
    } // end of fn create

    async fn fetch(
        &self,
        store_id: u32,
        label3pty: &StoreOnboardReqDto,
    ) -> Result<Option<(MerchantProfileModel, Merchant3partyModel)>, AppRepoError> {
        let q_arg_prof = FetchProfileArgs::from(store_id);
        let q_arg_3pty = Fetch3partyArgs::from((store_id, label3pty));
        let mut conn = self._dstore.acquire().await.map_err(|e| {
            self._map_err_fetch(
                AppErrorCode::RemoteDbServerFailure,
                AppRepoErrorDetail::DataStore(e),
            )
        })?;
        let exec = &mut conn;

        let FetchProfileArgs(stmt, params) = q_arg_prof;
        let maybe_row = stmt
            .with(params)
            .first::<MercProfRowType, &mut Conn>(exec)
            .await
            .map_err(|e| {
                self._map_err_fetch(
                    AppErrorCode::RemoteDbServerFailure,
                    AppRepoErrorDetail::DatabaseQuery(e.to_string()),
                )
            })?;

        if let Some(row_profile) = maybe_row {
            let Fetch3partyArgs(stmt, params, paymethod) = q_arg_3pty;
            let row_3pty = stmt
                .with(params)
                .first::<Merc3ptyRowType, &mut Conn>(exec)
                .await
                .map_err(|e| {
                    self._map_err_fetch(
                        AppErrorCode::RemoteDbServerFailure,
                        AppRepoErrorDetail::DatabaseQuery(e.to_string()),
                    )
                })?
                .ok_or(self._map_err_fetch(
                    AppErrorCode::DataCorruption,
                    AppRepoErrorDetail::PayDetail(paymethod, "missing-3party-row".to_string()),
                ))?;
            let arg = (store_id, row_profile);
            let storeprof_m = MerchantProfileModel::try_from(arg)
                .map_err(|(code, detail)| self._map_err_fetch(code, detail))?;
            let arg = (label3pty, row_3pty);
            let store3pty_m = Merchant3partyModel::try_from(arg)
                .map_err(|(code, detail)| self._map_err_fetch(code, detail))?;
            Ok(Some((storeprof_m, store3pty_m)))
        } else {
            Ok(None)
        }
    } // end of fn fetch

    async fn update_3party(
        &self,
        _store_id: u32,
        _m3pty: Merchant3partyModel,
    ) -> Result<(), AppRepoError> {
        Err(AppRepoError {
            fn_label: AppRepoErrorFnLabel::UpdateMerchant3party,
            code: AppErrorCode::NotImplemented,
            detail: AppRepoErrorDetail::Unknown,
        })
    }
} // end of impl MariadbMerchantRepo
