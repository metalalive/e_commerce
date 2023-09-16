mod edit_product_policy;

use std::env;
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use std::result::Result as DefaultResult;
use async_trait::async_trait;
use tokio::task;

use order::{
    AppSharedState, AppConfig, AppBasepathCfg, AbstractRpcContext, AppRpcCfg,
    AbstractRpcServer, AbstractRpcClient, AbsRpcClientCtx, AbsRpcServerCtx,
    AppRpcClientReqProperty, AppRpcReply
};
use order::error::{AppError, AppErrorCode};
use order::usecase::{initiate_rpc_request, rpc_server_process};
use order::constant::{ENV_VAR_SERVICE_BASE_PATH, ENV_VAR_SYS_BASE_PATH};
use order::logging::AppLogContext;
use order::confidentiality::AbstractConfidentiality;

use crate::EXAMPLE_REL_PATH;

type TestAcquireResult<T> = DefaultResult<Box<T>, AppError>;
type TestAcquireClientResult = TestAcquireResult<dyn AbstractRpcClient>;
type TestAcquireServerResult = TestAcquireResult<dyn AbstractRpcServer>;

type TestClientPublishResult = TestAcquireClientResult;
type TestClientReplyResult = DefaultResult<AppRpcReply, AppError>;
type TestServerSubscribeResult = DefaultResult<AppRpcClientReqProperty, AppError> ;
type TestServerReplyResult = DefaultResult<(), AppError> ;

struct MockRpcContext {
    _mock_acquire_s: Mutex<RefCell<Option<TestAcquireServerResult>>>,
    _mock_acquire_c: Mutex<RefCell<Option<TestAcquireClientResult>>>,
}
struct MockRpcHandler {
    _mock_client_publish: Option<TestClientPublishResult>,
    _mock_server_subscribe: Option<TestServerSubscribeResult>,
    _mock_client_rreply: Option<TestClientReplyResult>,
    _mock_server_rreply: Option<TestServerReplyResult>,
}

#[async_trait]
impl AbsRpcClientCtx for MockRpcContext
{
    async fn acquire (&self, _num_retry:u8) -> TestAcquireClientResult
    { _do_acquire::<dyn AbstractRpcClient>(&self._mock_acquire_c, _num_retry).await }
}
#[async_trait]
impl AbsRpcServerCtx for MockRpcContext {
    async fn acquire (&self, _num_retry:u8) -> TestAcquireServerResult
    { _do_acquire::<dyn AbstractRpcServer>(&self._mock_acquire_s, _num_retry).await }
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
    fn _build(_cfg: &AppRpcCfg) -> Self {
        Self{
            _mock_acquire_c: Mutex::new(RefCell::new(None)),
            _mock_acquire_s: Mutex::new(RefCell::new(None))
        }
    }
    fn mock_c (&self, a:TestAcquireClientResult) {
        let guard = self._mock_acquire_c.lock().unwrap();
        let mut objref = guard.borrow_mut();
        *objref = Some(a);
    }
    fn mock_s (&self, a:TestAcquireServerResult) {
        let guard = self._mock_acquire_s.lock().unwrap();
        let mut objref = guard.borrow_mut();
        *objref = Some(a);
    }
} // end of impl MockRpcContext

async fn _do_acquire<T:?Sized> (
    _acquire:&Mutex<RefCell<Option<TestAcquireResult<T>>>>,
    _num_retry:u8) -> TestAcquireResult<T>
{
    if let Ok(guard) = _acquire.lock() {
        let mut objref = guard.borrow_mut();
        if let Some(mocked) = objref.take() {
            let xx = mocked;
            xx
        } else {
            let detail = String::from("no mock object specified");
            Err(AppError{detail:Some(detail), code:AppErrorCode::Unknown })
        }
    } else {
        let detail = String::from("lock failure on acquiring RPC handler");
        Err(AppError{detail:Some(detail), code:AppErrorCode::Unknown })
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
        if let Some(mocked) = self._mock_client_rreply.take() {
            mocked
        } else {
            let detail = String::from("no mock object specified");
            Err(AppError{detail:Some(detail), code:AppErrorCode::Unknown })
        }
    }
} // end of impl AbstractRpcClient

#[async_trait]
impl AbstractRpcServer for MockRpcHandler {
    async fn receive_request(&mut self) -> DefaultResult<AppRpcClientReqProperty, AppError>    
    {
        if let Some(mocked) = self._mock_server_subscribe.take() {
            mocked
        } else {
            let detail = String::from("no mock object specified");
            Err(AppError{detail:Some(detail), code:AppErrorCode::Unknown })
        }
    }
    async fn send_response(mut self:Box<Self>, _props:AppRpcReply) -> DefaultResult<(), AppError>
    {
        if let Some(mocked) = self._mock_server_rreply.take() {
            mocked
        } else {
            let detail = String::from("no mock object specified");
            Err(AppError{detail:Some(detail), code:AppErrorCode::Unknown })
        }
    }
}

impl Default for MockRpcHandler {
    fn default() -> Self {
        Self { _mock_client_publish:None, _mock_client_rreply:None,
            _mock_server_subscribe:None, _mock_server_rreply:None  }
    }
}
impl MockRpcHandler {
    fn mock_c_pub(mut self, m:TestClientPublishResult) -> Self
    { self._mock_client_publish = Some(m); self }
    fn mock_c_reply(mut self, m:TestClientReplyResult) -> Self
    { self._mock_client_rreply = Some(m); self }
    fn mock_s_sub(mut self, m:TestServerSubscribeResult) -> Self
    { self._mock_server_subscribe = Some(m); self }
    fn mock_s_reply(mut self, m:TestServerReplyResult) -> Self
    { self._mock_server_rreply = Some(m); self }
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
            let h2 = h2.mock_c_reply(Ok(m2));
            h.mock_c_pub(Ok(Box::new(h2)))
        };
        let a: Box<dyn AbstractRpcClient> = Box::new(hdlr);
        _ctx.mock_c(Ok(a));
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
        _ctx.mock_c(Err(a));
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
        _ctx.mock_c(Ok(a));
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
            let h2 = h2.mock_c_reply(Err(m2));
            h.mock_c_pub(Ok(Box::new(h2)))
        };
        let a:Box<dyn AbstractRpcClient> = Box::new(hdlr);
        _ctx.mock_c(Ok(a));
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


struct MockConfidential {}
impl AbstractConfidentiality for MockConfidential {
    fn try_get_payload(&self, _id:&str) -> DefaultResult<String, AppError> {
        Ok("unit-test".to_string())
    }
}

fn ut_setup_share_state() -> AppSharedState {
    let service_basepath = env::var(ENV_VAR_SERVICE_BASE_PATH).unwrap();
    let sys_basepath = env::var(ENV_VAR_SYS_BASE_PATH).unwrap();
    const CFG_FNAME: &str = "config_ok.json";
    let fullpath = service_basepath.clone() + EXAMPLE_REL_PATH + CFG_FNAME;
    let cfg = AppConfig {
        api_server: AppConfig::parse_from_file(fullpath).unwrap(),
        basepath: AppBasepathCfg { system:sys_basepath , service:service_basepath },
    };
    let logctx = AppLogContext::new(&cfg.basepath, &cfg.api_server.logging);
    let cfdntl:Box<dyn AbstractConfidentiality> = Box::new(MockConfidential{});
    AppSharedState::new(cfg, logctx, cfdntl)
}

async fn mock_rpc_request_handler (_r:AppRpcClientReqProperty, _ss:AppSharedState )
    -> AppRpcReply
{ // request and error handling
    AppRpcReply { body: br#"unit test replied"#.to_vec() }
}

#[tokio::test]
async fn server_run_rpc_ok ()
{
    let rctx : Arc<Box<dyn AbstractRpcContext>> = {
        let cfg = AppRpcCfg::dummy;
        let _ctx = MockRpcContext::_build(&cfg);
        let hdlr = {
            let h = MockRpcHandler::default();
            let m = AppRpcClientReqProperty { retry: 5, route: "app1.func23".to_string(),
                msgbody: br#"client request"#.to_vec() };
            h.mock_s_sub(Ok(m)).mock_s_reply(Ok(()))
        };
        let a:Box<dyn AbstractRpcServer> = Box::new(hdlr);
        _ctx.mock_s(Ok(a));
        Arc::new(Box::new(_ctx))
    };
    let shr_state = ut_setup_share_state();
    let result = rpc_server_process(shr_state, rctx, mock_rpc_request_handler).await;
    assert!(result.is_ok());
    let newtask = result.unwrap();
    let joinh = task::spawn(newtask);
    let result = joinh.await;
    let result = result.unwrap();
    assert!(result.is_ok());
    let _ = result.unwrap();
} // end of fn server_run_rpc_ok

#[tokio::test]
async fn server_run_rpc_acquire_error ()
{
    let rctx : Arc<Box<dyn AbstractRpcContext>> = {
        let cfg = AppRpcCfg::dummy;
        let _ctx = MockRpcContext::_build(&cfg);
        let e = AppError {code:AppErrorCode::ExceedingMaxLimit,
            detail:Some("unit-test-fail-acquire".to_string()) };
        _ctx.mock_s(Err(e));
        Arc::new(Box::new(_ctx))
    };
    let shr_state = ut_setup_share_state();
    let result = rpc_server_process(shr_state, rctx, mock_rpc_request_handler).await;
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.code, AppErrorCode::ExceedingMaxLimit);
        assert_eq!(e.detail.unwrap(), "unit-test-fail-acquire");
    }
}

#[tokio::test]
async fn server_run_rpc_receive_request_error ()
{
    let rctx : Arc<Box<dyn AbstractRpcContext>> = {
        let cfg = AppRpcCfg::dummy;
        let _ctx = MockRpcContext::_build(&cfg);
        let hdlr = {
            let h = MockRpcHandler::default();
            let e = AppError { code:AppErrorCode::RpcConsumeFailure,
                  detail:Some("unit-test-fail-subscribe".to_string()) };
            h.mock_s_sub(Err(e))
        };
        let a:Box<dyn AbstractRpcServer> = Box::new(hdlr);
        _ctx.mock_s(Ok(a));
        Arc::new(Box::new(_ctx))
    };
    let shr_state = ut_setup_share_state();
    let result = rpc_server_process(shr_state, rctx, mock_rpc_request_handler).await;
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.code, AppErrorCode::RpcConsumeFailure);
        assert_eq!(e.detail.unwrap(), "unit-test-fail-subscribe");
    }
}

#[tokio::test]
async fn server_run_rpc_send_response_error ()
{
    let rctx : Arc<Box<dyn AbstractRpcContext>> = {
        let cfg = AppRpcCfg::dummy;
        let _ctx = MockRpcContext::_build(&cfg);
        let hdlr = {
            let h = MockRpcHandler::default();
            let m = AppRpcClientReqProperty { retry: 5, route: "app1.func23".to_string(),
                msgbody: br#"client request"#.to_vec() };
            let e = AppError { code: AppErrorCode::RpcRemoteUnavail,
                  detail:Some("unit-test-fail-send-reply".to_string()) };
            h.mock_s_sub(Ok(m)).mock_s_reply(Err(e))
        };
        let a:Box<dyn AbstractRpcServer> = Box::new(hdlr);
        _ctx.mock_s(Ok(a));
        Arc::new(Box::new(_ctx))
    };
    let shr_state = ut_setup_share_state();
    let result = rpc_server_process(shr_state, rctx, mock_rpc_request_handler).await;
    assert!(result.is_ok());
    let newtask = result.unwrap();
    let joinh = task::spawn(newtask);
    let result = joinh.await;
    let result = result.unwrap();
    if let Err(e) = result {
        assert_eq!(e.code, AppErrorCode::RpcRemoteUnavail);
        assert_eq!(e.detail.unwrap(), "unit-test-fail-send-reply");
    }
}

