use std::boxed::Box;
use std::result::Result as DefaultResult;
use std::sync::Arc;

use ecommerce_common::api::dto::CurrencyDto;
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::logging::{app_log_event, AppLogContext, AppLogLevel};

use crate::api::rpc::dto::ProductPriceDto;
use crate::error::AppError;
use crate::model::ProductPriceModelSet;
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
        let result = Self::_execute(repo, data).await;
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

    pub async fn _execute(
        repo: Box<dyn AbsProductPriceRepo>,
        data: ProductPriceDto,
    ) -> DefaultResult<(), AppError> {
        let rm_all = data.rm_all;
        let rm_items = data.deleting.items.is_some();
        let rm_pkgs = data.deleting.pkgs.is_some();
        if rm_all {
            repo.delete_all(data.s_id).await
        } else if rm_items || rm_pkgs {
            // currently the storefront service separates delete operation from
            // create and update operations, we can expect there is no product overlapped
            // in the `deleting`, `creating`, and `updating` lists
            repo.delete(data.s_id, data.deleting).await
        } else {
            // create and update
            let ProductPriceDto {
                s_id,
                rm_all: _,
                currency,
                deleting: _,
                updating,
                creating,
            } = data;
            let new_currency = currency.ok_or(AppError {
                code: AppErrorCode::InvalidInput,
                detail: Some("missing-currency".to_string()),
            })?;
            let ids = updating
                .iter()
                .map(|d| (d.product_type.clone(), d.product_id))
                .collect();
            let pre_saved = match repo.fetch(s_id, ids).await {
                Ok(v) => Ok(v),
                Err(e) => {
                    if e.code == AppErrorCode::ProductNotExist {
                        Ok(ProductPriceModelSet {
                            store_id: s_id,
                            currency: CurrencyDto::TWD,
                            items: vec![],
                        })
                    } else {
                        Err(e)
                    }
                }
            }?;
            let updated = pre_saved.update(updating, creating, new_currency)?;
            repo.save(updated).await
        }
    } // end of fn _execute
} // end of impl EditProductPriceUseCase
