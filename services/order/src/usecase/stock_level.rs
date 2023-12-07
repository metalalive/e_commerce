use std::boxed::Box;
use std::result::Result as DefaultResult;

use crate::api::rpc::dto::{
    InventoryEditStockLevelDto, StockLevelPresentDto, StockLevelReturnDto, StockReturnErrorDto
};
use crate::error::AppError;
use crate::repository::AbsOrderRepo;
use crate::model::{ProductStockIdentity, StockLevelModelSet};

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

    pub async fn try_return(data:StockLevelReturnDto, repo:Box<dyn AbsOrderRepo>)
        -> DefaultResult<Vec<StockReturnErrorDto>, AppError>
    { // TODO,
      // this use case does not check the quantity of returning items by loading past
      // order-line returns, the checking process should be done in inventory service
        let st_repo = repo.stock();
        st_repo.try_return(Self::read_stocklvl_cb, data).await
    }
    fn read_stocklvl_cb(ms:&mut StockLevelModelSet, data:StockLevelReturnDto)
        -> Vec<StockReturnErrorDto>
    { ms.return_by_id(data) }
} // end of impl StockLevelUseCase

