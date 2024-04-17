mod charge;
mod dto;

use actix_http::Method;
use actix_web::Route;
use std::collections::HashMap;

use charge::{create_charge, refresh_charge_status};

pub struct AppRouteTable {
    pub version: String,
    pub entries: HashMap<String, Route>,
}

impl Default for AppRouteTable {
    fn default() -> Self {
        let version = "v0.0.1".to_string();
        let entries = Self::v0_0_1_entries();
        Self { version, entries }
    }
}
impl AppRouteTable {
    fn v0_0_1_entries() -> HashMap<String, Route> {
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
