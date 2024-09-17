use std::boxed::Box;
use std::result::Result;
use std::sync::Arc;

use ecommerce_common::error::AppErrorCode;

use crate::adapter::processor::{AbstractPaymentProcessor, AppProcessorError};
use crate::adapter::repository::{AbstractChargeRepo, AbstractMerchantRepo, AppRepoError};
use crate::api::web::dto::CapturePayRespDto;
use crate::auth::AppAuthedClaim;
use crate::model::{BuyerPayInState, Label3party, PayoutModel, PayoutModelError};

use super::try_parse_charge_id;

pub enum ChargeCaptureUcError {
    ChargeIdDecode(AppErrorCode, String),
    MissingCharge,
    MissingMerchant,
    InvalidMerchantStaff(u32),
    PayInNotCompleted(BuyerPayInState),
    CorruptedPayMethod(String),
    CorruptedModel(PayoutModelError),
    ThirdParty(AppProcessorError),
    RepoOpFailure(AppRepoError),
}

pub struct ChargeCaptureUseCase {
    pub auth_claim: AppAuthedClaim,
    pub processors: Arc<Box<dyn AbstractPaymentProcessor>>,
    pub repo_c: Box<dyn AbstractChargeRepo>,
    pub repo_m: Box<dyn AbstractMerchantRepo>,
}

impl ChargeCaptureUseCase {
    pub async fn execute(
        self,
        charge_id: String,
        store_id: u32,
    ) -> Result<CapturePayRespDto, ChargeCaptureUcError> {
        let (buyer_id, charge_ctime) = try_parse_charge_id(charge_id.as_str())
            .map_err(|e| ChargeCaptureUcError::ChargeIdDecode(e.0, e.1))?;
        let charge_m = self
            .repo_c
            .fetch_charge_by_merchant(buyer_id, charge_ctime, store_id)
            .await
            .map_err(ChargeCaptureUcError::RepoOpFailure)?
            .ok_or(ChargeCaptureUcError::MissingCharge)?;

        if !charge_m.meta.progress().completed() {
            let curr_prog = charge_m.meta.progress().clone();
            let e = ChargeCaptureUcError::PayInNotCompleted(curr_prog);
            return Err(e);
        }
        let label3pty = {
            let m3pt = charge_m.meta.method_3party();
            Label3party::try_from(m3pt).map_err(ChargeCaptureUcError::CorruptedPayMethod)?
        };

        let (merchant_prof, merchant_3pty) = self
            .repo_m
            .fetch(store_id, label3pty)
            .await
            .map_err(ChargeCaptureUcError::RepoOpFailure)?
            .ok_or(ChargeCaptureUcError::MissingMerchant)?;

        let merchant_staff_id = self.auth_claim.profile;
        if !merchant_prof.valid_staff(merchant_staff_id) {
            let e = ChargeCaptureUcError::InvalidMerchantStaff(merchant_staff_id);
            return Err(e);
        }

        let opt_payout_m = self
            .repo_c
            .fetch_payout(store_id, buyer_id, charge_ctime)
            .await
            .map_err(ChargeCaptureUcError::RepoOpFailure)?;

        let payout_m = {
            let arg = (
                charge_m,
                merchant_prof,
                merchant_3pty,
                merchant_staff_id,
                opt_payout_m,
            );
            PayoutModel::try_from(arg).map_err(ChargeCaptureUcError::CorruptedModel)?
        };

        let result = self
            .processors
            .pay_out(payout_m)
            .await
            .map_err(ChargeCaptureUcError::ThirdParty)?;

        let (respdto, payout_m) = result.into_parts();
        self.repo_c
            .create_payout(payout_m)
            .await
            .map_err(ChargeCaptureUcError::RepoOpFailure)?;

        Ok(respdto)
    } // end of fn execute
} // end of impl ChargeCaptureUseCase
