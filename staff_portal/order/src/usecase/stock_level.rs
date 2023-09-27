use std::boxed::Box;
use std::result::Result as DefaultResult;

use crate::api::rpc::dto::{InventoryEditStockLevelDto, StockLevelPresentDto};
use crate::error::AppError;
use crate::repository::AbsOrderRepo;

pub struct StockLevelUseCase {}

impl StockLevelUseCase {
    pub async fn try_edit(data:Vec<InventoryEditStockLevelDto>, repo:Box<dyn AbsOrderRepo>)
        -> DefaultResult<Vec<StockLevelPresentDto>, AppError>
    {
        let ids = data.iter().map(|d| (d.store_id, d.product_type,
                                       d.product_id)) .collect();
        let stockrepo = repo.stock();
        let saved = stockrepo.fetch(ids).await?;
        let updated = saved.update(data)?;
        let _ = stockrepo.save(updated.clone()).await?;
        Ok(updated.present())
    }
} // end of impl StockLevelUseCase

