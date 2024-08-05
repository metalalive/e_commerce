use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
pub struct ChargeMethodStripeModel {
    // TODO, rename to Charge3partyStripeModel
    pub checkout_session_id: String,
    pub session_state: StripeSessionStatusModel,
    pub payment_state: StripeCheckoutPaymentStatusModel,
    pub payment_intent_id: String,
    pub expiry: DateTime<Utc>,
}

impl StripeCheckoutPaymentStatusModel {
    fn status_dto(&self) -> ChargeStatusDto {
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
impl ChargeMethodStripeModel {
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
