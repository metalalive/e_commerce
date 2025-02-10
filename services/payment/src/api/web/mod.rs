mod charge;
pub mod dto;
mod onboard;
mod refund;
mod reporting;

use actix_http::Method;
use actix_web::body::BoxBody;
use actix_web::error::ResponseError;
use actix_web::http::header::ContentType;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, Route};
use std::collections::HashMap;

use charge::{capture_authorized_charge, create_charge, refresh_charge_status};
use onboard::{onboard_store, track_onboarding_status};
use refund::mechant_complete_refund;
use reporting::report_charge_lines;

pub struct AppRouteTable {
    pub version: String,
    pub entries: HashMap<String, Route>,
} // note, figure out how do multiple versions of API endpoints co-exist

impl AppRouteTable {
    pub fn get(ver_req: &str) -> Self {
        let (version, entries) = match ver_req {
            "0.1.0" => (format!("v{ver_req}"), Self::v0_1_0_entries()),
            _others => (String::new(), HashMap::new()),
        };
        Self { version, entries }
    }
    fn v0_1_0_entries() -> HashMap<String, Route> {
        let data = [
            (
                "create_new_charge".to_string(),
                Route::new().method(Method::POST).to(create_charge),
            ),
            (
                "refresh_charge_status".to_string(),
                Route::new().method(Method::PATCH).to(refresh_charge_status),
            ),
            (
                "capture_authed_charge".to_string(),
                Route::new()
                    .method(Method::POST)
                    .to(capture_authorized_charge),
            ),
            (
                "onboard_store".to_string(),
                Route::new().method(Method::POST).to(onboard_store),
            ),
            (
                "track_onboarding_status".to_string(),
                Route::new()
                    .method(Method::PATCH)
                    .to(track_onboarding_status),
            ),
            (
                "complete_refund".to_string(),
                Route::new()
                    .method(Method::PATCH)
                    .to(mechant_complete_refund),
            ),
            (
                "report_charge_lines".to_string(),
                Route::new().method(Method::GET).to(report_charge_lines),
            ),
        ];
        HashMap::from(data)
    }
} // end of impl AppRouteTable

#[derive(Debug)]
struct RepoInitFailure;

impl std::fmt::Display for RepoInitFailure {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Ok(())
    }
}
impl ResponseError for RepoInitFailure {
    fn status_code(&self) -> StatusCode {
        StatusCode::SERVICE_UNAVAILABLE
    }
    fn error_response(&self) -> HttpResponse<BoxBody> {
        HttpResponse::ServiceUnavailable()
            .append_header(ContentType::plaintext())
            .body("")
    }
}
