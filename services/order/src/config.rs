use std::result::Result as DefaultResult;
use std::fs::File;
use std::io::BufReader;
use std::collections::{HashSet, HashMap};
use std::collections::hash_map::RandomState;
use std::string::ToString;
use std::sync::Arc;

use serde::Deserialize;
use serde::de::{Error as DeserializeError, Expected};
use serde_json;

use crate::{WebApiPath, AppLogAlias, constant as AppConst};
use crate::error::{AppErrorCode, AppError};

#[derive(Deserialize)]
pub struct AppLogHandlerCfg {
    pub min_level: AppConst::logging::Level,
    pub destination: AppConst::logging::Destination,
    pub alias: AppLogAlias,
    pub path: Option<String>
}

#[derive(Deserialize)]
pub struct AppLoggerCfg {
    pub alias: AppLogAlias,
    pub handlers: Vec<String>,
    pub level: Option<AppConst::logging::Level>
}

#[derive(Deserialize)]
pub struct AppLoggingCfg {
    pub handlers : Vec<AppLogHandlerCfg>,
    pub loggers : Vec<AppLoggerCfg>
}

#[derive(Deserialize)]
struct PIDfileCfg {
    web_api : String,
    rpc_consumer : String
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
    pub handler: String
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
    max_failures: u8,
    pub max_connections: u32,
    pub cors: String,
    pub routes: Vec<WebApiRouteCfg>,
}

#[allow(non_camel_case_types)]
#[derive(Deserialize)]
#[serde(tag="side")]
pub enum AppAmqpBindingReplyCfg {
    client {
        #[serde(deserialize_with = "jsn_deny_empty_string")]
        queue: String,
        #[serde(deserialize_with = "jsn_deny_empty_string")]
        correlation_id_prefix: String,
        ttl_sec: u16, // TODO, add `max-length`, `durable`
    },
    server {
        #[serde(deserialize_with = "jsn_deny_empty_string")]
        task_handler: String,
        ttl_sec: u16, // TODO, add `max-length`
    },
}

#[derive(Deserialize)]
pub struct AppAmqpBindingCfg {
    #[serde(deserialize_with = "jsn_deny_empty_string")]
    pub queue: String,
    #[serde(deserialize_with = "jsn_deny_empty_string")]
    pub exchange: String,
    #[serde(deserialize_with = "jsn_deny_empty_string")]
    pub routing_key: String,
    pub ensure_declare: bool,
    pub durable: bool, // TODO, add `ttl-secs`, `max-length`
    pub reply: AppAmqpBindingReplyCfg,
}

#[derive(Deserialize)]
pub struct AppAmqpAttriCfg {
    #[serde(deserialize_with = "jsn_deny_empty_string")]
	pub vhost: String,
    pub max_channels: u16,
    pub timeout_secs: u16
}

#[derive(Deserialize)]
pub struct AppRpcAmqpCfg {
	pub bindings: Arc<Vec<AppAmqpBindingCfg>>,
	pub attributes: AppAmqpAttriCfg,
	max_connections: u16,
    #[serde(deserialize_with = "jsn_deny_empty_string")]
	pub confidential_id: String // TODO, rename to `confidential_path`
}

#[allow(non_camel_case_types)]
#[derive(Deserialize)]
#[serde(tag="handler_type")]
pub enum AppRpcCfg {
    dummy,
    AMQP(AppRpcAmqpCfg),
}

#[derive(Deserialize)]
pub struct AppAuthCfg {
	pub keystore_url: String,
    pub update_interval_minutes: u32
} // TODO, certificate for secure connection

#[derive(Deserialize)]
#[serde(tag="source")]
pub enum AppConfidentialCfg {
    UserSpace {
        #[serde(deserialize_with = "jsn_deny_empty_string")]
        sys_path: String,
    } // TODO, support kernel key management utility,
      // or hardware-specific approach e.g. ARM TrustZone
}

#[allow(non_camel_case_types)]
#[derive(Deserialize, Debug, Clone)]
pub enum AppDbServerType {
    MariaDB, PostgreSQL
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
    pub srv_type:AppDbServerType,
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
    pub confidentiality: AppConfidentialCfg
}

pub struct AppBasepathCfg {
    pub system: String,
    pub service: String 
}

pub struct AppConfig {
    pub basepath: AppBasepathCfg,
    pub api_server: ApiServerCfg,
}

impl AppConfig {
    pub fn new(mut args: HashMap<String, String, RandomState>)
        -> DefaultResult<Self, AppError>
    {
        let sys_basepath = if let Some(s) = args.remove(AppConst::ENV_VAR_SYS_BASE_PATH) {
            s + &"/"
        } else {
            return Err(AppError{ detail:None, code:AppErrorCode::MissingSysBasePath });
        };
        let app_basepath = if let Some(a) = args.remove(AppConst::ENV_VAR_SERVICE_BASE_PATH) {
            a + &"/" 
        } else {
            return Err(AppError{ detail:None, code:AppErrorCode::MissingAppBasePath });
        };
        let api_srv_cfg = if let Some(cfg_path) = args.remove(AppConst::ENV_VAR_CONFIG_FILE_PATH) {
            let fullpath = app_basepath.clone() + &cfg_path; 
            Self::parse_from_file(fullpath) ?
        } else {
            return Err(AppError{ detail:None,
                code:AppErrorCode::MissingConfigPath
            });
        };
        Ok(Self{api_server: api_srv_cfg, basepath:AppBasepathCfg{
            system:sys_basepath, service:app_basepath }})
    } // end of new

    pub fn parse_from_file(filepath:String) -> DefaultResult<ApiServerCfg, AppError>
    { // load and parse a config file with given path
        match File::open(filepath) {
            Ok(fileobj) => {
                let reader = BufReader::new(fileobj);
                match serde_json::from_reader::<BufReader<File>, ApiServerCfg>(reader)
                {
                    Ok(jsnobj) => {
                        Self::_check_web_listener(&jsnobj.listen) ? ;
                        Self::_check_rpc(&jsnobj.rpc)? ;
                        Self::_check_logging(&jsnobj.logging) ? ;
                        Self::_check_datastore (&jsnobj.data_store) ? ;
                        Ok(jsnobj)
                    },
                    Err(e) => Err(AppError{ detail:Some(e.to_string()),
                            code:AppErrorCode::InvalidJsonFormat })
                }
            },
            Err(e) => Err(AppError{ detail:Some(e.to_string()),
                    code:AppErrorCode::IOerror(e.kind()) })
        }
    }

    fn _check_web_listener(obj:&WebApiListenCfg) -> DefaultResult<(), AppError>
    {
        let version:Vec<&str> = obj.api_version.split(".").collect();
        let mut iter = version.iter().filter(
            |i| { !i.parse::<u16>().is_ok() }
        );
        let mut iter2 = obj.routes.iter().filter(
            |i| { i.path.is_empty() || i.handler.is_empty() }
        );
        if obj.routes.len() == 0 {
            Err(AppError{ detail:None, code:AppErrorCode::NoRouteApiServerCfg }) 
        } else if let Some(_) = iter.next() {
            let err_msg = Some("version must be numeric".to_string());
            Err(AppError{ detail:err_msg, code:AppErrorCode::InvalidVersion }) 
        } else if let Some(badroute) = iter2.next() {
            let err_msg = Some(badroute.to_string());
            Err(AppError{ detail:err_msg, code:AppErrorCode::InvalidRouteConfig }) 
        } else { Ok(()) }
    } // end of _check_web_listener
    
    fn _check_rpc(obj:&AppRpcCfg) -> DefaultResult<(), AppError>
    {
        match obj {
            AppRpcCfg::dummy => Ok(()),
            AppRpcCfg::AMQP(c) => if c.bindings.is_empty() {
                Err(AppError{ detail:Some("rpc".to_string()),
                    code:AppErrorCode::NoRouteApiServerCfg }) 
            } else { Ok(()) },
        }
    } // end of _check_rpc
    
    fn _check_logging (obj:&AppLoggingCfg) -> DefaultResult<(), AppError>
    {
        let mut filtered = obj.loggers.iter().filter(
            |item| {item.handlers.is_empty()}
        );
        let mut filtered2 = obj.handlers.iter().filter(
            |item| {
                match &item.destination {
                    AppConst::logging::Destination::LOCALFS => item.path.is_none(),
                    _other => false
                }
            }
        ); // for file-type handler, the field `path` has to be provided
        let mut filtered3 = obj.handlers.iter().filter(
            |item| {item.alias.is_empty()}
        );
        let mut filtered4 = obj.loggers.iter().filter(
            |item| {item.alias.is_empty()}
        );
        if obj.handlers.len() == 0 {
            Err(AppError{ detail:None, code:AppErrorCode::NoLogHandlerCfg }) 
        } else if obj.loggers.len() == 0 {
            Err(AppError{ detail:None, code:AppErrorCode::NoLoggerCfg }) 
        } else if let Some(alogger) = filtered.next() {
            let msg = format!("the logger does not have handler: {}", alogger.alias);
            Err(AppError{ detail: Some(msg), code:AppErrorCode::NoHandlerInLoggerCfg }) 
        } else if let Some(_hdlr) = filtered3.next() {
            Err(AppError{ detail: None, code:AppErrorCode::MissingAliasLogHdlerCfg }) 
        } else if let Some(_logger) = filtered4.next() {
            Err(AppError{ detail: None, code:AppErrorCode::MissingAliasLoggerCfg }) 
        } else if let Some(alogger) = filtered2.next() {
            let msg = format!("file-type handler does not contain path: {}", alogger.alias);
            Err(AppError{ detail: Some(msg), code:AppErrorCode::InvalidHandlerLoggerCfg }) 
        } else {
            let iter = obj.handlers.iter().map(|i| { i.alias.as_str() });
            let hdlr_alias_map:HashSet<&str> = HashSet::from_iter(iter);
            let mut filtered = obj.loggers.iter().filter(
                |item| {
                    let mut inner_iter = item.handlers.iter().filter(
                        |i| {! hdlr_alias_map.contains(i.as_str())}
                    ); // dump invalid handler alias
                    inner_iter.next().is_some()
                }
            ); // handler alias in each logger has to be present
            if let Some(alogger) = filtered.next() {
                let msg = format!("the logger contains invalid handler alias: {}", alogger.alias);
                Err(AppError{ detail: Some(msg), code:AppErrorCode::InvalidHandlerLoggerCfg }) 
            } else {
                Ok(())
            }
        }
    } // end of _check_logging
    
    fn _check_datastore (obj:&Vec<AppDataStoreCfg>) -> DefaultResult<(), AppError>
    {
        if obj.is_empty() {
            return Err(AppError{ detail: None, code:AppErrorCode::NoDatabaseCfg }) 
        }
        for item in obj {
            match item {
                AppDataStoreCfg::InMemory(c) => {
                    let lmt = AppConst::limit::MAX_ITEMS_STORED_PER_MODEL;
                    if c.max_items > lmt {
                        let e = AppError{ detail:Some(format!("limit:{}", lmt)),
                                code:AppErrorCode::ExceedingMaxLimit };
                        return Err(e)
                    }
                },
                AppDataStoreCfg::DbServer(c) => {
                    let lmt_conn = AppConst::limit::MAX_DB_CONNECTIONS;
                    let lmt_idle = AppConst::limit::MAX_SECONDS_DB_IDLE;
                    if c.max_conns > lmt_conn {
                        let e = AppError{ detail:Some(format!("limit-conn:{}", lmt_conn)),
                                code:AppErrorCode::ExceedingMaxLimit };
                        return Err(e)
                    } else if c.idle_timeout_secs > lmt_idle {
                        let e = AppError{ detail:Some(format!("limit-idle-time:{}", lmt_idle)),
                                code:AppErrorCode::ExceedingMaxLimit };
                        return Err(e)
                    }
                },
            }
        } // end of loop
        Ok(())
    } // end of _check_datastore
} // end of impl AppConfig

struct ExpectNonEmptyString {
    min_len: u32
}

impl Expected for ExpectNonEmptyString
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        let msg = format!("minimum string length >= {}", self.min_len);
        formatter.write_str(msg.as_str())
    }
}

fn jsn_deny_empty_string<'de, D>(raw:D) -> Result<String, D::Error>
    where D: serde::Deserializer<'de>
{
    match String::deserialize(raw) {
        Ok(s) => {
            if s.is_empty() {
                let unexp = s.len();
                let exp = ExpectNonEmptyString{min_len:1};
                let e = DeserializeError::invalid_length(unexp, &exp) ;
                Err(e)
            } else {Ok(s)}
        },
        Err(e) => Err(e),
    }
}

