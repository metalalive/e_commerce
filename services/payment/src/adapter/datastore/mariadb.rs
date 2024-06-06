use std::boxed::Box;
use std::result::Result;
use std::sync::Arc;

use mysql_async::{
    Conn, Error as MysqlError, IoError, Opts, OptsBuilder, Pool, PoolConstraints, PoolOpts,
};
use serde::Deserialize;

use ecommerce_common::confidentiality::AbstractConfidentiality;
use ecommerce_common::config::AppDbServerCfg;
use ecommerce_common::error::{AppConfidentialityError, AppErrorCode};
use ecommerce_common::logging::AppLogContext;

use super::AppDStoreError;

#[allow(non_snake_case)]
#[derive(Deserialize)]
struct DbSecret {
    HOST: String,
    PORT: u16,
    USER: String,
    PASSWORD: String,
}

impl From<AppConfidentialityError> for AppDStoreError {
    fn from(value: AppConfidentialityError) -> Self {
        Self::ConfidentialLoad(value.code, value.detail)
    }
}
impl From<serde_json::Error> for AppDStoreError {
    fn from(value: serde_json::Error) -> Self {
        Self::ConfidentialResolve(value.to_string())
    }
}
impl From<MysqlError> for AppDStoreError {
    fn from(value: MysqlError) -> Self {
        match value {
            MysqlError::Io(e) => match e {
                IoError::Io(ioe) => Self::GetConnIo(ioe.kind(), ioe.to_string()),
                IoError::Tls(tlse) => Self::GetConnTls(tlse.to_string()),
            },
            MysqlError::Driver(e) => Self::GetConnDbDriver(e.to_string()),
            MysqlError::Server(e) => {
                let ecode = match e.code {
                    1037 | 1038 | 1041 => AppErrorCode::IOerror(std::io::ErrorKind::OutOfMemory),
                    1129 | 1040 => AppErrorCode::DatabaseServerBusy,
                    _others => AppErrorCode::DataCorruption,
                }; // see error code reference in mariadb doc
                Self::GetConnDbServer(ecode, e.code, e.to_string())
            }
            MysqlError::Url(e) => {
                Self::GetConnUnclassified(AppErrorCode::InvalidInput, e.to_string())
            }
            MysqlError::Other(e) => Self::GetConnUnclassified(AppErrorCode::Unknown, e.to_string()),
        }
    }
} // end of impl AppDStoreError

pub(crate) struct AppDStoreMariaDB {
    pool: Pool,
    _alias: String,
    _logctx: Arc<AppLogContext>,
}

impl AppDStoreMariaDB {
    pub(super) fn try_build(
        cfg: &AppDbServerCfg,
        cfdntl: Arc<Box<dyn AbstractConfidentiality>>,
        logctx: Arc<AppLogContext>,
    ) -> Result<Self, AppDStoreError> {
        let secret = {
            let serial = cfdntl.try_get_payload(cfg.confidentiality_path.as_str())?;
            serde_json::from_str::<DbSecret>(serial.as_str())?
        };
        let pool_opts = {
            let max_conns = (cfg.max_conns as usize).max(1);
            let constraints = PoolConstraints::new(1, max_conns).unwrap();
            PoolOpts::default().with_constraints(constraints)
        };
        let opts = {
            let builder = OptsBuilder::default()
                .user(Some(secret.USER.as_str()))
                .pass(Some(secret.PASSWORD.as_str()))
                .tcp_port(secret.PORT)
                .ip_or_hostname(secret.HOST)
                .db_name(Some(cfg.db_name.as_str()))
                .wait_timeout(Some(cfg.idle_timeout_secs as usize))
                .pool_opts(pool_opts);
            Opts::from(builder)
        };
        Ok(Self {
            _logctx: logctx,
            _alias: cfg.alias.clone(),
            pool: Pool::new(opts),
        })
    }

    pub(super) fn alias(&self) -> &str {
        self._alias.as_str()
    }

    pub(crate) fn log_context(&self) -> Arc<AppLogContext> {
        self._logctx.clone()
    }

    pub(crate) async fn acquire(&self) -> Result<Conn, AppDStoreError> {
        // actuire active connection
        let c = self.pool.get_conn().await?;
        Ok(c)
    }

    // pub(super) async fn disconnect(&self) -> Result<(), AppDStoreError> {
    //     // TODO , disconnect all connections in the pool during graceful shutdown
    //     Err(AppDStoreError::NotImplemented)
    // }
} // end of impl AppDStoreMariaDB
