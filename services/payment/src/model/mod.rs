mod charge;
mod external_processor;
mod merchant;
mod order_replica;
mod payout;

use rust_decimal::Decimal;
use std::str::FromStr;

use ecommerce_common::api::dto::{CurrencyDto, PayAmountDto};

use crate::api::web::dto::StoreOnboardReqDto;

pub use self::charge::{
    BuyerPayInState, Charge3partyModel, ChargeBuyerMetaModel, ChargeBuyerModel,
    ChargeLineBuyerModel, ChargeToken,
};
pub use self::external_processor::{
    Charge3partyStripeModel, Merchant3partyStripeModel, Payout3partyStripeModel,
    StripeAccountCapabilityModel, StripeAccountCapableState, StripeAccountLinkModel,
    StripeAccountSettingModel, StripeCheckoutPaymentStatusModel, StripeSessionStatusModel,
};
pub use self::merchant::{Merchant3partyModel, MerchantModelError, MerchantProfileModel};
pub use self::order_replica::{
    OLineRefundModel, OrderCurrencySnapshot, OrderLineModel, OrderLineModelSet, OrderModelError,
    OrderRefundModel, RefundModelError,
};
pub(crate) use self::payout::PayoutInnerModel;
pub use self::payout::{Payout3partyModel, PayoutAmountModel, PayoutModel, PayoutModelError};

#[derive(Debug)]
pub enum PayLineAmountError {
    // the first argument indicates stringified `amount per unit`
    Overflow(String, u32),
    Mismatch(PayAmountDto, u32),
    // the 2 fields indicate `stringified value` and `detail reason`
    ParseUnit(String, String),
    ParseTotal(String, String),
    // the final tuple of u32 indicates expected number of
    // decimal places in `unit` field in `PayAmountDto`
    PrecisionUnit(String, (u32, u32)),
}

/// this type does not contain the currency of the amount,
/// such currency is defined by upper structure
#[derive(Default)]
pub struct PayLineAmountModel {
    pub unit: Decimal,
    pub total: Decimal,
    pub qty: u32,
}

#[derive(Copy, Clone)]
pub enum Label3party {
    Stripe,
}

impl TryFrom<(u32, PayAmountDto, CurrencyDto)> for PayLineAmountModel {
    type Error = PayLineAmountError;
    fn try_from(value: (u32, PayAmountDto, CurrencyDto)) -> Result<Self, Self::Error> {
        let (quantity, amount_dto, currency_label) = value;
        let result_amount_unit = Decimal::from_str(amount_dto.unit.as_str());
        let result_amount_total = Decimal::from_str(amount_dto.total.as_str());
        if let Err(e) = &result_amount_unit {
            let amt = amount_dto.unit;
            let detail = e.to_string();
            Err(Self::Error::ParseUnit(amt, detail))
        } else if let Err(e) = &result_amount_total {
            let amt = amount_dto.total;
            let detail = e.to_string();
            Err(Self::Error::ParseTotal(amt, detail))
        } else {
            let m = Self {
                qty: quantity,
                unit: result_amount_unit.unwrap(),
                total: result_amount_total.unwrap(),
            };
            let fraction_limit = currency_label.amount_fraction_scale();
            let amt_unit_fraction = m.unit.scale();
            if fraction_limit < amt_unit_fraction {
                let mismatch = (fraction_limit, amt_unit_fraction);
                Err(Self::Error::PrecisionUnit(amount_dto.unit, mismatch))
            } else if !m.total_amount_eq()? {
                Err(Self::Error::Mismatch(amount_dto, quantity))
            } else {
                Ok(m)
            }
        }
    } // end of fn try-from
} // end of impl TryFrom for PayLineAmountModel

impl PayLineAmountModel {
    fn total_amount_eq(&self) -> Result<bool, PayLineAmountError> {
        let qty_d = Decimal::new(self.qty as i64, 0);
        let tot_actual = qty_d
            .checked_mul(self.unit)
            .ok_or(PayLineAmountError::Overflow(
                self.unit.to_string(),
                self.qty,
            ))?;
        Ok(tot_actual == self.total)
    }
} // end of impl TryFrom for PayLineAmountModel

impl<'a> TryFrom<&'a str> for Label3party {
    type Error = &'a str;
    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        match value {
            "Stripe" => Ok(Self::Stripe),
            _others => Err(value),
        }
    }
}

impl<'a> From<&'a StoreOnboardReqDto> for Label3party {
    fn from(value: &'a StoreOnboardReqDto) -> Self {
        match value {
            StoreOnboardReqDto::Stripe(_) => Self::Stripe,
        }
    }
}

impl<'a> TryFrom<&'a Charge3partyModel> for Label3party {
    type Error = String;
    fn try_from(value: &'a Charge3partyModel) -> Result<Self, Self::Error> {
        match value {
            Charge3partyModel::Stripe(_) => Ok(Self::Stripe),
            Charge3partyModel::Unknown => Err("unknown".to_string()),
        }
    }
}

impl<'a> From<&'a Payout3partyModel> for Label3party {
    fn from(value: &'a Payout3partyModel) -> Self {
        match value {
            Payout3partyModel::Stripe(_) => Self::Stripe,
        }
    }
}

impl ToString for Label3party {
    fn to_string(&self) -> String {
        let s = match self {
            Self::Stripe => "Stripe",
        };
        s.to_string()
    }
}
