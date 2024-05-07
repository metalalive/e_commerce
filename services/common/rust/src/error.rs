use std::fmt::Debug;
use std::num::ParseIntError;

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
    ProductNotExist,
    IOerror(std::io::ErrorKind),
} // end of AppErrorCode

#[derive(Debug)]
pub struct ProductTypeParseError(pub ParseIntError);

