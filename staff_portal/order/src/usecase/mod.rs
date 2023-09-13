mod edit_product_policy;

use std::sync::Arc;
use std::boxed::Box;
use std::result::Result as DefaultResult;

pub use edit_product_policy::{
    EditProductPolicyUseCase, EditProductPolicyResult
};

use crate::rpc::{AppRpcReply, AbstractRpcContext, AppRpcClientReqProperty, AbsRpcClientCtx};
use crate::error::AppError;

pub type AppUseKsRPCreply = DefaultResult<AppRpcReply, AppError>;
// the generic type R is `impl Future<Output = AppUseKsRPCreply>`
// it is workaround since I don't enable TAIT (type-alias-impl-trait) feature
pub type AppUCrunRPCfn<R> = fn(Arc<Box<dyn AbstractRpcContext>>, AppRpcClientReqProperty) -> R;

pub async fn initiate_rpc_request (rc_ctx: Arc<Box<dyn AbstractRpcContext>>,
                                   prop: AppRpcClientReqProperty) -> AppUseKsRPCreply
{
    // `get_mut` returns `None` to avoid multiple mutable states
    // let ctx = Arc::get_mut(&mut rc_ctx).unwrap();
    let ctx = rc_ctx.as_ref(); // pointer to a Box instance
    let ctx = ctx.as_ref(); // pointer to a trait object
    let client = AbsRpcClientCtx::acquire(ctx, 3u8).await ?;
    let mut evt = client.send_request(prop).await ?;
    evt.receive_response().await
}

