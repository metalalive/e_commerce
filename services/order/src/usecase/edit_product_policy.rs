use std::collections::hash_map::RandomState;
use std::collections::HashSet;
use std::future::Future;
use std::result::Result as DefaultResult;
use std::sync::Arc;
use std::vec::Vec;

use chrono::Local;
use serde::{Deserialize, Serialize};

use crate::constant::ProductType;
use crate::error::{AppError, AppErrorCode};
use crate::logging::{app_log_event, AppLogContext, AppLogLevel};
use crate::model::ProductPolicyModelSet;
use crate::repository::app_repo_product_policy;
use crate::rpc::{AbstractRpcContext, AppRpcClientReqProperty};
use crate::{AppAuthPermissionCode, AppAuthQuotaMatCode, AppAuthedClaim, AppDataStoreContext};

use crate::api::web::dto::{ProductPolicyClientErrorDto, ProductPolicyDto, QuotaResourceErrorDto};

use super::{initiate_rpc_request, AppUCrunRPCfn, AppUseKsRPCreply};

// the product info types below represent message body to remote product service
#[derive(Serialize)]
pub struct ProductInfoReq {
    item_ids: Vec<u64>,
    pkg_ids: Vec<u64>,
    item_fields: Vec<String>,
    pkg_fields: Vec<String>,
    profile: u32,
}

#[derive(Deserialize)]
pub struct ProductItemResp {
    id: u64,
}

#[derive(Deserialize)]
pub struct ProductPkgResp {
    id: u64,
}

#[derive(Deserialize)]
pub struct ProductInfoResp {
    item: Vec<ProductItemResp>,
    pkg: Vec<ProductPkgResp>,
}

#[derive(PartialEq, Debug)]
pub enum EditProductPolicyResult {
    OK,
    PermissionDeny,
    QuotaExceed(QuotaResourceErrorDto),
    ClientError(Vec<ProductPolicyClientErrorDto>),
    Other(AppErrorCode),
}

impl EditProductPolicyUseCase {
    fn validate_permission_quota(
        authed_usr: &AppAuthedClaim,
        num_items: usize,
    ) -> DefaultResult<(), EditProductPolicyResult> {
        let perm_allowed =
            authed_usr.contain_permission(AppAuthPermissionCode::can_create_product_policy);
        if perm_allowed {
            let limit = authed_usr.quota_limit(AppAuthQuotaMatCode::NumProductPolicies);
            if (limit as usize) >= num_items {
                Ok(())
            } else {
                let err = QuotaResourceErrorDto {
                    max_: limit,
                    given: num_items,
                };
                Err(EditProductPolicyResult::QuotaExceed(err))
            }
        } else {
            Err(EditProductPolicyResult::PermissionDeny)
        }
    }

    pub async fn execute(self) -> EditProductPolicyResult {
        if let Err(e) = Self::validate_permission_quota(&self.authed_usr, self.data.len()) {
            return e;
        }
        let Self {
            authed_usr,
            data,
            log,
            rpc_ctx,
            dstore,
            rpc_serialize_msg,
            rpc_deserialize_msg,
        } = self;
        if let Err(ce) = ProductPolicyModelSet::validate(&data) {
            return EditProductPolicyResult::ClientError(ce);
        }
        let usr_prof_id = authed_usr.profile;
        let rpctype = rpc_ctx.label();
        let result = Self::check_product_existence(
            &data,
            usr_prof_id,
            rpc_ctx,
            initiate_rpc_request,
            rpc_serialize_msg,
            rpc_deserialize_msg,
        )
        .await;
        if let Err((code, detail)) = result {
            if code == EditProductPolicyResult::Other(AppErrorCode::RpcRemoteInvalidReply)
                && rpctype == "dummy"
            {
                // pass, for mocking purpose, TODO: better design
                app_log_event!(log, AppLogLevel::WARNING, "dummy-rpc-applied");
            } else {
                app_log_event!(log, AppLogLevel::ERROR, "detail:{:?}", detail);
                return code;
            }
        } else if let Ok(missing_prod_ids) = result {
            if !missing_prod_ids.is_empty() {
                app_log_event!(
                    log,
                    AppLogLevel::ERROR,
                    "missing_prod_ids:{:?}",
                    missing_prod_ids
                );
                let c_err = missing_prod_ids
                    .into_iter()
                    .map(|(product_type, product_id)| ProductPolicyClientErrorDto {
                        product_id,
                        product_type,
                        err_type: format!("{:?}", AppErrorCode::ProductNotExist),
                        warranty_hours: None,
                        auto_cancel_secs: None,
                        num_rsv: None,
                    })
                    .collect();
                return EditProductPolicyResult::ClientError(c_err);
            }
        }
        if let Err(e) = Self::_save_to_repo(dstore, data).await {
            // no need to pass `usr_prof_id`, the product ownership should be verified by previous RPC
            app_log_event!(log, AppLogLevel::ERROR, "error:{:?}", e);
            EditProductPolicyResult::Other(e.code)
        } else {
            EditProductPolicyResult::OK
        }
    } // end of _execute

    // TODO, check whether the user has permission to edit specific product, this relies
    // on validation by RPC call to `storefront` service

    pub async fn check_product_existence(
        data: &Vec<ProductPolicyDto>,
        usr_prof_id: u32,
        rpc_ctx: Arc<Box<dyn AbstractRpcContext>>,
        run_rpc_fn: AppUCrunRPCfn<impl Future<Output = AppUseKsRPCreply>>,
        rpc_serialize_msg: fn(ProductInfoReq) -> DefaultResult<Vec<u8>, AppError>,
        rpc_deserialize_msg: fn(&Vec<u8>) -> DefaultResult<ProductInfoResp, AppError>,
    ) -> DefaultResult<Vec<(ProductType, u64)>, (EditProductPolicyResult, String)> {
        let mut msg_req = ProductInfoReq {
            pkg_ids: Vec::new(),
            pkg_fields: vec!["id".to_string()],
            profile: usr_prof_id,
            item_ids: Vec::new(),
            item_fields: vec!["id".to_string()],
        };
        data.iter()
            .map(|item| match &item.product_type {
                ProductType::Item => {
                    msg_req.item_ids.push(item.product_id);
                }
                ProductType::Package => {
                    msg_req.pkg_ids.push(item.product_id);
                }
                _others => {}
            })
            .count();
        let msgbody = match rpc_serialize_msg(msg_req) {
            Ok(m) => m,
            Err(e) => {
                let detail = format!("app-error: {:?}", e);
                return Err((EditProductPolicyResult::Other(e.code), detail));
            }
        };
        let properties = AppRpcClientReqProperty {
            msgbody,
            start_time: Local::now().fixed_offset(),
            route: "rpc.product.get_product".to_string(),
        };
        match run_rpc_fn(rpc_ctx, properties).await {
            Ok(r) => match rpc_deserialize_msg(&r.body) {
                Ok(reply) => Ok(Self::_compare_rpc_reply(reply, data)),
                Err(e) => {
                    let detail = format!("rpc-reply-decode-error: {:?}", e);
                    Err((EditProductPolicyResult::Other(e.code), detail))
                }
            },
            Err(e) => {
                let detail = format!("rpc-error: {:?}", e);
                Err((EditProductPolicyResult::Other(e.code), detail))
            }
        }
    } // end of check_product_existence

    fn _compare_rpc_reply(
        reply: ProductInfoResp,
        req: &Vec<ProductPolicyDto>,
    ) -> Vec<(ProductType, u64)> {
        let (r_items, r_pkgs) = (reply.item, reply.pkg);
        let iter_item = r_items.into_iter().map(|x| (ProductType::Item, x.id));
        let iter_pkg = r_pkgs.into_iter().map(|x| (ProductType::Package, x.id));
        let iter_req = req.iter().map(|x| (x.product_type.clone(), x.product_id));
        let mut c1: HashSet<(ProductType, u64), RandomState> = HashSet::from_iter(iter_item);
        c1.extend(iter_pkg);
        let c2 = HashSet::from_iter(iter_req);
        c2.difference(&c1)
            .map(|(typ_, id_)| (typ_.clone(), id_.clone()))
            .collect()
    }

    async fn _save_to_repo(
        ds: Arc<AppDataStoreContext>,
        data: Vec<ProductPolicyDto>,
    ) -> DefaultResult<(), AppError> {
        let repo = app_repo_product_policy(ds).await?;
        let ids = data
            .iter()
            .map(|d| (d.product_type.clone(), d.product_id))
            .collect();
        let previous_saved = repo.fetch(ids).await?;
        let updated = previous_saved.update(data)?;
        repo.save(updated).await?;
        Ok(())
    }
} // end of impl EditProductPolicyUseCase

pub struct EditProductPolicyUseCase {
    pub log: Arc<AppLogContext>,
    pub rpc_ctx: Arc<Box<dyn AbstractRpcContext>>,
    pub dstore: Arc<AppDataStoreContext>,
    pub authed_usr: AppAuthedClaim,
    pub data: Vec<ProductPolicyDto>,
    pub rpc_serialize_msg: fn(ProductInfoReq) -> DefaultResult<Vec<u8>, AppError>,
    pub rpc_deserialize_msg: fn(&Vec<u8>) -> DefaultResult<ProductInfoResp, AppError>,
}
