use std::collections::HashMap;

use axum::routing::{delete, get, patch, post, MethodRouter};

use crate::constant::api::web as WebConst;
use crate::{AppSharedState, WebApiHdlrLabel};

mod cart;
pub mod dto;
mod order;
mod product_policy;

// type parameter `B` for http body of the method router has to match the same
// type parameter in `axum::Router`
pub type ApiRouteType = MethodRouter<AppSharedState>;
pub type ApiRouteTableType = HashMap<WebApiHdlrLabel, ApiRouteType>;

pub fn route_table() -> ApiRouteTableType {
    let mut out: ApiRouteTableType = HashMap::new();
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
