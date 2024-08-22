use std::result::Result;
use std::sync::Arc;

use async_trait::async_trait;
use ecommerce_common::error::AppErrorCode;

use super::super::{AbstractMerchantRepo, AppRepoError, AppRepoErrorDetail, AppRepoErrorFnLabel};
use crate::adapter::datastore::AppDataStoreContext;
use crate::model::MerchantProfileModel;

pub(crate) struct MariadbMerchantRepo;

impl MariadbMerchantRepo {
    pub(crate) async fn new(_dstore: Arc<AppDataStoreContext>) -> Result<Self, AppRepoError> {
        let e = AppRepoError {
            code: AppErrorCode::NotImplemented,
            fn_label: AppRepoErrorFnLabel::InitMerchantRepo,
            detail: AppRepoErrorDetail::Unknown,
        };
        Err(e)
    }
}

#[async_trait]
impl AbstractMerchantRepo for MariadbMerchantRepo {
    async fn create(&self, _m: MerchantProfileModel) -> Result<(), AppRepoError> {
        let e = AppRepoError {
            code: AppErrorCode::NotImplemented,
            fn_label: AppRepoErrorFnLabel::InitMerchantRepo,
            detail: AppRepoErrorDetail::Unknown,
        };
        Err(e)
    }
}
