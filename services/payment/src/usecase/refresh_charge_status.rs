use std::boxed::Box;
use std::result::Result;
use std::sync::Arc;

use chrono::{DateTime, Local, Utc};

use ecommerce_common::error::AppErrorCode;
use ecommerce_common::util::hex_to_octet;

use crate::adapter::processor::{AbstractPaymentProcessor, AppProcessorError};
use crate::adapter::repository::{AbstractChargeRepo, AppRepoError};
use crate::adapter::rpc::{AbstractRpcContext, AppRpcClientRequest, AppRpcCtxError};
use crate::api::web::dto::ChargeRefreshRespDto;
use crate::model::{BuyerPayInState, ChargeBuyerMetaModel, ChargeToken};

pub enum ChargeRefreshUcError {
    OwnerMismatch,
    ChargeNotExist,
    DataStore(AppRepoError),
    RpcContext(AppRpcCtxError),
    ExternalProcessor(AppProcessorError),
    ChargeIdDecode(AppErrorCode, String),
}

pub struct ChargeStatusRefreshUseCase {
    pub processors: Arc<Box<dyn AbstractPaymentProcessor>>,
    pub repo: Box<dyn AbstractChargeRepo>,
    pub rpc_ctx: Arc<Box<dyn AbstractRpcContext>>,
}

impl ChargeStatusRefreshUseCase {
    pub async fn execute(
        self,
        auth_usr_id: u32,
        charge_id_serial: String,
    ) -> Result<ChargeRefreshRespDto, ChargeRefreshUcError> {
        let (owner_id, create_time) = self
            .try_parse_charge_id(charge_id_serial.as_str())
            .map_err(|(ecode, detail)| ChargeRefreshUcError::ChargeIdDecode(ecode, detail))?;
        if owner_id != auth_usr_id {
            return Err(ChargeRefreshUcError::OwnerMismatch);
        }
        let mut saved_meta = self
            .repo
            .fetch_charge_meta(owner_id, create_time)
            .await
            .map_err(ChargeRefreshUcError::DataStore)?
            .ok_or(ChargeRefreshUcError::ChargeNotExist)?;
        let proceed_allowed = if saved_meta.method.pay_in_completed() {
            true
        } else {
            self.refresh_3pty_processor(&mut saved_meta).await?
        };
        if proceed_allowed && !saved_meta.state.completed() {
            self.sync_order_app(&mut saved_meta).await?;
        }
        let resp = ChargeRefreshRespDto::from(&saved_meta);
        self.repo
            .update_charge_progress(saved_meta)
            .await
            .map_err(ChargeRefreshUcError::DataStore)?;
        Ok(resp)
    } // end of fn execute()

    fn try_parse_charge_id(
        &self,
        id_serial: &str,
    ) -> Result<(u32, DateTime<Utc>), (AppErrorCode, String)> {
        let id_octets = hex_to_octet(id_serial)?;
        let token = ChargeToken::try_from(id_octets)?;
        let (owner_id, ctime) = token.try_into()?;
        Ok((owner_id, ctime))
    }

    async fn refresh_3pty_processor(
        &self,
        meta: &mut ChargeBuyerMetaModel,
    ) -> Result<bool, ChargeRefreshUcError> {
        let mthd_3pty = self
            .processors
            .pay_in_progress(meta)
            .await
            .map_err(ChargeRefreshUcError::ExternalProcessor)?;
        let is_completed = mthd_3pty.pay_in_completed();
        if is_completed {
            let now = Local::now().to_utc();
            let new_state = BuyerPayInState::ProcessorCompleted(now);
            meta.update_progress(&new_state);
            meta.method = mthd_3pty;
        }
        Ok(is_completed)
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
            usr_id: meta.owner,
            // Note, the reason to specify this `create-time` field instead of current
            // time is that order-processing service can handle idempotency based on
            // this create time, TODO, find better design approach
            time: meta.create_time,
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
        // TODO
        // - switch the rpc response to `OrderPaymentUpdateErrorDto`
        // - inspect the rpc error
        let _error = serde_json::from_slice::<serde_json::Value>(&reply.message);
        Ok(())
    } //

    async fn rpc_build_charge_lines(
        &self,
        _meta: &ChargeBuyerMetaModel,
    ) -> Result<Vec<u8>, ChargeRefreshUcError> {
        //TODO, load charged lines and serialise
        Ok(Vec::new())
    }
} // end of impl ChargeStatusRefreshUseCase
