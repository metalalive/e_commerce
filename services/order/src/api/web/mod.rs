use std::collections::HashMap;

use axum::routing::{delete, get, patch, post, MethodRouter};
use http_body::Body as HttpBody;

use crate::constant::api::web as WebConst;
use crate::{AppSharedState, WebApiHdlrLabel};

mod cart;
pub mod dto;
mod order;
mod product_policy;

// type parameter `B` for http body of the method router has to match the same
// type parameter in `axum::Router`
pub type ApiRouteType<HB> = MethodRouter<AppSharedState, HB>;
pub type ApiRouteTableType<HB> = HashMap<WebApiHdlrLabel, ApiRouteType<HB>>;

pub fn route_table<HB>() -> ApiRouteTableType<HB>
where
    HB: HttpBody + Send + 'static,
    <HB as HttpBody>::Data: Send,
    <HB as HttpBody>::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    let mut out: ApiRouteTableType<HB> = HashMap::new();
    out.insert(
        WebConst::ADD_PRODUCT_POLICY,
        post(product_policy::post_handler),
    );
    out.insert(WebConst::CREATE_NEW_ORDER, post(order::create_handler));
    out.insert(
        WebConst::ACCESS_EXISTING_ORDER,
        patch(order::edit_billing_shipping_handler),
    );
    out.insert(
        WebConst::RETURN_OLINES_REQ,
        patch(order::return_lines_request_handler),
    );
    out.insert(WebConst::MODIFY_CART_LINES, patch(cart::modify_lines));
    out.insert(WebConst::DISCARD_CART, delete(cart::discard));
    out.insert(WebConst::RETRIEVE_CART_LINES, get(cart::retrieve));
    out
}
