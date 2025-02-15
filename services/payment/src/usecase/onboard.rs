use std::boxed::Box;
use std::sync::Arc;

use chrono::Local;

use ecommerce_common::adapter::rpc::py_celery;
use ecommerce_common::api::rpc::dto::{StoreProfileReplicaDto, StoreProfileReplicaReqDto};
use ecommerce_common::error::AppErrorCode;

use crate::adapter::processor::{AbstractPaymentProcessor, AppProcessorError};
use crate::adapter::repository::{AbstractMerchantRepo, AppRepoError};
use crate::adapter::rpc::{AbstractRpcContext, AppRpcClientRequest, AppRpcCtxError};
use crate::api::web::dto::{StoreOnboardReqDto, StoreOnboardRespDto};
use crate::auth::{AppAuthPermissionCode, AppAuthedClaim};
use crate::model::{Label3party, MerchantModelError, MerchantProfileModel};

pub enum OnboardStoreUcError {
    RpcStoreReplica(AppRpcCtxError),
    RpcMsgSerialize(AppErrorCode, String),
    CorruptedStoreProfile(Box<Vec<u8>>, String),
    InvalidStoreProfile(MerchantModelError),
    InvalidStoreSupervisor(u32),
    PermissionDenied(u32),
    ThirdParty(AppProcessorError),
    RepoCreate(AppRepoError),
}

pub struct OnboardStoreUseCase {
    pub auth_claim: AppAuthedClaim,
    pub processors: Arc<Box<dyn AbstractPaymentProcessor>>,
    pub rpc_ctx: Arc<Box<dyn AbstractRpcContext>>,
    pub repo: Box<dyn AbstractMerchantRepo>,
}

pub struct RefreshOnboardStatusUseCase {
    pub auth_claim: AppAuthedClaim,
    pub processors: Arc<Box<dyn AbstractPaymentProcessor>>,
    pub repo: Box<dyn AbstractMerchantRepo>,
}

impl From<AppRpcCtxError> for OnboardStoreUcError {
    fn from(value: AppRpcCtxError) -> Self {
        Self::RpcStoreReplica(value)
    }
}
impl From<MerchantModelError> for OnboardStoreUcError {
    fn from(value: MerchantModelError) -> Self {
        Self::InvalidStoreProfile(value)
    }
}
impl From<AppProcessorError> for OnboardStoreUcError {
    fn from(value: AppProcessorError) -> Self {
        Self::ThirdParty(value)
    }
}
impl From<AppRepoError> for OnboardStoreUcError {
    fn from(value: AppRepoError) -> Self {
        Self::RepoCreate(value)
    }
}

fn uc_permission_check(claim: &AppAuthedClaim) -> Result<(), OnboardStoreUcError> {
    let auth_usr_id = claim.profile;
    let success = claim.contain_permission(AppAuthPermissionCode::can_onboard_merchant);
    if success {
        Ok(())
    } else {
        Err(OnboardStoreUcError::PermissionDenied(auth_usr_id))
    }
}

impl OnboardStoreUseCase {
    pub async fn execute(
        &self,
        store_id: u32,
        req_body: StoreOnboardReqDto,
    ) -> Result<StoreOnboardRespDto, OnboardStoreUcError> {
        uc_permission_check(&self.auth_claim)?;
        let auth_usr_id = self.auth_claim.profile;
        let storeprof_d = self._rpc_validate_store(store_id).await?;
        let storeprof_m = MerchantProfileModel::try_from((store_id, &storeprof_d))?;
        if !storeprof_m.valid_supervisor(auth_usr_id) {
            let e = OnboardStoreUcError::InvalidStoreSupervisor(auth_usr_id);
            return Err(e);
        }
        let res_3pty = self
            .processors
            .onboard_merchant(storeprof_d, req_body)
            .await?;
        let (res_dto, m3pty) = res_3pty.into_parts();
        self.repo.create(storeprof_m, m3pty).await?;
        Ok(res_dto)
    }

    async fn _rpc_validate_store(
        &self,
        store_id: u32,
    ) -> Result<StoreProfileReplicaDto, OnboardStoreUcError> {
        let client = self.rpc_ctx.acquire().await?;
        let usr_id = self.auth_claim.profile;
        let time = Local::now().to_utc();
        let route = "rpc.storefront.get_profile".to_string();
        let message = {
            let q = StoreProfileReplicaReqDto { store_id };
            py_celery::serialize_msg_body(q)
                .map_err(|(code, detail)| OnboardStoreUcError::RpcMsgSerialize(code, detail))?
        };
        let props = AppRpcClientRequest {
            usr_id,
            time,
            message,
            route,
        };
        let mut pub_evt = client.send_request(props).await?;
        let reply = pub_evt.receive_response().await?;
        py_celery::deserialize_reply::<StoreProfileReplicaDto>(&reply.message).map_err(
            |(_code, detail)| {
                OnboardStoreUcError::CorruptedStoreProfile(Box::new(reply.message), detail)
            },
        )
    }
} // end of impl OnboardStoreUseCase

impl RefreshOnboardStatusUseCase {
    pub async fn execute(
        &self,
        store_id: u32,
        req_body: StoreOnboardReqDto,
    ) -> Result<StoreOnboardRespDto, OnboardStoreUcError> {
        uc_permission_check(&self.auth_claim)?;
        let auth_usr_id = self.auth_claim.profile;
        let label3pt = Label3party::from(&req_body);
        let (storeprof_m, store3pty_m) = self.repo.fetch(store_id, label3pt).await?.ok_or(
            OnboardStoreUcError::InvalidStoreProfile(MerchantModelError::NotExist),
        )?;
        if !storeprof_m.valid_supervisor(auth_usr_id) {
            let e = OnboardStoreUcError::InvalidStoreSupervisor(auth_usr_id);
            return Err(e);
        }
        let res_3pty = self
            .processors
            .refresh_onboard_status(store3pty_m, req_body)
            .await?;
        let (res_dto, store3pty_m) = res_3pty.into_parts();
        self.repo.update_3party(storeprof_m.id, store3pty_m).await?;
        Ok(res_dto)
    }
} // end of impl RefreshOnboardStatusUseCase
