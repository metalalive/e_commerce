use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::mem::drop;
use std::result::Result as DefaultResult;
use std::sync::RwLock;

use serde_json::Value as JsnVal;

use ecommerce_common::error::AppErrorCode;

use super::AbstractConfidentiality;
use crate::error::AppError;

const SOURCE_SIZE_LIMIT_NBYTES: u64 = 8196;

pub struct UserSpaceConfidentiality {
    _src_fullpath: String,
    // the inner cache should NOT be large for each application
    // so far the modules interacting with this confidential handler are :
    // (1) SQL database servers (2) AMQP message broker
    // TODO, add expiry
    _cached: RwLock<HashMap<String, String>>,
}

impl UserSpaceConfidentiality {
    pub fn build(fullpath: String) -> Self {
        let _cached = RwLock::new(HashMap::new());
        Self {
            _cached,
            _src_fullpath: fullpath,
        }
    }

    fn rawdata_from_source(&self) -> DefaultResult<(usize, Vec<u8>), AppError> {
        let srcpath = self._src_fullpath.as_str();
        let mut rawbuf = Vec::new(); // the source file should NOT be large
        match File::open(srcpath) {
            Ok(mut file) => {
                let actual_f_sz = file.metadata().unwrap().len();
                if actual_f_sz < SOURCE_SIZE_LIMIT_NBYTES {
                    match file.read_to_end(&mut rawbuf) {
                        Ok(sz) => Ok((sz, rawbuf)),
                        Err(e) => Err(AppError {
                            detail: Some(e.to_string()),
                            code: AppErrorCode::IOerror(e.kind()),
                        }),
                    }
                } else {
                    Err(AppError {
                        code: AppErrorCode::ExceedingMaxLimit,
                        detail: Some("source-file".to_string()),
                    })
                }
            }
            Err(e) => Err(AppError {
                code: AppErrorCode::IOerror(e.kind()),
                detail: Some(e.to_string()),
            }),
        }
    } // end of rawdata_from_source

    fn to_json(&self, raw: Vec<u8>) -> DefaultResult<JsnVal, AppError> {
        match serde_json::from_slice::<JsnVal>(&raw) {
            Ok(obj) => Ok(obj),
            Err(e) => Err(AppError {
                code: AppErrorCode::InvalidJsonFormat,
                detail: Some(e.to_string()),
            }),
        }
    } // end of to_json
    fn search_json_payload<'a>(
        &self,
        toplvl: &'a JsnVal,
        id_: &str,
    ) -> DefaultResult<&'a JsnVal, AppError> {
        let mut curr_lvl = toplvl;
        for tok in id_.split('/') {
            let err_detail = match curr_lvl {
                JsnVal::Object(o) => match o.get(tok) {
                    Some(nxtlvl) => {
                        curr_lvl = nxtlvl;
                        None
                    }
                    None => Some(format!("json-object,id:{}", id_)),
                },
                JsnVal::Array(a) => match tok.parse::<usize>() {
                    Ok(t) => match a.get(t) {
                        Some(nxtlvl) => {
                            curr_lvl = nxtlvl;
                            None
                        }
                        None => Some(format!("json-array,id:{}", id_)),
                    },
                    Err(e) => Some(format!("path-error,id:{},detail:{}", id_, e)),
                },
                _others => Some(format!("json-scalar,id:{}", id_)),
            };
            if let Some(msg) = err_detail {
                return Err(AppError {
                    detail: Some(msg),
                    code: AppErrorCode::NoConfidentialityCfg,
                });
            }
        } // end of loop
        Ok(curr_lvl)
    } // end of fn search_json_payload
} // end of fn UserSpaceConfidentiality

impl AbstractConfidentiality for UserSpaceConfidentiality {
    fn try_get_payload(&self, id_: &str) -> DefaultResult<String, AppError> {
        let rguard = match self._cached.read() {
            Ok(rg) => rg,
            Err(e) => {
                let detail = e.to_string() + ", source: UserSpaceConfidentiality";
                return Err(AppError {
                    detail: Some(detail),
                    code: AppErrorCode::AcquireLockFailure,
                });
            }
        };
        if let Some(v) = rguard.get(id_) {
            Ok(v.clone())
        } else {
            drop(rguard);
            let (_sz, rawdata) = self.rawdata_from_source()?;
            let toplvl = self.to_json(rawdata)?;
            let found = self.search_json_payload(&toplvl, id_)?;
            let found = serde_json::to_string(found).unwrap();
            match self._cached.write() {
                Ok(mut wguard) => {
                    let _old_data = wguard.insert(id_.to_string(), found.clone());
                }
                Err(e) => {
                    let detail = e.to_string() + ", source: UserSpaceConfidentiality";
                    return Err(AppError {
                        detail: Some(detail),
                        code: AppErrorCode::AcquireLockFailure,
                    });
                }
            };
            Ok(found)
        }
    } // end of fn try_get_payload
} // end of impl AbstractConfidentiality
