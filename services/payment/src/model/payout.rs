
use super::{ChargeBuyerModel, Merchant3partyModel, MerchantProfileModel};

#[derive(Debug)]
pub enum PayoutModelError {
    NotImplemented,
}

pub struct PayoutModel {
}

type ConvertFromArgs = (
    ChargeBuyerModel,
    MerchantProfileModel,
    Merchant3partyModel,
    u32,
    Option<PayoutModel>,
);

impl TryFrom<ConvertFromArgs> for PayoutModel {
    type Error = PayoutModelError;
    fn try_from(_value: ConvertFromArgs) -> Result<Self, Self::Error> {
        Err(PayoutModelError::NotImplemented)
    }
} // end of impl PayoutModel
