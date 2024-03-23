use std::env;
use std::fs::File;
use std::io::Read;
use std::sync::Arc;
use std::result::Result as DefaultResult;

use async_trait::async_trait;
use axum::extract::rejection::TypedHeaderRejection;
use chrono::{Duration, Local, DateTime, FixedOffset};
use hyper::{Request, Body};
use hyper::header::{HeaderName, HeaderValue};
use jsonwebtoken::{encode as jwt_encode, EncodingKey, Algorithm};
use jsonwebtoken::jwk::{JwkSet, Jwk};
use tower_http::auth::AsyncAuthorizeRequest;

use order::{
    AppAuthKeystore, AppJwtAuthentication, AbstractAuthKeystore, AppAuthCfg,
    AppKeystoreRefreshResult, AppAuthedClaim, AppAuthClaimQuota, AppAuthClaimPermission,
    AppAuthPermissionCode, AppAuthQuotaMatCode
};
use order::error::{AppError, AppErrorCode};
use order::constant::{app_meta, ENV_VAR_SERVICE_BASE_PATH};

use crate::EXAMPLE_REL_PATH;

#[test]
fn keystore_refresh_ok() {
    let rawdata_old_keys = br#"
        {"keys": [
            {"kid": "1b7a039bf4", "alg": "RS256", "kty": "RSA", "use": "sig", "e":"AQAB", "n": "w0EZljVyEeO8wnEQ"},
            {"kid": "00db7af03e", "alg": "RS256", "kty": "RSA", "use": "sig", "e":"QAYB", "n": "0EZljVyEeO8wnEQk"}
        ]}
    "#;
    let rawdata_new_keys = br#"
        {"keys": [
            {"kid":"b110fb3480", "alg": "RS256", "kty": "RSA", "use": "sig", "e":"AQBB", "n": "ko4qOeuhr-ZljVEm"},
            {"kid":"00db7af03e", "alg": "RS256", "kty": "RSA", "use": "sig", "e":"QAYB", "n": "0EZljVyEeO8wnEQk"},
            {"kid":"95667b348d", "alg": "RS256", "kty": "RSA", "use": "sig", "e":"JQAB", "n": "iu3W4otyVJq3huGy"}
        ]}
    "#;
    let (expect_num_discarded, expect_num_added) = (1, 2);
    let mut target = serde_json::from_slice::<JwkSet>(rawdata_old_keys).unwrap();
    let new        = serde_json::from_slice::<JwkSet>(rawdata_new_keys).unwrap();
    let actual = AppAuthKeystore::merge(& mut target, new);
    assert_eq!(actual.0, expect_num_discarded);
    assert_eq!(actual.1, expect_num_added);
    assert!(target.find("95667b348d").is_some());
    assert!(target.find("00db7af03e").is_some());
    assert!(target.find("b110fb3480").is_some());
    assert!(target.find("1b7a039bf4").is_none());
} // end of fn keystore_refresh_ok


struct MockAuthKeystore {
    key: Jwk
}
impl MockAuthKeystore {
    fn build(key_file_name: &str) -> Self
    {
        let value = Self::ut_load_jwk_file(key_file_name);
        let key = serde_json::from_value::<Jwk>(value).unwrap();
        Self { key }
    }
    fn ut_load_jwk_file(filename:&str) -> serde_json::Value
    {
        let basepath = env::var(ENV_VAR_SERVICE_BASE_PATH).unwrap();
        let fullpath = basepath + EXAMPLE_REL_PATH + filename;
        let f = File::open(fullpath).unwrap();
        let result = serde_json::from_reader::<File, serde_json::Value>(f);
        let out = result.unwrap();
        out
    }
}

#[async_trait]
impl AbstractAuthKeystore for MockAuthKeystore {
    fn new(_cfg:&AppAuthCfg) -> Self
    { panic!("not support in unit test"); }
    
    fn update_period(&self) -> Duration
    { Duration::seconds(0) }

    async fn refresh(&self) -> DefaultResult<AppKeystoreRefreshResult, AppError>
    { Err(AppError { code: AppErrorCode::NotImplemented, detail: None }) }
    
    async fn find(&self, _kid:&str) -> DefaultResult<Jwk, AppError>
    { Ok(self.key.clone()) }
}

fn ut_jwt_encode_token(kid:Option<String>, alg:Algorithm,
                       privkey_filename:&str, payld:&AppAuthedClaim) -> String
{
    let header = jsonwebtoken::Header {
        typ: Some(format!("jwt")), alg, kid, cty: None, jku: None, jwk: None,
        x5u:None, x5c:None, x5t: None, x5t_s256:None
    };
    let basepath = env::var(ENV_VAR_SERVICE_BASE_PATH).unwrap();
    let fullpath = basepath + EXAMPLE_REL_PATH + privkey_filename;
    let mut f = File::open(fullpath).unwrap();
    let mut raw_content = String::new();
    let _ = f.read_to_string(&mut raw_content).unwrap();
    let key = EncodingKey::from_rsa_pem(raw_content.as_bytes()).unwrap();
    let result = jwt_encode(&header, payld, &key);
    result.unwrap()
}

#[tokio::test]
async fn jwt_verify_rsa_ok()
{
    let kstore = MockAuthKeystore::build("jwk_rsa_pubkey_valid.json");
    let kid = kstore.key.common.key_id.as_ref().unwrap().clone();
    let mock_ks : Arc<Box<dyn AbstractAuthKeystore>> =  Arc::new(Box::new(kstore));
    let mut auth = AppJwtAuthentication::new(mock_ks, None);
    let mock_req = {
        let timestamp = Local::now().fixed_offset().timestamp();
        let payld = AppAuthedClaim { profile: 247, iat:timestamp - 5, exp:timestamp + 65,
            perms: vec![AppAuthClaimPermission{app_code: app_meta::RESOURCE_QUOTA_AP_CODE,
                    codename: AppAuthPermissionCode::can_create_return_req }],
            quota: vec![AppAuthClaimQuota{app_code: app_meta::RESOURCE_QUOTA_AP_CODE,
                    mat_code: AppAuthQuotaMatCode::NumOrderLines, maxnum: 61 }],
            aud: vec![format!("another-service"), app_meta::LABAL.to_string()]
        };
        let encoded = ut_jwt_encode_token(Some(kid), Algorithm::RS256,
                      "rsa256_priv_key.pem", &payld);
        let token = format!("Bearer {}", encoded);
        let mut req = Request::new(Body::empty());
        let hdr_name = HeaderName::from_bytes(b"authorization").unwrap();
        let hdr_val = HeaderValue::from_bytes(token.as_bytes()).unwrap();
        req.headers_mut().insert(hdr_name, hdr_val);
        req
    };
    let result = auth.authorize(mock_req).await;
    assert!(result.is_ok());
    let req = result.unwrap();
    let (mut parts, _body) = req.into_parts();
    let result = parts.extensions.remove::<AppAuthedClaim>();
    assert!(result.is_some());
    let decoded_claim = result.unwrap();
    assert_eq!(decoded_claim.profile , 247);
    assert!(decoded_claim.aud.contains(&app_meta::LABAL.to_string()));
    assert_eq!(decoded_claim.perms.len() , 1);
    assert_eq!(decoded_claim.quota.len() , 1);
    assert_eq!(decoded_claim.perms[0].app_code, app_meta::RESOURCE_QUOTA_AP_CODE);
    assert!(matches!(decoded_claim.perms[0].codename, AppAuthPermissionCode::can_create_return_req ));
    assert_eq!(decoded_claim.quota[0].app_code, app_meta::RESOURCE_QUOTA_AP_CODE);
    assert!(matches!(decoded_claim.quota[0].mat_code, AppAuthQuotaMatCode::NumOrderLines ));
    assert_eq!(decoded_claim.quota[0].maxnum, 61u32);
} // end of fn jwt_verify_rsa_ok

#[tokio::test]
async fn jwt_verify_rsa_invalid_req_header() {
    let kstore = MockAuthKeystore::build("jwk_rsa_pubkey_valid.json");
    let mock_ks : Arc<Box<dyn AbstractAuthKeystore>> =  Arc::new(Box::new(kstore));
    let mut auth = AppJwtAuthentication::new(mock_ks, None);
    let mock_req = {
        // error cause, header value should be case-sensitive
        let token = format!("bearer invalid.encoded.token");
        let mut req = Request::new(Body::empty());
        let hdr_name = HeaderName::from_bytes(b"authorization").unwrap();
        let hdr_val = HeaderValue::from_bytes(token.as_bytes()).unwrap();
        req.headers_mut().insert(hdr_name, hdr_val);
        req
    };
    let result = auth.authorize(mock_req).await;
    assert!(result.is_err());
    let resp = result.unwrap_err();
    let result = resp.extensions().get::<TypedHeaderRejection>();
    let error = result.unwrap();
    assert_eq!(error.name().as_str(), "authorization");
}

#[tokio::test]
async fn jwt_verify_rsa_header_missing_key_id() {
    let kstore = MockAuthKeystore::build("jwk_rsa_pubkey_valid.json");
    let mock_ks : Arc<Box<dyn AbstractAuthKeystore>> =  Arc::new(Box::new(kstore));
    let mut auth = AppJwtAuthentication::new(mock_ks, None);
    let mock_req = {
        let timestamp = Local::now().fixed_offset().timestamp();
        let payld = AppAuthedClaim { profile: 247, iat:timestamp - 5, exp:timestamp + 65,
            perms: vec![], quota: vec![], aud: vec![app_meta::LABAL.to_string()]
        };
        let encoded = ut_jwt_encode_token(None, Algorithm::RS256,
                      "rsa256_priv_key.pem", &payld);
        let token = format!("Bearer {}", encoded);
        let mut req = Request::new(Body::empty());
        let hdr_name = HeaderName::from_bytes(b"authorization").unwrap();
        let hdr_val = HeaderValue::from_bytes(token.as_bytes()).unwrap();
        req.headers_mut().insert(hdr_name, hdr_val);
        req
    };
    let result = auth.authorize(mock_req).await;
    assert!(result.is_err());
    let resp = result.unwrap_err();
    let result = resp.extensions().get::<AppError>();
    let error = result.unwrap();
    assert_eq!(error.code, AppErrorCode::InvalidJsonFormat);
    assert_eq!(error.detail.as_ref().unwrap().as_str(), "jwt-missing-key-id");
}


async fn jwt_verify_rsa_error_jwk_common(jwk_file_name:&str, now_time:DateTime<FixedOffset>)
    -> AppError
{
    let kstore = MockAuthKeystore::build(jwk_file_name);
    let kid = kstore.key.common.key_id.as_ref().unwrap().clone();
    let mock_ks : Arc<Box<dyn AbstractAuthKeystore>> =  Arc::new(Box::new(kstore));
    let mut auth = AppJwtAuthentication::new(mock_ks, None);
    let mock_req = {
        let timestamp = now_time.timestamp();
        let payld = AppAuthedClaim { profile: 247, iat:timestamp - 5, exp:timestamp + 65,
            perms: vec![], quota: vec![], aud: vec![app_meta::LABAL.to_string()]
        };
        let encoded = ut_jwt_encode_token(Some(kid), Algorithm::RS256,
                      "rsa256_priv_key.pem", &payld);
        let token = format!("Bearer {}", encoded);
        let mut req = Request::new(Body::empty());
        let hdr_name = HeaderName::from_bytes(b"authorization").unwrap();
        let hdr_val = HeaderValue::from_bytes(token.as_bytes()).unwrap();
        req.headers_mut().insert(hdr_name, hdr_val);
        req
    };
    let result = auth.authorize(mock_req).await;
    assert!(result.is_err());
    let mut resp = result.unwrap_err();
    let result = resp.extensions_mut().remove::<AppError>();
    result.unwrap()
}

#[tokio::test]
async fn jwt_verify_rsa_error_jwk_algo() {
    let error = jwt_verify_rsa_error_jwk_common(
        "jwk_rsa_pubkey_incorrect_alg.json",
        Local::now().fixed_offset()
    ).await;
    assert_eq!(error.code, AppErrorCode::CryptoFailure);
    assert_eq!(error.detail.as_ref().unwrap().as_str(), "invalid-signature");
}
#[tokio::test]
async fn jwt_verify_rsa_error_jwk_key() {
    let error = jwt_verify_rsa_error_jwk_common(
        "jwk_rsa_pubkey_incorrect_base64.json",
        Local::now().fixed_offset()
    ).await;
    assert_eq!(error.code, AppErrorCode::DataCorruption);
    assert!(error.detail.as_ref().unwrap().contains("encoder:Base64"));
}
#[tokio::test]
async fn jwt_verify_rsa_signature_verify_error() {
    // pub key can be decoded, but failed to verify the token
    let error = jwt_verify_rsa_error_jwk_common(
        "jwk_rsa_pubkey_mismatched_key.json",
        Local::now().fixed_offset()
    ).await;
    assert_eq!(error.code, AppErrorCode::CryptoFailure);
    assert_eq!(error.detail.as_ref().unwrap().as_str(), "invalid-signature");
}

#[tokio::test]
async fn jwt_verify_rsa_token_expired() {
    let error = jwt_verify_rsa_error_jwk_common(
        "jwk_rsa_pubkey_valid.json",
        Local::now().fixed_offset() - Duration::minutes(5)
    ).await;
    assert_eq!(error.code, AppErrorCode::CryptoFailure);
    assert_eq!(error.detail.as_ref().unwrap().as_str(), "ExpiredSignature");
}

