use std::result::Result as DefaultResult;
use std::vec::Vec;

use serde::de::DeserializeOwned;
use serde::{Serialize, Deserialize};
use serde_json::Value as JsnVal;

use crate::AppSharedState;
use crate::rpc::AppRpcClientReqProperty;
use crate::error::{AppError, AppErrorCode} ;
use crate::constant::api::rpc as RpcConst;

pub mod dto;
mod store_products;
mod stock_level;
mod order_status;

pub async fn route_to_handler(req:AppRpcClientReqProperty, shr_state:AppSharedState)
    -> DefaultResult<Vec<u8>, AppError>
{ // TODO, build a route table if number of different handling functions
  // grows over time
    let hdlr_label = RpcConst::extract_handler_label(req.route.as_str())?;
    match hdlr_label {
        RpcConst::EDIT_PRODUCT_PRICE => Ok(store_products::process(req, shr_state).await),
        RpcConst::STOCK_LEVEL_EDIT => Ok(stock_level::inventory_edit(req, shr_state).await),
        RpcConst::STOCK_RETURN_CANCELLED => Ok(stock_level::inventory_return_cancelled(req, shr_state).await),
        RpcConst::ORDER_RET_READ_REFUND => Ok(order_status::read_cancelled_refund(req, shr_state).await),
        RpcConst::ORDER_RSV_READ_PAYMENT => Ok(order_status::read_reserved_payment(req, shr_state).await),
        RpcConst::ORDER_RSV_READ_INVENTORY => Ok(order_status::read_reserved_inventory(req, shr_state).await),
        RpcConst::ORDER_RSV_UPDATE_PAYMENT => Ok(order_status::update_paid_lines(req, shr_state).await),
        RpcConst::ORDER_RSV_DISCARD_UNPAID => Ok(order_status::discard_unpaid_lines(req, shr_state).await) ,
        _others => {
            let err = AppError { code: AppErrorCode::NotImplemented,
            detail: Some("rpc-routing-failure".to_string()) };
            Err(err)
        }
    }
}


pub(super) fn py_celery_deserialize_req<T, U>(raw:&Vec<u8>)
     -> DefaultResult<(T,U), AppError>
     where T: DeserializeOwned, U: DeserializeOwned
{
    const NUM_TOPLVL_BLK:usize = 3;
    let result = serde_json::from_slice::<JsnVal>(raw);
    if let Err(e) = &result {
        return Err(AppError {detail:Some(e.to_string()), code:AppErrorCode::InvalidJsonFormat});
    }
    let reqbody = result.unwrap();
    let (pargs, kwargs) = if let JsnVal::Array(mut b) = reqbody {
        if b.len() == NUM_TOPLVL_BLK {
            let (_pargs, _kwargs, _metadata) = (b.remove(0), b.remove(0), b.remove(0));
            if _pargs.is_array() && _kwargs.is_object() && _metadata.is_object() {
                (_pargs, _kwargs)
            } else {
                return Err(AppError {detail:Some("celery-de-arrayitem-mismatch".to_string()),
                    code:AppErrorCode::InvalidJsonFormat});
            }
        } else {
            return Err(AppError {detail:Some("celery-de-topblk-incomplete".to_string()),
                code:AppErrorCode::InvalidJsonFormat});
        }
    } else {
        return Err(AppError {detail:Some("celery-de-not-array".to_string()),
            code:AppErrorCode::InvalidJsonFormat});
    };
    let out0 = match serde_json::from_value(pargs) {
        Ok(v) => v,
        Err(e) => {
            return Err(AppError {detail:Some(e.to_string() + ", celery-de-args-fail"),
                code:AppErrorCode::InvalidJsonFormat});
        },
    };
    let out1 = match serde_json::from_value(kwargs) {
        Ok(v) => v,
        Err(e) => {
            return Err(AppError {detail:Some(e.to_string() + ", celery-de-kwargs-fail"),
                code:AppErrorCode::InvalidJsonFormat});
        },
    };
    Ok((out0, out1))
} // end of fn py_celery_deserialize_req


#[derive(Deserialize, Debug)]
pub(crate) enum PyCeleryRespStatus {
    STARTED, SUCCESS, ERROR,
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
    result: T
}

pub(crate) fn py_celery_reply_status(raw:&Vec<u8>)
     -> DefaultResult<PyCeleryRespStatus, AppError>
{
    let result = serde_json::from_slice::<PyCeleryRespPartialPayload>(raw);
    match result {
        Ok(payld) => Ok(payld.status),
        Err(e) => Err(AppError {detail:Some(e.to_string()),
                  code:AppErrorCode::InvalidJsonFormat})
    }
}

pub(super) fn py_celery_deserialize_reply<T>(raw:&Vec<u8>)
     -> DefaultResult<T, AppError>  where T: DeserializeOwned
{
    let result = serde_json::from_slice::<PyCeleryRespPayload<T>>(raw);
    match result {
        Ok(payld) => Ok(payld.result),
        Err(e) => Err(AppError {detail:Some(e.to_string()),
                  code:AppErrorCode::InvalidJsonFormat})
    }
}


#[derive(Serialize)]
struct PyCeleryReqMetadata {
    callbacks: Option<Vec<String>>,
    errbacks: Option<Vec<String>>,
    chain: Option<Vec<String>>,
    chord: Option<String>,
} // TODO, figure out the detail in `chain` and `chord` field

pub(super) fn py_celery_serialize<T:Serialize>(inner:T)
    -> DefaultResult<Vec<u8>, AppError>
{
    let args = JsnVal::Array(Vec::new());
    let kwargs = match serde_json::to_value(inner) {
        Ok(v) => v,
        Err(e) => {
            let detail = e.to_string() + ", src: py-celery-serialize";
            let ae = AppError { detail: Some(detail),
                code: AppErrorCode::InvalidJsonFormat, };
            return Err(ae);
        }
    };
    let metadata = {
        let md = PyCeleryReqMetadata {callbacks:None, errbacks:None,
                 chain:None, chord:None };
        serde_json::to_value(md).unwrap()
    };
    let top = JsnVal::Array(vec![args, kwargs, metadata]);
    Ok(top.to_string().into_bytes())
}

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
    #[derive(Deserialize, Debug)]
    struct TestData {live:bool, beat:u16}
    
    let data = br#"[["stock", "hay"], {"live":true, "beat":782}, {"callbacks":[]}]"#.to_vec();
    let result = py_celery_deserialize_req::<Vec<String>, TestData>(&data);
    assert!(result.is_ok());
    let (arg, kwarg) = result.unwrap();
    assert_eq!(arg.len(), 2);
    assert!(arg.contains(&"stock".to_string()));
    assert!(arg.contains(&"hay".to_string()));
    assert!(kwarg.live);
    assert_eq!(kwarg.beat , 782);
}

#[test]
fn test_pycelery_deserialize_error()
{
    #[derive(Deserialize, Debug)]
    struct TestData {tone:i8}
    
    let data = br#"[[], {}]"#.to_vec();
    let result = py_celery_deserialize_req::<Vec<i64>, TestData>(&data);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert_eq!(error.code, AppErrorCode::InvalidJsonFormat);
    assert_eq!(error.detail.unwrap().as_str(), "celery-de-topblk-incomplete");
    let data = br#"[[], {}, 567]"#.to_vec();
    let result = py_celery_deserialize_req::<Vec<i64>, TestData>(&data);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert_eq!(error.detail.unwrap().as_str(), "celery-de-arrayitem-mismatch");
}
