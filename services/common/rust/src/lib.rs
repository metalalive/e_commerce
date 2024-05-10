pub mod api;
pub mod config;
pub mod constant;
pub mod error;
pub mod logging;

use std::sync::Arc;

pub type WebApiPath = String;
pub(crate) type AppLogAlias = Arc<String>;
