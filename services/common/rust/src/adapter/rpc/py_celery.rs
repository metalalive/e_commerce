use std::result::Result;
use std::vec::Vec;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsnVal;

use crate::error::AppErrorCode;

#[allow(clippy::upper_case_acronyms)]
#[derive(Deserialize, Debug)]
pub enum PyCeleryRespStatus {
    STARTED,
    SUCCESS,
    ERROR,
}

#[derive(Deserialize)]
struct PyCeleryRespPartialPayload {
    #[allow(dead_code)]
    task_id: String, // the field is never read, only for validation purpose
    status: PyCeleryRespStatus,
} // this is only for validating current progress done on Celery consumer side

#[derive(Deserialize)]
struct PyCeleryRespPayload<T> {
    #[allow(dead_code)]
    task_id: String,
    #[allow(dead_code)]
    status: PyCeleryRespStatus,
    result: T,
}

#[derive(Default, Serialize)]
struct PyCeleryReqMetadata {
    callbacks: Option<Vec<String>>,
    errbacks: Option<Vec<String>>,
    chain: Option<Vec<String>>,
    chord: Option<String>,
} // TODO, figure out the detail in `chain` and `chord` field

pub fn serialize_msg_body<T: Serialize>(inner: T) -> Result<Vec<u8>, (AppErrorCode, String)> {
    let args = JsnVal::Array(Vec::new());
    let kwargs = serde_json::to_value(inner).map_err(|e| {
        let detail = e.to_string() + ", src: py-celery-serialize";
        (AppErrorCode::InvalidJsonFormat, detail)
    })?;
    let metadata = {
        let md = PyCeleryReqMetadata::default();
        serde_json::to_value(md).unwrap()
    };
    let top = JsnVal::Array(vec![args, kwargs, metadata]);
    Ok(top.to_string().into_bytes())
}

pub fn extract_reply_status(raw: &[u8]) -> Result<PyCeleryRespStatus, (AppErrorCode, String)> {
    let result = serde_json::from_slice::<PyCeleryRespPartialPayload>(raw);
    match result {
        Ok(payld) => Ok(payld.status),
        Err(e) => Err((AppErrorCode::InvalidJsonFormat, e.to_string())),
    }
}

pub fn deserialize_reply<T>(raw: &[u8]) -> Result<T, (AppErrorCode, String)>
where
    T: DeserializeOwned,
{
    serde_json::from_slice::<PyCeleryRespPayload<T>>(raw)
        .map(|payld| payld.result)
        .map_err(|e| (AppErrorCode::InvalidJsonFormat, e.to_string()))
}
