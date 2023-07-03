use axum::debug_handler;
use axum::extract::{Json as ExtractJson, State as ExtractState};
use axum::response::IntoResponse;
use axum::http::{
    StatusCode as HttpStatusCode,
    HeaderMap as HttpHeaderMap,
    HeaderValue as HttpHeaderValue,
    header as HttpHeader
};
use serde::Deserialize;

use crate::logging::AppLogLevel;
use crate::{AppConst, AppSharedState, app_log_event};

#[derive(Deserialize)]
pub(crate) struct PolicyData {
    product_id: u64,
    auto_cancel_secs: u32,
    warranty_hours: u32,
    async_stock_chk: bool,
}

#[debug_handler(state = AppSharedState)]
pub(crate) async fn post_handler(
    appstate: ExtractState<AppSharedState>,
    _body: ExtractJson<Vec<PolicyData>> ) -> impl IntoResponse
{
    let resp_ctype_val = HttpHeaderValue::from_str(AppConst::HTTP_CONTENT_TYPE_JSON).unwrap();
    let mut hdr_map = HttpHeaderMap::new();
    hdr_map.insert(HttpHeader::CONTENT_TYPE, resp_ctype_val);
    let serial_resp_body = "[]";
    let log_ctx = appstate.log_context();
    app_log_event!(log_ctx, AppLogLevel::INFO,
            "product policy updated, {} ", 3.18);
    (HttpStatusCode::OK, hdr_map, serial_resp_body)
} // end of endpoint
