use jsonwebtoken::jwk::JwkSet;
use payment::AppAuthKeystore;

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
