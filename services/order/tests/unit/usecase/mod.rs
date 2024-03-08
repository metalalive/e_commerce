mod edit_product_policy;
mod edit_product_price;
mod stock_level;
mod manage_order;

use std::vec;
use std::boxed::Box;
use std::future::Future;
use std::pin::Pin;
use std::cell::{RefCell, Cell};
use std::sync::{Arc, Mutex};
use std::result::Result as DefaultResult;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use tokio::sync::Mutex as AsyncMutex;

use order::{
    AppSharedState, AbstractRpcContext, AppRpcCfg, AbstractRpcClient, AppRpcReply,
    AbsRpcClientCtx, AbsRpcServerCtx, AppRpcClientReqProperty, AppRpcRouteHdlrFn
};
use order::api::dto::ShippingMethod;
use order::api::rpc::dto::{
    OrderPaymentUpdateDto, OrderPaymentUpdateErrorDto, StockLevelReturnDto, StockReturnErrorDto
};
use order::error::{AppError, AppErrorCode};
use order::model::{
    StockLevelModelSet, ProductStockIdentity, OrderLineModel, BillingModel,
    ShippingModel, OrderLineModelSet, OrderLineIdentity, OrderReturnModel,
    ContactModel, ShippingOptionModel
};
use order::repository::{
    AbsOrderRepo, AbsOrderStockRepo, AppStockRepoReserveUserFunc, AppStockRepoReserveReturn,
    AppOrderRepoUpdateLinesUserFunc, AppOrderFetchRangeCallback, AppStockRepoReturnUserFunc,
    AbsOrderReturnRepo
};
use order::usecase::initiate_rpc_request;

use crate::{ut_setup_share_state, MockConfidential};

struct MockStockRepo {
    _mocked_save_r:  DefaultResult<(), AppError>,
    _mocked_fetch_r: DefaultResult<StockLevelModelSet, AppError>,
    _mocked_stk_return: AsyncMutex<Cell<Vec<DefaultResult<Vec<StockReturnErrorDto>, AppError>>>>,
}
struct MockOrderRepo {
    _mocked_stock_save:  DefaultResult<(), AppError>,
    _mocked_stock_fetch: DefaultResult<StockLevelModelSet, AppError>,
    _mocked_stock_return: Mutex<Cell<Vec<DefaultResult<Vec<StockReturnErrorDto>, AppError>>>>,
    _mocked_ol_sets: AsyncMutex<Cell<Vec<OrderLineModelSet>>>,
    _mocked_olines :  AsyncMutex<Vec<OrderLineModel>>,
    _mock_oids_ctime: AsyncMutex<Vec<String>>,
    _mock_usr_id: Option<u32>,
    _mock_ctime: Option<DateTime<FixedOffset>>,
}
struct MockOrderReturnRepo {
     _mocked_fetched_returns: AsyncMutex<Option<DefaultResult<Vec<OrderReturnModel>, AppError>>> ,
     _mocked_fetched_oid_returns: AsyncMutex<Option<DefaultResult<Vec<(String,OrderReturnModel)>, AppError>>> ,
     _mocked_save_result: AsyncMutex<Option<DefaultResult<usize, AppError>>> ,
}

#[async_trait]
impl AbsOrderStockRepo for MockStockRepo {
    async fn fetch(&self, _pids:Vec<ProductStockIdentity>) -> DefaultResult<StockLevelModelSet, AppError>
    { self._mocked_fetch_r.clone() }
    async fn save(&self, _slset:StockLevelModelSet) -> DefaultResult<(), AppError>
    { self._mocked_save_r.clone() }
    async fn try_reserve(&self, _cb: AppStockRepoReserveUserFunc,
                         _order_req: &OrderLineModelSet) -> AppStockRepoReserveReturn
    {
        let e = AppError { code: AppErrorCode::NotImplemented, detail: None };
        Err(Err(e))
    }
    async fn try_return(&self, _cb: AppStockRepoReturnUserFunc, _data: StockLevelReturnDto )
        -> DefaultResult<Vec<StockReturnErrorDto>, AppError>
    {
        let mut g = self._mocked_stk_return.lock().await;
        let returns = g.get_mut();
        if returns.is_empty() {
            let detail = format!("MockStockRepo::try_return");
            Err(AppError { code: AppErrorCode::InvalidInput, detail: Some(detail) })
        } else {
            returns.remove(0)
        }
    }
}

#[async_trait]
impl AbsOrderRepo for MockOrderRepo {
    fn stock(&self) -> Arc<Box<dyn AbsOrderStockRepo>> {
        let mock_return = if let Ok(mut g) = self._mocked_stock_return.lock() {
            let v = g.get_mut();
            if v.is_empty() {
                vec![]
            } else {
                vec![v.remove(0)]
            }
        } else { vec![] };
        let obj = MockStockRepo {
            _mocked_save_r:  self._mocked_stock_save.clone(),
            _mocked_fetch_r: self._mocked_stock_fetch.clone(),
            _mocked_stk_return: AsyncMutex::new(Cell::new(mock_return)),
        };
        Arc::new(Box::new(obj))
    }

    async fn save_contact(&self, _oid:&str, _bl:BillingModel, _sh:ShippingModel)
        -> DefaultResult<(), AppError>
    {
        Err(AppError { code: AppErrorCode::NotImplemented, detail: None })
    }
    async fn fetch_all_lines(&self, _oid:String) -> DefaultResult<Vec<OrderLineModel>, AppError>
    {
        let mut g = self._mocked_olines.lock().await;
        if g.is_empty() {
            let detail = format!("MockOrderRepo::fetch_all_lines");
            Err(AppError { code: AppErrorCode::InvalidInput, detail: Some(detail) })
        } else {
            Ok(g.drain(0..).collect())
        }
    }
    async fn fetch_billing(&self, _oid:String) -> DefaultResult<BillingModel, AppError>
    {
        Err(AppError { code: AppErrorCode::NotImplemented, detail: None })
    }
    async fn fetch_shipping(&self, _oid:String) -> DefaultResult<ShippingModel, AppError>
    {
        let contact = ContactModel { first_name: "Llama".to_string(),
            last_name: "Ant".to_string(), emails: vec![], phones: vec![] };
        let option = vec![ShippingOptionModel {seller_id:123, method:ShippingMethod::FedEx }];
        let obj = ShippingModel { contact, address:None, option };
        Ok(obj)
    }
    async fn update_lines_payment(&self, _data:OrderPaymentUpdateDto,
                                  _cb:AppOrderRepoUpdateLinesUserFunc)
        -> DefaultResult<OrderPaymentUpdateErrorDto, AppError>
    {
        Err(AppError { code: AppErrorCode::NotImplemented, detail: None })
    }
    async fn fetch_lines_by_rsvtime(&self, _time_start: DateTime<FixedOffset>,
                                  _time_end: DateTime<FixedOffset>,
                                  usr_cb: AppOrderFetchRangeCallback )
        -> DefaultResult<(), AppError>
    {
        let mut g = self._mocked_ol_sets.lock().await;
        let ol_sets = g.get_mut();
        while let Some(ms) = ol_sets.pop() {
            usr_cb(self, ms).await?
        }
        Ok(())
    } 
    async fn fetch_lines_by_pid(&self, _oid:&str, _pids:Vec<OrderLineIdentity>)
        -> DefaultResult<Vec<OrderLineModel>, AppError>
    {
        let mut g = self._mocked_olines.lock().await;
        if g.is_empty() {
            let detail = format!("MockOrderRepo::fetch_lines_by_pid");
            Err(AppError { code: AppErrorCode::InvalidInput, detail: Some(detail) })
        } else {
            let dst = g.drain(0..).collect::<Vec<OrderLineModel>>();
            Ok(dst)
        }
    }
    async fn fetch_ids_by_created_time(&self, _start: DateTime<FixedOffset>,
                                       _end: DateTime<FixedOffset>)
        -> DefaultResult<Vec<String>, AppError>
    {
        let mut g = self._mock_oids_ctime.lock().await;
        if g.is_empty() {
            let detail = format!("MockOrderRepo::fetch_ids_by_created_time");
            Err(AppError { code: AppErrorCode::InvalidInput, detail: Some(detail) })
        } else {
            Ok(g.drain(..).collect())
        }
    }
    async fn owner_id(&self, _order_id:&str) -> DefaultResult<u32, AppError>
    {
        if let Some(usr_id) = self._mock_usr_id.as_ref() {
            Ok(usr_id.clone())
        } else {
            let detail = format!("MockOrderRepo::owner_id");
            Err(AppError { code: AppErrorCode::InvalidInput, detail: Some(detail) })
        }
    }
    async fn created_time(&self, _order_id:&str) -> DefaultResult<DateTime<FixedOffset>, AppError>
    {
        if let Some(create_time) = self._mock_ctime.as_ref() {
            Ok(create_time.clone())
        } else {
            let detail = format!("MockOrderRepo::created_time");
            Err(AppError { code: AppErrorCode::InvalidInput, detail: Some(detail) })
        }
    }
    async fn cancel_unpaid_last_time(&self) -> DefaultResult<DateTime<FixedOffset>, AppError>
    {
        let t = DateTime::parse_from_rfc3339("1999-07-31T23:59:58+09:00").unwrap();
        Ok(t)
    }
    async fn cancel_unpaid_time_update(&self) -> DefaultResult<(), AppError>
    { Ok(()) }
} // end of impl MockOrderRepo

impl MockOrderRepo {
    fn build(stk_save_r:DefaultResult<(), AppError>,
             stk_fetch_r:DefaultResult<StockLevelModelSet, AppError>,
             stk_returns: Vec<DefaultResult<Vec<StockReturnErrorDto>, AppError>>,
             ol_sets: Vec<OrderLineModelSet>,
             olines : Vec<OrderLineModel>,
             oids_ctime: Vec<String>,
             usr_id: Option<u32>,
             create_time: Option<DateTime<FixedOffset>>,
        ) -> Self
    {
        Self{_mocked_stock_save: stk_save_r,
             _mocked_stock_fetch: stk_fetch_r,
             _mocked_stock_return: Mutex::new(Cell::new(stk_returns)),
             _mocked_ol_sets: AsyncMutex::new(Cell::new(ol_sets)),
             _mocked_olines : AsyncMutex::new(olines),
             _mock_oids_ctime: AsyncMutex::new(oids_ctime),
             _mock_ctime: create_time,
             _mock_usr_id: usr_id,
        }
    }
}

#[async_trait]
impl AbsOrderReturnRepo for MockOrderReturnRepo {
    async fn fetch_by_pid(&self, _oid:&str, _pids:Vec<OrderLineIdentity>)
        -> DefaultResult<Vec<OrderReturnModel>, AppError>
    {
        let mut g = self._mocked_fetched_returns.lock().await;
        if let Some(v) = g.take() {
            v
        } else {
            let detail = format!("MockOrderRepo::fetch_by_pid");
            Err(AppError { code: AppErrorCode::InvalidInput, detail: Some(detail) })
        }
    }
    async fn fetch_by_created_time(&self, _start:DateTime<FixedOffset>, _end:DateTime<FixedOffset>)
        -> DefaultResult<Vec<(String, OrderReturnModel)>, AppError>
    {
        let mut g = self._mocked_fetched_oid_returns.lock().await;
        if let Some(v) = g.take() {
            v
        } else {
            let detail = format!("MockOrderRepo::fetch_by_created_time");
            Err(AppError { code: AppErrorCode::InvalidInput, detail: Some(detail) })
        }
    }
    async fn fetch_by_oid_ctime(&self, oid:&str, _start: DateTime<FixedOffset>, _end: DateTime<FixedOffset>)
        -> DefaultResult<Vec<OrderReturnModel>, AppError>
    { // use the same mock variable internally, TODO, improve the test readability
        self.fetch_by_pid(oid, vec![]).await
    }
    async fn create(&self, _oid:&str, _reqs:Vec<OrderReturnModel>)
        -> DefaultResult<usize, AppError>
    {
        let mut g = self._mocked_save_result.lock().await;
        if let Some(v) = g.take() {
            v
        } else {
            let detail = format!("MockOrderRepo::fetch_by_pid");
            Err(AppError { code: AppErrorCode::InvalidInput, detail: Some(detail) })
        }
    }
} // end of impl MockOrderReturnRepo

impl MockOrderReturnRepo {
    fn build( fetched_returns: DefaultResult<Vec<OrderReturnModel>, AppError>,
              fetched_oid_returns: DefaultResult<Vec<(String,OrderReturnModel)>, AppError> ,
              save_result: DefaultResult<usize, AppError>
        ) -> Self
    {
        Self {
            _mocked_fetched_returns: AsyncMutex::new(Some(fetched_returns)),
            _mocked_fetched_oid_returns: AsyncMutex::new(Some(fetched_oid_returns)),
            _mocked_save_result: AsyncMutex::new(Some(save_result))
        }
    }
}

// ---------- RPC ----------

type TestAcquireResult<T> = DefaultResult<Box<T>, AppError>;
type TestAcquireClientResult = TestAcquireResult<dyn AbstractRpcClient>;

type TestClientPublishResult = TestAcquireClientResult;
type TestClientReplyResult = DefaultResult<AppRpcReply, AppError>;
type TestServerSubscribeResult = DefaultResult<AppRpcClientReqProperty, AppError> ;
type TestServerReplyResult = DefaultResult<(), AppError> ;

struct MockRpcContext {
    _mock_srv_recv_req: AsyncMutex<Option<AppRpcClientReqProperty>>,
    _mock_acquire_c: Mutex<RefCell<Option<TestAcquireClientResult>>>,
}
struct MockRpcHandler {
    _mock_client_publish: Option<TestClientPublishResult>,
    _mock_client_rreply: Option<TestClientReplyResult>,
}

#[async_trait]
impl AbsRpcClientCtx for MockRpcContext
{
    async fn acquire(&self, _num_retry:u8) -> TestAcquireClientResult
    {
        if let Ok(guard) = self._mock_acquire_c.lock() {
            let mut objref = guard.borrow_mut();
            if let Some(mocked) = objref.take() {
                mocked
            } else {
                Err(AppError{detail:Some("client-acquire-empty".to_string()),
                    code:AppErrorCode::Unknown })
            }
        } else {
            let detail = String::from("lock-failure");
            Err(AppError{detail:Some(detail), code:AppErrorCode::Unknown })
        }
    }
}

#[async_trait]
impl AbsRpcServerCtx for MockRpcContext
{
    async fn server_start(
        &self, shr_state:AppSharedState, route_hdlr: AppRpcRouteHdlrFn
    ) -> DefaultResult<(), AppError>
    {
        let mut guard = self._mock_srv_recv_req.lock().await;
        assert!(guard.is_some());
        if let Some(req) = guard.take() {
            let _resp = route_hdlr(req, shr_state).await ?;
            Ok(())
        } else {
            Err(AppError{detail:Some("server-recv-empty-req".to_string()),
                code:AppErrorCode::Unknown })
        }
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
    fn _build(_cfg: &AppRpcCfg) -> Self {
        Self{
            _mock_acquire_c: Mutex::new(RefCell::new(None)),
            _mock_srv_recv_req: AsyncMutex::new(None)
        }
    }
    fn mock_c(&self, a:TestAcquireClientResult)
    {
        let guard = self._mock_acquire_c.lock().unwrap();
        let mut objref = guard.borrow_mut();
        *objref = Some(a);
    }
    async fn mock_recv_req(&self, req: AppRpcClientReqProperty)
    {
        let mut guard = self._mock_srv_recv_req.lock().await;
        *guard = Some(req);
        drop(guard);
        let guard = self._mock_srv_recv_req.lock().await;
        assert!(guard.is_some());
    }
} // end of impl MockRpcContext


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


impl Default for MockRpcHandler {
    fn default() -> Self {
        Self { _mock_client_publish:None, _mock_client_rreply:None }
    }
}
impl MockRpcHandler {
    fn mock_c_pub(mut self, m:TestClientPublishResult) -> Self
    { self._mock_client_publish = Some(m); self }
    fn mock_c_reply(mut self, m:TestClientReplyResult) -> Self
    { self._mock_client_rreply = Some(m); self }
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
        start_time: DateTime::parse_from_rfc3339("2022-08-31T15:02:35+08:00").unwrap(),
        msgbody: Vec::new(), route: "".to_string()
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
        start_time: DateTime::parse_from_rfc3339("2022-08-31T15:02:35+08:00").unwrap(),
        msgbody:  Vec::new(), route: "".to_string()
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
        start_time: DateTime::parse_from_rfc3339("2022-08-31T15:02:35+08:00").unwrap(),
        msgbody: Vec::new(), route: "".to_string()
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
        start_time: DateTime::parse_from_rfc3339("2022-08-31T15:02:35+08:00").unwrap(),
        msgbody: Vec::new(), route: "".to_string()
    };
    let actual = initiate_rpc_request(ctx, prop).await;
    assert_eq!(actual.is_err(), true);
    let error = actual.err().unwrap();
    assert_eq!(error.code, AppErrorCode::RpcConsumeFailure);
    assert_eq!(error.detail, Some(ut_error_detail));
}


async fn mock_route_handler_ok(req:AppRpcClientReqProperty, _ss:AppSharedState )
    -> DefaultResult<Vec<u8>, AppError>
{ // request and error handling
    assert_eq!(req.route.as_str(), "app1.func23");
    Ok(br#"unit test replied"#.to_vec())
}
fn mock_route_handler_ok_wrapper(req:AppRpcClientReqProperty, shr_state: AppSharedState)
    -> Pin<Box<dyn Future<Output=DefaultResult<Vec<u8>, AppError>> + Send>> 
{
    Box::pin(async move {
        mock_route_handler_ok(req, shr_state).await
    })
}
#[tokio::test]
async fn server_run_rpc_ok ()
{
    let rctx : Arc<Box<dyn AbstractRpcContext>> = {
        let cfg = AppRpcCfg::dummy;
        let _ctx = MockRpcContext::_build(&cfg);
        let m = AppRpcClientReqProperty {
            start_time: DateTime::parse_from_rfc3339("2022-08-31T15:02:35+08:00").unwrap(),
            route: "app1.func23".to_string(), msgbody: br#"client request"#.to_vec()
        };
        _ctx.mock_recv_req(m).await;
        Arc::new(Box::new(_ctx))
    };
    let shr_state = ut_setup_share_state("config_ok.json", Box::new(MockConfidential{}));
    let result = rctx.server_start(shr_state, mock_route_handler_ok_wrapper).await;
    assert!(result.is_ok());
} // end of fn server_run_rpc_ok


async fn mock_route_handler_fail(req:AppRpcClientReqProperty, _ss:AppSharedState )
    -> DefaultResult<Vec<u8>, AppError>
{ // request and error handling
    assert_eq!(req.route.as_str(), "app2.func56");
    let e = AppError{
        code:AppErrorCode::RpcConsumeFailure,
        detail:Some("unit-test-route-handle-fail".to_string())
    };
    Err(e)
}
fn mock_route_handler_fail_wrapper(req:AppRpcClientReqProperty, shr_state: AppSharedState)
    -> Pin<Box<dyn Future<Output=DefaultResult<Vec<u8>, AppError>> + Send>> 
{
    Box::pin(async move {
        mock_route_handler_fail(req, shr_state).await
    })
}
#[tokio::test]
async fn server_run_rpc_receive_request_error ()
{
    let shr_state = ut_setup_share_state("config_ok.json", Box::new(MockConfidential{}));
    let rctx : Arc<Box<dyn AbstractRpcContext>> = {
        let cfg = AppRpcCfg::dummy;
        let _ctx = MockRpcContext::_build(&cfg);
        let m = AppRpcClientReqProperty {
            start_time: DateTime::parse_from_rfc3339("2022-07-24T15:02:58+04:00").unwrap(),
            route: "app2.func56".to_string(), msgbody: "another request".as_bytes().to_vec()
        };
        _ctx.mock_recv_req(m).await;
        Arc::new(Box::new(_ctx))
    };
    let result = rctx.server_start(shr_state, mock_route_handler_fail_wrapper).await;
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.detail.as_ref().unwrap(), "unit-test-route-handle-fail");
        assert_eq!(e.code, AppErrorCode::RpcConsumeFailure);
    }
}
