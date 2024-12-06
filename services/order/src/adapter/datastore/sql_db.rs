use std::boxed::Box;
use std::result::Result as DefaultResult;
use std::sync::Arc;

#[cfg(feature = "mariadb")]
use std::time::Duration;

#[cfg(feature = "mariadb")]
use serde::Deserialize;
#[cfg(feature = "mariadb")]
use sqlx::mysql::MySqlConnectOptions;
#[cfg(feature = "mariadb")]
use sqlx::pool::{PoolConnection, PoolOptions};
#[cfg(feature = "mariadb")]
use sqlx::MySql;
#[cfg(feature = "mariadb")]
use sqlx::Pool;

use tokio::sync::OnceCell;

use ecommerce_common::confidentiality::AbstractConfidentiality;
use ecommerce_common::config::{AppDbServerCfg, AppDbServerType};
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::logging::{app_log_event, AppLogContext, AppLogLevel};

use crate::error::AppError;

#[cfg(feature = "mariadb")]
#[allow(non_snake_case)]
#[derive(Deserialize)]
struct DbSecret {
    HOST: String,
    PORT: u16,
    USER: String,
    PASSWORD: String,
}

#[cfg(feature = "mariadb")]
pub struct AppMariaDbStore {
    pub alias: String,
    max_conns: u32,
    acquire_timeout_secs: u16,
    idle_timeout_secs: u16,
    conn_opts: MySqlConnectOptions,
    pool: OnceCell<Pool<MySql>>,
    logctx: Arc<AppLogContext>,
}
#[cfg(not(feature = "mariadb"))]
pub struct AppMariaDbStore {}

#[cfg(feature = "mariadb")]
impl AppMariaDbStore {
    pub fn try_build(
        cfg: &AppDbServerCfg,
        confidential: Arc<Box<dyn AbstractConfidentiality>>,
        logctx: Arc<AppLogContext>,
    ) -> DefaultResult<Self, AppError> {
        if !matches!(cfg.srv_type, AppDbServerType::MariaDB) {
            let detail = format!("db-cfg-server-type: {:?}", cfg.srv_type);
            return Err(AppError {
                code: AppErrorCode::InvalidInput,
                detail: Some(detail),
            });
        }
        let serial = confidential.try_get_payload(cfg.confidentiality_path.as_str())?;
        let conn_opts = match serde_json::from_str::<DbSecret>(serial.as_str()) {
            Ok(s) => MySqlConnectOptions::new()
                .host(s.HOST.as_str())
                .port(s.PORT)
                .username(s.USER.as_str())
                .password(s.PASSWORD.as_str())
                .database(cfg.db_name.as_str()),
            Err(e) => {
                let detail = e.to_string() + ", secret-parsing-error, source: AppMariaDbStore";
                return Err(AppError {
                    code: AppErrorCode::InvalidJsonFormat,
                    detail: Some(detail),
                });
            }
        };
        Ok(Self {
            conn_opts,
            max_conns: cfg.max_conns,
            idle_timeout_secs: cfg.idle_timeout_secs,
            acquire_timeout_secs: cfg.acquire_timeout_secs,
            pool: OnceCell::new(),
            alias: cfg.alias.clone(),
            logctx: logctx,
        })
    } // end of fn try-build

    pub async fn acquire(&self) -> DefaultResult<PoolConnection<MySql>, AppError> {
        // Note
        // `sqlx` requires to get (mutable) reference the pool-connection
        // instance in order to get low-level connection for query execution.
        let pl = self
            .pool
            .get_or_init(|| async {
                let pol_opts = PoolOptions::<MySql>::new()
                    .max_connections(self.max_conns)
                    .idle_timeout(Some(Duration::new(self.idle_timeout_secs as u64, 0)))
                    .acquire_timeout(Duration::new(self.acquire_timeout_secs as u64, 0))
                    .min_connections(0);
                let _conn_opts = self.conn_opts.clone();
                // the following connect method will spawn new task for maintain connection lifetime
                // , this part of code has to run after tokio runtime executor is ready
                pol_opts.connect_lazy_with(_conn_opts)
            })
            .await;
        let lctx = &self.logctx;
        app_log_event!(lctx, AppLogLevel::DEBUG, "acquire-op-done");
        pl.acquire().await.map_err(|e| {
            app_log_event!(lctx, AppLogLevel::ERROR, "pool:{:?}, e:{:?}", pl, e);
            e.into()
        })
    }
} // end of impl AppMariaDbStore

#[cfg(not(feature = "mariadb"))]
impl AppMariaDbStore {
    pub fn try_build(
        cfg: &AppDbServerCfg,
        _confidential: Arc<Box<dyn AbstractConfidentiality>>,
        _logctx: Arc<AppLogContext>,
    ) -> DefaultResult<Self, AppError> {
        let detail = format!(
            "sql-db, type:{:?}, alias:{}",
            cfg.srv_type,
            cfg.alias.as_str()
        );
        Err(AppError {
            code: AppErrorCode::FeatureDisabled,
            detail: Some(detail),
        })
    }
} // end of impl AppMariaDbStore
