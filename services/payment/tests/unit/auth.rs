use std::boxed::Box;
use std::env;
use std::fs::File;
use std::io::Read;
use std::sync::Arc;

use actix_http::HttpMessage;
use actix_web::test::TestRequest;
use actix_web_httpauth::extractors::bearer::BearerAuth;
use actix_web_httpauth::headers::authorization::Bearer;
use async_trait::async_trait;
use chrono::{Duration, Local};
use jsonwebtoken::errors::ErrorKind as JwtErrorKind;
use jsonwebtoken::jwk::{Jwk, JwkSet};
use jsonwebtoken::{Algorithm, EncodingKey};

use ecommerce_common::constant::env_vars::SERVICE_BASEPATH;

use payment::{
    app_meta, validate_jwt, AbstractAuthKeystore, AppAuthClaimPermission, AppAuthClaimQuota,
    AppAuthKeystore, AppAuthPermissionCode, AppAuthQuotaMatCode, AppAuthedClaim,
    AppKeystoreRefreshResult, AuthJwtError, AuthKeystoreError,
};

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
    let new = serde_json::from_slice::<JwkSet>(rawdata_new_keys).unwrap();
    let actual = AppAuthKeystore::merge(&mut target, new);
    assert_eq!(actual.0, expect_num_discarded);
    assert_eq!(actual.1, expect_num_added);
    assert!(target.find("95667b348d").is_some());
    assert!(target.find("00db7af03e").is_some());
    assert!(target.find("b110fb3480").is_some());
    assert!(target.find("1b7a039bf4").is_none());
} // end of fn keystore_refresh_ok

struct MockAuthKeystore {
    _mock_key: Jwk,
}

#[async_trait]
impl AbstractAuthKeystore for MockAuthKeystore {
    type Error = AuthKeystoreError;

    fn update_period(&self) -> Duration {
        Duration::minutes(25)
    }
    async fn refresh(&self) -> Result<AppKeystoreRefreshResult, Self::Error> {
        Ok(AppKeystoreRefreshResult {
            period_next_op: self.update_period(),
            num_discarded: 0,
            num_added: 0,
        })
    }
    async fn find(&self, _kid: &str) -> Result<Jwk, Self::Error> {
        Ok(self._mock_key.clone())
    }
}
impl MockAuthKeystore {
    fn build(pubkey_filename: &str) -> Self {
        let basepath = env::var(SERVICE_BASEPATH).unwrap();
        let fullpath = basepath + EXAMPLE_REL_PATH + pubkey_filename;
        let f = File::open(fullpath).unwrap();
        let result = serde_json::from_reader::<File, Jwk>(f);
        let _mock_key = result.unwrap();
        Self { _mock_key }
    }
} // end of impl MockAuthKeystore

pub(super) fn ut_setup_auth_claim(usr_id: u32, exp_bias_secs: i64) -> AppAuthedClaim {
    let ts_now = Local::now().fixed_offset().timestamp();
    AppAuthedClaim {
        profile: usr_id,
        iat: ts_now,
        exp: ts_now + exp_bias_secs * 100,
        aud: vec![app_meta::LABAL.to_string()],
        perms: vec![AppAuthClaimPermission {
            app_code: app_meta::RESOURCE_QUOTA_AP_CODE,
            codename: AppAuthPermissionCode::can_create_charge,
        }],
        quota: vec![AppAuthClaimQuota {
            app_code: app_meta::RESOURCE_QUOTA_AP_CODE,
            mat_code: AppAuthQuotaMatCode::NumChargesPerOrder,
            maxnum: 299,
        }],
    }
} // end of fn ut_setup_auth_claim

fn ut_setup_encode_jwt(
    kid: Option<String>,
    alg: Algorithm,
    privkey_filename: &str,
    payld: &AppAuthedClaim,
) -> String {
    let header = jsonwebtoken::Header {
        typ: Some(format!("jwt")),
        alg,
        kid,
        cty: None,
        jku: None,
        jwk: None,
        x5u: None,
        x5c: None,
        x5t: None,
        x5t_s256: None,
    };
    let basepath = env::var(SERVICE_BASEPATH).unwrap();
    let fullpath = basepath + EXAMPLE_REL_PATH + privkey_filename;
    let mut f = File::open(fullpath).unwrap();
    let mut raw_content = String::new();
    let _ = f.read_to_string(&mut raw_content).unwrap();
    let key = EncodingKey::from_rsa_pem(raw_content.as_bytes()).unwrap();
    let result = jsonwebtoken::encode(&header, payld, &key);
    result.unwrap()
}

#[actix_web::test]
async fn jwt_verify_rsa_ok() {
    let mock_keystore = MockAuthKeystore::build("jwk-rsa-pubkey-valid.json");
    let kid = mock_keystore._mock_key.common.key_id.clone();
    let mock_keystore: Arc<Box<dyn AbstractAuthKeystore<Error = AuthKeystoreError>>> =
        Arc::new(Box::new(mock_keystore));
    let claim = ut_setup_auth_claim(5678, 60);
    let encoded = ut_setup_encode_jwt(kid, Algorithm::RS256, "rsa256-priv-key.pem", &claim);
    let hdr_bearer = Bearer::new(encoded);
    let mut mock_req = TestRequest::default()
        .app_data(mock_keystore)
        .insert_header(("authorization", hdr_bearer))
        .to_srv_request();
    let mock_credentials = mock_req.extract::<BearerAuth>().await.unwrap();
    let result = validate_jwt(mock_req, mock_credentials).await;
    assert!(result.is_ok());
    if let Ok(mut mock_req) = result {
        let r = mock_req.extract::<AppAuthedClaim>().await;
        let mut claim_recv = r.unwrap();
        assert_eq!(claim_recv.profile, 5678);
        let perm = claim_recv.perms.pop().unwrap();
        assert_eq!(perm.app_code, app_meta::RESOURCE_QUOTA_AP_CODE);
        assert!(matches!(
            perm.codename,
            AppAuthPermissionCode::can_create_charge
        ));
    }
} // end of fn jwt_verify_rsa_ok

#[actix_web::test]
async fn jwt_verify_missing_keystore() {
    let hdr_bearer = Bearer::new("one-two-three".to_string());
    let mut mock_req = TestRequest::default()
        .insert_header(("authorization", hdr_bearer))
        .to_srv_request();
    let mock_credentials = mock_req.extract::<BearerAuth>().await.unwrap();
    let result = validate_jwt(mock_req, mock_credentials).await;
    assert!(result.is_err());
    if let Err((_e, mock_req)) = result {
        let r = mock_req.extensions_mut().remove::<AuthJwtError>();
        let detail = r.unwrap();
        assert!(matches!(detail, AuthJwtError::MissingKeystore));
    }
}

#[actix_web::test]
async fn jwt_verify_rsa_invalid_header() {
    let mock_keystore = MockAuthKeystore::build("jwk-rsa-pubkey-valid.json");
    let mock_keystore: Arc<Box<dyn AbstractAuthKeystore<Error = AuthKeystoreError>>> =
        Arc::new(Box::new(mock_keystore));
    let hdr_bearer = Bearer::new("wrong-encoded-token".to_string());
    let mut mock_req = TestRequest::default()
        .app_data(mock_keystore)
        .insert_header(("authorization", hdr_bearer))
        .to_srv_request();
    let mock_credentials = mock_req.extract::<BearerAuth>().await.unwrap();
    let result = validate_jwt(mock_req, mock_credentials).await;
    assert!(result.is_err());
    if let Err((_e, mock_req)) = result {
        let detail = mock_req.extensions_mut().remove::<AuthJwtError>().unwrap();
        if let AuthJwtError::VerifyFailure(ekind) = detail {
            assert!(matches!(ekind, JwtErrorKind::InvalidToken));
        } else {
            assert!(false);
        }
    }
}

#[actix_web::test]
async fn jwt_verify_rsa_missing_key_id() {
    let mock_keystore = MockAuthKeystore::build("jwk-rsa-pubkey-valid.json");
    let mock_keystore: Arc<Box<dyn AbstractAuthKeystore<Error = AuthKeystoreError>>> =
        Arc::new(Box::new(mock_keystore));
    let claim = ut_setup_auth_claim(5678, 60);
    let encoded = ut_setup_encode_jwt(None, Algorithm::RS256, "rsa256-priv-key.pem", &claim);
    let hdr_bearer = Bearer::new(encoded);
    let mut mock_req = TestRequest::default()
        .app_data(mock_keystore)
        .insert_header(("authorization", hdr_bearer))
        .to_srv_request();
    let mock_credentials = mock_req.extract::<BearerAuth>().await.unwrap();
    let result = validate_jwt(mock_req, mock_credentials).await;
    assert!(result.is_err());
    if let Err((_e, mock_req)) = result {
        let detail = mock_req.extensions_mut().remove::<AuthJwtError>().unwrap();
        assert!(matches!(detail, AuthJwtError::MissingKeyId));
    }
}

async fn jwt_verify_rsa_error_jwk_common(
    jwk_pub_filename: &str,
    exp_bias_secs: i64,
) -> AuthJwtError {
    let mock_keystore = MockAuthKeystore::build(jwk_pub_filename);
    let kid = mock_keystore._mock_key.common.key_id.clone();
    let mock_keystore: Arc<Box<dyn AbstractAuthKeystore<Error = AuthKeystoreError>>> =
        Arc::new(Box::new(mock_keystore));
    let claim = ut_setup_auth_claim(5678, exp_bias_secs);
    let encoded = ut_setup_encode_jwt(kid, Algorithm::RS256, "rsa256-priv-key.pem", &claim);
    let hdr_bearer = Bearer::new(encoded);
    let mut mock_req = TestRequest::default()
        .app_data(mock_keystore)
        .insert_header(("authorization", hdr_bearer))
        .to_srv_request();
    let mock_credentials = mock_req.extract::<BearerAuth>().await.unwrap();
    let result = validate_jwt(mock_req, mock_credentials).await;
    assert!(result.is_err());
    let (_e, mock_req) = result.unwrap_err();
    let detail = mock_req.extensions_mut().remove::<AuthJwtError>().unwrap();
    detail
}

#[actix_web::test]
async fn jwt_verify_rsa_error_jwk_algo() {
    let error = jwt_verify_rsa_error_jwk_common("jwk-EdDSA-key-ed25519.json", 65).await;
    if let AuthJwtError::VerifyFailure(ekind) = error {
        assert!(matches!(ekind, JwtErrorKind::InvalidAlgorithm));
    } else {
        assert!(false);
    }
}

#[actix_web::test]
async fn jwt_verify_rsa_corrupted_key_element() {
    let error = jwt_verify_rsa_error_jwk_common("jwk-rsa-pubkey-corrupted-key-elm.json", 60).await;
    if let AuthJwtError::VerifyFailure(JwtErrorKind::Base64(ekind)) = error {
        assert_eq!(ekind.to_string().to_lowercase(), "invalid padding");
    } else {
        assert!(false);
    }
}

#[actix_web::test]
async fn jwt_verify_rsa_signature_failure() {
    let error = jwt_verify_rsa_error_jwk_common("jwk-rsa-pubkey-mismatched-key.json", 50).await;
    if let AuthJwtError::VerifyFailure(ekind) = error {
        assert!(matches!(ekind, JwtErrorKind::InvalidSignature));
    } else {
        assert!(false);
    }
}

#[actix_web::test]
async fn jwt_verify_rsa_token_expired() {
    let error = jwt_verify_rsa_error_jwk_common("jwk-rsa-pubkey-valid.json", -16).await;
    if let AuthJwtError::VerifyFailure(ekind) = error {
        assert!(matches!(ekind, JwtErrorKind::ExpiredSignature));
    } else {
        assert!(false);
    }
}
