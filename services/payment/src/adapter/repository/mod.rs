mod mariadb;

use std::boxed::Box;
use std::result::Result;
use std::sync::Arc;

use async_trait::async_trait;
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::model::order::BillingModel;

use crate::model::{ChargeBuyerModel, OrderLineModelSet};

use self::mariadb::charge::MariadbChargeRepo;
use super::datastore::{AppDStoreError, AppDataStoreContext};

#[derive(Debug)]
pub enum AppRepoErrorFnLabel {
    InitRepo,
    GetUnpaidOlines,
    CreateOrder,
    CreateCharge,
}
#[derive(Debug)]
pub enum AppRepoErrorDetail {
    OrderIDparse(String),
    OrderContactInfo(String),
    DataStore(AppDStoreError),
    DatabaseTxStart(String),
    DatabaseTxCommit(String),
    DatabaseExec(String),
    DatabaseQuery(String),
    DataRowParse(String),
    Unknown,
}

#[derive(Debug)]
pub struct AppRepoError {
    pub fn_label: AppRepoErrorFnLabel,
    pub code: AppErrorCode,
    pub detail: AppRepoErrorDetail,
}

#[async_trait]
pub trait AbstractChargeRepo: Sync + Send {
    async fn get_unpaid_olines(
        &self,
        usr_id: u32,
        oid: &str,
    ) -> Result<Option<OrderLineModelSet>, AppRepoError>;

    // Note, without the syntax `&self` , this trait type will be considered as
    // imcomplete type, then cause cycle checking error in compiler, TODO
    // , understand the reason.
    async fn create_order(
        &self,
        olines: &OrderLineModelSet,
        billing: &BillingModel,
    ) -> Result<(), AppRepoError>;

    async fn create_charge(&self, cline_set: ChargeBuyerModel) -> Result<(), AppRepoError>;
    // TODO, extra trait methods only for test data injection
}

pub async fn app_repo_charge(
    dstore: Arc<AppDataStoreContext>,
) -> Result<Box<dyn AbstractChargeRepo>, AppRepoError> {
    let repo = MariadbChargeRepo::new(dstore).await?;
    Ok(Box::new(repo))
}
