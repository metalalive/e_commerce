use std::boxed::Box;
use std::sync::Arc;
use std::result::Result as DefaultResult;
use async_trait::async_trait;
use serde_json::from_str as deserialize_json;

use order::{AbstractRpcContext, AppRpcTypeCfg, AppRpcCfg, AbstractRpcHandler, AppRpcReplyResult, AppRpcPublishProperty};
use order::error::{AppError, AppErrorCode};
use order::api::web::dto::ProductPolicyDto;
use order::usecase::{EditProductPolicyUseCase, AppUCrunRPCreturn, EditProductPolicyResult};

const UTEST_USR_PROF_ID: u32 = 99674;
struct UTestDummyRpcContext {}

#[async_trait]
impl AbstractRpcContext for UTestDummyRpcContext {
    fn label(&self) -> AppRpcTypeCfg
    { AppRpcTypeCfg::dummy }

    fn build(_cfg: &AppRpcCfg) -> DefaultResult<Box<dyn AbstractRpcContext> , AppError>
        where Self:Sized
    {
        let obj = Self{};
        Ok(Box::new(obj))
    }

    async fn acquire(&self, _num_retry:u8)
        -> DefaultResult<Arc<Box<dyn AbstractRpcHandler>>, AppError>
    {
        let detail = "remote server down".to_string();
        let error = AppError{ code: AppErrorCode::RpcRemoteUnavail
                       , detail:Some(detail) };
        Err(error)
    }
}

impl UTestDummyRpcContext {
    fn test_build () -> Arc<Box<dyn AbstractRpcContext>>
    {
        let cfg = AppRpcCfg { handler_type: AppRpcTypeCfg::dummy };
        let obj = Self::build(&cfg).unwrap();
        Arc::new(obj)
    }
}

fn setup_data () -> Vec<ProductPolicyDto>
{
    let raw = r#"
        [
            {"product_id":22, "auto_cancel_secs":600, "warranty_hours":1800, "async_stock_chk":true },
            {"product_id":168, "auto_cancel_secs":610, "warranty_hours":1700, "async_stock_chk":false },
            {"product_id":79, "auto_cancel_secs":630, "warranty_hours":1600, "async_stock_chk":true },
            {"product_id":19, "auto_cancel_secs":660, "warranty_hours":1500, "async_stock_chk":false }
        ]
    "#;
    deserialize_json(raw).unwrap()
}

async fn mock_run_rpc_ok (_ctx: Arc<Box<dyn AbstractRpcContext>>, _prop: AppRpcPublishProperty)
    -> AppUCrunRPCreturn
{
    let raw = r#"
        {
        "item":[{"id":79},{"id":168},{"id":22}],
        "pkg":[{"id":19}]
        }
    "#;
    let res = AppRpcReplyResult { body: raw.to_string() };
    Ok(res)
}

#[tokio::test]
async fn check_product_existence_ok ()
{
    let data = setup_data();
    let rpc_ctx = UTestDummyRpcContext::test_build();
    let result = EditProductPolicyUseCase::check_product_existence(
        &data, rpc_ctx, mock_run_rpc_ok, UTEST_USR_PROF_ID
    ).await;
    assert_eq!(result.is_ok(), true);
    let missing_product_ids = result.unwrap();
    assert_eq!(missing_product_ids.is_empty(), true);
}

async fn mock_run_rpc_remote_down (_ctx: Arc<Box<dyn AbstractRpcContext>>, _prop: AppRpcPublishProperty)
    -> AppUCrunRPCreturn
{
    let result = _ctx.acquire(1).await;
    let out = result.err().unwrap();
    Err(out)
}

#[tokio::test]
async fn check_product_existence_rpc_error ()
{
    let data = setup_data();
    let rpc_ctx = UTestDummyRpcContext::test_build();
    let actual = EditProductPolicyUseCase::check_product_existence(
        &data, rpc_ctx, mock_run_rpc_remote_down, UTEST_USR_PROF_ID
    ).await;
    assert_eq!(actual.is_err(), true);
    let (result, msg) = actual.err().unwrap();
    assert_eq!(result, EditProductPolicyResult::Other(AppErrorCode::RpcRemoteUnavail));
    assert_eq!(msg, "remote server down");
}


async fn mock_run_rpc_reply_empty (_ctx: Arc<Box<dyn AbstractRpcContext>>, _prop: AppRpcPublishProperty)
    -> AppUCrunRPCreturn
{
    let raw = r#" {}  "#;
    let res = AppRpcReplyResult { body: raw.to_string() };
    Ok(res)
}

#[tokio::test]
async fn check_product_existence_rpc_reply_invalid_format ()
{
    let data = setup_data();
    let rpc_ctx = UTestDummyRpcContext::test_build();
    let actual = EditProductPolicyUseCase::check_product_existence(
        &data, rpc_ctx, mock_run_rpc_reply_empty, UTEST_USR_PROF_ID
    ).await;
    assert_eq!(actual.is_err(), true);
    let (result, _) = actual.err().unwrap();
    assert_eq!(result, EditProductPolicyResult::Other(AppErrorCode::RpcRemoteInvalidReply));
}


async fn mock_run_rpc_nonexist_found (
    _ctx: Arc<Box<dyn AbstractRpcContext>>,
    _prop: AppRpcPublishProperty) -> AppUCrunRPCreturn
{
    let raw = r#"
        {
        "item":[{"id":79},{"id":19},{"id":22}],
        "pkg":[]
        }
    "#;
    let res = AppRpcReplyResult { body: raw.to_string() };
    Ok(res)
}

#[tokio::test]
async fn check_product_existence_found_nonexist_item ()
{
    let data = setup_data();
    let rpc_ctx = UTestDummyRpcContext::test_build();
    let result = EditProductPolicyUseCase::check_product_existence(
        &data, rpc_ctx, mock_run_rpc_nonexist_found, UTEST_USR_PROF_ID
    ).await;
    assert_eq!(result.is_ok(), true);
    let missing_product_ids = result.unwrap();
    assert_eq!(missing_product_ids, vec![168]);
}

