pub mod adapter;
pub mod api;
pub mod auth;
pub mod confidentiality;
pub mod config;
pub mod constant;
pub mod error;
pub mod logging;
pub mod model;

use std::sync::Arc;

pub type WebApiPath = String;
pub(crate) type AppLogAlias = Arc<String>;

pub mod util {
    use crate::error::AppErrorCode;
    use std::result::Result;
    use std::vec::Vec;

    pub fn hex_to_octet(src: &str) -> Result<Vec<u8>, (AppErrorCode, String)> {
        if src.len() % 2 == 0 {
            let mut parse_errors = Vec::new();
            let result = (0..src.len())
                .step_by(2)
                .map(|idx| {
                    if let Some(hx) = src.get(idx..idx + 2) {
                        u8::from_str_radix(hx, 16)
                            .map_err(|_e| format!("parse-char-at-idx: {hx} , {idx}"))
                    } else {
                        Err(format!("no-chars-at-idx: {idx}"))
                    }
                })
                .filter_map(|r| match r {
                    Ok(v) => Some(v),
                    Err(e) => {
                        parse_errors.push(e);
                        None
                    }
                })
                .collect::<Vec<_>>();
            if let Some(d) = parse_errors.pop() {
                Err((AppErrorCode::InvalidInput, d))
            } else {
                Ok(result)
            } // cannot convert to u8 array using try-from method,  the size of given
              // char vector might not be the same as OID_BYTE_LENGTH
        } else {
            let detail = format!("hex-string-incorrect-size: {src}");
            Err((AppErrorCode::InvalidInput, detail))
        }
    } // end of fn hex_to_octet
}
