mod mariadb;

use std::boxed::Box;
use std::io::ErrorKind;
use std::result::Result;
use std::sync::Arc;

use ecommerce_common::confidentiality::AbstractConfidentiality;
use ecommerce_common::config::{AppDataStoreCfg, AppDbServerType};
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::logging::AppLogContext;

pub(crate) use mariadb::AppDStoreMariaDB;

#[derive(Debug)]
pub enum AppDStoreError {
    ConfidentialLoad(AppErrorCode, String),
    ConfidentialResolve(String),
    GetConnIo(ErrorKind, String),
    GetConnTls(String),
    GetConnDbDriver(String),
    GetConnDbServer(AppErrorCode, u16, String),
    GetConnUnclassified(AppErrorCode, String),
    BackendNotSupport,
    NotImplemented,
}

pub struct AppDataStoreContext {
    _mariadb: Vec<Arc<AppDStoreMariaDB>>,
}

impl AppDataStoreContext {
    pub fn new(
        cfgs: &[AppDataStoreCfg],
        cfdntl: Arc<Box<dyn AbstractConfidentiality>>,
        logctx: Arc<AppLogContext>,
    ) -> Result<Self, AppDStoreError> {
        let mut errors = Vec::new();
        let _mariadb = cfgs
            .iter()
            .map(|cfg| match cfg {
                AppDataStoreCfg::InMemory(_c) => Err(AppDStoreError::BackendNotSupport),
                AppDataStoreCfg::DbServer(c) => match c.srv_type {
                    AppDbServerType::MariaDB => {
                        AppDStoreMariaDB::try_build(c, cfdntl.clone(), logctx.clone())
                    }
                    AppDbServerType::PostgreSQL => Err(AppDStoreError::BackendNotSupport),
                },
            })
            .filter_map(|r| match r {
                Ok(v) => Some(Arc::new(v)),
                Err(e) => {
                    errors.push(e);
                    None
                }
            })
            .collect();
        if let Some(e) = errors.pop() {
            Err(e)
        } else {
            Ok(Self { _mariadb })
        }
    }

    pub(crate) fn mariadb(&self, maybe_alias: Option<&str>) -> Option<Arc<AppDStoreMariaDB>> {
        let result = if let Some(a) = maybe_alias {
            self._mariadb.iter().find(|m| m.alias() == a)
        } else {
            self._mariadb.first()
        };
        result.map(Clone::clone)
    }
} // end of impl AppDataStoreContext
