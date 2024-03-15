mod edit_product_policy;
mod edit_product_price;
mod stock_level;
mod manage_order;

use std::sync::Arc;
use std::boxed::Box;
use std::result::Result as DefaultResult;

pub use edit_product_policy::{
    EditProductPolicyUseCase, EditProductPolicyResult, ProductInfoReq, ProductInfoResp
};
pub use edit_product_price::EditProductPriceUseCase;
pub use stock_level::StockLevelUseCase;
pub use manage_order::{
    CreateOrderUseCase, CreateOrderUsKsErr, OrderReplicaInventoryUseCase, OrderReplicaPaymentUseCase,
    OrderPaymentUpdateUseCase, OrderDiscardUnpaidItemsUseCase, ReturnLinesReqUcOutput,
    OrderReplicaRefundUseCase, ReturnLinesReqUseCase
};

use crate::AppSharedState;
use crate::rpc::{AppRpcReply, AbstractRpcContext, AppRpcClientReqProperty, AbsRpcClientCtx};
use crate::error::AppError;

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

