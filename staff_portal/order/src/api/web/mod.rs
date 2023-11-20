use std::collections::HashMap;
use axum::routing::{patch, post, MethodRouter};

use crate::{AppSharedState, constant as AppConst, WebApiHdlrLabel};

mod product_policy;
mod order;
pub mod dto;

pub type ApiRouteType = MethodRouter<AppSharedState>;
pub type ApiRouteTableType = HashMap<WebApiHdlrLabel, ApiRouteType>;

pub fn route_table () -> ApiRouteTableType
{
    let mut out: ApiRouteTableType = HashMap::new();
    out.insert( AppConst::WEBAPI_ADD_PRODUCT_POLICY,
                post(product_policy::post_handler)  );
    out.insert( AppConst::WEBAPI_CREATE_NEW_ORDER,
                post(order::create_handler)  );
    out.insert( AppConst::WEBAPI_ACCESS_EXISTING_ORDER,
                patch(order::edit_billing_shipping_handler)  );
    out.insert( AppConst::WEBAPI_RETURN_OLINES_REQ,
                patch(order::return_lines_request_handler)  );
    out
}

