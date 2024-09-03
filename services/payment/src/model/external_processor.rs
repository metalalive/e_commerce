use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use ecommerce_common::api::dto::CountryCode;

use crate::api::web::dto::ChargeStatusDto;

#[allow(non_camel_case_types)]
#[rustfmt::skip]
#[derive(Serialize, Deserialize)]
pub enum StripeSessionStatusModel {
    complete, expired, open,
}

#[allow(non_camel_case_types)]
#[rustfmt::skip]
#[derive(Serialize, Deserialize)]
pub enum StripeCheckoutPaymentStatusModel {
    no_payment_required, paid, unpaid,
}

#[derive(Serialize, Deserialize)]
pub struct Charge3partyStripeModel {
    pub checkout_session_id: String,
    pub session_state: StripeSessionStatusModel,
    pub payment_state: StripeCheckoutPaymentStatusModel,
    pub payment_intent_id: String,
    pub expiry: DateTime<Utc>,
}

#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize)]
pub enum StripeAccountCapableState {
    active,
    inactive,
    pending,
}
#[derive(Serialize, Deserialize)]
pub struct StripeAccountCapabilityModel {
    pub transfers: StripeAccountCapableState,
}
#[derive(Serialize, Deserialize)]
pub struct StripeAccountSettingModel {
    pub payout_delay_days: i16,
    pub payout_interval: String,
    pub debit_negative_balances: bool,
}
#[derive(Serialize, Deserialize)]
pub struct Merchant3partyStripeModel {
    // map to connected account in Stripe platform
    pub id: String,
    pub country: CountryCode,
    pub email: String,
    pub capabilities: StripeAccountCapabilityModel,
    pub tos_accepted: Option<DateTime<Utc>>,
    pub charges_enabled: bool,
    pub payouts_enabled: bool,
    pub details_submitted: bool,
    pub created: DateTime<Utc>,
    pub settings: StripeAccountSettingModel,
}

impl StripeCheckoutPaymentStatusModel {
    fn status_dto(&self) -> ChargeStatusDto {
        // This service always configures payment mode to Stripe API server,
        // so it doesn't make sense that Stripe shows a session is `completed`
        // but the corresponding payment is in `unpaid` state. Currently such
        // case is considered as 3rd party processor refuses the payment,
        // however this might be wrong.
        // TODO, find better design approach for such situation
        match self {
            Self::paid | Self::no_payment_required => ChargeStatusDto::InternalSyncing,
            Self::unpaid => ChargeStatusDto::PspRefused,
        }
    }
    fn is_paid(&self) -> bool {
        matches!(self, Self::paid | Self::no_payment_required)
    }
}
impl StripeSessionStatusModel {
    fn status_dto(&self, paystate: &StripeCheckoutPaymentStatusModel) -> ChargeStatusDto {
        match self {
            Self::open => ChargeStatusDto::PspProcessing,
            Self::expired => ChargeStatusDto::SessionExpired,
            Self::complete => paystate.status_dto(),
        }
    }
    fn is_done(&self) -> bool {
        matches!(self, Self::complete)
    }
    fn is_expired(&self) -> bool {
        matches!(self, Self::expired)
    }
}
impl Charge3partyStripeModel {
    pub(super) fn status_dto(&self) -> ChargeStatusDto {
        self.session_state.status_dto(&self.payment_state)
    }
    pub(super) fn pay_in_comfirmed(&self) -> Option<bool> {
        if self.session_state.is_done() {
            Some(self.payment_state.is_paid())
        } else if self.session_state.is_expired() {
            Some(false)
        } else {
            None
        }
    }
}
