pub mod api;
pub mod config;
pub mod constant;
pub mod error;

use std::sync::Arc;

pub type WebApiPath = String;
pub type AppLogAlias = Arc<String>;
