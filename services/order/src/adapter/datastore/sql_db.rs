use std::boxed::Box;
use std::result::Result as DefaultResult;
use std::sync::Arc;

#[cfg(feature = "mariadb")]
use std::time::Duration;

#[cfg(feature = "mariadb")]
use serde::Deserialize;
#[cfg(feature = "mariadb")]
use sqlx::pool::{PoolConnection, PoolOptions};
#[cfg(feature = "mariadb")]
use sqlx::Pool;

#[cfg(feature = "mariadb")]
use sqlx::MySql;

#[cfg(feature = "mariadb")]
use sqlx::mysql::MySqlConnectOptions;

use crate::confidentiality::AbstractConfidentiality;
use crate::config::{AppDbServerCfg, AppDbServerType};
use crate::error::AppError;
use ecommerce_common::error::AppErrorCode;

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
    pool: Pool<MySql>,
}
#[cfg(not(feature = "mariadb"))]
pub struct AppMariaDbStore {}

#[cfg(feature = "mariadb")]
impl AppMariaDbStore {
    pub fn try_build(
        cfg: &AppDbServerCfg,
        confidential: Arc<Box<dyn AbstractConfidentiality>>,
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
        let pol_opts = PoolOptions::<MySql>::new()
            .max_connections(cfg.max_conns)
            .idle_timeout(Some(Duration::new(cfg.idle_timeout_secs as u64, 0)))
            .acquire_timeout(Duration::new(cfg.acquire_timeout_secs as u64, 0))
            .min_connections(0);
        let pool = pol_opts.connect_lazy_with(conn_opts);
        Ok(Self {
            pool,
            alias: cfg.alias.clone(),
        })
    }

    pub async fn acquire(&self) -> DefaultResult<PoolConnection<MySql>, AppError> {
        // TODO,
        // - figure out why `sqlx` requires to get (mutable) reference the pool-connection
        //   instance in order to get low-level connection for query execution.
        // - logging error message
        let pl = &self.pool;
        match pl.acquire().await {
            Ok(conn) => Ok(conn),
            Err(e) => {
                println!("[ERROR] pool stats : {:?}", pl);
                Err(e.into())
            }
        }
    }
} // end of impl AppMariaDbStore

#[cfg(not(feature = "mariadb"))]
impl AppMariaDbStore {
    pub fn try_build(
        cfg: &AppDbServerCfg,
        _confidential: Arc<Box<dyn AbstractConfidentiality>>,
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
