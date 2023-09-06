mod edit_product_policy;

use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use std::result::Result as DefaultResult;
use async_trait::async_trait;

use order::{
    AbstractRpcContext, AppRpcCfg, AppRpcPublishProperty,
    AbstractRpcHandler, AppRpcPublishedResult, AppRpcConsumeResult,
    AppRpcConsumeProperty
};
use order::error::{AppError, AppErrorCode};
use order::usecase::run_rpc;

type TestRpcAcquireReturn = DefaultResult<Arc<Box<dyn AbstractRpcHandler>>, AppError>;
type TestRpcPublishReturn = DefaultResult<AppRpcPublishedResult, AppError>;
type TestRpcConsumeReturn = DefaultResult<AppRpcConsumeResult, AppError>;

struct MockRpcContext {
    _mock_acquire: Mutex<RefCell<Option<TestRpcAcquireReturn>>> ,
}
struct MockRpcHandler {
    _mock_publish: Option<TestRpcPublishReturn>,
    _mock_consume: Option<TestRpcConsumeReturn>,
}

#[async_trait]
impl AbstractRpcContext for MockRpcContext
{
    fn label(&self) -> &'static str { "unit-test" }

    async fn acquire (&self, _num_retry:u8) -> TestRpcAcquireReturn
    {
        if let Ok(guard) = self._mock_acquire.lock() {
            let mut objref = guard.borrow_mut();
            if let Some(mocked) = objref.take() {
                mocked
            } else {
                let detail = String::from("no mock object specified");
                Err(AppError{detail:Some(detail), code:AppErrorCode::Unknown })
            }
        } else {
            let detail = String::from("lock failure on acquiring RPC handler");
            Err(AppError{detail:Some(detail), code:AppErrorCode::Unknown })
        }
    }
} // end of impl AbstractRpcContext
impl MockRpcContext {
    fn build(cfg: &AppRpcCfg) -> Result<Box<dyn AbstractRpcContext> , AppError>
        where Self:Sized
    {
        let obj = Self::_build(cfg) ;
        Ok(Box::new(obj))
    }
    fn _build(cfg: &AppRpcCfg) -> Self
    {
        Self{ _mock_acquire: Mutex::new(RefCell::new(None)) }
    }
    fn mock (&self, a:TestRpcAcquireReturn)
    {
        let guard = self._mock_acquire.lock().unwrap();
        let mut objref = guard.borrow_mut();
        *objref = Some(a);
    }
}


#[async_trait]
impl AbstractRpcHandler for MockRpcHandler {
    async fn publish(&mut self, _props:AppRpcPublishProperty) -> TestRpcPublishReturn
    {
        if let Some(mocked) = self._mock_publish.take() {
            mocked
        } else {
            let detail = String::from("no mock object specified");
            Err(AppError{detail:Some(detail), code:AppErrorCode::Unknown })
        }
    }

    async fn consume(&mut self, _props:AppRpcConsumeProperty) -> TestRpcConsumeReturn
    {
        if let Some(mocked) = self._mock_consume.take() {
            mocked
        } else {
            let detail = String::from("no mock object specified");
            Err(AppError{detail:Some(detail), code:AppErrorCode::Unknown })
        }
    }
}
impl Default for MockRpcHandler {
    fn default() -> Self {
        Self { _mock_publish:None, _mock_consume:None }
    }
}
impl MockRpcHandler {
    fn mock_pub(mut self, m:TestRpcPublishReturn) -> Self
    { self._mock_publish = Some(m); self }
    fn mock_con(mut self, m:TestRpcConsumeReturn) -> Self
    { self._mock_consume = Some(m); self }
}



#[tokio::test]
async fn uc_run_rpc_ok ()
{
    const UTEST_REPLY_BODY_SERIAL : &'static str = "achieved";
    let ctx : Arc<Box<dyn AbstractRpcContext>> = {
        let cfg = AppRpcCfg::dummy;
        let _ctx = MockRpcContext::_build(&cfg);
        let hdlr = {
            let h = MockRpcHandler::default();
            let m1 = AppRpcPublishedResult {
                reply_route: "rpc.id.9G382re".to_string(), job_id: "m31".to_string()
            };
            let m2 = AppRpcConsumeResult { properties:None,
                body: UTEST_REPLY_BODY_SERIAL.to_string() };
            h.mock_pub(Ok(m1)).mock_con(Ok(m2))
        };
        let a:Arc<Box<dyn AbstractRpcHandler>> = Arc::new(Box::new(hdlr));
        _ctx.mock(Ok(a));
        Arc::new(Box::new(_ctx))
    }; // setup

    let prop = AppRpcPublishProperty {
        retry: 4u8, msgbody: "".to_string(), route: "".to_string()
    };
    let actual = run_rpc(ctx, prop).await;
    assert_eq!(actual.is_ok(), true);
    let body = actual.unwrap().body;
    assert_eq!(body, UTEST_REPLY_BODY_SERIAL);
} // end of uc_run_rpc_ok


#[tokio::test]
async fn uc_run_rpc_acquire_handler_failure ()
{
    let ut_error_detail = format!("unit-test connection timeout");
    let ctx : Arc<Box<dyn AbstractRpcContext>> = {
        let cfg = AppRpcCfg::dummy;
        let _ctx = MockRpcContext::_build(&cfg);
        let a = AppError { code: AppErrorCode::RpcRemoteUnavail,
             detail: Some(ut_error_detail.clone()) };
        _ctx.mock(Err(a));
        Arc::new(Box::new(_ctx))
    }; // setup
    let prop = AppRpcPublishProperty {
        retry: 4u8, msgbody: "".to_string(), route: "".to_string()
    };
    let actual = run_rpc(ctx, prop).await;
    assert_eq!(actual.is_err(), true);
    let error = actual.err().unwrap();
    assert_eq!(error.code, AppErrorCode::RpcRemoteUnavail);
    assert_eq!(error.detail, Some(ut_error_detail));
} // end of uc_run_rpc_acquire_handler_failure


#[tokio::test]
async fn uc_run_rpc_publish_error ()
{
    let ut_error_detail = format!("some properties are invalid");
    let ctx : Arc<Box<dyn AbstractRpcContext>> = {
        let cfg = AppRpcCfg::dummy ;
        let _ctx = MockRpcContext::_build(&cfg);
        let hdlr = {
            let h = MockRpcHandler::default();
            let m1 = AppError { code: AppErrorCode::RpcPublishFailure,
                 detail: Some(ut_error_detail.clone()) };
            h.mock_pub(Err(m1))
        };
        let a:Arc<Box<dyn AbstractRpcHandler>> = Arc::new(Box::new(hdlr));
        _ctx.mock(Ok(a));
        Arc::new(Box::new(_ctx))
    }; // setup

    let prop = AppRpcPublishProperty {
        retry: 4u8, msgbody: "".to_string(), route: "".to_string()
    };
    let actual = run_rpc(ctx, prop).await;
    assert_eq!(actual.is_err(), true);
    let error = actual.err().unwrap();
    assert_eq!(error.code, AppErrorCode::RpcPublishFailure);
    assert_eq!(error.detail, Some(ut_error_detail));
}


#[tokio::test]
async fn uc_run_rpc_consume_reply_error ()
{
    let ut_error_detail = format!("job ID not found");
    let ctx : Arc<Box<dyn AbstractRpcContext>> = {
        let cfg = AppRpcCfg::dummy;
        let _ctx = MockRpcContext::_build(&cfg);
        let hdlr = {
            let h = MockRpcHandler::default();
            let m1 = AppRpcPublishedResult {
                reply_route: "rpc.id.9G382re".to_string(), job_id: "m31".to_string()
            };
            let m2 = AppError { code: AppErrorCode::RpcConsumeFailure,
                 detail: Some(ut_error_detail.clone()) };
            h.mock_pub(Ok(m1)).mock_con(Err(m2))
        };
        let a:Arc<Box<dyn AbstractRpcHandler>> = Arc::new(Box::new(hdlr));
        _ctx.mock(Ok(a));
        Arc::new(Box::new(_ctx))
    }; // setup

    let prop = AppRpcPublishProperty {
        retry: 4u8, msgbody: "".to_string(), route: "".to_string()
    };
    let actual = run_rpc(ctx, prop).await;
    assert_eq!(actual.is_err(), true);
    let error = actual.err().unwrap();
    assert_eq!(error.code, AppErrorCode::RpcConsumeFailure);
    assert_eq!(error.detail, Some(ut_error_detail));
}

