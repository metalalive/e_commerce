use std::boxed::Box;
use std::future::Future;
use std::marker::Send;
use std::pin::Pin;
use std::result::Result;
use std::sync::Arc;

use chrono::{DateTime, Utc};

use crate::adapter::processor::{AbstractPaymentProcessor, AppProcessorError};
use crate::adapter::repository::{
    AbstractChargeRepo, AbstractMerchantRepo, AbstractRefundRepo, AppRefundRslvReqCbReturn,
    AppRepoError, AppRepoErrorDetail,
};
use crate::api::web::dto::{RefundCompletionReqDto, RefundCompletionRespDto};
use crate::model::{ChargeBuyerModel, OrderRefundModel};

#[derive(Debug)]
pub enum FinalizeRefundUcError {
    DataStore(AppRepoError),
    MissingMerchant(u32),
    PermissionDenied(u32),
    MissingChargeId(String),
    MissingCharge(u32, DateTime<Utc>),
}

pub struct FinalizeRefundUseCase<'a> {
    pub repo_ch: Box<dyn AbstractChargeRepo>,
    pub repo_mc: Box<dyn AbstractMerchantRepo>,
    pub repo_rfd: Box<dyn AbstractRefundRepo<'a>>,
    pub processors: Arc<Box<dyn AbstractPaymentProcessor>>,
}

impl<'a> FinalizeRefundUseCase<'a> {
    pub async fn execute(
        self,
        oid: String,
        merchant_id: u32,
        staff_usr_id: u32,
        cmplt_req: RefundCompletionReqDto,
    ) -> Result<(RefundCompletionRespDto, Vec<AppProcessorError>), FinalizeRefundUcError> {
        let Self {
            repo_ch,
            repo_mc,
            repo_rfd,
            processors,
        } = self;
        let merc_prof = repo_mc
            .fetch_profile(merchant_id)
            .await
            .map_err(FinalizeRefundUcError::DataStore)?
            .ok_or(FinalizeRefundUcError::MissingMerchant(merchant_id))?;
        if !merc_prof.valid_staff(staff_usr_id) {
            return Err(FinalizeRefundUcError::PermissionDenied(staff_usr_id));
        }
        let (buyer_usr_id, charged_dtimes) = repo_ch
            .fetch_charge_ids(oid.as_str())
            .await
            .map_err(FinalizeRefundUcError::DataStore)?
            .ok_or(FinalizeRefundUcError::MissingChargeId(oid.clone()))?;

        let mut charge_ms = Vec::new();
        for ctime in charged_dtimes {
            let charge_m = repo_ch
                .fetch_charge_by_merchant(buyer_usr_id, ctime, merchant_id)
                .await
                .map_err(FinalizeRefundUcError::DataStore)?
                .ok_or(FinalizeRefundUcError::MissingCharge(buyer_usr_id, ctime))?;
            charge_ms.push(charge_m);
        }

        let result_rslv = repo_rfd
            .resolve_request(cmplt_req, charge_ms, processors, Self::hdlr_load_refund_req)
            .await
            .map_err(FinalizeRefundUcError::DataStore)?;

        let mut rslv_ms = Vec::new();
        let mut errs_proc = Vec::new();
        result_rslv
            .into_iter()
            .map(|r| match r {
                Ok(v) => {
                    rslv_ms.push(v);
                }
                Err(e) => {
                    errs_proc.push(e);
                }
            })
            .count();

        let o = RefundCompletionRespDto::from(rslv_ms);
        Ok((o, errs_proc))
    } // end of fn execute

    fn hdlr_load_refund_req(
        refund_m: &'a mut OrderRefundModel,
        cmplt_req: RefundCompletionReqDto,
        charge_ms: Vec<ChargeBuyerModel>,
        processor: Arc<Box<dyn AbstractPaymentProcessor>>,
    ) -> Pin<Box<dyn Future<Output = AppRefundRslvReqCbReturn> + Send + 'a>> {
        let fut =
            async move { Self::_load_refund_req(refund_m, cmplt_req, charge_ms, processor).await };
        Box::pin(fut)
    }
    async fn _load_refund_req(
        refund_m: &mut OrderRefundModel,
        mut cmplt_req: RefundCompletionReqDto,
        charge_ms: Vec<ChargeBuyerModel>,
        processor: Arc<Box<dyn AbstractPaymentProcessor>>,
    ) -> AppRefundRslvReqCbReturn {
        refund_m
            .validate(&cmplt_req)
            .map_err(AppRepoErrorDetail::RefundResolution)?;

        let mut out = Vec::new();
        for charge_m in charge_ms {
            let resolve_m = refund_m.estimate_amount(&charge_m, &mut cmplt_req);
            let result = processor.refund(charge_m, resolve_m).await;
            if let Ok(resolve_m) = &result {
                refund_m.update(resolve_m);
            }
            out.push(result);
        }
        Ok(out)
    } // end of fn hdlr_load_refund_req
} // end of impl FinalizeRefundUseCase
