use chrono::{DateTime, Local, Utc};
use rust_decimal::Decimal;

use ecommerce_common::api::dto::CurrencyDto;
use ecommerce_common::error::AppErrorCode;

use super::{
    Charge3partyModel, ChargeBuyerMetaModel, ChargeBuyerModel, Merchant3partyModel,
    MerchantProfileModel, OrderCurrencySnapshot, Payout3partyStripeModel,
};
use crate::api::web::dto::CapturePay3partyRespDto;
use crate::hard_limit::CURRENCY_RATE_PRECISION;

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
    /// total amount to transfer in buyer configured currency
    total_buyer: Decimal,
    /// total amount to transfer in merchant configured currency
    total_mc: Decimal,
    /// total amount to transfer in base currency (USD in this project)
    total_bs: Decimal,
    target_rate: Decimal, // the conversion rate from buyer's currency to seller's
    currency_seller: OrderCurrencySnapshot,
    currency_buyer: OrderCurrencySnapshot,
}

pub(crate) struct PayoutInnerModel {
    merchant_id: u32,
    capture_time: DateTime<Utc>,
    buyer_id: u32, // note the 2 fields `buyer-id` and `charge-ctime` uniquely identify
    // a single charge object.
    charge_ctime: DateTime<Utc>, // the time the charge was created
    storestaff_id: u32,          // for logging and monitoring purpose
    amount: PayoutAmountModel,
    order_id: String,
}
pub struct PayoutModel {
    _inner: PayoutInnerModel,
    _p3pty: Payout3partyModel,
}

#[rustfmt::skip]
type PayoutModelCvtArgs2 = (
    u32, DateTime<Utc>, u32, DateTime<Utc>, String,
    u32, PayoutAmountModel, Payout3partyModel,
);

impl From<PayoutModelCvtArgs2> for PayoutModel {
    #[rustfmt::skip]
    fn from(value: PayoutModelCvtArgs2) -> Self {
        let (
            merchant_id, capture_time, buyer_id, charge_ctime,
            order_id, storestaff_id, amount, _p3pty
        ) = value;
        let _inner = PayoutInnerModel {
            merchant_id, capture_time, buyer_id, charge_ctime,
            storestaff_id, amount, order_id
        };
        Self { _inner, _p3pty }
    }
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
            let id0 = v.merchant_id();
            let id1 = merc_prof.id;
            if id0 != id1 {
                return Err(PayoutModelError::MerchantInconsistent(id0, id1));
            }
            v.validate_charge_meta(&charge_m.meta)?
        }
        if !merc_3pt.can_perform_payout() {
            return Err(PayoutModelError::MerchantPermissionDenied(merc_prof.id));
        }

        let _p3pty = {
            let arg = (charge_m.meta.method_3party(), &merc_3pt);
            opt_old_payout
                .as_ref()
                .map_or(Payout3partyModel::try_from(arg), |v| {
                    v._p3pty.try_clone(arg.0, arg.1)
                })?
        };
        let amount_tot = charge_m.capture_amount(merc_prof.id)?;

        let amount_new = if let Some(v) = opt_old_payout.as_ref() {
            amount_tot.try_update(&v._inner.amount)?
        } else {
            amount_tot
        };

        let _inner = PayoutInnerModel {
            merchant_id: merc_prof.id,
            capture_time: Local::now().to_utc(),
            buyer_id: charge_m.meta.owner(),
            charge_ctime: *charge_m.meta.create_time(),
            order_id: charge_m.meta.oid().clone(),
            amount: amount_new,
            storestaff_id,
        };
        Ok(Self { _inner, _p3pty })
    } // end of fn try-from
} // end of impl PayoutModel

impl PayoutModel {
    fn validate_charge_meta(&self, c_meta: &ChargeBuyerMetaModel) -> Result<(), PayoutModelError> {
        let id0 = self._inner.buyer_id;
        let id1 = c_meta.owner();
        if id0 != id1 {
            return Err(PayoutModelError::BuyerInconsistent(id0, id1));
        }
        let ctime0 = self._inner.charge_ctime;
        let ctime1 = *c_meta.create_time();
        if ctime0 != ctime1 {
            return Err(PayoutModelError::ChargeTimeInconsistent(ctime0, ctime1));
        }
        Ok(())
    }
    pub(crate) fn into_parts(self) -> (PayoutInnerModel, Payout3partyModel) {
        let Self { _inner, _p3pty } = self;
        (_inner, _p3pty)
    }
    pub(crate) fn from_parts(_inner: PayoutInnerModel, _p3pty: Payout3partyModel) -> Self {
        Self { _inner, _p3pty }
    }
    pub fn merchant_id(&self) -> u32 {
        self._inner.merchant_id()
    }
    pub fn amount_merchant(&self) -> (Decimal, Decimal, &OrderCurrencySnapshot) {
        self._inner.amount_merchant()
    }
    pub fn amount_base(&self) -> Decimal {
        self._inner.amount_base()
    }
    pub fn amount_buyer(&self) -> Decimal {
        self._inner.amount_buyer()
    }
    pub fn thirdparty(&self) -> &Payout3partyModel {
        &self._p3pty
    }
} // end of impl PayoutModel

#[rustfmt::skip]
type PayoutInnerDecomposedArgs = (
    u32, DateTime<Utc>, u32, DateTime<Utc>,
    u32, PayoutAmountModel, String,
);

impl PayoutInnerModel {
    pub(crate) fn merchant_id(&self) -> u32 {
        self.merchant_id
    }
    pub(crate) fn referenced_charge(&self) -> (u32, DateTime<Utc>) {
        (self.buyer_id, self.charge_ctime)
    }
    pub(crate) fn amount_merchant(&self) -> (Decimal, Decimal, &OrderCurrencySnapshot) {
        self.amount.merchant()
    }
    pub(crate) fn amount_base(&self) -> Decimal {
        self.amount.base()
    }
    pub(crate) fn amount_buyer(&self) -> Decimal {
        self.amount.buyer()
    }
    #[rustfmt::skip]
    pub(crate) fn into_parts(self) -> PayoutInnerDecomposedArgs {
        let Self {
            merchant_id, capture_time, buyer_id, charge_ctime,
            storestaff_id, amount: amount_m, order_id
        } = self;
        (merchant_id, capture_time, buyer_id, charge_ctime,
         storestaff_id, amount_m, order_id)
    }
} // end of impl PayoutInnerModel

#[rustfmt::skip]
type PayoutAmountCvtArgs = (Decimal, OrderCurrencySnapshot, OrderCurrencySnapshot);

impl TryFrom<PayoutAmountCvtArgs> for PayoutAmountModel {
    type Error = PayoutModelError;
    #[rustfmt::skip]
    fn try_from(value: PayoutAmountCvtArgs) -> Result<Self, Self::Error> {
        let (tot_amt_buyer, currency_seller, currency_buyer) = value;
        let target_rate =ChargeBuyerModel::calc_target_rate(&currency_seller, &currency_buyer)
            .map_err(|d| PayoutModelError::AmountEstimate(AppErrorCode::DataCorruption, d))?
            .trunc_with_scale(CURRENCY_RATE_PRECISION);
        let total_bs = tot_amt_buyer
            .checked_div(currency_buyer.rate)
            .ok_or(format!("convert-overflow, base, rate:{}, amount:{}",
                           currency_buyer.rate, tot_amt_buyer))
            .map_err(|d| PayoutModelError::AmountEstimate(AppErrorCode::DataCorruption, d))?
            .trunc_with_scale(CurrencyDto::USD.amount_fraction_scale());
        let total_mc = tot_amt_buyer
            .checked_mul(target_rate)
            .ok_or(format!("convert-overflow, merchant, rate:{}, amount:{}",
                           target_rate, tot_amt_buyer))
            .map_err(|d| PayoutModelError::AmountEstimate(AppErrorCode::DataCorruption, d))?
            .trunc_with_scale(currency_seller.label.amount_fraction_scale());
        Ok(Self {
            total_buyer: tot_amt_buyer, total_mc, total_bs, target_rate,
            currency_seller, currency_buyer
        })
    }
} // end of impl PayoutAmountModel

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
        let remain_mc =
            self.total_mc
                .checked_sub(given.total_mc)
                .ok_or(PayoutModelError::AmountEstimate(
                    AppErrorCode::DataCorruption,
                    format!(
                        "overflow-merchant, orig:{:?}, given:{:?}",
                        self.total_mc, given.total_mc
                    ),
                ))?;
        let remain_bs =
            self.total_bs
                .checked_sub(given.total_bs)
                .ok_or(PayoutModelError::AmountEstimate(
                    AppErrorCode::DataCorruption,
                    format!(
                        "overflow-base-curr, orig:{:?}, given:{:?}",
                        self.total_bs, given.total_bs
                    ),
                ))?;
        if (remain_mc > Decimal::ZERO) && (remain_bs > Decimal::ZERO) {
            self.total_mc = remain_mc;
            self.total_bs = remain_bs;
            Ok(self)
        } else {
            Err(PayoutModelError::AmountNotEnough(
                self.total_mc,
                given.total_mc,
            ))
        }
    } // end of fn try_update

    /// return amount in merchant's configured currency
    fn merchant(&self) -> (Decimal, Decimal, &OrderCurrencySnapshot) {
        (self.total_mc, self.target_rate, &self.currency_seller)
    }

    /// return amount in base currency (USD in this project)
    fn base(&self) -> Decimal {
        self.total_bs
    }

    fn buyer(&self) -> Decimal {
        self.total_buyer
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
impl<'a> From<&'a Payout3partyModel> for CapturePay3partyRespDto {
    fn from(value: &'a Payout3partyModel) -> Self {
        match value {
            Payout3partyModel::Stripe(s) => Self::Stripe {
                amount: s.amount().unwrap().to_string(),
                currency: CurrencyDto::USD,
            },
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
