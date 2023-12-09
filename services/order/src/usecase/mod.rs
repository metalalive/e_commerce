mod edit_product_policy;
mod edit_product_price;
mod stock_level;
mod manage_order;

use std::sync::Arc;
use std::boxed::Box;
use std::marker::Send;
use std::future::Future;
use std::result::Result as DefaultResult;

pub use edit_product_policy::{EditProductPolicyUseCase, EditProductPolicyResult};
pub use edit_product_price::EditProductPriceUseCase;
pub use stock_level::StockLevelUseCase;
pub use manage_order::{
    CreateOrderUseCase, CreateOrderUsKsErr, OrderReplicaInventoryUseCase, OrderReplicaPaymentUseCase,
    OrderPaymentUpdateUseCase, OrderDiscardUnpaidItemsUseCase, ReturnLinesReqUcOutput,
    OrderReplicaRefundUseCase, ReturnLinesReqUseCase
};

use crate::AppSharedState;
use crate::rpc::{AppRpcReply, AbstractRpcContext, AppRpcClientReqProperty, AbsRpcClientCtx,
    AbsRpcServerCtx, AbstractRpcServer };
use crate::error::AppError;
use crate::logging::{app_log_event, AppLogLevel};

pub type AppUseKsRPCreply = DefaultResult<AppRpcReply, AppError>;

// the generic type R is `impl Future<Output=AppUseKsRPCreply>`
// it is workaround since I don't enable TAIT (type-alias-impl-trait) feature
pub type AppUCrunRPCfn<R> = fn(Arc<Box<dyn AbstractRpcContext>>, AppRpcClientReqProperty) -> R;

// the generic type R is `impl Future<Output=AppRpcReply> + Send + 'static`
// same reason as described above
pub type AppRpcReqHandlingFn<R> = fn(AppRpcClientReqProperty, AppSharedState) -> R;

pub async fn initiate_rpc_request (rc_ctx: Arc<Box<dyn AbstractRpcContext>>,
                                   prop: AppRpcClientReqProperty) -> AppUseKsRPCreply
{
    // `get_mut` returns `None` to avoid multiple mutable states
    // let ctx = Arc::get_mut(&mut rc_ctx).unwrap();
    let ctx = rc_ctx.as_ref(); // pointer to a Box instance
    let client = AbsRpcClientCtx::acquire(ctx, 3u8).await ?;
    let mut evt = client.send_request(prop).await ?;
    evt.receive_response().await
}


fn rpc_srv_inner_hdle_req(
        shr_state:AppSharedState,
        srv: Box<dyn AbstractRpcServer>,
        req: AppRpcClientReqProperty,
        reqhdlr:AppRpcReqHandlingFn<impl Future<Output=AppRpcReply> + Send + 'static>,
    ) -> impl Future<Output=DefaultResult<(), AppError>> + Send 
{ // de-sugar function signature for adding extra trait (`Send` in this case)
    async move {
        let reply = reqhdlr(req, shr_state.clone()).await;
        let result = srv.send_response(reply).await;
        if let Err(e) = &result {
            let logctx_p = shr_state.log_context().clone();
            app_log_event!(logctx_p, AppLogLevel::ERROR,
                           "[rpc][consumer] failed to respond, {}", e);
        }
        result
    }
}

pub async fn rpc_server_process (
        shr_state:AppSharedState,
        rpc_ctx:Arc<Box<dyn AbstractRpcContext>>,
        // request handler is a function pointer type that :
        // - should be able to switch and send between tasks, so I add Send auto-traits
        // - should live long enough, static lifetime is added, but not necessarily as long
        //   as the entire program lifetime, the syntax `impl Future` can be treat as an
        //   owned type with descriptive lifetime hint.
        reqhdlr:AppRpcReqHandlingFn<impl Future<Output=AppRpcReply> + Send + 'static>,
    ) -> DefaultResult<impl Future<Output=DefaultResult<(), AppError>> + Send, AppError> 
{
    let ctx = rpc_ctx.as_ref();
    let mut srv = AbsRpcServerCtx::acquire(ctx, 2).await?;
    let req = srv.receive_request().await?;
    // don't use captured closure, this function loads task-spawn function from
    // callers at runtime, while captured closure moves data at compile time
    let tskprep = rpc_srv_inner_hdle_req(shr_state, srv, req, reqhdlr);
    Ok(tskprep)
} // end of fn rpc_server_process

