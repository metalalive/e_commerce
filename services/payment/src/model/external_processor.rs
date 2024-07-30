use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
    pub checkout_session_id: String,
    pub session_state: StripeSessionStatusModel,
    pub payment_state: StripeCheckoutPaymentStatusModel,
    pub payment_intent_id: String,
    pub expiry: DateTime<Utc>,
}
