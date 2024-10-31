use std::boxed::Box;
use std::result::Result;
use std::sync::Arc;

use chrono::{DateTime, Local, Utc};

use ecommerce_common::api::rpc::dto::OrderPaymentUpdateErrorDto;
use ecommerce_common::error::AppErrorCode;

use crate::adapter::processor::{AbstractPaymentProcessor, AppProcessorError};
use crate::adapter::repository::{AbstractChargeRepo, AppRepoError};
use crate::adapter::rpc::{AbstractRpcContext, AppRpcClientRequest, AppRpcCtxError};
use crate::api::web::dto::ChargeRefreshRespDto;
use crate::model::{BuyerPayInState, ChargeBuyerMetaModel};
use crate::{AppAuthPermissionCode, AppAuthedClaim};

use super::try_parse_charge_id;

pub enum ChargeRefreshUcError {
    OwnerMismatch,
    PermissionDenied(u32),
    ChargeNotExist(u32, DateTime<Utc>),
    DataStore(AppRepoError),
    RpcContext(AppRpcCtxError),
    RpcContentSerialisation(String),
    ExternalProcessor(AppProcessorError),
    ChargeIdDecode(AppErrorCode, String),
    RpcUpdateOrder(OrderPaymentUpdateErrorDto),
}

pub struct ChargeStatusRefreshUseCase {
    pub processors: Arc<Box<dyn AbstractPaymentProcessor>>,
    pub repo: Box<dyn AbstractChargeRepo>,
    pub rpc_ctx: Arc<Box<dyn AbstractRpcContext>>,
}

impl ChargeStatusRefreshUseCase {
    pub async fn execute(
        self,
        authed_claim: AppAuthedClaim,
        charge_id_serial: String,
    ) -> Result<ChargeRefreshRespDto, ChargeRefreshUcError> {
        let auth_usr_id = authed_claim.profile;
        let success =
            authed_claim.contain_permission(AppAuthPermissionCode::can_update_charge_progress);
        if !success {
            return Err(ChargeRefreshUcError::PermissionDenied(auth_usr_id));
        }
        let (owner_id, create_time) = try_parse_charge_id(charge_id_serial.as_str())
            .map_err(|(code, detail)| ChargeRefreshUcError::ChargeIdDecode(code, detail))?;
        if owner_id != auth_usr_id {
            return Err(ChargeRefreshUcError::OwnerMismatch);
        }
        let mut saved_meta = self
            .repo
            .fetch_charge_meta(owner_id, create_time)
            .await
            .map_err(ChargeRefreshUcError::DataStore)?
            .ok_or(ChargeRefreshUcError::ChargeNotExist(owner_id, create_time))?;
        let proceed_allowed = match saved_meta.method_3party().pay_in_comfirmed() {
            Some(confirmed) => confirmed,
            None => self.refresh_3pty_processor(&mut saved_meta).await?,
        };
        let result_rpc = if proceed_allowed && !saved_meta.progress().completed() {
            self.sync_order_app(&mut saved_meta).await
            // postpone the error return, always write charge status to database
            // repository if necessary
        } else {
            Ok(())
        };
        let resp = ChargeRefreshRespDto::from(&saved_meta);
        // TODO,
        // - logging dto if buy-in state is `completed` but the state from 3rd party
        //   processor is `processing`
        // - clever way of detecting the state of charge meta model has been modified.
        self.repo
            .update_charge_progress(saved_meta)
            .await
            .map_err(ChargeRefreshUcError::DataStore)?;
        result_rpc?;
        Ok(resp)
    } // end of fn execute()

    async fn refresh_3pty_processor(
        &self,
        meta: &mut ChargeBuyerMetaModel,
    ) -> Result<bool, ChargeRefreshUcError> {
        let mthd_3pty = self
            .processors
            .pay_in_progress(meta)
            .await
            .map_err(ChargeRefreshUcError::ExternalProcessor)?;
        let proceed_allowed = mthd_3pty
            .pay_in_comfirmed()
            .map(|confirmed| {
                // always update state regardless of success signal from 3rd party
                let now = Local::now().to_utc();
                let new_state = BuyerPayInState::ProcessorCompleted(now);
                meta.update_progress(&new_state);
                meta.update_3party(mthd_3pty);
                confirmed
            })
            .unwrap_or(false);
        Ok(proceed_allowed)
    }

    async fn sync_order_app(
        &self,
        meta: &mut ChargeBuyerMetaModel,
    ) -> Result<(), ChargeRefreshUcError> {
        let client = self
            .rpc_ctx
            .acquire()
            .await
            .map_err(ChargeRefreshUcError::RpcContext)?;
        let message = self.rpc_build_charge_lines(meta).await?;
        let props = AppRpcClientRequest {
            usr_id: meta.owner(),
            // Note, the reason to specify this `create-time` field instead of current
            // time is that order-processing service can handle idempotency based on
            // this create time, TODO, find better design approach
            time: *meta.create_time(),
            route: "rpc.order.order_reserved_update_payment".to_string(),
            message,
        };
        let mut event = client
            .send_request(props)
            .await
            .map_err(ChargeRefreshUcError::RpcContext)?;
        let reply = event
            .receive_response()
            .await
            .map_err(ChargeRefreshUcError::RpcContext)?;
        let resp_detail = serde_json::from_slice::<OrderPaymentUpdateErrorDto>(&reply.message)
            .map_err(|e| ChargeRefreshUcError::RpcContentSerialisation(e.to_string()))?;
        let has_err = resp_detail.charge_time.is_some() | !resp_detail.lines.is_empty();
        if has_err {
            Err(ChargeRefreshUcError::RpcUpdateOrder(resp_detail))
        } else {
            let now = Local::now().to_utc();
            let new_state = BuyerPayInState::OrderAppSynced(now);
            meta.update_progress(&new_state);
            Ok(())
        }
    } // end of fn sync_order_app

    async fn rpc_build_charge_lines(
        &self,
        meta: &ChargeBuyerMetaModel,
    ) -> Result<Vec<u8>, ChargeRefreshUcError> {
        let chg_lines = self
            .repo
            .fetch_all_charge_lines(meta.owner(), *meta.create_time())
            .await
            .map_err(ChargeRefreshUcError::DataStore)?;
        let update_dto = meta.pay_update_dto(chg_lines);
        let serialised = serde_json::to_vec(&update_dto)
            .map_err(|e| ChargeRefreshUcError::RpcContentSerialisation(e.to_string()))?;
        Ok(serialised)
    }
} // end of impl ChargeStatusRefreshUseCase
