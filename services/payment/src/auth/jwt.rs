use std::boxed::Box;
use std::fmt::Display;
use std::future::Future;
use std::pin::Pin;
use std::result::Result;
use std::sync::Arc;

use actix_http::body::BoxBody;
use actix_http::{HttpMessage, Payload, StatusCode};
use actix_web::dev::ServiceRequest;
use actix_web::error::{Error as ActixError, ResponseError};
use actix_web::{FromRequest, HttpRequest, HttpResponse};
use actix_web_httpauth::extractors::bearer::BearerAuth;
use jsonwebtoken::errors::{Error as JwtError, ErrorKind as JwtErrorKind};
use jsonwebtoken::{decode as jwt_decode, decode_header, DecodingKey, Validation as JwtValidation};
use serde::{Deserialize, Serialize};

use ecommerce_common::auth::{jsn_validate_ap_code, quota_matcode_deserialize_error};

use super::keystore::{AbstractAuthKeystore, AuthKeystoreError};
use crate::app_meta;

#[allow(non_camel_case_types)]
type KEYSTORE_TYPE = Arc<Box<dyn AbstractAuthKeystore<Error = AuthKeystoreError>>>;

#[derive(Debug, Clone)]
pub enum AuthJwtError {
    MissingKeystore,
    MissingKeyId,
    MissingAuthedClaim,
    KeystoreUnknown,
    VerifyFailure(JwtErrorKind),
}

#[allow(non_camel_case_types)]
#[derive(Deserialize, Serialize)]
pub enum AppAuthPermissionCode {
    can_create_refund_req,
}
#[derive(Clone)]
pub enum AppAuthQuotaMatCode {
    NumSubChargesPerOrder, // TODO, finish implementation
}
#[derive(Deserialize, Serialize)]
pub struct AppAuthClaimPermission {
    #[serde(deserialize_with = "AppAuthedClaim::_jsn_validate_ap_code")]
    pub app_code: u8,
    pub codename: AppAuthPermissionCode,
}
#[derive(Deserialize, Serialize)]
pub struct AppAuthClaimQuota {
    #[serde(deserialize_with = "AppAuthedClaim::_jsn_validate_ap_code")]
    pub app_code: u8,
    pub mat_code: AppAuthQuotaMatCode, // u8,
    pub maxnum: u32,
}
#[derive(Deserialize, Serialize)]
pub struct AppAuthedClaim {
    pub profile: u32,
    pub iat: i64,
    pub exp: i64, // TODO, add timezone
    pub aud: Vec<String>,
    pub perms: Vec<AppAuthClaimPermission>,
    pub quota: Vec<AppAuthClaimQuota>,
}

impl AppAuthedClaim {
    fn _jsn_validate_ap_code<'de, D>(raw: D) -> Result<u8, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        jsn_validate_ap_code(raw, app_meta::RESOURCE_QUOTA_AP_CODE, app_meta::LABAL)
    }
}

impl FromRequest for AppAuthedClaim {
    type Error = ActixError;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;
    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let result = if let Some(v) = req.extensions_mut().remove::<Self>() {
            Ok(v)
        } else {
            Err(AuthJwtError::MissingAuthedClaim.into())
        };
        Box::pin(async move { result })
    }
} // end of impl AppAuthedClaim

impl TryFrom<u8> for AppAuthQuotaMatCode {
    type Error = u8;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::NumSubChargesPerOrder),
            _others => Err(value),
        }
    }
}
impl From<AppAuthQuotaMatCode> for u8 {
    fn from(value: AppAuthQuotaMatCode) -> u8 {
        match value {
            AppAuthQuotaMatCode::NumSubChargesPerOrder => 1,
        }
    }
}
impl<'de> Deserialize<'de> for AppAuthQuotaMatCode {
    fn deserialize<D>(raw: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let val = u8::deserialize(raw)?;
        match Self::try_from(val) {
            Ok(code) => Ok(code),
            Err(val) => Err(quota_matcode_deserialize_error::<D>(val, (1, 1))),
        }
    }
}
impl Serialize for AppAuthQuotaMatCode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let raw = u8::from(self.clone());
        serializer.serialize_u8(raw)
    }
}

impl Display for AuthJwtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}
impl ResponseError for AuthJwtError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::MissingKeystore => StatusCode::NOT_IMPLEMENTED,
            Self::MissingKeyId | Self::MissingAuthedClaim => StatusCode::UNAUTHORIZED,
            Self::VerifyFailure(ekind) => match ekind {
                JwtErrorKind::Json(_d) => StatusCode::BAD_REQUEST,
                JwtErrorKind::MissingRequiredClaim(_d) => StatusCode::UNAUTHORIZED,
                JwtErrorKind::InvalidToken => StatusCode::BAD_REQUEST,
                JwtErrorKind::InvalidAudience
                | JwtErrorKind::InvalidIssuer
                | JwtErrorKind::ExpiredSignature
                | JwtErrorKind::InvalidAlgorithmName => StatusCode::UNAUTHORIZED,
                _others => StatusCode::INTERNAL_SERVER_ERROR,
            },
            Self::KeystoreUnknown => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
    fn error_response(&self) -> HttpResponse<BoxBody> {
        let mut builder = match self {
            Self::MissingKeystore => HttpResponse::NotImplemented(),
            Self::MissingKeyId | Self::MissingAuthedClaim => HttpResponse::Unauthorized(),
            Self::VerifyFailure(ekind) => match ekind {
                JwtErrorKind::Json(_d) => HttpResponse::BadRequest(),
                JwtErrorKind::MissingRequiredClaim(_d) => HttpResponse::Unauthorized(),
                JwtErrorKind::InvalidToken => HttpResponse::BadRequest(),
                JwtErrorKind::InvalidAudience
                | JwtErrorKind::InvalidIssuer
                | JwtErrorKind::ExpiredSignature
                | JwtErrorKind::InvalidAlgorithmName => HttpResponse::Unauthorized(),
                _others => HttpResponse::InternalServerError(),
            },
            Self::KeystoreUnknown => HttpResponse::InternalServerError(),
        };
        builder.finish()
    }
} // end of impl AuthJwtError

impl From<JwtError> for AuthJwtError {
    fn from(value: JwtError) -> Self {
        Self::VerifyFailure(value.into_kind())
    }
}

impl From<AuthKeystoreError> for AuthJwtError {
    fn from(value: AuthKeystoreError) -> Self {
        match value {
            AuthKeystoreError::MissingKey => Self::MissingKeyId,
            _others => Self::KeystoreUnknown,
        }
    }
}

pub async fn validate_jwt(
    req: ServiceRequest,
    credentials: BearerAuth,
) -> Result<ServiceRequest, (ActixError, ServiceRequest)> {
    if let Some(ks) = req.app_data::<KEYSTORE_TYPE>() {
        match _validate_jwt(ks.clone(), credentials.token()).await {
            Ok(claim) => {
                req.extensions_mut().insert(claim);
                Ok(req)
            }
            Err(e) => {
                req.extensions_mut().insert(e.clone());
                Err((e.into(), req))
            }
        }
    } else {
        let e = AuthJwtError::MissingKeystore;
        req.extensions_mut().insert(e.clone());
        Err((e.into(), req))
    }
} // end of fn validate_jwt

async fn _validate_jwt(
    keystore: KEYSTORE_TYPE,
    encoded: &str,
) -> Result<AppAuthedClaim, AuthJwtError> {
    let hdr = decode_header(encoded)?;
    // TODO , optional logging
    // println!("header decoded without key");
    let key_id = if let Some(k) = hdr.kid.as_ref() {
        k.as_str()
    } else {
        return Err(AuthJwtError::MissingKeyId);
    };
    let jwk = keystore.find(key_id).await?;
    let key = DecodingKey::from_jwk(&jwk)?;
    let validator = {
        let aud = [app_meta::LABAL];
        let required_claims = ["profile", "aud", "exp", "iat", "perms", "quota"];
        let mut v = JwtValidation::new(hdr.alg);
        v.set_audience(&aud);
        v.set_required_spec_claims(&required_claims);
        v
    };
    let decoded = jwt_decode::<AppAuthedClaim>(encoded, &key, &validator)?;
    // println!("header decoded with key");
    Ok(decoded.claims)
} // end of fn _validate_jwt
