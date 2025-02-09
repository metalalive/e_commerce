use std::fmt::Debug;

#[derive(Debug, Clone, PartialEq)]
pub enum AppErrorCode {
    Unknown,
    NotImplemented,
    MissingSysBasePath,
    MissingAppBasePath,
    MissingSecretPath,
    MissingConfigPath,
    MissingDataStore,
    MissingConfig,
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
    InvalidInput,   // for frontend client error
    CryptoFailure,
    RpcRemoteUnavail,
    RpcPublishFailure,
    RpcConsumeFailure,
    RpcRemoteInvalidReply,
    RpcReplyNotReady,
    NoConfidentialityCfg,
    NoDatabaseCfg,
    RemoteDbServerFailure,
    ExceedingMaxLimit,
    AcquireLockFailure,
    DatabaseServerBusy,
    DataTableNotExist,
    DataCorruption,
    HttpHandshakeFailure,
    ProductNotExist,
    IOerror(std::io::ErrorKind),
} // end of AppErrorCode

#[derive(Debug, Clone)]
pub struct AppCfgError {
    pub code: AppErrorCode,
    pub detail: Option<String>,
}

#[derive(Debug)]
pub struct AppConfidentialityError {
    pub code: AppErrorCode,
    pub detail: String,
}
