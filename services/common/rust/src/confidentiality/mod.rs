mod userspace;

use std::boxed::Box;
use std::marker::{Send, Sync};
use std::result::Result as DefaultResult;

use crate::config::{AppConfidentialCfg, AppConfig};
use crate::error::AppConfidentialityError;

pub use userspace::UserSpaceConfidentiality;

pub fn build_context(
    cfg: &AppConfig,
) -> DefaultResult<Box<dyn AbstractConfidentiality>, AppConfidentialityError> {
    let confidential = &cfg.api_server.confidentiality;
    match confidential {
        AppConfidentialCfg::UserSpace { sys_path } => {
            let fullpath = cfg.basepath.system.clone() + sys_path;
            let obj = UserSpaceConfidentiality::build(fullpath);
            Ok(Box::new(obj))
        }
    }
}

pub trait AbstractConfidentiality: Send + Sync {
    // read-only interface to fetch user-defined private data
    fn try_get_payload(&self, id_: &str) -> DefaultResult<String, AppConfidentialityError>;
}
