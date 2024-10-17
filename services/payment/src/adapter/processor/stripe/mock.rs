use std::boxed::Box;
use std::result::Result;

use async_trait::async_trait;
use chrono::{Duration, Local};

use ecommerce_common::api::dto::CountryCode;
use ecommerce_common::api::rpc::dto::StoreProfileReplicaDto;

use crate::api::web::dto::{
    PaymentMethodRespDto, StoreOnboardRespDto, StoreOnboardStripeReqDto,
    StripeCheckoutSessionReqDto, StripeCheckoutSessionRespDto, StripeCheckoutUImodeDto,
};
use crate::model::{
    BuyerPayInState, Charge3partyModel, Charge3partyStripeModel, ChargeBuyerModel,
    Merchant3partyModel, Merchant3partyStripeModel, Payout3partyStripeModel, PayoutInnerModel,
    StripeAccountCapabilityModel, StripeAccountCapableState, StripeAccountLinkModel,
    StripeAccountSettingModel, StripeCheckoutPaymentStatusModel, StripeSessionStatusModel,
    RefundReqRslvInnerModel,
};

use super::super::{AppProcessorErrorReason, AppProcessorMerchantResult, AppProcessorPayInResult};
use super::AbstStripeContext;

// TODO, conditional compilation for test
pub(crate) struct MockProcessorStripeCtx;

impl MockProcessorStripeCtx {
    pub(crate) fn build() -> Box<dyn AbstStripeContext> {
        Box::new(Self)
    }
}

#[async_trait]
impl AbstStripeContext for MockProcessorStripeCtx {
    async fn pay_in_start(
        &self,
        req: &StripeCheckoutSessionReqDto,
        charge_buyer: &ChargeBuyerModel,
    ) -> Result<(AppProcessorPayInResult, Charge3partyModel), AppProcessorErrorReason> {
        let (redirect_url, client_session) = match req.ui_mode {
            StripeCheckoutUImodeDto::RedirectPage => (Some("https://abc.new.au".to_string()), None),
            StripeCheckoutUImodeDto::EmbeddedJs => {
                (None, Some("mock-client-session-seq".to_string()))
            }
        };
        let checkout_session_id = "mock-stripe-checkout-session-id".to_string();
        let mthd_detail = StripeCheckoutSessionRespDto {
            id: checkout_session_id.clone(),
            redirect_url,
            client_session,
        };
        let ctime = *charge_buyer.meta.create_time();
        let result = AppProcessorPayInResult {
            charge_id: charge_buyer.meta.token().0.to_vec(),
            method: PaymentMethodRespDto::Stripe(mthd_detail),
            state: BuyerPayInState::ProcessorAccepted(ctime),
            completed: false,
        };
        let stripe_m = Charge3partyStripeModel {
            checkout_session_id,
            payment_intent_id: "mock-stripe-payment-intent-id".to_string(),
            transfer_group: "mock-stripe-transfer-group".to_string(),
            session_state: StripeSessionStatusModel::open,
            payment_state: StripeCheckoutPaymentStatusModel::unpaid,
            expiry: ctime + Duration::seconds(35),
        }; // TODO, configuable parameter expiry time
        let mthd_m = Charge3partyModel::Stripe(stripe_m);
        Ok((result, mthd_m))
    }

    async fn pay_in_progress(
        &self,
        old: &Charge3partyStripeModel,
    ) -> Result<Charge3partyStripeModel, AppProcessorErrorReason> {
        let new_m = Charge3partyStripeModel {
            checkout_session_id: old.checkout_session_id.clone(),
            payment_intent_id: old.payment_intent_id.clone(),
            transfer_group: old.transfer_group.clone(),
            session_state: StripeSessionStatusModel::complete,
            payment_state: StripeCheckoutPaymentStatusModel::paid,
            expiry: old.expiry,
        };
        Ok(new_m)
    }

    async fn onboard_merchant(
        &self,
        _store_profile: StoreProfileReplicaDto,
        req: StoreOnboardStripeReqDto,
    ) -> Result<AppProcessorMerchantResult, AppProcessorErrorReason> {
        let t_now = Local::now().to_utc();
        let t_exp = t_now + Duration::minutes(10);
        let d = StoreOnboardRespDto::Stripe {
            fields_required: Vec::new(),
            disabled_reason: Some("mock-for-test".to_string()),
            url: Some(req.refresh_url),
            expiry: Some(t_exp),
        };
        let capabilities = StripeAccountCapabilityModel {
            transfers: StripeAccountCapableState::inactive,
        };
        let settings = StripeAccountSettingModel {
            payout_delay_days: 2,
            payout_interval: "daily".to_string(),
            debit_negative_balances: false,
        };
        let update_link = Some(StripeAccountLinkModel {
            url: "https://docs.rs/async-stripe".to_string(),
            expiry: t_exp,
        });
        let s = Merchant3partyStripeModel {
            id: "acct_1oij3gwtiy832y".to_string(),
            country: CountryCode::IN,
            email: Some("hayley@wo0dberry.org".to_string()),
            capabilities,
            tos_accepted: None,
            charges_enabled: false,
            payouts_enabled: false,
            details_submitted: false,
            created: t_now,
            settings,
            update_link,
        };
        let m = Merchant3partyModel::Stripe(s);
        let out = AppProcessorMerchantResult { dto: d, model: m };
        Ok(out)
    }

    async fn refresh_onboard_status(
        &self,
        mut old_m3pty: Merchant3partyStripeModel,
        _req3pt: StoreOnboardStripeReqDto,
    ) -> Result<AppProcessorMerchantResult, AppProcessorErrorReason> {
        // assume the onboarding process has completed
        let t_now = Local::now().to_utc();
        let d = StoreOnboardRespDto::Stripe {
            fields_required: Vec::new(),
            disabled_reason: None,
            url: None,
            expiry: None,
        };
        old_m3pty.payouts_enabled = true;
        old_m3pty.details_submitted = true;
        old_m3pty.tos_accepted = Some(t_now);
        old_m3pty.capabilities.transfers = StripeAccountCapableState::active;
        old_m3pty.update_link = None;
        let m = Merchant3partyModel::Stripe(old_m3pty);
        let out = AppProcessorMerchantResult { dto: d, model: m };
        Ok(out)
    }

    async fn pay_out(
        &self,
        p_inner: &PayoutInnerModel,
        mut p3pty: Payout3partyStripeModel,
    ) -> Result<Payout3partyStripeModel, AppProcessorErrorReason> {
        p3pty.set_transfer_id("mock-stripe-transfer-id".to_string());
        p3pty.set_amount(p_inner.amount_base());
        Ok(p3pty)
    }
    
    async fn refund(
        &self,
        _rslv_inner: &RefundReqRslvInnerModel,
        detail3pty: Charge3partyStripeModel,
    ) -> Result<Charge3partyStripeModel, AppProcessorErrorReason> {
        Ok(detail3pty)
    }
} // end of impl MockProcessorStripeCtx
