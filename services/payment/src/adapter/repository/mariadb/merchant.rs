use std::result::Result;
use std::sync::Arc;

use async_trait::async_trait;
use mysql_async::prelude::Queryable;
use mysql_async::{IsolationLevel, Params, TxOpts};

use ecommerce_common::error::AppErrorCode;
use ecommerce_common::logging::{app_log_event, AppLogLevel};

use crate::adapter::datastore::{AppDStoreMariaDB, AppDataStoreContext};
use crate::model::{Merchant3partyModel, MerchantProfileModel};

use super::super::{AbstractMerchantRepo, AppRepoError, AppRepoErrorDetail, AppRepoErrorFnLabel};
use super::DATETIME_FMT_P0F;

struct InsertUpdateProfileArgs(String, Params);
struct Insert3partyArgs(String, Params);

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
} // end of impl MariadbMerchantRepo
