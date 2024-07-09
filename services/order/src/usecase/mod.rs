mod currency;
mod edit_product_policy;
mod edit_product_price;
mod manage_cart;
mod manage_order;
mod stock_level;

use std::boxed::Box;
use std::result::Result as DefaultResult;
use std::sync::Arc;

pub use currency::CurrencyRateRefreshUseCase;
pub use edit_product_policy::{
    EditProductPolicyResult, EditProductPolicyUseCase, ProductInfoReq, ProductInfoResp,
};
pub use edit_product_price::EditProductPriceUseCase;
pub(crate) use manage_cart::{
    DiscardCartUsKsResult, DiscardCartUseCase, ModifyCartLineUseCase, ModifyCartUsKsResult,
    RetrieveCartUsKsResult, RetrieveCartUseCase,
};
pub use manage_order::{
    CreateOrderUsKsErr, CreateOrderUseCase, OrderDiscardUnpaidItemsUseCase,
    OrderPaymentUpdateUseCase, OrderReplicaInventoryUseCase, OrderReplicaPaymentUseCase,
    OrderReplicaRefundUseCase, ReturnLinesReqUcOutput, ReturnLinesReqUseCase,
};
pub use stock_level::StockLevelUseCase;

use crate::error::AppError;
use crate::rpc::{AbsRpcClientCtx, AbstractRpcContext, AppRpcClientReqProperty, AppRpcReply};
use crate::AppSharedState;

pub type AppUseKsRPCreply = DefaultResult<AppRpcReply, AppError>;

// the generic type R is `impl Future<Output=AppUseKsRPCreply>`
// it is workaround since I don't enable TAIT (type-alias-impl-trait) feature
pub type AppUCrunRPCfn<R> = fn(Arc<Box<dyn AbstractRpcContext>>, AppRpcClientReqProperty) -> R;

// the generic type R is `impl Future<Output=AppRpcReply> + Send + 'static`
// same reason as described above
pub type AppRpcReqHandlingFn<R> = fn(AppRpcClientReqProperty, AppSharedState) -> R;

pub async fn initiate_rpc_request(
    rc_ctx: Arc<Box<dyn AbstractRpcContext>>,
    prop: AppRpcClientReqProperty,
) -> AppUseKsRPCreply {
    // `get_mut` returns `None` to avoid multiple mutable states
    // let ctx = Arc::get_mut(&mut rc_ctx).unwrap();
    let ctx = rc_ctx.as_ref(); // pointer to a Box instance
    let client = AbsRpcClientCtx::acquire(ctx, 3u8).await?;
    let mut evt = client.send_request(prop).await?;
    evt.receive_response().await
}
