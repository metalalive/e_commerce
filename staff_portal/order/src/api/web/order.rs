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
use serde::{Deserialize, Serialize};
use serde_json;

use crate::logging::AppLogLevel;
use crate::{AppConst, AppSharedState, app_log_event};


#[derive(Deserialize)]
enum CountryCode {TW,TH,IN,ID,US}

#[derive(Deserialize)]
enum ShippingMethod {UPS, FedEx, BlackCatExpress}

#[derive(Deserialize, Serialize)]
struct PayAmountModel {unit: u32, total: u32}

#[derive(Deserialize, Serialize)]
struct OrderLinePendingModel {
    seller_id: u32,
    product_id: u32,
    quantity: u32
}

#[derive(Deserialize, Serialize)]
struct OrderLinePayModel {
    seller_id: u32,
    product_id: u32,
    quantity: u32,
    amount: PayAmountModel
}

#[derive(Deserialize)]
struct PhoneNumberModel {
    nation: u16,
    number: String,
}

#[derive(Deserialize)]
struct ContactModel {
    first_name: String,
    last_name: String,
    emails: Vec<String>,
    phones: Vec<PhoneNumberModel>,
}

#[derive(Deserialize)]
struct PhyAddrModel {
    country: CountryCode,
    region: String,
    city: String,
    distinct: String,
    street_name: Option<String>,
    detail: String
}

#[derive(Deserialize)]
struct ShippingOptionModel {
    seller_id: u32,
    // #[serde(rename_all="_")]
    method: ShippingMethod,
}

#[derive(Deserialize)]
struct BillingModel {
    contact: ContactModel,
    address: Option<PhyAddrModel>,
}

#[derive(Deserialize)]
struct ShippingModel {
    contact: ContactModel,
    address: Option<PhyAddrModel>,
    option: Vec<ShippingOptionModel>,
}

#[derive(Deserialize)]
pub(crate) struct OrderCreateReqData {
    order_lines: Vec<OrderLinePendingModel>,
    billing: BillingModel,
    shipping: ShippingModel
}

#[derive(Serialize)]
pub(crate) struct OrderCreateRespAsyncData {
    order_id: String,
    usr_id: u32,
    time: u64,
    reserved_lines: Vec<OrderLinePayModel>,
    async_stock_chk: Vec<OrderLinePendingModel> 
}



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
    let reserved_item = OrderLinePayModel{
        seller_id: 389u32, product_id: 1018u32, quantity: 9u32,
        amount: PayAmountModel{unit:4u32, total:35u32}
    };
    let async_chk_item = OrderLinePendingModel{
        seller_id: 3827u32, product_id: 2088u32, quantity: 13u32,
    };
    let resp_body = OrderCreateRespAsyncData {
        order_id: "ty033u29G".to_string(), usr_id: 789u32, time: 29274692u64,
        reserved_lines: vec![reserved_item],
        async_stock_chk: vec![async_chk_item]
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


#[derive(Deserialize)]
pub(crate) struct OrderEditReqData {
    billing: BillingModel,
    shipping: ShippingModel
}

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

