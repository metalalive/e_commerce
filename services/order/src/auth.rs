use std::borrow::{Borrow, BorrowMut};
use std::boxed::Box;
use std::collections::hash_map::RandomState;
use std::collections::HashSet;
use std::future::Future;
use std::io::ErrorKind;
use std::pin::Pin;
use std::result::Result as DefaultResult;
use std::sync::Arc;

use async_trait::async_trait;
use axum::http::request::Parts;
use chrono::{DateTime, Duration, FixedOffset, Local as LocalTime};
use serde::{Deserialize, Serialize};

use axum::body::Body as AxumBody;
use axum::extract::FromRequestParts;
use axum::RequestPartsExt;
use axum_extra::TypedHeader;
use headers::authorization::Bearer;
use headers::Authorization;
use http_body_util::{BodyExt, Empty};
use hyper::body::{Bytes as HyBodyBytes, Incoming as HyIncoming};
use hyper::client::conn::http1::{handshake, Connection as HyClientConn, SendRequest};
use hyper::{header, Request, Response, StatusCode, Uri};
use hyper_util::rt::TokioIo;
use tokio::net::TcpStream;
use tokio::sync::RwLock;
use tokio::task;
use tower_http::auth::AsyncAuthorizeRequest;

use jsonwebtoken::jwk::{Jwk, JwkSet};
use jsonwebtoken::{
    decode as jwt_decode, decode_header as jwt_decode_header, errors as JwtErrors, DecodingKey,
    Validation as JwtValidation,
};

use ecommerce_common::auth::{jsn_validate_ap_code, quota_matcode_deserialize_error};
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::logging::{app_log_event, AppLogContext, AppLogLevel};

use crate::constant::{app_meta, HTTP_CONTENT_TYPE_JSON};
use crate::error::AppError;
use crate::{AppAuthCfg, AppSharedState};

const MAX_NBYTES_LOADED_RESPONSE_KEYSTORE: usize = 102400;

type UnauthRespBody = AxumBody;

#[async_trait]
pub trait AbstractAuthKeystore: Send + Sync {
    fn new(cfg: &AppAuthCfg) -> Self
    where
        Self: Sized;

    fn update_period(&self) -> Duration;

    async fn refresh(&self) -> DefaultResult<AppKeystoreRefreshResult, AppError>;

    async fn find(&self, kid: &str) -> DefaultResult<Jwk, AppError>;
}

pub struct AppAuthKeystore {
    update_period: Duration,
    inner: RwLock<InnerKeystoreContext>,
}
struct InnerKeystoreContext {
    keyset: JwkSet,
    keystore_url: Uri,
    last_update: DateTime<FixedOffset>,
}
pub struct AppKeystoreRefreshResult {
    // number of minutes to next refresh operation
    pub period_next_op: Duration,
    pub num_discarded: usize,
    pub num_added: usize,
}

pub struct AppJwtAuthentication {
    logctx: Option<Arc<AppLogContext>>,
    keystore: Arc<Box<dyn AbstractAuthKeystore>>,
}

#[allow(non_camel_case_types)]
#[derive(Deserialize, Serialize, Clone, PartialEq)]
pub enum AppAuthPermissionCode {
    can_create_return_req,
    can_create_product_policy,
}

#[derive(Clone, PartialEq)]
pub enum AppAuthQuotaMatCode {
    NumPhones,
    NumEmails,
    NumOrderLines,
    NumProductPolicies,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct AppAuthClaimPermission {
    #[serde(deserialize_with = "AppAuthedClaim::_jsn_validate_ap_code")]
    pub app_code: u8,
    pub codename: AppAuthPermissionCode,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct AppAuthClaimQuota {
    #[serde(deserialize_with = "AppAuthedClaim::_jsn_validate_ap_code")]
    pub app_code: u8,
    pub mat_code: AppAuthQuotaMatCode, // u8,
    pub maxnum: u32,
}

// TODO, reuse `AppAuthedClaim` and parameterize `AppAuthPermissionCode`
// and `AppAuthQuotaMatCode` for different applications
// (possible approaches are generic type or macro)
#[derive(Deserialize, Serialize, Clone)]
pub struct AppAuthedClaim {
    pub profile: u32,
    pub iat: i64,
    pub exp: i64, // TODO, add timezone
    pub aud: Vec<String>,
    pub perms: Vec<AppAuthClaimPermission>,
    pub quota: Vec<AppAuthClaimQuota>,
}

fn error_response() -> Response<UnauthRespBody> {
    Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .body(AxumBody::empty())
        .unwrap()
}

impl FromRequestParts<AppSharedState> for AppAuthedClaim {
    type Rejection = Response<UnauthRespBody>;

    async fn from_request_parts(
        parts: &mut Parts,
        shr_state: &AppSharedState,
    ) -> DefaultResult<Self, Self::Rejection> {
        if let Some(claim) = parts.extensions.remove::<Self>() {
            Ok(claim)
        } else {
            let logctx = shr_state.log_context().clone();
            app_log_event!(logctx, AppLogLevel::DEBUG, "not authenticated");
            Err(error_response())
        }
    }
} // end of impl AppAuthedClaim

impl AppAuthedClaim {
    fn _jsn_validate_ap_code<'de, D>(raw: D) -> DefaultResult<u8, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        jsn_validate_ap_code(raw, app_meta::RESOURCE_QUOTA_AP_CODE, app_meta::LABAL)
    }
    pub fn contain_permission(&self, code: AppAuthPermissionCode) -> bool {
        self.perms
            .iter()
            .any(|p| p.app_code == app_meta::RESOURCE_QUOTA_AP_CODE && p.codename == code)
    }
    pub fn quota_limit(&self, code: AppAuthQuotaMatCode) -> u32 {
        let result = self
            .quota
            .iter()
            .find(|q| q.app_code == app_meta::RESOURCE_QUOTA_AP_CODE && q.mat_code == code);
        if let Some(rule) = result {
            rule.maxnum
        } else {
            0
        }
    }
} // end of impl AppAuthedClaim

impl TryFrom<u8> for AppAuthQuotaMatCode {
    type Error = u8;
    fn try_from(value: u8) -> DefaultResult<Self, Self::Error> {
        match value {
            1 => Ok(Self::NumPhones),
            2 => Ok(Self::NumEmails),
            3 => Ok(Self::NumOrderLines),
            4 => Ok(Self::NumProductPolicies),
            _others => Err(value),
        }
    }
}
impl From<AppAuthQuotaMatCode> for u8 {
    fn from(value: AppAuthQuotaMatCode) -> u8 {
        match value {
            AppAuthQuotaMatCode::NumPhones => 1,
            AppAuthQuotaMatCode::NumEmails => 2,
            AppAuthQuotaMatCode::NumOrderLines => 3,
            AppAuthQuotaMatCode::NumProductPolicies => 4,
        }
    }
}
impl<'de> Deserialize<'de> for AppAuthQuotaMatCode {
    fn deserialize<D>(raw: D) -> DefaultResult<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let val = u8::deserialize(raw)?;
        match Self::try_from(val) {
            Ok(code) => Ok(code),
            Err(val) => Err(quota_matcode_deserialize_error::<D>(val, (1, 4))),
        }
    }
}
impl Serialize for AppAuthQuotaMatCode {
    fn serialize<S>(&self, serializer: S) -> DefaultResult<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let raw = self.clone().into();
        serializer.serialize_u8(raw)
    }
}

#[async_trait]
impl AbstractAuthKeystore for AppAuthKeystore {
    fn new(cfg: &AppAuthCfg) -> Self {
        let update_period = Duration::minutes(cfg.update_interval_minutes as i64);
        // caller can start refresh operation immediately after initialization
        let last_update = LocalTime::now().fixed_offset() - update_period - Duration::seconds(5);
        let keystore_url = cfg.keystore_url.parse::<Uri>().unwrap();
        let inner = InnerKeystoreContext {
            keyset: JwkSet { keys: vec![] },
            keystore_url,
            last_update,
        };
        Self {
            inner: RwLock::new(inner),
            update_period,
        }
    }
    fn update_period(&self) -> Duration {
        self.update_period // copy implicitly
    }

    async fn refresh(&self) -> DefaultResult<AppKeystoreRefreshResult, AppError> {
        let mut guard = self.inner.write().await;
        let ctx = guard.borrow_mut();
        let expect_time = ctx.last_update + self.update_period;
        let t0 = LocalTime::now().fixed_offset();
        // this ensures there's only one task refreshing the key store
        // in multithreaded application
        if t0 > expect_time {
            let keys = self.request_new_keys(&ctx.keystore_url).await?;
            let (num_discarded, num_added) = Self::merge(&mut ctx.keyset, keys);
            ctx.last_update = t0;
            Ok(AppKeystoreRefreshResult {
                num_discarded,
                num_added,
                period_next_op: self.update_period,
            })
        } else {
            let period_next_op = expect_time - t0;
            assert!(period_next_op.num_seconds() >= 0);
            Ok(AppKeystoreRefreshResult {
                period_next_op,
                num_discarded: 0,
                num_added: 0,
            })
        }
    }

    async fn find(&self, kid: &str) -> DefaultResult<Jwk, AppError> {
        let guard = self.inner.write().await;
        let ctx = guard.borrow();
        match ctx.keyset.find(kid) {
            Some(v) => Ok(v.clone()),
            None => Err(AppError {
                detail: Some("auth-key".to_string()),
                code: AppErrorCode::IOerror(ErrorKind::NotFound),
            }),
        }
    }
} // end of impl AppAuthKeystore

impl AppAuthKeystore {
    pub fn merge(target: &mut JwkSet, new: JwkSet) -> (usize, usize) {
        let get_kid =
            |item: &Jwk| -> Option<String> { item.common.key_id.as_ref().map(|id| id.to_string()) }; // in this application, key ID must be present
        let kids_iter_1 = target.keys.iter().filter_map(get_kid);
        let kids_iter_2 = new.keys.iter().filter_map(get_kid);
        let kidset1: HashSet<String, RandomState> = HashSet::from_iter(kids_iter_1);
        let kidset2 = HashSet::from_iter(kids_iter_2);
        let added = kidset2.difference(&kidset1).collect::<Vec<_>>();
        let discarded = kidset1.difference(&kidset2).collect::<Vec<_>>();
        discarded
            .iter()
            .map(|d_kid| {
                let result = target.keys.iter().position(|item| {
                    let t_kid = item.common.key_id.as_ref().unwrap().as_str();
                    d_kid.as_str() == t_kid
                });
                if let Some(idx) = result {
                    let _item = target.keys.remove(idx);
                }
            })
            .count();
        let new_iter = new.keys.into_iter().filter(|item| {
            if let Some(id) = item.common.key_id.as_ref() {
                added.contains(&id)
            } else {
                false
            }
        });
        target.keys.extend(new_iter);
        (discarded.len(), added.len())
    } // end of fn merge

    async fn request_new_keys(&self, url: &Uri) -> DefaultResult<JwkSet, AppError> {
        // TODO, config parameter for http version
        let (sender, connector) = self.setup_tcp_keyserver(url).await?;
        // make the low-level connection process inbound / outbound messages
        // in a spawned task, optionally return error
        let _handle = task::spawn(connector);
        let resp = self._request_to_key_server(url, sender).await?;
        let keys = self.resp_body_to_keys(resp).await?;
        Ok(keys)
    }

    async fn setup_tcp_keyserver(
        &self,
        url: &Uri,
    ) -> DefaultResult<
        (
            SendRequest<Empty<HyBodyBytes>>,
            HyClientConn<TokioIo<TcpStream>, Empty<HyBodyBytes>>,
        ),
        AppError,
    > {
        let host = url.host().ok_or(AppError {
            code: AppErrorCode::InvalidInput,
            detail: Some("keyserver-uri-missing-host".to_string()),
        })?;
        let port = url
            .port()
            .ok_or(AppError {
                code: AppErrorCode::InvalidInput,
                detail: Some("keyserver-uri-missing-port".to_string()),
            })?
            .as_u16();
        let addr = format!("{host}:{port}");
        match TcpStream::connect(addr).await {
            Ok(stream) => {
                match handshake::<TokioIo<TcpStream>, Empty<HyBodyBytes>>(TokioIo::new(stream))
                    .await
                {
                    Ok(m) => Ok(m),
                    Err(net_e) => Err(AppError {
                        detail: Some(net_e.to_string()),
                        code: err_code_hyper2app(&net_e),
                    }),
                }
            }
            Err(net_e) => Err(AppError {
                detail: Some(net_e.to_string()),
                code: AppErrorCode::IOerror(net_e.kind()),
            }),
        }
    }

    async fn _request_to_key_server(
        &self,
        url: &Uri,
        mut sender: SendRequest<Empty<HyBodyBytes>>,
    ) -> DefaultResult<Response<HyIncoming>, AppError> {
        let hostname = url.host().unwrap();
        let req = Request::builder()
            .uri(url.path())
            .method(hyper::Method::GET)
            .header(header::ACCEPT, HTTP_CONTENT_TYPE_JSON)
            .header(header::HOST, hostname)
            .body(Empty::new())
            .map_err(|net_e| AppError {
                detail: Some(net_e.to_string()),
                code: AppErrorCode::InvalidInput,
            })?;
        let resp = sender.send_request(req).await.map_err(|net_e| AppError {
            detail: Some(net_e.to_string()),
            code: err_code_hyper2app(&net_e),
        })?;
        if resp.status() == StatusCode::OK {
            Ok(resp) // TODO, improve status check
        } else {
            Err(AppError {
                detail: Some(format!(
                    "remote-key-server-response-status:{}",
                    resp.status()
                )),
                code: AppErrorCode::IOerror(ErrorKind::ConnectionRefused),
            })
        }
    }

    async fn resp_body_to_keys(
        &self,
        mut resp: Response<HyIncoming>,
    ) -> DefaultResult<JwkSet, AppError> {
        // TODO, generalize using macro, generic type parameter cause
        let body = resp.body_mut();
        let mut raw_collected: Vec<u8> = Vec::new();
        while let Some(frm) = body.frame().await {
            let result = match frm {
                Ok(raw) => {
                    let rawblk = raw.into_data().unwrap(); // TODO, error handling
                    raw_collected.extend(rawblk.to_vec());
                    let result = serde_json::from_slice::<JwkSet>(raw_collected.as_slice());
                    if let Ok(out) = result {
                        Some(Ok(out))
                    } else if raw_collected.len() > MAX_NBYTES_LOADED_RESPONSE_KEYSTORE {
                        Some(Err(AppError {
                            detail: Some("auth-keys-resp-body".to_string()),
                            code: AppErrorCode::ExceedingMaxLimit,
                        }))
                    } else {
                        None
                    }
                }
                Err(net_e) => Some(Err(AppError {
                    detail: Some(net_e.to_string()),
                    code: err_code_hyper2app(&net_e),
                })),
            };
            if let Some(v) = result {
                return v;
            }
        }
        Err(AppError {
            detail: Some("resp-body-recv-complete".to_string()),
            code: AppErrorCode::DataCorruption,
        })
    } // end of resp_body_to_keys
} // end of fn AppAuthKeystore

fn err_code_hyper2app(value: &hyper::Error) -> AppErrorCode {
    if value.is_closed() {
        AppErrorCode::IOerror(ErrorKind::NotConnected)
    } else if value.is_parse() || value.is_incomplete_message() {
        AppErrorCode::DataCorruption
    } else if value.is_parse_too_large() {
        AppErrorCode::ExceedingMaxLimit
    } else if value.is_user() {
        AppErrorCode::IOerror(ErrorKind::InvalidInput)
    } else if value.is_timeout() {
        AppErrorCode::IOerror(ErrorKind::TimedOut)
    } else if value.is_canceled() {
        AppErrorCode::IOerror(ErrorKind::Interrupted)
    } else {
        AppErrorCode::IOerror(ErrorKind::Other)
    }
}

impl Clone for AppJwtAuthentication {
    fn clone(&self) -> Self {
        Self {
            keystore: self.keystore.clone(),
            logctx: self.logctx.clone(),
        }
    }
}

impl<REQB> AsyncAuthorizeRequest<REQB> for AppJwtAuthentication
where
    REQB: Send + 'static,
{
    // response body type of authentication middleware is coupled to web API endpoints
    // TODO, better design approach
    type RequestBody = REQB;
    type ResponseBody = UnauthRespBody;
    type Future = Pin<
        Box<
            dyn Future<
                    Output = DefaultResult<
                        Request<Self::RequestBody>,
                        Response<Self::ResponseBody>,
                    >,
                > + Send
                + 'static,
        >,
    >;

    fn authorize(&mut self, request: Request<REQB>) -> Self::Future {
        type AuthTokenHdr = TypedHeader<Authorization<Bearer>>;
        let logctx = self.logctx.clone();
        let ks = self.keystore.clone();
        let fut = async move {
            let (mut parts, body) = request.into_parts();
            let mut resp = error_response();
            match parts.extract::<AuthTokenHdr>().await {
                Ok(TypedHeader(Authorization(bearer))) => {
                    match Self::validate_token(ks, bearer.token(), logctx).await {
                        Ok(claim) => {
                            let _ = parts.extensions.insert(claim);
                            Ok(Request::from_parts(parts, body))
                        }
                        Err(e) => {
                            let _ = resp.extensions_mut().insert(e);
                            Err(resp)
                        }
                    }
                }
                Err(e) => {
                    if let Some(lctx) = logctx {
                        app_log_event!(
                            lctx,
                            AppLogLevel::INFO,
                            "failed to extract auth header : {:?}",
                            e
                        );
                    }
                    let ae = AppError {
                        code: AppErrorCode::InvalidInput,
                        detail: Some(e.to_string()),
                    };
                    let _ = resp.extensions_mut().insert(ae);
                    Err(resp)
                }
            }
        };
        Box::pin(fut)
    } // end of fn authorize
} // end of impl  AppJwtAuthentication

impl AppJwtAuthentication {
    pub fn new(ks: Arc<Box<dyn AbstractAuthKeystore>>, logctx: Option<Arc<AppLogContext>>) -> Self {
        Self {
            keystore: ks,
            logctx,
        }
    }

    async fn validate_token(
        ks: Arc<Box<dyn AbstractAuthKeystore>>,
        encoded: &str,
        logctx: Option<Arc<AppLogContext>>,
    ) -> DefaultResult<AppAuthedClaim, AppError> {
        let hdr = match jwt_decode_header(encoded) {
            Ok(v) => v,
            Err(ce) => {
                if let Some(lctx) = logctx.as_ref() {
                    app_log_event!(
                        lctx,
                        AppLogLevel::WARNING,
                        "failed to decode JWT header : {:?}",
                        ce
                    );
                }
                return Err(AppError::from(ce));
            }
        };
        if hdr.kid.is_none() {
            return Err(AppError {
                code: AppErrorCode::InvalidJsonFormat,
                detail: Some("jwt-missing-key-id".to_string()),
            });
        }
        let kid = hdr.kid.as_ref().unwrap();
        let jwk = ks.find(kid.as_str()).await?;
        let key = match DecodingKey::from_jwk(&jwk) {
            Ok(v) => v,
            Err(ce) => {
                if let Some(lctx) = logctx.as_ref() {
                    app_log_event!(lctx, AppLogLevel::ERROR, "Decoding key from jwk : {:?}", ce);
                }
                return Err(AppError::from(ce));
            }
        };
        let validation = {
            let required_claims = ["profile", "aud", "exp", "iat", "perms", "quota"];
            let mut vd = JwtValidation::new(hdr.alg);
            let aud = [app_meta::LABAL];
            vd.set_audience(&aud);
            vd.set_required_spec_claims(&required_claims);
            vd
        };
        match jwt_decode::<AppAuthedClaim>(encoded, &key, &validation) {
            Ok(v) => Ok(v.claims),
            Err(ce) => {
                if let Some(lctx) = logctx.as_ref() {
                    app_log_event!(
                        lctx,
                        AppLogLevel::WARNING,
                        "failed to decode jwt : {:?}",
                        ce
                    );
                }
                Err(AppError::from(ce))
            }
        }
    } // end of fn validate_token
} // end of impl AppJwtAuthentication

impl From<JwtErrors::Error> for AppError {
    fn from(value: JwtErrors::Error) -> Self {
        let (code, detail) = match value.kind() {
            JwtErrors::ErrorKind::Base64(r) => (
                AppErrorCode::DataCorruption,
                r.to_string() + ", encoder:Base64",
            ),
            JwtErrors::ErrorKind::Utf8(r) => (
                AppErrorCode::DataCorruption,
                r.to_string() + ", encoder:UTF-8",
            ),
            JwtErrors::ErrorKind::InvalidToken => {
                (AppErrorCode::DataCorruption, "invalid-token".to_string())
            }
            JwtErrors::ErrorKind::Crypto(r) => (AppErrorCode::CryptoFailure, r.to_string()),
            JwtErrors::ErrorKind::InvalidSignature | JwtErrors::ErrorKind::ImmatureSignature => {
                (AppErrorCode::CryptoFailure, "invalid-signature".to_string())
            }
            JwtErrors::ErrorKind::ExpiredSignature => {
                (AppErrorCode::CryptoFailure, value.to_string())
            }
            JwtErrors::ErrorKind::InvalidRsaKey(r) => (
                AppErrorCode::CryptoFailure,
                r.clone() + ", low-level:invalid-rsa-key",
            ),
            JwtErrors::ErrorKind::InvalidEcdsaKey => {
                (AppErrorCode::CryptoFailure, "ECDSA-key-invalid".to_string())
            }
            JwtErrors::ErrorKind::Json(r) => (AppErrorCode::InvalidJsonFormat, r.to_string()),
            JwtErrors::ErrorKind::RsaFailedSigning => {
                (AppErrorCode::CryptoFailure, "rsa-sign-key".to_string())
            }
            JwtErrors::ErrorKind::InvalidAudience
            | JwtErrors::ErrorKind::InvalidAlgorithm
            | JwtErrors::ErrorKind::InvalidAlgorithmName
            | JwtErrors::ErrorKind::InvalidKeyFormat
            | JwtErrors::ErrorKind::MissingAlgorithm => {
                (AppErrorCode::InvalidInput, value.to_string())
            }
            _others => (AppErrorCode::Unknown, value.to_string()),
        };
        Self {
            code,
            detail: Some(detail),
        }
    } // end of fn from
} // end of impl AppError
