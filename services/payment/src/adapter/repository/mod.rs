mod mariadb;

use std::boxed::Box;
use std::result::Result;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::model::order::BillingModel;

use crate::model::{
    BuyerPayInState, ChargeBuyerMetaModel, ChargeBuyerModel, ChargeLineBuyerModel,
    Merchant3partyModel, MerchantProfileModel, OrderLineModelSet,
};

use self::mariadb::charge::MariadbChargeRepo;
use self::mariadb::merchant::MariadbMerchantRepo;
use super::datastore::{AppDStoreError, AppDataStoreContext};

#[derive(Debug)]
pub enum AppRepoErrorFnLabel {
    InitChargeRepo,
    GetUnpaidOlines,
    CreateOrder,
    CreateCharge,
    FetchChargeMeta,
    FetchChargeLines,
    UpdateChargeProgress,
    InitMerchantRepo,
    CreateMerchant,
}
#[derive(Debug)]
pub enum AppRepoErrorDetail {
    OrderIDparse(String),
    OrderContactInfo(String),
    ChargeStatus(BuyerPayInState),
    PayMethodUnsupport(String),
    PayDetail(String, String), // pair of strings : 3rd-party name, error detail
    DataStore(AppDStoreError),
    DatabaseTxStart(String),
    DatabaseTxCommit(String),
    DatabaseExec(String),
    DatabaseQuery(String),
    DataRowParse(String),
    CurrencyPrecision(u32, String, String, u32, u32),
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

    async fn fetch_charge_meta(
        &self,
        usr_id: u32,
        create_time: DateTime<Utc>,
    ) -> Result<Option<ChargeBuyerMetaModel>, AppRepoError>;

    async fn fetch_all_charge_lines(
        &self,
        usr_id: u32,
        create_time: DateTime<Utc>,
    ) -> Result<Vec<ChargeLineBuyerModel>, AppRepoError>;

    async fn update_charge_progress(&self, meta: ChargeBuyerMetaModel) -> Result<(), AppRepoError>;
} // end of trait AbstractChargeRepo

#[async_trait]
pub trait AbstractMerchantRepo: Sync + Send {
    async fn create(
        &self,
        mprof: MerchantProfileModel,
        m3pty: Merchant3partyModel,
    ) -> Result<(), AppRepoError>;
} // end of trait AbstractMerchantRepo

pub async fn app_repo_charge(
    dstore: Arc<AppDataStoreContext>,
) -> Result<Box<dyn AbstractChargeRepo>, AppRepoError> {
    let repo = MariadbChargeRepo::new(dstore).await?;
    Ok(Box::new(repo))
}

pub async fn app_repo_merchant(
    dstore: Arc<AppDataStoreContext>,
) -> Result<Box<dyn AbstractMerchantRepo>, AppRepoError> {
    let repo = MariadbMerchantRepo::new(dstore)?;
    Ok(Box::new(repo))
}
