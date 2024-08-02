use std::fmt::Write;
use std::result::Result;

use crate::error::AppErrorCode;
use crate::util::hex_to_octet;

const OID_BYTE_LENGTH: usize = 16;

/*
* - size of order-id has to match database schema
* - In mariaDB, the BINARY column are right-padded with number of zero octets (0x0)
    to fill the length og declared binary column, this struct ensures any given hex
    string can be converted to correct binary format to database server.
* */
pub struct OidBytes(pub [u8; OID_BYTE_LENGTH]);

impl<'a> TryFrom<&'a str> for OidBytes {
    type Error = (AppErrorCode, String);
    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        if value.len() <= (OID_BYTE_LENGTH << 1) {
            let src = hex_to_octet(value)?;
            let mut dst = [0; OID_BYTE_LENGTH];
            let cp_sz = OID_BYTE_LENGTH.min(src.len());
            dst[0..cp_sz].copy_from_slice(&src);
            // `clone_from_slice()` does memory copy in network octet order
            // no need to rotate valid octets to starting address of destination
            let num_rotate = OID_BYTE_LENGTH - (value.len() >> 1);
            dst.rotate_right(num_rotate);
            Ok(OidBytes(dst))
        } else {
            let detail = format!("size-not-fit: {value}");
            Err((AppErrorCode::InvalidInput, detail))
        }
    }
}
impl OidBytes {
    pub fn as_column(&self) -> Vec<u8> {
        self.0.to_vec()
    }
    pub fn to_app_oid(raw: Vec<u8>) -> Result<String, (AppErrorCode, String)> {
        if raw.len() != OID_BYTE_LENGTH {
            let detail = format!("fetched-id-len: {}", raw.len());
            Err((AppErrorCode::DataCorruption, detail))
        } else {
            let mut start = false;
            let out = raw
                .into_iter()
                .filter(|b| {
                    if b != &0u8 {
                        start = true;
                    }
                    start
                })
                .fold(String::new(), |mut o, b| {
                    let result = write!(&mut o, "{:02x}", b);
                    assert!(result.is_ok());
                    o
                });
            Ok(out)
        }
    }
}
