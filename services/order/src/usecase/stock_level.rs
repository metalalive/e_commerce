use std::boxed::Box;
use std::result::Result as DefaultResult;

use crate::api::rpc::dto::{InventoryEditStockLevelDto, StockLevelPresentDto, StockLevelReturnDto};
use crate::error::AppError;
use crate::repository::AbsOrderRepo;
use crate::model::ProductStockIdentity;

pub struct StockLevelUseCase {}

impl StockLevelUseCase {
    pub async fn try_edit(data:Vec<InventoryEditStockLevelDto>, repo:Box<dyn AbsOrderRepo>)
        -> DefaultResult<Vec<StockLevelPresentDto>, AppError>
    {
        let ids = data.iter().map(|d| ProductStockIdentity {
            store_id:d.store_id, product_type: d.product_type.clone(),
            product_id: d.product_id, expiry:d.expiry }
        ).collect();
        let stockrepo = repo.stock();
        let saved = stockrepo.fetch(ids).await?;
        let updated = saved.update(data)?;
        let _ = stockrepo.save(updated.clone()).await?;
        Ok(updated.into())
    }
    pub async fn try_return(_data:StockLevelReturnDto, _repo:Box<dyn AbsOrderRepo>)
        -> DefaultResult<Vec<StockLevelPresentDto>, AppError>
    {
        Ok(vec![])
    }
} // end of impl StockLevelUseCase

