use std::vec::Vec;

use crate::AppSharedState;
use crate::rpc::AppRpcClientReqProperty;
use crate::usecase::EditProductPriceUseCase;
use crate::repository::app_repo_product_price;
use super::dto::ProductPriceDto;
use super::{build_error_response, py_celery_deserialize_req};

pub(super) async fn process(req:AppRpcClientReqProperty, shr_state:AppSharedState) -> Vec<u8>
{
    let ds = shr_state.datastore();
    let repo = match app_repo_product_price(ds).await {
        Ok(r) => r,
        Err(e) => { return build_error_response(e).to_string().into_bytes(); }
    };
    let result = py_celery_deserialize_req::<Vec<String>, ProductPriceDto>(&req.msgbody);
    match result {
        Ok((_arg, data)) => {
            let logctx = shr_state.log_context().clone();
            let result = EditProductPriceUseCase::execute(repo, data, logctx).await;
            if let Err(e) = result {
                build_error_response(e).to_string().into_bytes()
            } else { Vec::new() } // complete successfully
        },
        Err(e) => build_error_response(e).to_string().into_bytes()
    } // end of match statement
} // end of fn process

