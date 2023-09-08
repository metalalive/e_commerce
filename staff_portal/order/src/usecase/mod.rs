mod edit_product_policy;

use std::sync::Arc;
use std::boxed::Box;
use std::result::Result as DefaultResult;

pub use edit_product_policy::{
    EditProductPolicyUseCase, EditProductPolicyResult
};

use crate::rpc::{AppRpcReply, AbstractRpcContext, AppRpcClientReqProperty};
use crate::error::AppError;

pub type AppUCrunRPCreturn = DefaultResult<AppRpcReply, AppError>;
// the generic type R is `impl Future<Output = AppUCrunRPCreturn>`
// it is workaround since I don't enable TAIT (type-alias-impl-trait) feature
pub type AppUCrunRPCfn<R> = fn(Arc<Box<dyn AbstractRpcContext>>, AppRpcClientReqProperty) -> R;

pub async fn initiate_rpc_request (rc_ctx: Arc<Box<dyn AbstractRpcContext>>, prop: AppRpcClientReqProperty)
    -> AppUCrunRPCreturn
{
    // `get_mut` returns `None` to avoid multiple mutable states
    // let ctx = Arc::get_mut(&mut rc_ctx).unwrap();
    match rc_ctx.acquire(3u8).await {
        Ok(mut _client) => 
            match _client.send_request(prop).await {
                Ok(mut evt) => evt.receive_response().await ,
                Err(e) => Err(e),
            },
        Err(e) => Err(e)
    }
    // let _client1 = Arc::get_mut(&mut _client).unwrap();
}

