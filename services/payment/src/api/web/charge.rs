use actix_web::http::header::{ContentType, CONTENT_TYPE};
use actix_web::web::{Json as ExtJson, Path as ExtPath};
use actix_web::{HttpResponse, Result as ActixResult};

use super::dto::ChargeReqDto;

pub(super) async fn create_charge(
    _path: ExtPath<String>,
    _req_body: ExtJson<ChargeReqDto>,
) -> ActixResult<HttpResponse> {
    let resp = HttpResponse::Accepted()
        .append_header(ContentType::json())
        .body("{}");
    Ok(resp)
}

pub(super) async fn refresh_charge_status(_path: ExtPath<String>) -> ActixResult<HttpResponse> {
    let resp = HttpResponse::Ok()
        .append_header((CONTENT_TYPE.as_str(), "application/json"))
        .body("{}");
    Ok(resp)
}
