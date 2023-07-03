use std::fmt::{Display, Debug};

#[derive(Debug)]
pub enum AppErrorCode {
    Unknown,
    MissingSysBasePath,
    MissingAppBasePath,
    MissingSecretPath,
    MissingConfigPath,
    InvalidJsonFormat,
    InvalidVersion,
    NoRouteApiServerCfg,
    NoLogHandlerCfg,
    NoLoggerCfg,
    NoHandlerInLoggerCfg,
    InvalidHandlerLoggerCfg,
    IOerror(std::io::ErrorKind),
} // end of AppErrorCode

pub struct AppError {
    pub code: AppErrorCode,
    pub detail: Option<String>
}

impl Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result
    {
        let default_detail = "none";
        let dp = if let Some(s) = &self.detail {
            s.as_str()
        } else {
            default_detail
        };
        write!(f, "code:{:?}, detail:{}", self.code, dp)
    }
}
