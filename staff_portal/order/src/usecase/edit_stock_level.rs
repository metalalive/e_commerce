use std::boxed::Box;
use std::sync::Arc;
use std::result::Result as DefaultResult;

use crate::api::rpc::dto::{InventoryEditStockLevelDto, StockLevelPresentDto};
use crate::error::{AppError, AppErrorCode};
use crate::logging::AppLogContext;
use crate::repository::AbsOrderRepo;

pub struct EditStockLevelUseCase {}

impl EditStockLevelUseCase {
    pub async fn execute(data:Vec<InventoryEditStockLevelDto>, repo:Box<dyn AbsOrderRepo>,
                     logctx:Arc<AppLogContext>) -> DefaultResult<Vec<StockLevelPresentDto>, AppError>
    {
        let e = AppError{code:AppErrorCode::NotImplemented , detail:None};
        Err(e)
    }
} // end of impl EditStockLevelUseCase

