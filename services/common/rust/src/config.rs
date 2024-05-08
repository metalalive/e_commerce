use std::collections::hash_map::RandomState;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::BufReader;
use std::result::Result as DefaultResult;
use std::string::ToString;
use std::sync::Arc;

use serde::de::{Error as DeserializeError, Expected};
use serde::Deserialize;

use crate::constant::{env_vars, logging as const_log};
use crate::error::{AppCfgError, AppErrorCode};
use crate::{AppLogAlias, WebApiPath};

#[derive(Deserialize)]
pub struct AppLogHandlerCfg {
    pub min_level: const_log::Level,
    pub destination: const_log::Destination,
    pub alias: AppLogAlias,
    pub path: Option<String>,
}

#[derive(Deserialize)]
pub struct AppLoggerCfg {
    pub alias: AppLogAlias,
    pub handlers: Vec<String>,
    pub level: Option<const_log::Level>,
}

#[derive(Deserialize)]
pub struct AppLoggingCfg {
    pub handlers: Vec<AppLogHandlerCfg>,
    pub loggers: Vec<AppLoggerCfg>,
}

#[derive(Deserialize)]
struct PIDfileCfg {
    web_api: String,
    rpc_consumer: String,
}

#[derive(Deserialize)]
struct AccessLogCfg {
    path: String,
    format: String,
}

#[derive(Deserialize)]
pub struct WebApiRouteCfg {
    pub path: WebApiPath,
    #[serde(deserialize_with = "jsn_deny_empty_string")]
    pub handler: String,
}

impl ToString for WebApiRouteCfg {
    fn to_string(&self) -> String {
        format!("path:{}, handler:{}", self.path, self.handler)
    }
}

#[derive(Deserialize)]
pub struct WebApiListenCfg {
    #[serde(deserialize_with = "jsn_deny_empty_string")]
    pub api_version: String,
    #[serde(deserialize_with = "jsn_deny_empty_string")]
    pub host: String,
    pub port: u16,
    pub max_connections: u32,
    pub cors: String,
    pub routes: Vec<WebApiRouteCfg>,
}

#[derive(Deserialize)]
pub struct AppAmqpBindingReplyCfg {
    #[serde(deserialize_with = "jsn_deny_empty_string")]
    pub queue: String,
    #[serde(deserialize_with = "jsn_deny_empty_string")]
    pub correlation_id_prefix: String,
    pub ttl_secs: u16,
    pub max_length: u32, // max number of messages preserved in the queue
    pub durable: bool,
}

#[derive(Deserialize)]
pub struct AppAmqpBindingCfg {
    #[serde(deserialize_with = "jsn_deny_empty_string")]
    pub queue: String,
    #[serde(deserialize_with = "jsn_deny_empty_string")]
    pub exchange: String,
    #[serde(deserialize_with = "jsn_deny_empty_string")]
    pub routing_key: String,
    pub ttl_secs: u16,
    pub max_length: u32, // max number of messages preserved in the queue
    pub durable: bool,
    pub ensure_declare: bool,
    pub subscribe: bool,
    pub reply: Option<AppAmqpBindingReplyCfg>,
    pub python_celery_task: Option<String>,
}

#[derive(Deserialize)]
pub struct AppAmqpAttriCfg {
    #[serde(deserialize_with = "jsn_deny_empty_string")]
    pub vhost: String,
    pub max_channels: u16,
    pub timeout_secs: u16,
}

#[derive(Deserialize)]
pub struct AppRpcAmqpCfg {
    pub bindings: Arc<Vec<AppAmqpBindingCfg>>,
    pub attributes: AppAmqpAttriCfg,
    // max_connections: u16, // TODO, apply connection pool
    #[serde(deserialize_with = "jsn_deny_empty_string")]
    pub confidential_id: String, // TODO, rename to `confidential_path`
}

#[allow(non_camel_case_types)]
#[derive(Deserialize)]
#[serde(tag = "handler_type")]
pub enum AppRpcCfg {
    dummy,
    AMQP(AppRpcAmqpCfg),
}

#[derive(Deserialize)]
pub struct AppAuthCfg {
    pub keystore_url: String,
    pub update_interval_minutes: u32,
} // TODO, certificate for secure connection

#[derive(Deserialize)]
#[serde(tag = "source")]
pub enum AppConfidentialCfg {
    UserSpace {
        #[serde(deserialize_with = "jsn_deny_empty_string")]
        sys_path: String,
    }, // TODO, support kernel key management utility,
       // or hardware-specific approach e.g. ARM TrustZone
}

#[allow(non_camel_case_types)]
#[derive(Deserialize, Debug, Clone)]
pub enum AppDbServerType {
    MariaDB,
    PostgreSQL,
}

#[derive(Deserialize, Debug)]
pub struct AppInMemoryDbCfg {
    #[serde(deserialize_with = "jsn_deny_empty_string")]
    pub alias: String,
    pub max_items: u32,
}

#[derive(Deserialize, Debug)]
pub struct AppDbServerCfg {
    #[serde(deserialize_with = "jsn_deny_empty_string")]
    pub alias: String,
    pub srv_type: AppDbServerType,
    pub max_conns: u32,
    pub acquire_timeout_secs: u16, // for acquiring connection from pool
    pub idle_timeout_secs: u16,
    pub confidentiality_path: String,
    pub db_name: String,
}

#[allow(non_camel_case_types)]
#[derive(Deserialize)]
#[serde(tag = "_type")]
pub enum AppDataStoreCfg {
    InMemory(AppInMemoryDbCfg),
    DbServer(AppDbServerCfg),
}

#[derive(Deserialize)]
pub struct ApiServerCfg {
    pid_file: PIDfileCfg,
    pub logging: AppLoggingCfg,
    access_log: AccessLogCfg,
    pub listen: WebApiListenCfg,
    pub limit_req_body_in_bytes: usize,
    pub num_workers: u8,
    pub stack_sz_kb: u16,
    pub data_store: Vec<AppDataStoreCfg>,
    pub rpc: AppRpcCfg,
    pub auth: AppAuthCfg,
    pub confidentiality: AppConfidentialCfg,
}

pub struct AppBasepathCfg {
    pub system: String,
    pub service: String,
}

pub struct AppConfig {
    pub basepath: AppBasepathCfg,
    pub api_server: ApiServerCfg,
}

pub struct AppCfgHardLimit {
    pub nitems_per_inmem_table: u32,
    pub num_db_conns: u32,
    pub seconds_db_idle: u16,
}
pub struct AppCfgInitArgs {
    pub env_var_map: HashMap<String, String, RandomState>,
    pub limit: AppCfgHardLimit,
}

impl AppConfig {
    pub fn new(args: AppCfgInitArgs) -> DefaultResult<Self, AppCfgError> {
        let (mut env_var_map, limit) = (args.env_var_map, args.limit);
        let sys_basepath = if let Some(s) = env_var_map.remove(env_vars::SYS_BASEPATH) {
            s + "/"
        } else {
            return Err(AppCfgError {
                detail: None,
                code: AppErrorCode::MissingSysBasePath,
            });
        };
        let app_basepath = if let Some(a) = env_var_map.remove(env_vars::SERVICE_BASEPATH) {
            a + "/"
        } else {
            return Err(AppCfgError {
                detail: None,
                code: AppErrorCode::MissingAppBasePath,
            });
        };
        let api_srv_cfg = if let Some(cfg_path) = env_var_map.remove(env_vars::CFG_FILEPATH) {
            let fullpath = app_basepath.clone() + &cfg_path;
            Self::parse_from_file(fullpath, limit)?
        } else {
            return Err(AppCfgError {
                detail: None,
                code: AppErrorCode::MissingConfigPath,
            });
        };
        Ok(Self {
            api_server: api_srv_cfg,
            basepath: AppBasepathCfg {
                system: sys_basepath,
                service: app_basepath,
            },
        })
    } // end of new

    pub fn parse_from_file(
        filepath: String,
        limit: AppCfgHardLimit,
    ) -> DefaultResult<ApiServerCfg, AppCfgError> {
        // load and parse a config file with given path
        match File::open(filepath) {
            Ok(fileobj) => {
                let reader = BufReader::new(fileobj);
                match serde_json::from_reader::<BufReader<File>, ApiServerCfg>(reader) {
                    Ok(jsnobj) => {
                        Self::_check_web_listener(&jsnobj.listen)?;
                        Self::_check_rpc(&jsnobj.rpc)?;
                        Self::_check_logging(&jsnobj.logging)?;
                        Self::_check_datastore(&jsnobj.data_store, limit)?;
                        Ok(jsnobj)
                    }
                    Err(e) => Err(AppCfgError {
                        detail: Some(e.to_string()),
                        code: AppErrorCode::InvalidJsonFormat,
                    }),
                }
            }
            Err(e) => Err(AppCfgError {
                detail: Some(e.to_string()),
                code: AppErrorCode::IOerror(e.kind()),
            }),
        }
    }

    fn _check_web_listener(obj: &WebApiListenCfg) -> DefaultResult<(), AppCfgError> {
        let version: Vec<&str> = obj.api_version.split('.').collect();
        let mut iter = version.iter().filter(|i| i.parse::<u16>().is_err());
        let mut iter2 = obj
            .routes
            .iter()
            .filter(|i| i.path.is_empty() || i.handler.is_empty());
        if obj.routes.is_empty() {
            Err(AppCfgError {
                detail: None,
                code: AppErrorCode::NoRouteApiServerCfg,
            })
        } else if iter.next().is_some() {
            let err_msg = Some("version must be numeric".to_string());
            Err(AppCfgError {
                detail: err_msg,
                code: AppErrorCode::InvalidVersion,
            })
        } else if let Some(badroute) = iter2.next() {
            let err_msg = Some(badroute.to_string());
            Err(AppCfgError {
                detail: err_msg,
                code: AppErrorCode::InvalidRouteConfig,
            })
        } else {
            Ok(())
        }
    } // end of _check_web_listener

    fn _check_rpc(obj: &AppRpcCfg) -> DefaultResult<(), AppCfgError> {
        match obj {
            AppRpcCfg::dummy => Ok(()),
            AppRpcCfg::AMQP(c) => {
                if c.bindings.is_empty() {
                    Err(AppCfgError {
                        detail: Some("rpc".to_string()),
                        code: AppErrorCode::NoRouteApiServerCfg,
                    })
                } else {
                    Ok(())
                }
            }
        }
    } // end of _check_rpc

    fn _check_logging(obj: &AppLoggingCfg) -> DefaultResult<(), AppCfgError> {
        let mut filtered = obj.loggers.iter().filter(|item| item.handlers.is_empty());
        let mut filtered2 = obj.handlers.iter().filter(|item| match &item.destination {
            const_log::Destination::LOCALFS => item.path.is_none(),
            _other => false,
        }); // for file-type handler, the field `path` has to be provided
        let mut filtered3 = obj.handlers.iter().filter(|item| item.alias.is_empty());
        let mut filtered4 = obj.loggers.iter().filter(|item| item.alias.is_empty());
        if obj.handlers.is_empty() {
            Err(AppCfgError {
                detail: None,
                code: AppErrorCode::NoLogHandlerCfg,
            })
        } else if obj.loggers.is_empty() {
            Err(AppCfgError {
                detail: None,
                code: AppErrorCode::NoLoggerCfg,
            })
        } else if let Some(alogger) = filtered.next() {
            let msg = format!("the logger does not have handler: {}", alogger.alias);
            Err(AppCfgError {
                detail: Some(msg),
                code: AppErrorCode::NoHandlerInLoggerCfg,
            })
        } else if let Some(_hdlr) = filtered3.next() {
            Err(AppCfgError {
                detail: None,
                code: AppErrorCode::MissingAliasLogHdlerCfg,
            })
        } else if let Some(_logger) = filtered4.next() {
            Err(AppCfgError {
                detail: None,
                code: AppErrorCode::MissingAliasLoggerCfg,
            })
        } else if let Some(alogger) = filtered2.next() {
            let msg = format!("file-type handler does not contain path: {}", alogger.alias);
            Err(AppCfgError {
                detail: Some(msg),
                code: AppErrorCode::InvalidHandlerLoggerCfg,
            })
        } else {
            let iter = obj.handlers.iter().map(|i| i.alias.as_str());
            let hdlr_alias_map: HashSet<&str> = HashSet::from_iter(iter);
            let mut filtered = obj.loggers.iter().filter(|item| {
                let mut inner_iter = item
                    .handlers
                    .iter()
                    .filter(|i| !hdlr_alias_map.contains(i.as_str())); // dump invalid handler alias
                inner_iter.next().is_some()
            }); // handler alias in each logger has to be present
            if let Some(alogger) = filtered.next() {
                let msg = format!(
                    "the logger contains invalid handler alias: {}",
                    alogger.alias
                );
                Err(AppCfgError {
                    detail: Some(msg),
                    code: AppErrorCode::InvalidHandlerLoggerCfg,
                })
            } else {
                Ok(())
            }
        }
    } // end of _check_logging

    fn _check_datastore(
        obj: &Vec<AppDataStoreCfg>,
        limit: AppCfgHardLimit,
    ) -> DefaultResult<(), AppCfgError> {
        if obj.is_empty() {
            return Err(AppCfgError {
                detail: None,
                code: AppErrorCode::NoDatabaseCfg,
            });
        }
        for item in obj {
            match item {
                AppDataStoreCfg::InMemory(c) => {
                    let lmt = limit.nitems_per_inmem_table;
                    if c.max_items > lmt {
                        let e = AppCfgError {
                            detail: Some(format!("limit:{}", lmt)),
                            code: AppErrorCode::ExceedingMaxLimit,
                        };
                        return Err(e);
                    }
                }
                AppDataStoreCfg::DbServer(c) => {
                    let lmt_conn = limit.num_db_conns;
                    let lmt_idle = limit.seconds_db_idle;
                    if c.max_conns > lmt_conn {
                        let e = AppCfgError {
                            detail: Some(format!("limit-conn:{}", lmt_conn)),
                            code: AppErrorCode::ExceedingMaxLimit,
                        };
                        return Err(e);
                    } else if c.idle_timeout_secs > lmt_idle {
                        let e = AppCfgError {
                            detail: Some(format!("limit-idle-time:{}", lmt_idle)),
                            code: AppErrorCode::ExceedingMaxLimit,
                        };
                        return Err(e);
                    }
                }
            }
        } // end of loop
        Ok(())
    } // end of _check_datastore
} // end of impl AppConfig

struct ExpectNonEmptyString {
    min_len: u32,
}

impl Expected for ExpectNonEmptyString {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        let msg = format!("minimum string length >= {}", self.min_len);
        formatter.write_str(msg.as_str())
    }
}

fn jsn_deny_empty_string<'de, D>(raw: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match String::deserialize(raw) {
        Ok(s) => {
            if s.is_empty() {
                let unexp = s.len();
                let exp = ExpectNonEmptyString { min_len: 1 };
                let e = DeserializeError::invalid_length(unexp, &exp);
                Err(e)
            } else {
                Ok(s)
            }
        }
        Err(e) => Err(e),
    }
}
