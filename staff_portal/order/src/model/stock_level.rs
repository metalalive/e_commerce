use std::vec::Vec;
use std::result::Result as DefaultResult;

use crate::api::rpc::dto::{InventoryEditStockLevelDto, StockLevelPresentDto};
use crate::error::AppError;

pub struct StockLevelModelSet {}

impl Clone for StockLevelModelSet {
    fn clone(&self) -> Self {
        Self {}
    }
}

impl StockLevelModelSet {
    pub fn update(mut self, data:Vec<InventoryEditStockLevelDto>)
        -> DefaultResult<Self, AppError>
    { Ok(self) }
    
    pub fn present(&self) -> Vec<StockLevelPresentDto>
    { Vec::new() }
}

