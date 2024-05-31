use ecommerce_common::error::{
    AppCfgError, AppConfidentialityError, AppErrorCode, ProductTypeParseError,
};
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

impl From<ProductTypeParseError> for AppError {
    fn from(value: ProductTypeParseError) -> Self {
        let detail = format!("product-type, error:{}", value.0);
        AppError {
            code: AppErrorCode::DataCorruption,
            detail: Some(detail),
        }
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
