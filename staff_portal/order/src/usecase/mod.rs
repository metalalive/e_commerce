mod edit_product_policy;

use std::sync::Arc;
use std::boxed::Box;
use std::result::Result as DefaultResult;

pub use edit_product_policy::{
    EditProductPolicyUseCase, EditProductPolicyResult
};

use crate::rpc::{AppRpcConsumeResult, AbstractRpcContext, AppRpcPublishProperty, AppRpcConsumeProperty};
use crate::error::AppError;

pub type AppUCrunRPCreturn = DefaultResult<AppRpcConsumeResult, AppError>;
// the generic type R is `impl Future<Output = AppUCrunRPCreturn>`
// it is workaround since I don't enable TAIT (type-alias-impl-trait) feature
pub type AppUCrunRPCfn<R> = fn(Arc<Box<dyn AbstractRpcContext>>, AppRpcPublishProperty) -> R;

pub async fn run_rpc (rc_ctx: Arc<Box<dyn AbstractRpcContext>>, prop: AppRpcPublishProperty)
    -> AppUCrunRPCreturn
{
    // `get_mut` returns `None` to avoid multiple mutable states
    // let ctx = Arc::get_mut(&mut rc_ctx).unwrap();
    let mut hdlr = match rc_ctx.acquire(3u8).await {
         Ok(c) => c,
         Err(e) => {return Err(e);}
    };
    let hdlr1 = Arc::get_mut(&mut hdlr).unwrap();
    let published = match hdlr1.publish(prop).await {
        Ok(p) => p,
        Err(e) => {return Err(e);}
    };
    let prop = AppRpcConsumeProperty {
        retry:3u8, route:published.reply_route,
        corr_id: published.job_id };
    hdlr1.consume(prop).await
}

