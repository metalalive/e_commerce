use std::result::Result as DefaultResult;
use std::collections::HashMap;
use std::cell::RefCell;
use std::sync::Mutex;

use crate::config::AppInMemoryDbCfg;
use crate::error::{AppError, AppErrorCode};

type InnerRow = Vec<String>;
type InnerTable = HashMap<String, InnerRow>;
type AllTable = HashMap<String, InnerTable>;
pub type AppInMemUpdateData = AllTable;
pub type AppInMemDeleteInfo = InnerTable; // list of IDs per table
pub type AppInMemFetchKeys = InnerTable; // list of IDs per table
pub type AppInMemFetchedData = AllTable;

pub struct AppInMemoryDStore {
    max_items_per_table : u32,
    table_map : Mutex<RefCell<AllTable>> 
}

impl AppInMemoryDStore {
    pub fn new(cfg:&AppInMemoryDbCfg) -> Self {
        let t_map = HashMap::new();
        let t_map = Mutex::new(RefCell::new(t_map));
        Self { table_map: t_map, max_items_per_table: cfg.max_items }
    }

    pub fn create_table (&self, label:&str) -> DefaultResult<(), AppError>
    {
        Ok(())
    }

    pub fn save(_data:AppInMemUpdateData) -> DefaultResult<(), AppError>
    {
        Ok(())
    }

    pub fn delete(_info:AppInMemDeleteInfo) -> DefaultResult<(), AppError>
    {
        Ok(())
    }

    pub fn fetch(_keys:AppInMemFetchKeys) -> DefaultResult<AppInMemFetchedData, AppError>
    {
        Ok(HashMap::new())
    }
} // end of AppInMemoryDStore

