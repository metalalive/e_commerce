use chrono::{DateTime, Local, Utc};
use rust_decimal::Decimal;

use super::{
    Charge3partyModel, ChargeBuyerMetaModel, ChargeBuyerModel, Merchant3partyModel,
    MerchantProfileModel, OrderCurrencySnapshot, Payout3partyStripeModel,
};
use ecommerce_common::error::AppErrorCode;

#[derive(Debug)]
pub enum PayoutModelError {
    AmountEstimate(AppErrorCode, String),
    AmountNotEnough(Decimal, Decimal),
    BuyerInconsistent(u32, u32),
    MerchantInconsistent(u32, u32),
    ChargeTimeInconsistent(DateTime<Utc>, DateTime<Utc>),
    CurrencyInconsistent(String, OrderCurrencySnapshot, OrderCurrencySnapshot),
    MerchantPermissionDenied(u32),
    Init3partyFailure,
    Invalid3partyParams(String),
}

pub enum Payout3partyModel {
    Stripe(Payout3partyStripeModel),
}

pub struct PayoutAmountModel {
    pub(super) total: Decimal,
    pub(super) target_rate: Decimal, // the conversion rate from buyer's currency to seller's
    pub(super) currency_seller: OrderCurrencySnapshot,
    pub(super) currency_buyer: OrderCurrencySnapshot,
}

pub struct PayoutModel {
    merchant_id: u32,
    capture_time: DateTime<Utc>,
    buyer_id: u32, // note the 2 fields `buyer-id` and `charge-ctime` uniquely identify
    // a single charge object.
    charge_ctime: DateTime<Utc>, // the time the charge was created
    storestaff_id: u32,          // for logging and monitoring purpose
    amount: PayoutAmountModel,
    p3pty: Payout3partyModel,
    // TODO, consider to add referenced field `order-id`
}

type PayoutModelCvtArgs = (
    ChargeBuyerModel,
    MerchantProfileModel,
    Merchant3partyModel,
    u32,
    Option<PayoutModel>,
);

impl TryFrom<PayoutModelCvtArgs> for PayoutModel {
    type Error = PayoutModelError;
    fn try_from(value: PayoutModelCvtArgs) -> Result<Self, Self::Error> {
        let (charge_m, merc_prof, merc_3pt, storestaff_id, opt_old_payout) = value;
        if let Some(v) = &opt_old_payout {
            let id0 = v.merchant_id;
            let id1 = merc_prof.id;
            if id0 != id1 {
                return Err(PayoutModelError::MerchantInconsistent(id0, id1));
            }
            v.validate_charge_meta(&charge_m.meta)?
        }
        if !merc_3pt.can_perform_payout() {
            return Err(PayoutModelError::MerchantPermissionDenied(merc_prof.id));
        }

        let p3pty = {
            let arg = (charge_m.meta.method_3party(), &merc_3pt);
            opt_old_payout
                .as_ref()
                .map_or(Payout3partyModel::try_from(arg), |v| {
                    v.p3pty.try_clone(arg.0, arg.1)
                })?
        };
        let amount_tot = charge_m
            .capture_amount(merc_prof.id)
            .map_err(|(code, detail)| PayoutModelError::AmountEstimate(code, detail))?;

        let amount_new = if let Some(v) = opt_old_payout.as_ref() {
            amount_tot.try_update(&v.amount)?
        } else {
            amount_tot
        };

        let out = Self {
            merchant_id: merc_prof.id,
            capture_time: Local::now().to_utc(),
            buyer_id: charge_m.meta.owner(),
            charge_ctime: *charge_m.meta.create_time(),
            amount: amount_new,
            storestaff_id,
            p3pty,
        };
        Ok(out)
    } // end of fn try-from
} // end of impl PayoutModel

impl PayoutModel {
    fn validate_charge_meta(&self, c_meta: &ChargeBuyerMetaModel) -> Result<(), PayoutModelError> {
        let id0 = self.buyer_id;
        let id1 = c_meta.owner();
        if id0 != id1 {
            return Err(PayoutModelError::BuyerInconsistent(id0, id1));
        }
        let ctime0 = self.charge_ctime;
        let ctime1 = *c_meta.create_time();
        if ctime0 != ctime1 {
            return Err(PayoutModelError::ChargeTimeInconsistent(ctime0, ctime1));
        }
        Ok(())
    }

    pub fn merchant_id(&self) -> u32 {
        self.merchant_id
    }
    pub fn amount_merchant(&self) -> (Decimal, Decimal, &OrderCurrencySnapshot) {
        let a = &self.amount;
        (a.total, a.target_rate, &a.currency_seller)
    }
} // end of impl PayoutModel

impl PayoutAmountModel {
    fn try_update(mut self, given: &Self) -> Result<Self, PayoutModelError> {
        if self.currency_buyer != given.currency_buyer {
            let arg = (
                "buyer".to_string(),
                self.currency_buyer.clone(),
                given.currency_buyer.clone(),
            );
            return Err(PayoutModelError::CurrencyInconsistent(arg.0, arg.1, arg.2));
        }
        if self.currency_seller != given.currency_seller {
            let arg = (
                "merchant".to_string(),
                self.currency_seller.clone(),
                given.currency_seller.clone(),
            );
            return Err(PayoutModelError::CurrencyInconsistent(arg.0, arg.1, arg.2));
        } // TODO, implement domain logic at here if multi-payout feature is supported
        let remain =
            self.total
                .checked_sub(given.total)
                .ok_or(PayoutModelError::AmountEstimate(
                    AppErrorCode::DataCorruption,
                    format!("overflow, orig:{:?}, given:{:?}", self.total, given.total),
                ))?;
        if remain <= Decimal::ZERO {
            return Err(PayoutModelError::AmountNotEnough(self.total, given.total));
        }
        self.total = remain;
        Ok(self)
    }
} // end of impl PayoutAmountModel

type Payout3ptyCvtArgs<'a, 'b> = (&'a Charge3partyModel, &'b Merchant3partyModel);

impl<'a, 'b> TryFrom<Payout3ptyCvtArgs<'a, 'b>> for Payout3partyModel {
    type Error = PayoutModelError;
    fn try_from(value: Payout3ptyCvtArgs<'a, 'b>) -> Result<Self, Self::Error> {
        let (charge3pty, m3pty) = value;
        match (charge3pty, m3pty) {
            (Charge3partyModel::Stripe(cs), Merchant3partyModel::Stripe(ms)) => {
                let inner = Payout3partyStripeModel::new(cs, ms);
                Ok(Self::Stripe(inner))
            }
            _others => Err(PayoutModelError::Init3partyFailure),
        }
    }
}
impl Payout3partyModel {
    fn try_clone(
        &self,
        charge3pty: &Charge3partyModel,
        m3pty: &Merchant3partyModel,
    ) -> Result<Self, PayoutModelError> {
        match (self, charge3pty, m3pty) {
            (Self::Stripe(ps), Charge3partyModel::Stripe(cs), Merchant3partyModel::Stripe(ms)) => {
                ps.validate(cs, ms)
                    .map_err(PayoutModelError::Invalid3partyParams)?;
                Ok(Self::Stripe(ps.clone()))
            }
            _others => {
                let d = "mismatch".to_string();
                Err(PayoutModelError::Invalid3partyParams(d))
            }
        }
    }
}
