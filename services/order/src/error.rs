use ecommerce_common::error::{AppCfgError, AppConfidentialityError, AppErrorCode};
use std::fmt::{Debug, Display};

#[derive(Debug, Clone)]
pub struct AppError {
    pub code: AppErrorCode,
    pub detail: Option<String>,
}

impl Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let default_detail = "none";
        let dp = if let Some(s) = &self.detail {
            s.as_str()
        } else {
            default_detail
        };
        write!(f, "code:{:?}, detail:{}", self.code, dp)
    }
}

impl From<AppCfgError> for AppError {
    fn from(value: AppCfgError) -> Self {
        AppError {
            code: value.code,
            detail: value.detail,
        }
    }
}
impl From<AppConfidentialityError> for AppError {
    fn from(value: AppConfidentialityError) -> Self {
        AppError {
            code: value.code,
            detail: Some(value.detail),
        }
    }
}
impl From<(AppErrorCode, String)> for AppError {
    fn from(value: (AppErrorCode, String)) -> Self {
        AppError {
            code: value.0,
            detail: Some(value.1),
        }
    }
}
