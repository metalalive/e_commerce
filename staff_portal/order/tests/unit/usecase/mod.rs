mod edit_product_policy;

use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use std::result::Result as DefaultResult;
use async_trait::async_trait;

use order::{
    AbstractRpcContext, AppRpcCfg, AppRpcClientReqProperty, AbstractRpcServer,
    AbstractRpcClient, AppRpcReply, AbsRpcClientCtx, AbsRpcServerCtx
};
use order::error::{AppError, AppErrorCode};
use order::usecase::initiate_rpc_request;

type TestAcquireClientResult = DefaultResult<Box<dyn AbstractRpcClient>, AppError>;
type TestAcquireServerResult = DefaultResult<Box<dyn AbstractRpcServer>, AppError>;

type TestClientPublishResult = TestAcquireClientResult;
type TestClientReplyResult = DefaultResult<AppRpcReply, AppError>;

struct MockRpcContext {
    _mock_acquire: Mutex<RefCell<Option<TestAcquireClientResult>>> ,
}
struct MockRpcHandler {
    _mock_client_publish: Option<TestClientPublishResult>,
    _mock_rreply: Option<TestClientReplyResult>,
}

#[async_trait]
impl AbsRpcClientCtx for MockRpcContext {
    async fn acquire (&self, _num_retry:u8) -> TestAcquireClientResult
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
}
#[async_trait]
impl AbsRpcServerCtx for MockRpcContext {
    async fn acquire (&self, _num_retry:u8) -> TestAcquireServerResult
    {
        Err(AppError{detail:None, code:AppErrorCode::NotImplemented })
    }
}

impl AbstractRpcContext for MockRpcContext
{
    fn label(&self) -> &'static str { "unit-test" }
} // end of impl AbstractRpcContext

impl MockRpcContext {
    fn build(cfg: &AppRpcCfg) -> Result<Box<dyn AbstractRpcContext> , AppError>
        where Self:Sized
    {
        let obj = Self::_build(cfg) ;
        Ok(Box::new(obj))
    }
    fn _build(_cfg: &AppRpcCfg) -> Self
    {
        Self{ _mock_acquire: Mutex::new(RefCell::new(None)) }
    }
    fn mock (&self, a:TestAcquireClientResult)
    {
        let guard = self._mock_acquire.lock().unwrap();
        let mut objref = guard.borrow_mut();
        *objref = Some(a);
    }
}


#[async_trait]
impl AbstractRpcClient for MockRpcHandler {
    async fn send_request(mut self:Box<Self>, _props:AppRpcClientReqProperty)
        -> TestClientPublishResult
    {
        if let Some(mocked) = self._mock_client_publish.take() {
            mocked
        } else {
            let detail = String::from("no mock object specified");
            Err(AppError{detail:Some(detail), code:AppErrorCode::Unknown })
        }
    }

    async fn receive_response(&mut self) -> TestClientReplyResult
    {
        if let Some(mocked) = self._mock_rreply.take() {
            mocked
        } else {
            let detail = String::from("no mock object specified");
            Err(AppError{detail:Some(detail), code:AppErrorCode::Unknown })
        }
    }
}
impl Default for MockRpcHandler {
    fn default() -> Self {
        Self { _mock_client_publish:None, _mock_rreply:None }
    }
}
impl MockRpcHandler {
    fn mock_c_pub(mut self, m:TestClientPublishResult) -> Self
    { self._mock_client_publish = Some(m); self }
    fn mock_reply(mut self, m:TestClientReplyResult) -> Self
    { self._mock_rreply = Some(m); self }
}



#[tokio::test]
async fn client_run_rpc_ok ()
{
    const UTEST_REPLY_BODY_SERIAL :&[u8; 8] = br#"achieved"#;
    let ctx : Arc<Box<dyn AbstractRpcContext>> = {
        let cfg = AppRpcCfg::dummy;
        let _ctx = MockRpcContext::_build(&cfg);
        let hdlr = {
            let h  = MockRpcHandler::default();
            let h2 = MockRpcHandler::default();
            let m2 = AppRpcReply { body: UTEST_REPLY_BODY_SERIAL.to_vec() };
            let h2 = h2.mock_reply(Ok(m2));
            h.mock_c_pub(Ok(Box::new(h2)))
        };
        let a: Box<dyn AbstractRpcClient> = Box::new(hdlr);
        _ctx.mock(Ok(a));
        Arc::new(Box::new(_ctx))
    }; // setup

    let prop = AppRpcClientReqProperty {
        retry: 4u8, msgbody: Vec::new(), route: "".to_string()
    };
    let actual = initiate_rpc_request(ctx, prop).await;
    assert_eq!(actual.is_ok(), true);
    let body = actual.unwrap().body;
    assert_eq!(body, UTEST_REPLY_BODY_SERIAL);
}


#[tokio::test]
async fn client_run_rpc_acquire_handler_failure ()
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
    let prop = AppRpcClientReqProperty {
        retry: 4u8, msgbody:  Vec::new(), route: "".to_string()
    };
    let actual = initiate_rpc_request(ctx, prop).await;
    assert_eq!(actual.is_err(), true);
    let error = actual.err().unwrap();
    assert_eq!(error.code, AppErrorCode::RpcRemoteUnavail);
    assert_eq!(error.detail, Some(ut_error_detail));
} // end of uc_run_rpc_acquire_handler_failure


#[tokio::test]
async fn client_run_rpc_publish_error ()
{
    let ut_error_detail = format!("some properties are invalid");
    let ctx : Arc<Box<dyn AbstractRpcContext>> = {
        let cfg = AppRpcCfg::dummy ;
        let _ctx = MockRpcContext::_build(&cfg);
        let hdlr = {
            let h = MockRpcHandler::default();
            let m1 = AppError { code: AppErrorCode::RpcPublishFailure,
                 detail: Some(ut_error_detail.clone()) };
            h.mock_c_pub(Err(m1))
        };
        let a: Box<dyn AbstractRpcClient> = Box::new(hdlr);
        _ctx.mock(Ok(a));
        Arc::new(Box::new(_ctx))
    }; // setup

    let prop = AppRpcClientReqProperty {
        retry: 4u8, msgbody: Vec::new(), route: "".to_string()
    };
    let actual = initiate_rpc_request(ctx, prop).await;
    assert_eq!(actual.is_err(), true);
    let error = actual.err().unwrap();
    assert_eq!(error.code, AppErrorCode::RpcPublishFailure);
    assert_eq!(error.detail, Some(ut_error_detail));
}


#[tokio::test]
async fn client_run_rpc_consume_reply_error ()
{
    let ut_error_detail = format!("job ID not found");
    let ctx : Arc<Box<dyn AbstractRpcContext>> = {
        let cfg = AppRpcCfg::dummy;
        let _ctx = MockRpcContext::_build(&cfg);
        let hdlr = {
            let h = MockRpcHandler::default();
            let h2 = MockRpcHandler::default();
            let m2 = AppError { code: AppErrorCode::RpcConsumeFailure,
                 detail: Some(ut_error_detail.clone()) };
            let h2 = h2.mock_reply(Err(m2));
            h.mock_c_pub(Ok(Box::new(h2)))
        };
        let a:Box<dyn AbstractRpcClient> = Box::new(hdlr);
        _ctx.mock(Ok(a));
        Arc::new(Box::new(_ctx))
    }; // setup

    let prop = AppRpcClientReqProperty {
        retry: 4u8, msgbody: Vec::new(), route: "".to_string()
    };
    let actual = initiate_rpc_request(ctx, prop).await;
    assert_eq!(actual.is_err(), true);
    let error = actual.err().unwrap();
    assert_eq!(error.code, AppErrorCode::RpcConsumeFailure);
    assert_eq!(error.detail, Some(ut_error_detail));
}

