use std::result::Result as DefaultResult;
use std::boxed::Box;
use std::sync::Arc;
use crate::confidentiality::AbstractConfidentiality;
use crate::config::{AppDbServerCfg, AppDbServerType};
use crate::error::AppError;

pub struct AppSqlDbFetchedData {}

pub struct AppSqlDbStore
{
    _type: AppDbServerType,
    max_conns:u32,
    idle_max_secs:u16,
    confidential:Arc<Box<dyn AbstractConfidentiality>>
}

impl AppSqlDbStore {
    pub fn new(cfg :&AppDbServerCfg, confidential:Arc<Box<dyn AbstractConfidentiality>>)
        -> Self
    {
        Self { _type: cfg.srv_type.clone(), max_conns: cfg.max_conns,
            idle_max_secs: cfg.idle_timeout_secs, confidential }
    }

    pub async fn save(_query:String) -> DefaultResult<(), AppError>
    {
        Ok(())
    }

    pub async fn delete(_query:String) -> DefaultResult<(), AppError>
    {
        Ok(())
    }

    pub async fn fetch(_query:String) -> DefaultResult<AppSqlDbFetchedData, AppError>
    {
        Ok(AppSqlDbFetchedData {})
    }
}

