use std::boxed::Box;
use std::result::Result as DefaultResult;
use std::sync::Arc;

use ecommerce_common::logging::{app_log_event, AppLogContext, AppLogLevel};

use crate::api::rpc::dto::{
    InventoryEditStockLevelDto, StockLevelPresentDto, StockLevelReturnDto, StockReturnErrorDto,
};
use crate::error::AppError;
use crate::model::{ProductStockIdentity, StockLevelModelSet};
use crate::repository::AbsOrderRepo;

pub struct StockLevelUseCase {}

impl StockLevelUseCase {
    pub async fn try_edit(
        data: Vec<InventoryEditStockLevelDto>,
        repo: Box<dyn AbsOrderRepo>,
        logctx: Arc<AppLogContext>,
    ) -> DefaultResult<Vec<StockLevelPresentDto>, AppError> {
        let ids = data
            .iter()
            .map(|d| ProductStockIdentity {
                store_id: d.store_id,
                product_type: d.product_type.clone(),
                product_id: d.product_id,
                expiry: d.expiry.into(),
            })
            .collect();
        let stockrepo = repo.stock();
        let saved = stockrepo.fetch(ids).await?;
        app_log_event!(
            logctx,
            AppLogLevel::DEBUG,
            "num-stored:{}",
            saved.stores.len()
        );
        let updated = saved.update(data)?;
        stockrepo.save(updated.clone()).await?;
        Ok(updated.into())
    }

    pub async fn try_return(
        data: StockLevelReturnDto,
        repo: Box<dyn AbsOrderRepo>,
        logctx: Arc<AppLogContext>,
    ) -> DefaultResult<Vec<StockReturnErrorDto>, AppError> {
        // TODO,
        // this use case does not check the quantity of returning items by loading past
        // order-line returns, the checking process should be done in inventory service
        let st_repo = repo.stock();
        let result = st_repo.try_return(Self::read_stocklvl_cb, data).await;
        if let Ok(usr_err) = result.as_ref() {
            if let Some(e) = usr_err.first() {
                app_log_event!(logctx, AppLogLevel::WARNING, "input-error: {:?}", e);
            }
        }
        result
    }
    fn read_stocklvl_cb(
        ms: &mut StockLevelModelSet,
        data: StockLevelReturnDto,
    ) -> Vec<StockReturnErrorDto> {
        ms.return_by_expiry(data)
    }
} // end of impl StockLevelUseCase
