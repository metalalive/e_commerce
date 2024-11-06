mod mariadb;

use std::boxed::Box;
use std::future::Future;
use std::pin::Pin;
use std::result::Result;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::model::order::BillingModel;

use crate::adapter::processor::{AbstractPaymentProcessor, AppProcessorError};
use crate::api::web::dto::{RefundCompletionReqDto, ReportTimeRangeDto};
use crate::model::{
    BuyerPayInState, ChargeBuyerMetaModel, ChargeBuyerModel, ChargeLineBuyerModel, ChargeRefundMap,
    Label3party, Merchant3partyModel, MerchantProfileModel, OrderLineModelSet, OrderRefundModel,
    PayoutModel, RefundModelError, RefundReqResolutionModel,
};

use self::mariadb::charge::MariadbChargeRepo;
use self::mariadb::merchant::MariadbMerchantRepo;
use self::mariadb::refund::MariaDbRefundRepo;
use self::mariadb::reporting::MariadbReportingRepo;
use super::datastore::{AppDStoreError, AppDataStoreContext};

#[derive(Debug)]
pub enum AppRepoErrorFnLabel {
    InitChargeRepo,
    GetUnpaidOlines,
    CreateOrder,
    CreateCharge,
    CreateMerchant,
    CreatePayout,
    FetchChargeIds,
    FetchChargeMeta,
    FetchChargeLines,
    FetchMerchant,
    FetchMerchantProf,
    FetchChargeByMerchant,
    FetchPayout,
    UpdateChargeProgress,
    UpdateChargeLinesRefund,
    UpdateMerchant3party,
    InitMerchantRepo,
    InitRefundRepo,
    InitReportingRepo,
    RefundGetTimeSynced,
    RefundUpdateTimeSynced,
    RefundSaveReq,
    ResolveRefundReq,
    ReportChargeByMerchant,
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
    RefundResolution(Vec<RefundModelError>),
    ConstructChargeFailure(String),
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

    async fn fetch_charge_ids(
        &self,
        oid: &str,
    ) -> Result<Option<(u32, Vec<DateTime<Utc>>)>, AppRepoError>;

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

    async fn fetch_charge_by_merchant(
        &self,
        buyer_id: u32,
        create_time: DateTime<Utc>,
        store_id: u32,
    ) -> Result<Option<ChargeBuyerModel>, AppRepoError>;

    async fn update_lines_refund(&self, cl_map: ChargeRefundMap) -> Result<(), AppRepoError>;

    /// the method `fetch_payout()` returns payout summary of a specific payment made by client
    /// , which includes total amount that has been transferred to merchant's bank account.
    async fn fetch_payout(
        &self,
        store_id: u32,
        buyer_id: u32,
        create_time: DateTime<Utc>,
    ) -> Result<Option<PayoutModel>, AppRepoError>;

    async fn create_payout(&self, payout_m: PayoutModel) -> Result<(), AppRepoError>;
} // end of trait AbstractChargeRepo

#[async_trait]
pub trait AbstractMerchantRepo: Sync + Send {
    async fn create(
        &self,
        mprof: MerchantProfileModel,
        m3pty: Merchant3partyModel,
    ) -> Result<(), AppRepoError>;

    async fn fetch(
        &self,
        store_id: u32,
        label3pty: Label3party,
    ) -> Result<Option<(MerchantProfileModel, Merchant3partyModel)>, AppRepoError>;

    async fn update_3party(
        &self,
        store_id: u32,
        m3pty: Merchant3partyModel,
    ) -> Result<(), AppRepoError>;

    async fn fetch_profile(
        &self,
        store_id: u32,
    ) -> Result<Option<MerchantProfileModel>, AppRepoError>;
} // end of trait AbstractMerchantRepo

#[async_trait]
pub trait AbstractRefundRepo: Sync + Send {
    async fn last_time_synced(&self) -> Result<Option<DateTime<Utc>>, AppRepoError>;

    async fn update_sycned_time(&self, t: DateTime<Utc>) -> Result<(), AppRepoError>;

    async fn save_request(&self, req: Vec<OrderRefundModel>) -> Result<(), AppRepoError>;

    async fn resolve_request(
        &self,
        merchant_id: u32,
        cmplt_req: RefundCompletionReqDto,
        charge_ms: Vec<ChargeBuyerModel>,
        processor: Arc<Box<dyn AbstractPaymentProcessor>>,
        cb: AppRefundRslvReqCallback,
    ) -> Result<AppRefundRslvReqOkReturn, AppRepoError>;
}

pub type AppRefundRslvReqOkReturn = Vec<Result<RefundReqResolutionModel, AppProcessorError>>;

pub type AppRefundRslvReqCbReturn = Result<AppRefundRslvReqOkReturn, AppRepoErrorDetail>;

/*
 * Note / CAUTION :
 *
 * - A function signature requires lifetime annotation if it contains more than
 *   one references in its input and one reference in its output.
 *
 * - Ideally callers should pass reference to `Vec<ChargeBuyerModel>` to this
 *   callback, that is, the 3rd input argument is `&Vec<ChargeBuyerModel>`. However
 *   current use cases in this application cannot satisfy lifetime restriction
 *   , the 3rd input argument still remains as `Vec<ChargeBuyerModel>`
 *
 * - In this application, `&mut OrderRefundModel` in `AppRefundRslvReqCallback`
 *   lives only within the scope of trait method `resolve_request(...)`, if
 *   `&mut OrderRefundModel` annotated with lifetime label
 *   (such as `&'a mut OrderRefundModel`) , then the trait `AbstractRefundRepo`
 *   has to be declared with the same lifetime, this adds difficulty for other
 *   callers in use-case layer. Currently I have not figured out any approach
 *   which handles such issue efficiently.
 *
 * - It is extremely difficult to manage several references to other types
 *   outside the repo struct and each with different lifetime annotations.
 *
 * - To keep design simple, only one reference is allowed in this function
 *   pointer type
 *
 * */
pub type AppRefundRslvReqCallback =
    fn(
        &mut OrderRefundModel,
        RefundCompletionReqDto,
        Vec<ChargeBuyerModel>,
        Arc<Box<dyn AbstractPaymentProcessor>>,
    ) -> Pin<Box<dyn Future<Output = AppRefundRslvReqCbReturn> + Send + '_>>;

#[async_trait]
pub trait AbstractReportingRepo: Send + Sync {
    // TODO, pagination
    async fn fetch_charges_by_merchant(
        &self,
        store_id: u32,
        t_range: ReportTimeRangeDto,
    ) -> Result<Vec<ChargeBuyerModel>, AppRepoError>;
}

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

pub async fn app_repo_refund(
    dstore: Arc<AppDataStoreContext>,
) -> Result<Box<dyn AbstractRefundRepo>, AppRepoError> {
    let repo = MariaDbRefundRepo::new(dstore)?;
    Ok(Box::new(repo))
}

pub async fn app_repo_reporting(
    dstore: Arc<AppDataStoreContext>,
) -> Result<Box<dyn AbstractReportingRepo>, AppRepoError> {
    let repo = MariadbReportingRepo::new(dstore)?;

    Ok(Box::new(repo))
}
