use std::result::Result as DefaultResult;
use std::vec::Vec;
use serde_json::Value as JsnVal;

use crate::AppSharedState;
use crate::rpc::AppRpcClientReqProperty;
use crate::error::{AppError, AppErrorCode} ;
use crate::constant::{
    RPCAPI_EDIT_PRODUCT_PRICE, RPCAPI_STOCK_LEVEL_EDIT, RPCAPI_ORDER_RSV_READ_INVENTORY,
    RPCAPI_ORDER_RSV_READ_PAYMENT, RPCAPI_ORDER_RSV_UPDATE_PAYMENT, RPCAPI_ORDER_RSV_DISCARD_UNPAID,
    RPCAPI_STOCK_RETURN_CANCELLED
};

pub mod dto;
mod store_products;
mod stock_level;
mod order_status;

pub async fn route_to_handler(req:AppRpcClientReqProperty, shr_state:AppSharedState)
    -> DefaultResult<Vec<u8>, AppError>
{ // TODO, build a route table if number of different handling functions
  // grows over time
    match req.route.as_str() {
        RPCAPI_EDIT_PRODUCT_PRICE => Ok(store_products::process(req, shr_state).await),
        RPCAPI_STOCK_LEVEL_EDIT => Ok(stock_level::inventory_edit(req, shr_state).await),
        RPCAPI_STOCK_RETURN_CANCELLED => Ok(stock_level::inventory_return_cancelled(req, shr_state).await),
        RPCAPI_ORDER_RSV_READ_PAYMENT => Ok(order_status::read_reserved_payment(req, shr_state).await),
        RPCAPI_ORDER_RSV_READ_INVENTORY => Ok(order_status::read_reserved_inventory(req, shr_state).await),
        RPCAPI_ORDER_RSV_UPDATE_PAYMENT => Ok(order_status::update_paid_lines(req, shr_state).await),
        RPCAPI_ORDER_RSV_DISCARD_UNPAID => Ok(order_status::discard_unpaid_lines(req, shr_state).await) ,
        _others => {
            let err = AppError { code: AppErrorCode::NotImplemented,
            detail: Some("rpc-routing-failure".to_string()) };
            Err(err)
        }
    }
}

pub(super) fn py_celery_deserialize(raw:&Vec<u8>) -> DefaultResult<(JsnVal,JsnVal,JsnVal), AppError>
{
    const NUM_TOPLVL_BLK:usize = 3;
    let result = serde_json::from_slice::<JsnVal>(raw);
    if let Err(e) = &result {
        return Err(AppError {detail:Some(e.to_string()), code:AppErrorCode::InvalidJsonFormat});
    }
    let reqbody = result.unwrap();
    if let JsnVal::Array(mut b) = reqbody {
        if b.len() == NUM_TOPLVL_BLK {
            let (pargs, kwargs, _metadata) = (b.remove(0), b.remove(0), b.remove(0));
            if pargs.is_array() && kwargs.is_object() && _metadata.is_object() {
                Ok((pargs, kwargs, _metadata))
            } else {
                Err(AppError {detail:Some("celery-de-arrayitem-mismatch".to_string()),
                    code:AppErrorCode::InvalidJsonFormat})
            }
        } else {
            Err(AppError {detail:Some("celery-de-topblk-incomplete".to_string()),
                code:AppErrorCode::InvalidJsonFormat})
        }
    } else {
        Err(AppError {detail:Some("celery-de-not-array".to_string()),
            code:AppErrorCode::InvalidJsonFormat})
    }
} // end of fn py_celery_deserialize


pub fn build_error_response(e:AppError) -> serde_json::Value
{
    const PATTERN:&str = r#" {"status":"error", "detail":""} "#;
    let mut out: serde_json::Value = serde_json::from_str(PATTERN).unwrap();
    if let Some(m) = out.as_object_mut() {
        let _detail = format!("{}", e);
        m.insert("detail".to_string(), serde_json::Value::String(_detail));
    }
    out // return json object, to let callers add extra info
}

// write tests at here cuz the function is NOT visible outside this crate
#[test]
fn test_pycelery_deserialize_ok()
{
    let data = br#"[[19, "hay"], {"live":true, "mtk":782}, {"callbacks":[]}]"#.to_vec();
    let result = py_celery_deserialize(&data);
    assert!(result.is_ok());
    let (arg, kwarg, metadata) = result.unwrap();
    let (arg_p, kwarg_p, metadata_p) = (
        arg.as_array().unwrap(), kwarg.as_object().unwrap(),
        metadata.as_object().unwrap());
    assert_eq!(arg_p.len(), 2);
    assert!(arg_p[0].is_number());
    assert!(arg_p[1].is_string());
    assert!(kwarg_p["live"].is_boolean());
    assert!(kwarg_p["mtk"].is_number());
    assert!(metadata_p["callbacks"].is_array());
}

#[test]
fn test_pycelery_deserialize_error()
{
    let data = br#"[[], {}]"#.to_vec();
    let result = py_celery_deserialize(&data);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert_eq!(error.code, AppErrorCode::InvalidJsonFormat);
    assert_eq!(error.detail.unwrap().as_str(), "celery-de-topblk-incomplete");
    let data = br#"[[], {}, 567]"#.to_vec();
    let result = py_celery_deserialize(&data);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert_eq!(error.detail.unwrap().as_str(), "celery-de-arrayitem-mismatch");
}

