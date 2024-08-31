use std::result::Result;

use ecommerce_common::api::rpc::dto::StoreProfileReplicaDto;

use crate::adapter::processor::AppProcessorMerchantResult;

#[derive(Debug)]
pub struct MerchantModelError;

pub struct MerchantProfileModel;

impl TryFrom<&StoreProfileReplicaDto> for MerchantProfileModel {
    type Error = MerchantModelError;
    fn try_from(_value: &StoreProfileReplicaDto) -> Result<Self, Self::Error> {
        Ok(Self)
    }
}

impl MerchantProfileModel {
    pub(crate) fn update_3pty(&mut self, _res_3pty: &AppProcessorMerchantResult) {}
}
