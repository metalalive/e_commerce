pub mod adapter;
pub mod api;
pub mod auth;
pub mod confidentiality;
pub mod config;
pub mod constant;
pub mod error;
pub mod logging;
pub mod model;

use std::sync::Arc;

pub type WebApiPath = String;
pub(crate) type AppLogAlias = Arc<String>;
