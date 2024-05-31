mod cart;
mod oorder;
mod product_policy;
mod product_price;

use std::env;
use std::sync::Arc;

use ecommerce_common::confidentiality::UserSpaceConfidentiality;
use ecommerce_common::constant::env_vars::SYS_BASEPATH;
use order::AppDataStoreContext;

use crate::ut_setup_share_state;

fn dstore_ctx_setup() -> Arc<AppDataStoreContext> {
    let cfdntl = {
        let sys_basepath = env::var(SYS_BASEPATH).unwrap();
        let path = sys_basepath.clone() + "/common/data/secrets.json";
        UserSpaceConfidentiality::build(path)
    };
    let app_state = ut_setup_share_state("config_ok.json", Box::new(cfdntl));
    let dstore = app_state.datastore();
    assert!(dstore.sql_dbs.is_some());
    let db_stores = dstore.sql_dbs.as_ref().unwrap();
    assert!(!db_stores.is_empty());
    dstore
}
