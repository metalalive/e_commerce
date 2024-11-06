use std::result::Result;
use std::sync::Arc;

use async_trait::async_trait;
use mysql_async::prelude::{Query, Queryable, WithParams};
use mysql_async::{Conn, IsolationLevel, Params, TxOpts};

use ecommerce_common::error::AppErrorCode;
use ecommerce_common::logging::{app_log_event, AppLogLevel};

use crate::adapter::datastore::{AppDStoreMariaDB, AppDataStoreContext};
use crate::model::{
    Label3party, Merchant3partyModel, Merchant3partyStripeModel, MerchantProfileModel,
};

use super::super::{AbstractMerchantRepo, AppRepoError, AppRepoErrorDetail, AppRepoErrorFnLabel};
use super::{raw_column_to_datetime, DATETIME_FMT_P0F};

struct InsertUpdateProfileArgs(String, Params);
struct Insert3partyArgs(String, Params);
struct Update3partyArgs(String, Params);
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

impl TryInto<(String, String)> for Merchant3partyModel {
    type Error = (AppErrorCode, AppRepoErrorDetail);
    fn try_into(self) -> Result<(String, String), Self::Error> {
        match self {
            Self::Stripe(sm) => {
                let label = "Stripe".to_string();
                let d = serde_json::to_string(&sm).map_err(|e| {
                    (
                        AppErrorCode::DataCorruption,
                        AppRepoErrorDetail::PayDetail(label.clone(), e.to_string()),
                    )
                })?;
                Ok((label, d))
            }
            Self::Unknown => Err((
                AppErrorCode::InvalidInput,
                AppRepoErrorDetail::PayMethodUnsupport("unknown".to_string()),
            )),
        }
    }
} // end of impl Merchant3partyModel

impl TryFrom<(u32, Merchant3partyModel)> for Insert3partyArgs {
    type Error = (AppErrorCode, AppRepoErrorDetail);
    fn try_from(value: (u32, Merchant3partyModel)) -> Result<Self, Self::Error> {
        let (store_id, m3pty) = value;
        let (method, detail): (String, String) = m3pty.try_into()?;
        let arg = vec![store_id.into(), method.into(), detail.into()];
        let params = Params::Positional(arg);
        let stmt = "INSERT INTO `merchant_3party`(`sid`,`method`,`detail`) VALUES (?,?,?)";
        Ok(Self(stmt.to_string(), params))
    } // end of try-from
} // end of impl Insert3partyArgs

impl TryFrom<(u32, Merchant3partyModel)> for Update3partyArgs {
    type Error = (AppErrorCode, AppRepoErrorDetail);
    fn try_from(value: (u32, Merchant3partyModel)) -> Result<Self, Self::Error> {
        let (store_id, m3pty) = value;
        let (method, detail): (String, String) = m3pty.try_into()?;
        let arg = vec![detail.into(), store_id.into(), method.into()];
        let params = Params::Positional(arg);
        let stmt = "UPDATE `merchant_3party` SET `detail`=? WHERE `sid`=? AND `method`=?";
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

impl From<(u32, Label3party)> for Fetch3partyArgs {
    fn from(value: (u32, Label3party)) -> Self {
        let stmt = "SELECT `detail` FROM `merchant_3party` WHERE `sid`=? AND `method`=?";
        let (store_id, l3pt) = value;
        let paymethod = l3pt.to_string();
        let arg = vec![store_id.into(), paymethod.as_str().into()];
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
        let staff_ids = if staff_ids_raw.is_empty() {
            Vec::new()
        } else {
            staff_ids_raw
                .split(',')
                .filter_map(|v| {
                    v.parse::<u32>()
                        .map_err(|e| {
                            let msg = format!("invalid-staff-id: {v} : {:?}", e);
                            errors.push(msg);
                        })
                        .ok()
                })
                .collect::<Vec<_>>()
        };
        if errors.is_empty() {
            Ok(Self { id, name, supervisor_id, staff_ids, last_update })
        } else {
            let code = AppErrorCode::DataCorruption;
            let detail = AppRepoErrorDetail::DataRowParse(errors.remove(0));
            Err((code, detail))
        }
    } // end of fn try-from
} // end of impl MerchantProfileModel

impl TryFrom<(Label3party, Merc3ptyRowType)> for Merchant3partyModel {
    type Error = (AppErrorCode, AppRepoErrorDetail);
    fn try_from(value: (Label3party, Merc3ptyRowType)) -> Result<Self, Self::Error> {
        let (label, (detail_raw,)) = value;
        let out = match label {
            Label3party::Stripe => {
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
        ds.mariadb(Some("db-write-primary"))
            .map(|found| Self { _dstore: found })
            .ok_or(AppRepoError {
                fn_label: AppRepoErrorFnLabel::InitMerchantRepo,
                code: AppErrorCode::MissingDataStore,
                detail: AppRepoErrorDetail::Unknown,
            })
    }

    async fn fetch_profile_common(
        exec: &mut Conn,
        store_id: u32,
    ) -> Result<Option<MerchantProfileModel>, (AppErrorCode, AppRepoErrorDetail)> {
        let FetchProfileArgs(stmt, params) = FetchProfileArgs::from(store_id);
        let maybe_row = stmt
            .with(params)
            .first::<MercProfRowType, &mut Conn>(exec)
            .await
            .map_err(|e| {
                (
                    AppErrorCode::RemoteDbServerFailure,
                    AppRepoErrorDetail::DatabaseQuery(e.to_string()),
                )
            })?;
        if let Some(row_profile) = maybe_row {
            let arg = (store_id, row_profile);
            MerchantProfileModel::try_from(arg).map(Some)
        } else {
            Ok(None)
        }
    }

    #[rustfmt::skip]
    fn _map_log_err(
        &self, code: AppErrorCode, detail: AppRepoErrorDetail,
        fn_label: AppRepoErrorFnLabel,
    ) -> AppRepoError {
        let e = AppRepoError {fn_label, code, detail};
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
                self._map_log_err(code, detail, AppRepoErrorFnLabel::CreateMerchant)
            })?;
        let q_arg_prof = InsertUpdateProfileArgs::from(mprof);

        let mut conn = self._dstore.acquire().await.map_err(|e| {
            self._map_log_err(
                AppErrorCode::DatabaseServerBusy,
                AppRepoErrorDetail::DataStore(e),
                AppRepoErrorFnLabel::CreateMerchant,
            )
        })?;
        let mut options = TxOpts::new();
        // in the application, only the owner or initial supervisor of a shop can invoke
        // this function during onboarding process, non-repeatable read should not happen
        // in such scenario, that is why iso-level is set to `read-committed`
        // TODO: recheck this explanation
        options.with_isolation_level(IsolationLevel::ReadCommitted);
        let mut tx = conn.start_transaction(options).await.map_err(|e| {
            self._map_log_err(
                AppErrorCode::RemoteDbServerFailure,
                AppRepoErrorDetail::DatabaseTxStart(e.to_string()),
                AppRepoErrorFnLabel::CreateMerchant,
            )
        })?;
        let resultset = tx
            .exec_iter(q_arg_prof.0, q_arg_prof.1)
            .await
            .map_err(|e| {
                self._map_log_err(
                    AppErrorCode::RemoteDbServerFailure,
                    AppRepoErrorDetail::DatabaseExec(e.to_string()),
                    AppRepoErrorFnLabel::CreateMerchant,
                )
            })?;
        let cond = [1u64, 2].contains(&resultset.affected_rows());
        if !cond {
            let msg = format!("num-rows-affected: {}", resultset.affected_rows());
            let e = self._map_log_err(
                AppErrorCode::RemoteDbServerFailure,
                AppRepoErrorDetail::DatabaseExec(msg),
                AppRepoErrorFnLabel::CreateMerchant,
            );
            return Err(e);
        }
        let resultset = tx
            .exec_iter(q_arg_3pty.0, q_arg_3pty.1)
            .await
            .map_err(|e| {
                self._map_log_err(
                    AppErrorCode::RemoteDbServerFailure,
                    AppRepoErrorDetail::DatabaseExec(e.to_string()),
                    AppRepoErrorFnLabel::CreateMerchant,
                )
            })?;
        if resultset.affected_rows() != 1u64 {
            let msg = format!("num-rows-affected: {}", resultset.affected_rows());
            let e = self._map_log_err(
                AppErrorCode::RemoteDbServerFailure,
                AppRepoErrorDetail::DatabaseExec(msg),
                AppRepoErrorFnLabel::CreateMerchant,
            );
            return Err(e);
        }

        tx.commit().await.map_err(|e| {
            self._map_log_err(
                AppErrorCode::RemoteDbServerFailure,
                AppRepoErrorDetail::DatabaseTxCommit(e.to_string()),
                AppRepoErrorFnLabel::CreateMerchant,
            )
        })
    } // end of fn create

    async fn fetch(
        &self,
        store_id: u32,
        label3pty: Label3party,
    ) -> Result<Option<(MerchantProfileModel, Merchant3partyModel)>, AppRepoError> {
        let mut conn = self._dstore.acquire().await.map_err(|e| {
            self._map_log_err(
                AppErrorCode::DatabaseServerBusy,
                AppRepoErrorDetail::DataStore(e),
                AppRepoErrorFnLabel::FetchMerchant,
            )
        })?;
        let exec = &mut conn;
        let maybe_storeprof_m = Self::fetch_profile_common(exec, store_id)
            .await
            .map_err(|e| self._map_log_err(e.0, e.1, AppRepoErrorFnLabel::FetchMerchant))?;

        let storeprof_m = if let Some(v) = maybe_storeprof_m {
            v
        } else {
            return Ok(None);
        };

        let q_arg_3pty = Fetch3partyArgs::from((store_id, label3pty));
        let Fetch3partyArgs(stmt, params, paymethod) = q_arg_3pty;
        let row_3pty = stmt
            .with(params)
            .first::<Merc3ptyRowType, &mut Conn>(exec)
            .await
            .map_err(|e| {
                self._map_log_err(
                    AppErrorCode::RemoteDbServerFailure,
                    AppRepoErrorDetail::DatabaseQuery(e.to_string()),
                    AppRepoErrorFnLabel::FetchMerchant,
                )
            })?
            .ok_or(AppRepoErrorDetail::PayDetail(
                paymethod,
                format!("missing-3party-row:{store_id}"),
            ))
            .map_err(|detail| {
                self._map_log_err(
                    AppErrorCode::DataCorruption,
                    detail,
                    AppRepoErrorFnLabel::FetchMerchant,
                )
            })?;
        Merchant3partyModel::try_from((label3pty, row_3pty))
            .map(|store3pty_m| Some((storeprof_m, store3pty_m)))
            .map_err(|e| self._map_log_err(e.0, e.1, AppRepoErrorFnLabel::FetchMerchant))
    } // end of fn fetch

    async fn fetch_profile(
        &self,
        store_id: u32,
    ) -> Result<Option<MerchantProfileModel>, AppRepoError> {
        let mut conn = self._dstore.acquire().await.map_err(|e| {
            self._map_log_err(
                AppErrorCode::DatabaseServerBusy,
                AppRepoErrorDetail::DataStore(e),
                AppRepoErrorFnLabel::FetchMerchantProf,
            )
        })?;
        Self::fetch_profile_common(&mut conn, store_id)
            .await
            .map_err(|e| self._map_log_err(e.0, e.1, AppRepoErrorFnLabel::FetchMerchantProf))
    } // end of fn fetch_profile

    async fn update_3party(
        &self,
        store_id: u32,
        m3pty: Merchant3partyModel,
    ) -> Result<(), AppRepoError> {
        let q_arg_3pty =
            Update3partyArgs::try_from((store_id, m3pty)).map_err(|(code, detail)| {
                self._map_log_err(code, detail, AppRepoErrorFnLabel::UpdateMerchant3party)
            })?;
        let mut conn = self._dstore.acquire().await.map_err(|e| {
            self._map_log_err(
                AppErrorCode::RemoteDbServerFailure,
                AppRepoErrorDetail::DataStore(e),
                AppRepoErrorFnLabel::UpdateMerchant3party,
            )
        })?;

        let Update3partyArgs(stmt, params) = q_arg_3pty;
        let resultset = conn.exec_iter(stmt, params).await.map_err(|e| {
            self._map_log_err(
                AppErrorCode::RemoteDbServerFailure,
                AppRepoErrorDetail::DatabaseExec(e.to_string()),
                AppRepoErrorFnLabel::UpdateMerchant3party,
            )
        })?;

        if resultset.affected_rows() == 1u64 {
            Ok(())
        } else {
            let msg = format!("num-rows-affected: {}", resultset.affected_rows());
            let e = self._map_log_err(
                AppErrorCode::RemoteDbServerFailure,
                AppRepoErrorDetail::DatabaseExec(msg),
                AppRepoErrorFnLabel::UpdateMerchant3party,
            );
            Err(e)
        }
    } // end of fn update_3party
} // end of impl MariadbMerchantRepo
