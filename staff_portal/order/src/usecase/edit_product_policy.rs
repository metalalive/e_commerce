use std::future::Future;
use std::sync::Arc;
use std::vec::Vec;
use std::result::Result as DefaultResult;
use std::collections::HashSet;
use std::collections::hash_map::RandomState;

use serde::{Serialize, Deserialize};
use crate::model::ProductPolicyModelSet;
use crate::repository::app_repo_product_policy;
use crate::{AppSharedState, app_log_event, AppDataStoreContext} ;
use crate::error::{AppErrorCode, AppError};
use crate::rpc::{AbstractRpcContext, AppRpcClientReqProperty};
use crate::logging::AppLogLevel;

use crate::api::web::dto::{ProductPolicyDto, ProductPolicyClientErrorDto};

use super::{initiate_rpc_request, AppUCrunRPCfn, AppUseKsRPCreply};

// the product info types below represent message body to remote product service
#[derive(Serialize)]
struct ProductInfoReq {
    item_ids: Vec<u64>,
    pkg_ids: Vec<u64>,
    item_fields : Vec<String>,
    pkg_fields : Vec<String>,
    profile: u32
}

#[derive(Deserialize)]
struct ProductItemResp { id:u64 }

#[derive(Deserialize)]
struct ProductPkgResp { id:u64 }

#[derive(Deserialize)]
struct ProductInfoResp {
    item: Vec<ProductItemResp>, 
    pkg: Vec<ProductPkgResp>, 
}

#[derive(PartialEq, Debug)]
pub enum EditProductPolicyResult {
    OK, Other(AppErrorCode),
}

impl EditProductPolicyUseCase {
    pub async fn execute(self) -> Self
    {
        match self {
            Self::INPUT { profile_id, data, app_state } =>
                Self::_execute(data, app_state, profile_id).await,
            Self::OUTPUT { result, client_err } =>
                Self::OUTPUT { result, client_err }
        }
    }

    async fn _execute(data: Vec<ProductPolicyDto>, appstate: AppSharedState,
                      usr_prof_id : u32) -> Self
    {
        if let Err(ce) = ProductPolicyModelSet::validate(&data) {
            return Self::OUTPUT { client_err: Some(ce),
                result: EditProductPolicyResult::Other(AppErrorCode::InvalidInput) };
        }
        let log = appstate.log_context();
        let rpc = appstate.rpc();
        let rpctype = rpc.label();
        let result = Self::check_product_existence(&data,
                            rpc, initiate_rpc_request, usr_prof_id).await;
        if let Err((code, detail)) = result {
            if code == EditProductPolicyResult::Other(AppErrorCode::RpcRemoteInvalidReply) &&
                rpctype == "dummy" {
                // pass, for mocking purpose, TODO: better design
                app_log_event!(log, AppLogLevel::WARNING, "dummy rpc is applied");
            } else {
                app_log_event!(log, AppLogLevel::ERROR, "detail:{:?}", detail);
                return Self::OUTPUT { client_err:None, result:code };
            } 
        } else if let Ok(missing_prod_ids) = result {
            if !missing_prod_ids.is_empty() {
                app_log_event!(log, AppLogLevel::ERROR, "missing_prod_ids:{:?}", missing_prod_ids);
                let code = EditProductPolicyResult::Other(AppErrorCode::InvalidInput);
                let c_err = missing_prod_ids.into_iter().map(|product_id|{
                    ProductPolicyClientErrorDto { product_id, auto_cancel_secs: None,
                        err_type: format!("{:?}", AppErrorCode::ProductNotExist),
                        warranty_hours:None }
                }).collect();
                return Self::OUTPUT {result: code, client_err:Some(c_err)};
            }
        }
        if let Err(e) = Self::_save_to_repo(appstate.datastore(), &data, usr_prof_id).await
        {
            app_log_event!(log, AppLogLevel::ERROR, "error:{:?}", e);
            Self::OUTPUT {result:EditProductPolicyResult::Other(e.code), client_err:None}
        } else {
            Self::OUTPUT {result:EditProductPolicyResult::OK, client_err:None }
        }
    } // end of _execute


    pub async fn check_product_existence (
        data: &Vec<ProductPolicyDto>,
        rpc_ctx: Arc<Box<dyn AbstractRpcContext>>,
        run_rpc_fn: AppUCrunRPCfn<impl Future<Output = AppUseKsRPCreply>>,
        usr_prof_id: u32 ) -> DefaultResult<Vec<u64>, (EditProductPolicyResult, String)>
    {
        let mut msg_req = ProductInfoReq {
            pkg_ids: Vec::new(), pkg_fields: Vec::new(), profile: usr_prof_id,
            item_ids: Vec::new(), item_fields : vec!["id".to_string()]
        };
        let _: Vec<()>  = data.iter().map(|item| {
            msg_req.item_ids.push(item.product_id);
        }).collect();
        let msgbody = serde_json::to_string(&msg_req).unwrap().into_bytes();
        let properties = AppRpcClientReqProperty {
            retry:3u8, msgbody, route:"product.get_product".to_string()
        };
        match run_rpc_fn(rpc_ctx, properties).await {
            Ok(r) => match String::from_utf8(r.body) {
                Ok(r) => match serde_json::from_str::<ProductInfoResp>(r.as_str())
                {
                    Ok(reply) => Ok(Self::_compare_rpc_reply(reply, data)),
                    Err(e) => {
                        let code = EditProductPolicyResult::Other(AppErrorCode::RpcRemoteInvalidReply);
                        Err((code, e.to_string()))
                    }
                },
                Err(e) => Err((
                        EditProductPolicyResult::Other(AppErrorCode::DataCorruption),
                        e.utf8_error().to_string()  )),
            },
            Err(e) => {
                let detail = e.detail.unwrap_or("RPC undefined error".to_string());
                Err((EditProductPolicyResult::Other(e.code), detail))
            }
        }
    } // end of check_product_existence 

    fn _compare_rpc_reply (reply:ProductInfoResp, req:&Vec<ProductPolicyDto>)
        -> Vec<u64>
    {
        let iter1 = reply.item.iter().map(|x| {x.id});
        let iter2 = reply.pkg.iter().map(|x| {x.id});
        let iter3 = req.iter().map(|x| {x.product_id});
        let mut c1:HashSet<u64, RandomState> = HashSet::from_iter(iter1);
        c1.extend(iter2);
        let c2 = HashSet::from_iter(iter3);
        c2.difference(&c1).map(u64::clone).collect() // c1 == c2
    }

    async fn _save_to_repo(ds:Arc<AppDataStoreContext>, data:&Vec<ProductPolicyDto>,
                           usr_id : u32)  -> DefaultResult<(), AppError>
    {
        let repo = app_repo_product_policy(ds)?;
        let ids = data.iter().map(|d| {d.product_id}).collect();
        let previous_saved = repo.fetch(usr_id, ids).await?;
        let updated = previous_saved.update(usr_id, data)?;
        repo.save(updated).await ?;
        Ok(())
    }
} // end of impl EditProductPolicyUseCase


pub enum EditProductPolicyUseCase {
    INPUT {
        profile_id : u32,
        data : Vec<ProductPolicyDto>,
        app_state : AppSharedState
    },
    OUTPUT {
        result: EditProductPolicyResult,
        client_err: Option<Vec<ProductPolicyClientErrorDto>>
    }
}
