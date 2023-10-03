use axum::debug_handler;
use axum::response::IntoResponse;
use axum::extract::{
    Json as ExtractJson,    Path as ExtractPath,
    Query as ExtractQuery,  State as ExtractState,
};
use axum::http::{
    StatusCode as HttpStatusCode,
    HeaderMap as HttpHeaderMap,
    HeaderValue as HttpHeaderValue,
    header as HttpHeader
};
use serde_json;

use crate::logging::AppLogLevel;
use crate::{constant as AppConst, AppSharedState, app_log_event};
use crate::api::web::dto::{
    OrderCreateReqData, OrderLinePayDto, PayAmountDto, OrderCreateRespOkDto,
    OrderEditReqData,
};


// always to specify state type explicitly to the debug macro
#[debug_handler(state=AppSharedState)]
pub(crate) async fn post_handler(
    ExtractState(_appstate): ExtractState<AppSharedState>,
    _req_body: ExtractJson<OrderCreateReqData> ) -> impl IntoResponse
{
    let resp_ctype_val = HttpHeaderValue::from_str(AppConst::HTTP_CONTENT_TYPE_JSON).unwrap();
    let mut hdr_map = HttpHeaderMap::new();
    hdr_map.insert(HttpHeader::CONTENT_TYPE, resp_ctype_val);
    let mut resp_status_code = HttpStatusCode::ACCEPTED;
    let reserved_item = OrderLinePayDto{
        seller_id: 389u32, product_id: 1018u64, product_type:1u8, quantity: 9u32,
        amount: PayAmountDto{unit:4u32, total:35u32}
    };
    let resp_body = OrderCreateRespOkDto {
        order_id: "ty033u29G".to_string(), usr_id: 789u32, time: 29274692u64,
        reserved_lines: vec![reserved_item],
    };
    let serial_resp_body = match serde_json::to_string(&resp_body)
    {
        Ok(s) => s,
        Err(_) => {
            resp_status_code = HttpStatusCode::INTERNAL_SERVER_ERROR;
            "{\"reason\":\"serialization-faulire\"}".to_string()
        },
    };
    let log_ctx = _appstate.log_context();
    app_log_event!(log_ctx, AppLogLevel::INFO,
            "order create done, {} ", 3.16);
    (resp_status_code, hdr_map, serial_resp_body)
} // end of post_handler


#[debug_handler(state=AppSharedState)]
pub(crate) async fn patch_handler (
    oid:ExtractPath<String>,
    billing:Option<ExtractQuery<bool>>,
    shipping:Option<ExtractQuery<bool>>,
    ExtractState(_appstate): ExtractState<AppSharedState>,
    _req_body: ExtractJson<OrderEditReqData>) -> impl IntoResponse
{
    let resp_ctype_val = HttpHeaderValue::from_str(AppConst::HTTP_CONTENT_TYPE_JSON).unwrap();
    let mut hdr_map = HttpHeaderMap::new();
    hdr_map.insert(HttpHeader::CONTENT_TYPE, resp_ctype_val);
    let serial_resp_body = "{}".to_string();
    let log_ctx = _appstate.log_context();
    app_log_event!(log_ctx, AppLogLevel::INFO,
            "edited contact info of the order {} ", oid.clone());
    (HttpStatusCode::OK, hdr_map, serial_resp_body)
}

