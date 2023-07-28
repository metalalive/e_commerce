use std::result::Result as DefaultResult;
use std::fs::File;
use std::io::BufReader;
use std::collections::{HashSet, HashMap};
use std::collections::hash_map::RandomState;
use std::string::ToString;

use serde::Deserialize;
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
pub struct ApiServerRouteCfg {
    pub path: WebApiPath,
    pub handler: String
}

impl ToString for ApiServerRouteCfg {
    fn to_string(&self) -> String {
        format!("path:{}, handler:{}", self.path, self.handler)
    }
}

#[derive(Deserialize)]
pub struct ApiServerListenCfg {
    pub api_version: String,
    pub host: String,
    pub port: u16,
    max_failures: u8,
    pub routes: Vec<ApiServerRouteCfg>,
}

#[allow(non_camel_case_types)]
#[derive(Deserialize, Clone, PartialEq)]
pub enum AppRpcTypeCfg {dummy, AMQP}

#[derive(Deserialize)]
pub struct AppRpcCfg {
    pub handler_type: AppRpcTypeCfg
}

#[derive(Deserialize)]
pub struct ApiServerCfg {
    pid_file: PIDfileCfg,
    pub logging: AppLoggingCfg,
    access_log: AccessLogCfg,
    pub listen: ApiServerListenCfg,
    max_connections: u32,
    limit_req_body_in_bytes: u32,
    pub num_workers: u8,
    pub stack_sz_kb: u16,
    pub rpc: AppRpcCfg
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
        match args.remove(AppConst::ENV_VAR_SECRET_FILE_PATH) {
            Some(_secret_path) => {
                let _fullpath = sys_basepath.clone() + &_secret_path; 
            }, // TODO, parse necessary data
            None => {
                return Err(AppError{ detail:None,
                    code:AppErrorCode::MissingSecretPath });
            },
        }
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
                        Self::_check_srv_listener(&jsnobj.listen) ? ;
                        Self::_check_logging(&jsnobj.logging) ? ;
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

    fn _check_srv_listener(obj:&ApiServerListenCfg) -> DefaultResult<(), AppError>
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
        } else if version.len() == 0 {
            let err_msg = Some("empty string".to_string());
            Err(AppError{ detail:err_msg, code:AppErrorCode::InvalidVersion }) 
        } else if let Some(_) = iter.next() {
            let err_msg = Some("version must be numeric".to_string());
            Err(AppError{ detail:err_msg, code:AppErrorCode::InvalidVersion }) 
        } else if let Some(badroute) = iter2.next() {
            let err_msg = Some(badroute.to_string());
            Err(AppError{ detail:err_msg, code:AppErrorCode::InvalidRouteConfig }) 
        } else { Ok(()) }
    } // end of _check_srv_listener
    
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
} // end of impl AppConfig

