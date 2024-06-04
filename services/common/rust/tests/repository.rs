use ecommerce_common::adapter::repository::OidBytes;
use ecommerce_common::error::AppErrorCode;

#[test]
fn verify_hex_to_oidbytes() {
    let OidBytes(actual) = OidBytes::try_from("800EFF41").unwrap();
    let expect = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x80, 0x0E, 0xFF, 0x41];
    assert_eq!(actual, expect);
    let OidBytes(actual) = OidBytes::try_from("6D1405982C0EF7").unwrap();
    let expect = [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0x6D, 0x14, 0x05, 0x98, 0x2C, 0x0E, 0xF7,
    ];
    assert_eq!(actual, expect);
    let OidBytes(actual) = OidBytes::try_from("0902900390049005a004a005a006a007").unwrap();
    let expect = [
        0x09, 0x02, 0x90, 0x03, 0x90, 0x04, 0x90, 0x05, 0xa0, 0x04, 0xa0, 0x05, 0xa0, 0x06, 0xa0,
        0x07,
    ];
    assert_eq!(actual, expect);
    let result = OidBytes::try_from("ec0902900390049005a004a005a006a007");
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.0, AppErrorCode::InvalidInput);
    }
}
