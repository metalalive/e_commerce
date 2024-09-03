use std::result::Result;

use super::external_processor::Merchant3partyStripeModel;
use ecommerce_common::api::rpc::dto::StoreProfileReplicaDto;

#[derive(Debug)]
pub struct MerchantModelError;

pub enum Merchant3partyModel {
    Stripe(Merchant3partyStripeModel),
    Unknown,
}

pub struct MerchantProfileModel {
    m3pty: Merchant3partyModel,
}

impl Default for Merchant3partyModel {
    fn default() -> Self {
        Self::Unknown
    }
}

impl TryFrom<&StoreProfileReplicaDto> for MerchantProfileModel {
    type Error = MerchantModelError;
    fn try_from(_value: &StoreProfileReplicaDto) -> Result<Self, Self::Error> {
        let m3pty = Merchant3partyModel::default();
        Ok(Self { m3pty })
    }
}

impl MerchantProfileModel {
    pub(crate) fn update_3pty(&mut self, _m3pty: Merchant3partyModel) {}
}
