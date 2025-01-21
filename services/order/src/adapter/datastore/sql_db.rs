use std::boxed::Box;
use std::result::Result as DefaultResult;
use std::sync::Arc;

use serde::Deserialize;
use std::time::Duration;

use chrono::Local;
use deadpool::managed::{
    Manager, Metrics, Object, Pool, PoolConfig, QueueMode, RecycleError, Timeouts,
};
use deadpool::Runtime;
use sqlx::error::Error as SqlxError;
use sqlx::mysql::{MySqlConnectOptions, MySqlConnection};
use sqlx::{ConnectOptions, Connection}; //traits for generic connection methods

use ecommerce_common::confidentiality::AbstractConfidentiality;
use ecommerce_common::config::{AppDbServerCfg, AppDbServerType};
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::logging::{app_log_event, AppLogContext, AppLogLevel};

use crate::error::AppError;

#[allow(non_snake_case)]
#[derive(Deserialize)]
struct DbSecret {
    HOST: String,
    PORT: u16,
    USER: String,
    PASSWORD: String,
}

struct MariaDbManager {
    conn_opts: MySqlConnectOptions,
    logctx: Arc<AppLogContext>,
    idle_timeout: Duration,
}

pub struct AppMariaDbStore {
    pub alias: String,
    pool: Pool<MariaDbManager>,
    logctx: Arc<AppLogContext>,
}

impl Manager for MariaDbManager {
    type Type = MySqlConnection;
    type Error = SqlxError;

    async fn create(&self) -> DefaultResult<Self::Type, Self::Error> {
        self.conn_opts.connect().await
    }
    async fn recycle(
        &self,
        obj: &mut Self::Type,
        metrics: &Metrics,
    ) -> DefaultResult<(), RecycleError<SqlxError>> {
        let last_time_used = metrics.last_used();
        if last_time_used > self.idle_timeout {
            let msg = std::borrow::Cow::Owned("idle-timed-out".to_string());
            return Err(RecycleError::Message(msg));
        }
        let t0 = Local::now().fixed_offset();
        let result = obj.ping().await;
        let t1 = Local::now().fixed_offset();
        let lctx = self.logctx.as_ref();
        if let Err(e) = &result {
            let td = t1 - t0;
            app_log_event!(lctx, AppLogLevel::WARNING, "{:?}, td: {:?}", e, td);
        }
        result.map_err(RecycleError::Backend)
    }
}

impl std::fmt::Debug for MariaDbManager {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Ok(())
    }
}

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

        let mgr = MariaDbManager {
            conn_opts,
            idle_timeout: Duration::new(cfg.idle_timeout_secs as u64, 0),
            logctx: logctx.clone(),
        };
        let timeouts = Timeouts {
            wait: Some(Duration::new(cfg.acquire_timeout_secs as u64, 0)), // wait for internal slots available
            create: Some(Duration::new(cfg.acquire_timeout_secs as u64, 0)),
            recycle: Some(Duration::new(5u64, 0)),
        };
        let queue_mode = QueueMode::Fifo;
        let poolcfg = PoolConfig {
            timeouts,
            queue_mode,
            max_size: cfg.max_conns as usize,
        };
        let pool = Pool::builder(mgr)
            .config(poolcfg)
            .runtime(Runtime::Tokio1)
            .build()
            .map_err(|e| AppError {
                code: AppErrorCode::MissingDataStore,
                detail: Some(e.to_string()),
            })?;
        Ok(Self {
            pool,
            logctx,
            alias: cfg.alias.clone(),
        })
    } // end of fn try-build

    pub async fn acquire(
        &self,
    ) -> DefaultResult<Object<impl Manager<Type = MySqlConnection, Error = SqlxError>>, AppError>
    {
        // Note
        // due to unknown timeout issue in `sqlx` pool,  as discussed in the github repo,
        // https://github.com/launchbadge/sqlx/discussions/3232 ,
        // this application switches to `deadpool` for connection management
        let result = self.pool.get().await;
        let lctx = &self.logctx;
        let status = &self.pool.status();
        app_log_event!(lctx, AppLogLevel::DEBUG, "pool:{:?}", status,);
        result.map_err(|e| {
            app_log_event!(lctx, AppLogLevel::ERROR, "pool:{:?}, e:{:?}", status, e);
            AppError {
                code: AppErrorCode::DatabaseServerBusy,
                detail: Some(e.to_string()),
            }
        })
    }
} // end of impl AppMariaDbStore
