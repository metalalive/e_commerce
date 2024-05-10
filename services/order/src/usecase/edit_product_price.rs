use std::boxed::Box;
use std::result::Result as DefaultResult;
use std::sync::Arc;

use ecommerce_common::logging::{app_log_event, AppLogContext, AppLogLevel};

use crate::api::rpc::dto::ProductPriceDto;
use crate::error::AppError;
use crate::repository::AbsProductPriceRepo;

pub struct EditProductPriceUseCase {}

impl EditProductPriceUseCase {
    pub async fn execute(
        repo: Box<dyn AbsProductPriceRepo>,
        data: ProductPriceDto,
        logctx: Arc<AppLogContext>,
    ) -> DefaultResult<(), AppError> {
        let (num_insert, num_update) = (data.creating.len(), data.updating.len());
        let rm_all = data.rm_all;
        let rm_items = data.deleting.items.is_some();
        let rm_pkgs = data.deleting.pkgs.is_some();
        let result = if rm_all {
            repo.delete_all(data.s_id).await
        } else if rm_items || rm_pkgs {
            // currently the storefront service separates delete operation from
            // create and update operations, we can expect there is no product overlapped
            // in the `deleting`, `creating`, and `updating` lists
            repo.delete(data.s_id, data.deleting).await
        } else {
            // create and update
            let ids = data
                .updating
                .iter()
                .map(|d| (d.product_type.clone(), d.product_id))
                .collect();
            match repo.fetch(data.s_id, ids).await {
                Ok(pre_saved) => match pre_saved.update(data.updating, data.creating) {
                    Ok(updated) => repo.save(updated).await,
                    Err(e) => Err(e),
                },
                Err(e) => Err(e),
            }
        };
        if let Err(e) = &result {
            app_log_event!(
                logctx,
                AppLogLevel::ERROR,
                "detail:{}, num_insert:{}, num_update:{},\
                           rm_all:{}, rm_items:{}, rm_pkgs:{}",
                e,
                num_insert,
                num_update,
                rm_all,
                rm_items,
                rm_pkgs
            );
        }
        result
    } // end of fn execute
} // end of impl EditProductPriceUseCase
