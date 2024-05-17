use std::result::Result;
use std::sync::Arc;

use async_trait::async_trait;
use ecommerce_common::model::order::BillingModel;

use super::{AbstractChargeRepo, AppRepoError, AppRepoErrorFnLabel};
use crate::adapter::datastore::AppDataStoreContext;
use crate::model::{ChargeLineModelSet, OrderLineModelSet};

pub(super) struct MariadbChargeRepo {}

impl MariadbChargeRepo {
    pub async fn new(_ds: Arc<AppDataStoreContext>) -> Result<Self, AppRepoError> {
        Ok(Self {})
    }
}

#[async_trait]
impl AbstractChargeRepo for MariadbChargeRepo {
    async fn get_unpaid_olines(
        &self,
        _usr_id: u32,
        _oid: &str,
    ) -> Result<Option<OrderLineModelSet>, AppRepoError> {
        let fn_label = AppRepoErrorFnLabel::GetUnpaidOlines;
        Err(AppRepoError { fn_label })
    }

    async fn create_order(
        &self,
        _olines: &OrderLineModelSet,
        _billing: &BillingModel,
    ) -> Result<(), AppRepoError> {
        let fn_label = AppRepoErrorFnLabel::CreateOrder;
        Err(AppRepoError { fn_label })
    }

    async fn create_charge(&self, _cline_set: &ChargeLineModelSet) -> Result<(), AppRepoError> {
        let fn_label = AppRepoErrorFnLabel::CreateCharge;
        Err(AppRepoError { fn_label })
    }
}
