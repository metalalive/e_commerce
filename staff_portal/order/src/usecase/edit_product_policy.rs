use std::future::Future;
use std::sync::Arc;
use std::vec::Vec;
use std::result::Result as DefaultResult;
use std::collections::HashSet;
use std::collections::hash_map::RandomState;

use serde::{Serialize, Deserialize};
use crate::repository::app_repo_product_policy;
use crate::{AppSharedState, AppRpcTypeCfg, app_log_event, AppDataStoreContext} ;
use crate::error::{AppErrorCode, AppError};
use crate::rpc::{AbstractRpcContext, AppRpcPublishProperty};
use crate::logging::{AppLogLevel, AppLogContext};

use crate::api::web::dto::ProductPolicyDto;

use super::{run_rpc, AppUCrunRPCfn, AppUCrunRPCreturn};

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
    OK, ProductNotExists,
    Other(AppErrorCode),
}

impl EditProductPolicyUseCase {
    pub async fn execute(self) -> Self
    {
        match self {
            Self::INPUT { profile_id, data, app_state } =>
                Self::_execute(data, app_state, profile_id).await,
            Self::OUTPUT { result, detail } => Self::OUTPUT { result, detail }
        }
    }

    async fn _execute(data: Vec<ProductPolicyDto>, appstate: AppSharedState,
                      usr_prof_id : u32) -> Self
    {
        if data.is_empty() {
            return Self::_gen_output_error(
                EditProductPolicyResult::Other(AppErrorCode::EmptyInputData) ,
                None );
        }
        let log = appstate.log_context();
        let rpc = appstate.rpc();
        let rpctype = rpc.label();
        if let Err((code, detail)) = Self::check_product_existence(
            &data, rpc, run_rpc, usr_prof_id).await
        {
            if code == EditProductPolicyResult::Other(AppErrorCode::RpcRemoteInvalidReply)
                && rpctype == AppRpcTypeCfg::dummy
            {  // pass, for mocking purpose, TODO: better design
               app_log_event!(log, AppLogLevel::WARNING, "dummy rpc is applied");
            } else {
                return Self::_gen_output_error(code, Some(detail));
            }
        }
        if let Err(e) = Self::_save_to_repo(appstate.datastore(), &data, usr_prof_id).await
        {
            app_log_event!(log, AppLogLevel::ERROR, "{:?}", e);
            let uc_errcode = EditProductPolicyResult::Other(e.code);
            Self::_gen_output_error(uc_errcode, e.detail)
        } else {
            Self::OUTPUT {result: EditProductPolicyResult::OK,
                detail: Some("{}".to_string()) }
        }
    } // end of _execute


    pub async fn check_product_existence (
        data: &Vec<ProductPolicyDto>,
        rpc_ctx: Arc<Box<dyn AbstractRpcContext>>,
        run_rpc_fn: AppUCrunRPCfn<impl Future<Output = AppUCrunRPCreturn>>,
        usr_prof_id: u32 ) -> DefaultResult<(), (EditProductPolicyResult, String)>
    {
        let mut msg_req = ProductInfoReq {
            pkg_ids: Vec::new(), pkg_fields: Vec::new(), profile: usr_prof_id,
            item_ids: Vec::new(), item_fields : vec!["id".to_string()]
        };
        let _: Vec<()>  = data.iter().map(|item| {
            msg_req.item_ids.push(item.product_id);
        }).collect();
        let msgbody = serde_json::to_string(&msg_req).unwrap();
        let properties = AppRpcPublishProperty {
            retry:3u8, msgbody, route:"product.get_product".to_string()
        };
        let reply = match run_rpc_fn(rpc_ctx, properties).await
        {
            Ok(r) => match serde_json::from_str::<ProductInfoResp>(r.body.as_str())
            {
                Ok(s) => s,
                Err(e) => {
                    let code = EditProductPolicyResult::Other(AppErrorCode::RpcRemoteInvalidReply);
                    return Err((code, e.to_string()));
                }
            },
            Err(e) => {
                let detail = e.detail.unwrap_or("RPC undefined error".to_string());
                return Err((EditProductPolicyResult::Other(e.code), detail));
            }
        };
        if Self::_compare_rpc_reply(reply, data) {
            Ok(())
        } else {
            let detail = "xxx".to_string();
            Err((EditProductPolicyResult::ProductNotExists, detail))
        }
    } // end of check_product_existence
   

    fn _compare_rpc_reply (reply:ProductInfoResp, req:&Vec<ProductPolicyDto>) -> bool
    {
        let n = reply.item.len() + reply.pkg.len();
        if n != req.len() { return false; }
        let iter1 = reply.item.iter().map(|x| {x.id});
        let iter2 = reply.pkg.iter().map(|x| {x.id});
        let iter3 = req.iter().map(|x| {x.product_id});
        let mut c1:HashSet<u64, RandomState> = HashSet::from_iter(iter1);
        c1.extend(iter2);
        let c2 = HashSet::from_iter(iter3);
        c1 == c2
    }

    async fn _save_to_repo(ds:Arc<AppDataStoreContext>, data:&Vec<ProductPolicyDto>,
                           usr_id : u32)  -> DefaultResult<(), AppError>
    {
        let repo = app_repo_product_policy(ds)?;
        let ids = data.iter().map(|d| {d.product_id}).collect();
        let previous_saved = repo.fetch(usr_id, ids).await?;
        let updated = previous_saved.update(data);
        repo.save(updated).await ?;
        Ok(())
    }

    fn _gen_output_error (result:EditProductPolicyResult, reason:Option<String>)
        -> Self
    {
        let msg = if let Some(r) = reason {
            let o = format!(r#"{{"reason":"{:?}"}}"#, r);
            Some(o)
        } else { None };
        Self::OUTPUT{ detail: msg, result }
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
        detail: Option<String>
    }
}
