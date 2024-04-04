use order::repository::app_repo_product_policy;

use super::super::in_mem::product_policy::save_fetch_ok_common;
use crate::repository::mariadb::dstore_ctx_setup;

#[cfg(feature = "mariadb")]
#[tokio::test]
async fn save_fetch_ok() {
    let ds = dstore_ctx_setup();
    let repo = app_repo_product_policy(ds).await.unwrap();
    save_fetch_ok_common(repo).await;
}
