use std::collections::HashMap;

use axum::routing::{patch, post, MethodRouter};
use http_body::Body as HttpBody;

use crate::{AppSharedState, constant as AppConst, WebApiHdlrLabel};

mod product_policy;
mod order;
pub mod dto;

// type parameter `B` for http body of the method router has to match the same
// type parameter in `axum::Router`
pub type ApiRouteType<HB> = MethodRouter<AppSharedState, HB>;
pub type ApiRouteTableType<HB> = HashMap<WebApiHdlrLabel, ApiRouteType<HB>>;

pub fn route_table<HB>() -> ApiRouteTableType<HB>
    where HB:  HttpBody + Send + 'static,
          <HB as HttpBody>::Data: Send,
          <HB as HttpBody>::Error: Into<Box<dyn  std::error::Error + Send + Sync>>
{
    let mut out: ApiRouteTableType<HB> = HashMap::new();
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

