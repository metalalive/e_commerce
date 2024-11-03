use std::result::Result;
use std::sync::Arc;

use async_trait::async_trait;
use ecommerce_common::error::AppErrorCode;

use crate::adapter::datastore::{AppDStoreMariaDB, AppDataStoreContext};
use crate::adapter::repository::{
    AbstractReportingRepo, AppRepoError, AppRepoErrorDetail, AppRepoErrorFnLabel,
};
use crate::api::web::dto::ReportTimeRangeDto;
use crate::model::ChargeBuyerModel;

pub struct MariadbReportingRepo {
    _dstore: Arc<AppDStoreMariaDB>,
}

impl MariadbReportingRepo {
    pub(crate) fn new(_ds: Arc<AppDataStoreContext>) -> Result<Self, AppRepoError> {
        // TODO, separate primary and replica servers if necessary
        Err(AppRepoError {
            fn_label: AppRepoErrorFnLabel::InitReportingRepo,
            code: AppErrorCode::NotImplemented,
            detail: AppRepoErrorDetail::Unknown,
        })
    }
}

#[async_trait]
impl AbstractReportingRepo for MariadbReportingRepo {
    async fn fetch_charges_by_merchant(
        &self,
        _store_id: u32,
        _t_range: ReportTimeRangeDto,
    ) -> Result<Vec<ChargeBuyerModel>, AppRepoError> {
        Err(AppRepoError {
            fn_label: AppRepoErrorFnLabel::ReportChargeByMerchant,
            code: AppErrorCode::NotImplemented,
            detail: AppRepoErrorDetail::Unknown,
        })
    }
}
