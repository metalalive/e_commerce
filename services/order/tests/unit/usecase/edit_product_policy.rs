use std::boxed::Box;
use std::sync::Arc;
use std::result::Result as DefaultResult;
use async_trait::async_trait;
use order::constant::ProductType;
use serde_json::from_str as deserialize_json;

use order::{AbstractRpcContext, AppRpcCfg, AbstractRpcClient, AppRpcReply,
    AppRpcClientReqProperty, AbsRpcClientCtx, AbsRpcServerCtx, AppSharedState, AppRpcRouteHdlrFn
};
use order::error::{AppError, AppErrorCode};
use order::api::web::dto::ProductPolicyDto;
use order::usecase::{EditProductPolicyUseCase, AppUseKsRPCreply, EditProductPolicyResult};

const UTEST_USR_PROF_ID: u32 = 99674;
struct UTestDummyRpcContext {}

#[async_trait]
impl AbsRpcClientCtx for UTestDummyRpcContext {
    async fn acquire(&self, _num_retry:u8) -> DefaultResult<Box<dyn AbstractRpcClient>, AppError>
    {
        let detail = "remote server down".to_string();
        let error = AppError{ code: AppErrorCode::RpcRemoteUnavail
                       , detail:Some(detail) };
        Err(error)
    }
}
#[async_trait]
impl AbsRpcServerCtx for UTestDummyRpcContext {
    async fn server_start(
        &self, shr_state:AppSharedState, route_hdlr: AppRpcRouteHdlrFn
    ) -> DefaultResult<(), AppError>
    {
        Err(AppError{ code: AppErrorCode::NotImplemented
            , detail:None })
    }
}

impl AbstractRpcContext for UTestDummyRpcContext {
    fn label(&self) -> &'static str { "unit-test" }
}

impl UTestDummyRpcContext {
    fn build(_cfg: &AppRpcCfg) -> DefaultResult<Box<dyn AbstractRpcContext> , AppError>
        where Self:Sized
    {
        let obj = Self{};
        Ok(Box::new(obj))
    }

    fn test_build () -> Arc<Box<dyn AbstractRpcContext>>
    {
        let cfg = AppRpcCfg::dummy;
        let obj = Self::build(&cfg).unwrap();
        Arc::new(obj)
    }
}

fn setup_data () -> Vec<ProductPolicyDto>
{
    let raw = r#"
        [
            {"product_id":22, "product_type":1, "auto_cancel_secs":600,
                "warranty_hours":1800 },
            {"product_id":168, "product_type":1, "auto_cancel_secs":610,
                "warranty_hours":1700 },
            {"product_id":79, "product_type":1, "auto_cancel_secs":630,
                "warranty_hours":1600 },
            {"product_id":19, "product_type":2, "auto_cancel_secs":660,
                "warranty_hours":1500 }
        ]
    "#;
    deserialize_json(raw).unwrap()
}

async fn mock_run_rpc_ok (_ctx: Arc<Box<dyn AbstractRpcContext>>, _prop: AppRpcClientReqProperty)
    -> AppUseKsRPCreply
{
    let raw = br#"
        {
        "item":[{"id":79},{"id":168},{"id":22}],
        "pkg":[{"id":19}]
        }
    "#; // bytes of raw string
    let res = AppRpcReply { body:raw.to_vec() };
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
    // println!("missing_product_ids : {:?}", missing_product_ids );
    assert_eq!(missing_product_ids.is_empty(), true);
}

async fn mock_run_rpc_remote_down (_ctx: Arc<Box<dyn AbstractRpcContext>>, _prop: AppRpcClientReqProperty)
    -> AppUseKsRPCreply
{
    // The pointer to trait object is accepted by trait method call
    let _ctx = _ctx.as_ref();
    let _ctx = _ctx.as_ref();
    let result = AbsRpcClientCtx::acquire(_ctx, 1).await;
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


async fn mock_run_rpc_reply_empty (_ctx: Arc<Box<dyn AbstractRpcContext>>, _prop: AppRpcClientReqProperty)
    -> AppUseKsRPCreply
{
    let raw = br#" {}  "#;
    let res = AppRpcReply { body:raw.to_vec() };
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
    _prop: AppRpcClientReqProperty) -> AppUseKsRPCreply
{
    let raw = br#"
        {
        "item":[{"id":79},{"id":22}],
        "pkg":[{"id":19}]
        }
    "#;
    let res = AppRpcReply { body:raw.to_vec() };
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
    assert_eq!(missing_product_ids, vec![(ProductType::Item,168)]);
}

