mod mariadb;

use std::boxed::Box;
use std::result::Result;
use std::sync::Arc;

use async_trait::async_trait;
use ecommerce_common::model::order::BillingModel;

use crate::model::{ChargeLineModelSet, OrderLineModelSet};

use self::mariadb::MariadbChargeRepo;
use super::datastore::AppDataStoreContext;

pub enum AppRepoErrorFnLabel {
    GetUnpaidOlines,
    CreateOrder,
    CreateCharge,
}

pub struct AppRepoError {
    pub fn_label: AppRepoErrorFnLabel,
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

    async fn create_charge(&self, cline_set: &ChargeLineModelSet) -> Result<(), AppRepoError>;
    // TODO, extra trait methods only for test data injection
}

pub async fn app_repo_charge(
    dstore: Arc<AppDataStoreContext>,
) -> Result<Box<dyn AbstractChargeRepo>, AppRepoError> {
    let repo = MariadbChargeRepo::new(dstore).await?;
    Ok(Box::new(repo))
}
