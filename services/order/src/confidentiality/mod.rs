mod userspace;

use std::result::Result as DefaultResult;
use std::boxed::Box;
use std::marker::{Send, Sync};

use crate::{AppConfig, AppConfidentialCfg};
use crate::error::{AppError, AppErrorCode};

pub use userspace::UserSpaceConfidentiality;

pub fn build_context(cfg:&AppConfig) -> DefaultResult<Box<dyn AbstractConfidentiality>, AppError>
{
    let confidential = &cfg.api_server.confidentiality;
    match confidential {
        AppConfidentialCfg::UserSpace{sys_path} => {
            let fullpath = cfg.basepath.system.clone() + sys_path;
            let obj = UserSpaceConfidentiality::build(fullpath);
            Ok(Box::new(obj))
        },
        _others => Err(AppError{code:AppErrorCode::NoConfidentialityCfg, detail: None})
    }
}

pub trait AbstractConfidentiality : Send + Sync
{ // read-only interface to fetch user-defined private data
    fn try_get_payload(&self, id_:&str) -> DefaultResult<String, AppError>;
}

