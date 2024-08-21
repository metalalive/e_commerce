mod charge;
pub mod dto;
mod onboard;

use actix_http::Method;
use actix_web::Route;
use std::collections::HashMap;

use charge::{capture_authorized_charge, create_charge, refresh_charge_status};
use onboard::{onboard_store, track_onboarding_status};

pub struct AppRouteTable {
    pub version: String,
    pub entries: HashMap<String, Route>,
} // note, figure out how do multiple versions of API endpoints co-exist

impl AppRouteTable {
    pub fn get(ver_req: &str) -> Self {
        let (version, entries) = match ver_req {
            "0.0.1" | "0.0.2" | "0.0.4" | "0.0.5" => {
                (format!("v{ver_req}"), Self::v0_0_5_entries())
            }
            _others => (String::new(), HashMap::new()),
        };
        Self { version, entries }
    }
    fn v0_0_5_entries() -> HashMap<String, Route> {
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
        ];
        HashMap::from(data)
    }
}
