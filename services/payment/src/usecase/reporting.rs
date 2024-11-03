use std::result::Result;

use crate::adapter::repository::{AbstractMerchantRepo, AbstractReportingRepo, AppRepoError};
use crate::api::web::dto::{ReportChargeRespDto, ReportTimeRangeDto};
use crate::auth::AppAuthedClaim;

#[derive(Debug)]
pub enum MerchantReportChargeUcError {
    DataStore(AppRepoError),
    MissingMerchant(u32),
    PermissionDenied(u32),
}

pub struct MerchantReportChargeUseCase {
    authed_claim: AppAuthedClaim,
    repo_mc: Box<dyn AbstractMerchantRepo>,
    repo_rpt: Box<dyn AbstractReportingRepo>,
}

impl MerchantReportChargeUseCase {
    pub fn new(
        authed_claim: AppAuthedClaim,
        repo_mc: Box<dyn AbstractMerchantRepo>,
        repo_rpt: Box<dyn AbstractReportingRepo>,
    ) -> Self {
        Self {
            authed_claim,
            repo_rpt,
            repo_mc,
        }
    }

    pub async fn execute(
        self,
        merchant_id: u32,
        time_range: ReportTimeRangeDto,
    ) -> Result<ReportChargeRespDto, MerchantReportChargeUcError> {
        let staff_usr_id = self.authed_claim.profile;
        let merc_prof = self
            .repo_mc
            .fetch_profile(merchant_id)
            .await
            .map_err(MerchantReportChargeUcError::DataStore)?
            .ok_or(MerchantReportChargeUcError::MissingMerchant(merchant_id))?;
        if !merc_prof.valid_staff(staff_usr_id) {
            return Err(MerchantReportChargeUcError::PermissionDenied(staff_usr_id));
        }
        let saved_charges = self
            .repo_rpt
            .fetch_charges_by_merchant(merchant_id, time_range)
            .await
            .map_err(MerchantReportChargeUcError::DataStore)?;
        let summary = ReportChargeRespDto::from(saved_charges);
        Ok(summary)
    }
} // end of impl MerchantReportChargeUseCase
