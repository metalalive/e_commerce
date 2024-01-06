use std::fmt::{Display, Debug};

#[derive(Debug, Clone, PartialEq)]
pub enum AppErrorCode {
    Unknown,
    NotImplemented,
    MissingSysBasePath,
    MissingAppBasePath,
    MissingSecretPath,
    MissingConfigPath,
    MissingDataStore,
    InvalidJsonFormat,
    InvalidVersion,
    InvalidRouteConfig,
    MissingAliasLogHdlerCfg,
    MissingAliasLoggerCfg,
    NoRouteApiServerCfg,
    NoLogHandlerCfg,
    NoLoggerCfg,
    FeatureDisabled,
    NoHandlerInLoggerCfg,
    InvalidHandlerLoggerCfg,
    EmptyInputData, // for internal server error, do NOT dump detail to http response
    InvalidInput, // for frontend client error
    CryptoFailure,
    RpcRemoteUnavail,
    RpcPublishFailure,
    RpcConsumeFailure,
    RpcRemoteInvalidReply,
    NoConfidentialityCfg,
    NoDatabaseCfg,
    RemoteDbServerFailure,
    ExceedingMaxLimit,
    AcquireLockFailure,
    DataTableNotExist,
    DataCorruption,
    ProductNotExist,
    IOerror(std::io::ErrorKind),
} // end of AppErrorCode

#[derive(Debug, Clone)]
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

