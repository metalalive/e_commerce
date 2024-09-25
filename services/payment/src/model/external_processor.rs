use chrono::{DateTime, Local, Utc};
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
    pub transfer_group: String, // `transfer-group` field from payment intent object
    // TODO, consider to discard `payment-intent-id`,
    // it does not seem useful in this app
    pub expiry: DateTime<Utc>,
}

#[derive(Serialize, Deserialize)]
pub struct StripeAccountLinkModel {
    pub url: String,
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
    // TODO, limit field visibility to crate level
    pub id: String,
    pub country: CountryCode,
    pub email: Option<String>,
    pub capabilities: StripeAccountCapabilityModel,
    pub tos_accepted: Option<DateTime<Utc>>,
    pub charges_enabled: bool,
    pub payouts_enabled: bool,
    pub details_submitted: bool,
    pub created: DateTime<Utc>,
    pub settings: StripeAccountSettingModel,
    pub update_link: Option<StripeAccountLinkModel>,
}

#[derive(Clone)]
pub struct Payout3partyStripeModel {
    tx_grp: String,  // `transfer_group` field of payment-intent object
    acct_id: String, // identifier of Connected Account object
    transfer_id: Option<String>,
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

impl Merchant3partyStripeModel {
    pub(crate) fn renew_link_required(&self) -> bool {
        let t_now = Local::now().to_utc();
        self.update_link.as_ref().map_or(true, |v| v.expiry < t_now)
    }
    pub(super) fn can_perform_payout(&self) -> bool {
        let tx_active = matches!(
            self.capabilities.transfers,
            StripeAccountCapableState::active
        );
        // TODO, consider extra constraint, `payout-interval` shouldn't be `manual`
        // , this payment application hasn't supported that yet.
        self.payouts_enabled && self.tos_accepted.is_some() && tx_active
    }
}

type PayoutStripeCvtArgs = (String, String, Option<String>);

impl From<PayoutStripeCvtArgs> for Payout3partyStripeModel {
    #[rustfmt::skip]
    fn from(value: PayoutStripeCvtArgs) -> Self {
        let (tx_grp, acct_id, transfer_id) = value;
        Self { tx_grp, acct_id, transfer_id }
    }
}

impl Payout3partyStripeModel {
    pub(super) fn new(c3s: &Charge3partyStripeModel, m3s: &Merchant3partyStripeModel) -> Self {
        Self {
            tx_grp: c3s.transfer_group.clone(),
            acct_id: m3s.id.clone(),
            transfer_id: None,
        }
    }
    pub(super) fn validate(
        &self,
        c3s: &Charge3partyStripeModel,
        m3s: &Merchant3partyStripeModel,
    ) -> Result<(), String> {
        if self.tx_grp.as_str() != c3s.transfer_group.as_str() {
            Err("transfer-group".to_string())
        } else if self.acct_id.as_str() != m3s.id.as_str() {
            Err("account-id".to_string())
        } else {
            Ok(())
        }
    }
    pub(crate) fn transfer_group(&self) -> &str {
        self.tx_grp.as_str()
    }
    pub(crate) fn connect_account(&self) -> &str {
        self.acct_id.as_str()
    }
    pub(crate) fn set_transfer_id(&mut self, value: String) {
        self.transfer_id = Some(value);
    }
} // end of impl Payout3partyStripeModel
