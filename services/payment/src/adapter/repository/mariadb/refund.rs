use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use ecommerce_common::error::AppErrorCode;

use super::super::{AbstractRefundRepo, AppRepoError, AppRepoErrorDetail, AppRepoErrorFnLabel};
use crate::adapter::datastore::{AppDStoreMariaDB, AppDataStoreContext};
use crate::model::OrderRefundModel;

pub(crate) struct MariaDbRefundRepo {
    _dstore: Arc<AppDStoreMariaDB>,
}

impl MariaDbRefundRepo {
    pub(crate) fn new(ds: Arc<AppDataStoreContext>) -> Result<Self, AppRepoError> {
        ds.mariadb(None)
            .map(|found| Self { _dstore: found })
            .ok_or(AppRepoError {
                fn_label: AppRepoErrorFnLabel::InitRefundRepo,
                code: AppErrorCode::MissingDataStore,
                detail: AppRepoErrorDetail::Unknown,
            })
    }
} // end of impl MariaDbRefundRepo

#[async_trait]
impl AbstractRefundRepo for MariaDbRefundRepo {
    async fn last_time_synced(&self) -> Result<DateTime<Utc>, AppRepoError> {
        Err(AppRepoError {
            fn_label: AppRepoErrorFnLabel::RefundGetTimeSynced,
            code: AppErrorCode::NotImplemented,
            detail: AppRepoErrorDetail::Unknown,
        })
    }

    async fn update_sycned_time(&self, t: DateTime<Utc>) -> Result<(), AppRepoError> {
        Err(AppRepoError {
            fn_label: AppRepoErrorFnLabel::RefundUpdateTimeSynced,
            code: AppErrorCode::NotImplemented,
            detail: AppRepoErrorDetail::Unknown,
        })
    }

    async fn save_request(&self, req: Vec<OrderRefundModel>) -> Result<(), AppRepoError> {
        Err(AppRepoError {
            fn_label: AppRepoErrorFnLabel::RefundSaveReq,
            code: AppErrorCode::NotImplemented,
            detail: AppRepoErrorDetail::Unknown,
        })
    }
} // end of impl MariaDbRefundRepo
