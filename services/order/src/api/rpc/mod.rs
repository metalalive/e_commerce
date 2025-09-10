use std::result::Result as DefaultResult;
use std::vec::Vec;

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value as JsnVal;

#[cfg(test)]
use serde::Deserialize;

use ecommerce_common::adapter::rpc::py_celery::{deserialize_reply, serialize_msg_body};
use ecommerce_common::error::AppErrorCode;

use crate::constant::api::rpc as RpcConst;
use crate::error::AppError;
use crate::rpc::AppRpcClientReqProperty;
use crate::AppSharedState;

pub mod dto;
mod misc;
mod order_status;
mod stock_level;

pub async fn route_to_handler(
    req: AppRpcClientReqProperty,
    shr_state: AppSharedState,
) -> DefaultResult<Vec<u8>, AppError> {
    // TODO, build a route table if number of different handling functions
    // grows over time
    let hdlr_label = RpcConst::extract_handler_label(req.route.as_str())?;
    match hdlr_label {
        RpcConst::EDIT_PRODUCT_PRICE => Ok(misc::store_products(req, shr_state).await),
        RpcConst::STOCK_LEVEL_EDIT => Ok(stock_level::inventory_edit(req, shr_state).await),
        RpcConst::CURRENCY_RATE_REFRESH => Ok(misc::currency_refresh(req, shr_state).await),
        RpcConst::STOCK_RETURN_CANCELLED => {
            Ok(stock_level::inventory_return_cancelled(req, shr_state).await)
        }
        RpcConst::ORDER_RET_READ_REFUND => {
            Ok(order_status::read_cancelled_refund(req, shr_state).await)
        }
        RpcConst::ORDER_RSV_READ_PAYMENT => {
            Ok(order_status::read_reserved_payment(req, shr_state).await)
        }
        RpcConst::ORDER_RSV_READ_INVENTORY => {
            Ok(order_status::read_reserved_inventory(req, shr_state).await)
        }
        RpcConst::ORDER_RSV_UPDATE_PAYMENT => {
            Ok(order_status::update_paid_lines(req, shr_state).await)
        }
        RpcConst::ORDER_RSV_DISCARD_UNPAID => {
            Ok(order_status::discard_unpaid_lines(req, shr_state).await)
        }
        _others => {
            let err = AppError {
                code: AppErrorCode::NotImplemented,
                detail: Some("rpc-routing-failure".to_string()),
            };
            Err(err)
        }
    }
} // end of fn route_to_handler

pub(super) struct PyCelery;

impl PyCelery {
    pub(super) fn deserialize_req<T, U>(raw: &[u8]) -> DefaultResult<(T, U), AppError>
    where
        T: DeserializeOwned,
        U: DeserializeOwned,
    {
        const NUM_TOPLVL_BLK: usize = 3;
        let reqbody = serde_json::from_slice::<JsnVal>(raw).map_err(|e| AppError {
            detail: Some(e.to_string()),
            code: AppErrorCode::InvalidJsonFormat,
        })?;
        let (pargs, kwargs) = if let JsnVal::Array(mut b) = reqbody {
            if b.len() == NUM_TOPLVL_BLK {
                let (_pargs, _kwargs, _metadata) = (b.remove(0), b.remove(0), b.remove(0));
                if _pargs.is_array() && _kwargs.is_object() && _metadata.is_object() {
                    (_pargs, _kwargs)
                } else {
                    return Err(AppError {
                        detail: Some("celery-de-arrayitem-mismatch".to_string()),
                        code: AppErrorCode::InvalidJsonFormat,
                    });
                }
            } else {
                return Err(AppError {
                    detail: Some("celery-de-topblk-incomplete".to_string()),
                    code: AppErrorCode::InvalidJsonFormat,
                });
            }
        } else {
            return Err(AppError {
                detail: Some("celery-de-not-array".to_string()),
                code: AppErrorCode::InvalidJsonFormat,
            });
        };
        let out0 = serde_json::from_value(pargs).map_err(|e| AppError {
            detail: Some(e.to_string() + ", celery-de-args-fail"),
            code: AppErrorCode::InvalidJsonFormat,
        })?;
        let out1 = serde_json::from_value(kwargs).map_err(|e| AppError {
            detail: Some(e.to_string() + ", celery-de-kwargs-fail"),
            code: AppErrorCode::InvalidJsonFormat,
        })?;
        Ok((out0, out1))
    } // end of fn deserialize_req

    pub(super) fn deserialize_reply<T>(raw: &Vec<u8>) -> DefaultResult<T, AppError>
    where
        T: DeserializeOwned,
    {
        deserialize_reply::<T>(raw).map_err(|(code, msg)| AppError {
            detail: Some(msg),
            code,
        })
    }

    pub(super) fn serialize<T: Serialize>(inner: T) -> DefaultResult<Vec<u8>, AppError> {
        serialize_msg_body(inner).map_err(|(code, msg)| AppError {
            detail: Some(msg),
            code,
        })
    }

    pub(super) fn get_task_id(req: &AppRpcClientReqProperty) -> DefaultResult<&String, AppError> {
        req.correlation_id.as_ref().ok_or(AppError {
            detail: Some("celery-missing-task-id".to_string()),
            code: AppErrorCode::RpcConsumeFailure,
        })
    }

    pub(super) fn build_response(task_id: &str, status: &str) -> JsnVal {
        // TODO, accept result from callers
        const PATTERN: &str = r#" {"result": null, "traceback": null, "children": []} "#;
        let mut out: JsnVal = serde_json::from_str(PATTERN).unwrap();
        if let Some(m) = out.as_object_mut() {
            m.insert("status".to_string(), JsnVal::String(status.to_string()));
            m.insert("task_id".to_string(), JsnVal::String(task_id.to_string()));
        }
        out
    }
} // end of impl PyCelery

pub fn build_error_response(e: AppError) -> JsnVal {
    const PATTERN: &str = r#" {"status":"error", "detail":""} "#;
    let mut out: JsnVal = serde_json::from_str(PATTERN).unwrap();
    if let Some(m) = out.as_object_mut() {
        let _detail = format!("{}", e);
        m.insert("detail".to_string(), JsnVal::String(_detail));
    }
    out // return json object, to let callers add extra info
}

// write tests at here cuz the function is NOT visible outside this crate
#[test]
fn test_pycelery_deserialize_ok() {
    #[derive(Deserialize, Debug)]
    struct TestData {
        live: bool,
        beat: u16,
    }

    let data = br#"[["stock", "hay"], {"live":true, "beat":782}, {"callbacks":[]}]"#.to_vec();
    let result = py_celery_deserialize_req::<Vec<String>, TestData>(&data);
    assert!(result.is_ok());
    let (arg, kwarg) = result.unwrap();
    assert_eq!(arg.len(), 2);
    assert!(arg.contains(&"stock".to_string()));
    assert!(arg.contains(&"hay".to_string()));
    assert!(kwarg.live);
    assert_eq!(kwarg.beat, 782);
}

#[test]
fn test_pycelery_deserialize_error() {
    #[derive(Deserialize, Debug)]
    struct TestData {
        tone: i8,
    }

    let data = br#"[[], {}]"#.to_vec();
    let result = py_celery_deserialize_req::<Vec<i64>, TestData>(&data);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert_eq!(error.code, AppErrorCode::InvalidJsonFormat);
    assert_eq!(
        error.detail.unwrap().as_str(),
        "celery-de-topblk-incomplete"
    );
    let data = br#"[[], {}, 567]"#.to_vec();
    let result = py_celery_deserialize_req::<Vec<i64>, TestData>(&data);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert_eq!(
        error.detail.unwrap().as_str(),
        "celery-de-arrayitem-mismatch"
    );
}
