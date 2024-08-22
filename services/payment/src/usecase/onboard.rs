use std::boxed::Box;
use std::sync::Arc;

use chrono::Local;

use ecommerce_common::api::rpc::dto::{StoreProfileReplicaDto, StoreProfileReplicaReqDto};

use crate::adapter::processor::{
    AbstractPaymentProcessor, AppProcessorError, AppProcessorMerchantResult,
};
use crate::adapter::repository::{AbstractMerchantRepo, AppRepoError};
use crate::adapter::rpc::{AbstractRpcContext, AppRpcClientRequest, AppRpcCtxError};
use crate::api::web::dto::{StoreOnboardAcceptedRespDto, StoreOnboardReqDto};
use crate::auth::AppAuthedClaim;
use crate::model::{MerchantModelError, MerchantProfileModel};

pub enum OnboardStoreUcError {
    RpcStoreReplica(AppRpcCtxError),
    CorruptedStoreProfile(Box<Vec<u8>>, String),
    InvalidStoreProfile(MerchantModelError),
    ThirdParty(AppProcessorError),
    RepoCreate(AppRepoError),
}

pub enum OnboardStoreUcOk {
    Accepted(StoreOnboardAcceptedRespDto),
}

pub struct OnboardStoreUseCase {
    pub auth_claim: AppAuthedClaim,
    pub processors: Arc<Box<dyn AbstractPaymentProcessor>>,
    pub rpc_ctx: Arc<Box<dyn AbstractRpcContext>>,
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
impl From<AppProcessorMerchantResult> for OnboardStoreUcOk {
    fn from(value: AppProcessorMerchantResult) -> Self {
        match value {
            AppProcessorMerchantResult::Stripe => {
                Self::Accepted(StoreOnboardAcceptedRespDto::Stripe)
            }
        }
    }
}

impl OnboardStoreUseCase {
    pub async fn execute(
        &self,
        store_id: u32,
        req_body: StoreOnboardReqDto,
    ) -> Result<OnboardStoreUcOk, OnboardStoreUcError> {
        let storeprof_d = self._rpc_validate_store(store_id).await?;
        let mut storeprof_m = MerchantProfileModel::try_from(&storeprof_d)?;
        let res_3pty = self
            .processors
            .onboard_merchant(storeprof_d, req_body)
            .await?;
        storeprof_m.update_3pty(&res_3pty);
        self.repo.create(storeprof_m).await?;
        Ok(OnboardStoreUcOk::from(res_3pty))
    }

    async fn _rpc_validate_store(
        &self,
        store_id: u32,
    ) -> Result<StoreProfileReplicaDto, OnboardStoreUcError> {
        let client = self.rpc_ctx.acquire().await?;
        let usr_id = self.auth_claim.profile;
        let time = Local::now().to_utc();
        let route = "rpc.store.profile_replica".to_string();
        let message = {
            let q = StoreProfileReplicaReqDto { store_id };
            serde_json::to_vec(&q).unwrap()
        };
        let props = AppRpcClientRequest {
            usr_id,
            time,
            message,
            route,
        };
        let mut pub_evt = client.send_request(props).await?;
        let reply = pub_evt.receive_response().await?;
        serde_json::from_slice::<StoreProfileReplicaDto>(&reply.message).map_err(|e| {
            OnboardStoreUcError::CorruptedStoreProfile(Box::new(reply.message), e.to_string())
        })
    }
} // end of impl OnboardStoreUseCase
