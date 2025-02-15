use std::boxed::Box;
use std::sync::Arc;

use chrono::{DurationRound, Local, TimeDelta};
use ecommerce_common::api::dto::GenericRangeErrorDto;
use ecommerce_common::api::rpc::dto::{OrderReplicaPaymentDto, OrderReplicaPaymentReqDto};
use ecommerce_common::api::web::dto::BillingErrorDto;
use ecommerce_common::model::order::BillingModel;

use crate::adapter::cache::{AbstractOrderSyncLockCache, OrderSyncLockError};
use crate::adapter::processor::{AbstractPaymentProcessor, AppProcessorError};
use crate::adapter::repository::{AbstractChargeRepo, AppRepoError};
use crate::adapter::rpc::{AbstractRpcContext, AppRpcClientRequest, AppRpcCtxError};
use crate::api::web::dto::{
    ChargeCreateRespDto, ChargeReqDto, ChargeRespErrorDto, PaymentMethodErrorReason,
};
use crate::model::{ChargeBuyerModel, OrderLineModelSet, OrderModelError};
use crate::{AppAuthPermissionCode, AppAuthQuotaMatCode, AppAuthedClaim};

// TODO, consider to add debug function for logging purpose
pub enum ChargeCreateUcError {
    OrderOwnerMismatch,                   // client error, e.g. status code 403
    ClientBadRequest(ChargeRespErrorDto), // status code 400
    OrderNotExist,
    LockCacheError,
    LoadOrderConflict, // client error, e.g. status code 429
    LoadOrderInternalError(AppRpcCtxError),
    LoadOrderByteCorruption(String),
    RpcBillingParseError(BillingErrorDto),
    RpcOlineParseError(Vec<OrderModelError>),
    ExternalProcessorError(PaymentMethodErrorReason),
    DataStoreError(AppRepoError),
    PermissionDenied(u32),
    QuotaExceedLimit(ChargeRespErrorDto),
}

impl From<OrderSyncLockError> for ChargeCreateUcError {
    fn from(_value: OrderSyncLockError) -> Self {
        Self::LockCacheError
    }
}
impl From<AppRpcCtxError> for ChargeCreateUcError {
    fn from(value: AppRpcCtxError) -> Self {
        Self::LoadOrderInternalError(value)
    }
}
impl From<AppRepoError> for ChargeCreateUcError {
    fn from(value: AppRepoError) -> Self {
        Self::DataStoreError(value)
    }
}
impl From<AppProcessorError> for ChargeCreateUcError {
    fn from(value: AppProcessorError) -> Self {
        Self::ExternalProcessorError(value.reason.into())
    }
}
impl From<serde_json::Error> for ChargeCreateUcError {
    fn from(value: serde_json::Error) -> Self {
        Self::LoadOrderByteCorruption(value.to_string())
    }
}
impl From<BillingErrorDto> for ChargeCreateUcError {
    fn from(value: BillingErrorDto) -> Self {
        Self::RpcBillingParseError(value)
    }
}
impl From<Vec<OrderModelError>> for ChargeCreateUcError {
    fn from(value: Vec<OrderModelError>) -> Self {
        Self::RpcOlineParseError(value)
    }
}
impl From<ChargeRespErrorDto> for ChargeCreateUcError {
    fn from(value: ChargeRespErrorDto) -> Self {
        Self::ClientBadRequest(value)
    }
}

pub struct ChargeCreateUseCase {
    pub processors: Arc<Box<dyn AbstractPaymentProcessor>>,
    pub rpc_ctx: Arc<Box<dyn AbstractRpcContext>>,
    pub ordersync_lockset: Arc<Box<dyn AbstractOrderSyncLockCache>>,
    pub repo: Box<dyn AbstractChargeRepo>,
}

impl ChargeCreateUseCase {
    pub async fn execute(
        &self,
        authed_claim: AppAuthedClaim,
        req_body: ChargeReqDto,
    ) -> Result<ChargeCreateRespDto, ChargeCreateUcError> {
        let usr_id = authed_claim.profile;
        let oid = req_body.order.id.as_str();
        self.validate_access_control(authed_claim, oid).await?;
        let result = self.repo.get_unpaid_olines(usr_id, oid).await?;
        let validated_order = if let Some(v) = result {
            v
        } else {
            let d = self.rpc_sync_order(usr_id, oid).await?;
            self.try_save_order(usr_id, oid, d).await?
        };
        // TODO, verify whether all merchants/shops in the client request support
        // the specific 3rd-party processor
        let resp = self
            .try_execute_processor(validated_order, req_body)
            .await?;
        Ok(resp)
    } // end of fn execute

    #[allow(clippy::field_reassign_with_default)]
    async fn validate_access_control(
        &self,
        authed_claim: AppAuthedClaim,
        oid: &str,
    ) -> Result<(), ChargeCreateUcError> {
        let usr_id = authed_claim.profile;
        let success = authed_claim.contain_permission(AppAuthPermissionCode::can_create_charge);
        if !success {
            return Err(ChargeCreateUcError::PermissionDenied(usr_id));
        }
        let (saved_usr_id, ctimes) = self
            .repo
            .fetch_charge_ids(oid)
            .await?
            .unwrap_or((usr_id, Vec::new()));
        if saved_usr_id != usr_id {
            return Err(ChargeCreateUcError::PermissionDenied(usr_id));
        }
        let limit = authed_claim.quota_limit(AppAuthQuotaMatCode::NumChargesPerOrder);
        let actual_num_charges = ctimes.len() as u32;
        if limit < actual_num_charges {
            let range = GenericRangeErrorDto {
                max_: limit as u16,
                min_: 0u16,
                given: actual_num_charges,
            };
            let mut e = ChargeRespErrorDto::default();
            e.num_charges_exceed = Some(range);
            Err(ChargeCreateUcError::QuotaExceedLimit(e))
        } else {
            Ok(())
        }
    } // end of fn validate_access_control

    async fn rpc_sync_order(
        &self,
        usr_id: u32,
        oid: &str,
    ) -> Result<OrderReplicaPaymentDto, ChargeCreateUcError> {
        let success = self.ordersync_lockset.acquire(usr_id, oid).await?;
        if success {
            let out = self._rpc_sync_order(oid, usr_id).await;
            self.ordersync_lockset.release(usr_id, oid).await?;
            out
        } else {
            Err(ChargeCreateUcError::LoadOrderConflict)
        }
    }
    async fn _rpc_sync_order(
        &self,
        oid: &str,
        usr_id: u32,
    ) -> Result<OrderReplicaPaymentDto, ChargeCreateUcError> {
        let client = self.rpc_ctx.acquire().await?;
        let payld = OrderReplicaPaymentReqDto {
            order_id: oid.to_string(),
        };
        let props = AppRpcClientRequest {
            usr_id,
            time: Local::now()
                .to_utc()
                .duration_trunc(TimeDelta::seconds(6))
                .unwrap(),
            message: serde_json::to_vec(&payld).unwrap(),
            route: "rpc.order.order_reserved_replica_payment".to_string(),
        };
        let mut event = client.send_request(props).await?;
        let reply = event.receive_response().await?;
        let out = serde_json::from_slice::<OrderReplicaPaymentDto>(&reply.message)?;
        Ok(out)
    }

    async fn try_save_order(
        &self,
        usr_id_uncheck: u32,
        oid_uncheck: &str,
        rpc_data: OrderReplicaPaymentDto,
    ) -> Result<OrderLineModelSet, ChargeCreateUcError> {
        let OrderReplicaPaymentDto {
            oid,
            usr_id,
            lines,
            billing,
            currency,
        } = rpc_data;
        let billing = BillingModel::try_from(billing)?;
        let olines = OrderLineModelSet::try_from((oid, usr_id, lines, currency))?;
        self.repo.create_order(&olines, &billing).await?;
        let mismatch = (olines.id.as_str() != oid_uncheck) || (olines.buyer_id != usr_id_uncheck);
        if mismatch {
            Err(ChargeCreateUcError::OrderOwnerMismatch)
        } else {
            Ok(olines)
        }
    }

    async fn try_execute_processor(
        &self,
        saved_order: OrderLineModelSet,
        reqbody: ChargeReqDto,
    ) -> Result<ChargeCreateRespDto, ChargeCreateUcError> {
        let (req_order, req_mthd) = reqbody.into_parts();
        let mut charge_buyer = ChargeBuyerModel::try_from((saved_order, req_order))?;
        let (result, method_m) = self
            .processors
            .pay_in_start(&charge_buyer, req_mthd)
            .await?;
        charge_buyer.meta.update_progress(&result.state);
        charge_buyer.meta.update_3party(method_m);
        self.repo.create_charge(charge_buyer).await?;
        if result.completed {
            // TODO, if the pay-in process is complete, invoke RPC to order service
            // for payment status update
        }
        let resp = ChargeCreateRespDto::from(result);
        Ok(resp)
    }
} // end of impl ChargeCreateUseCase
