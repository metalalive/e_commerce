mod charge;
pub mod dto;

use actix_http::Method;
use actix_web::Route;
use std::collections::HashMap;

use charge::{create_charge, refresh_charge_status};

pub struct AppRouteTable {
    pub version: String,
    pub entries: HashMap<String, Route>,
} // note, figure out how do multiple versions of API endpoints co-exist

impl AppRouteTable {
    pub fn get(ver_req: &str) -> Self {
        let (version, entries) = match ver_req {
            "0.0.1" | "0.0.2" => (format!("v{ver_req}"), Self::v0_0_2_entries()),
            _others => (String::new(), HashMap::new()),
        };
        Self { version, entries }
    }
    fn v0_0_2_entries() -> HashMap<String, Route> {
        let data = [
            (
                "create_new_charge".to_string(),
                Route::new().method(Method::POST).to(create_charge),
            ),
            (
                "refresh_charge_status".to_string(),
                Route::new().method(Method::PATCH).to(refresh_charge_status),
            ),
        ];
        HashMap::from(data)
    }
}
