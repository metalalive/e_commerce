use rust_decimal::Decimal;
use std::collections::HashMap;

use ecommerce_common::api::dto::CurrencyDto;

use crate::api::web::dto::{ReportChargeLineRespDto, ReportChargeRespDto, ReportTimeRangeDto};

use super::ChargeBuyerModel;

#[derive(Debug)]
pub enum ReportModelError {
    MissingCurrency(String, u32),
    RateOverflow(String),
    // fields represent expect, actual values
    MerchantNotConsistent(u32, u32),
    // fields represent `rate`, original amount in buyer's currency
    AmountOverflow(Decimal, Decimal),
}

#[derive(Hash, Eq, PartialEq)]
struct ReportChargeLineKey {
    product_id: u64,
    currency: CurrencyDto, // currency applied by merchant at that time
}

#[derive(Default)]
struct ReportChargeLineEntry {
    // the fields below are amount and quantity in total during the time period in `time_range` field
    amount: Decimal,
    qty: u32,
    // amount per single unit item might change, so I don't put it in report model
}

pub struct MerchantReportChargeModel {
    id: u32, // merchant ID
    time_range: ReportTimeRangeDto,
    linemap: HashMap<ReportChargeLineKey, ReportChargeLineEntry>,
}

impl ReportChargeLineKey {
    #[rustfmt::skip]
    fn new(product_id:u64, curr_label:&CurrencyDto) -> Self {
        Self { product_id, currency: curr_label.clone() }
    }
}

impl From<(u32, ReportTimeRangeDto)> for MerchantReportChargeModel {
    #[rustfmt::skip]
    fn from(value: (u32,ReportTimeRangeDto)) -> Self {
        let (id, time_range) = value;
        Self { id, time_range, linemap: HashMap::new() }
    }
}

impl MerchantReportChargeModel {
    fn try_calc_rate(
        seller_id: u32,
        charge_m: &ChargeBuyerModel,
    ) -> Result<(CurrencyDto, Decimal), ReportModelError> {
        let buyer_currency = charge_m.get_buyer_currency().ok_or({
            let buyer_usr_id = charge_m.meta.owner();
            ReportModelError::MissingCurrency("buyer".to_string(), buyer_usr_id)
        })?;
        let seller_currency =
            charge_m
                .get_seller_currency(seller_id)
                .ok_or(ReportModelError::MissingCurrency(
                    "seller".to_string(),
                    seller_id,
                ))?;
        let seller_rate = ChargeBuyerModel::calc_target_rate(&seller_currency, &buyer_currency)
            .map_err(ReportModelError::RateOverflow)?;
        Ok((seller_currency.label.clone(), seller_rate))
    } // end of fn try-calc-rate

    fn try_merge_one(
        &mut self,
        charge_m: ChargeBuyerModel,
    ) -> Result<usize, Vec<ReportModelError>> {
        if !charge_m.meta.progress().completed() {
            return Ok(0); // skip charges which haven't completed pay-in flow
        }
        let (curr_label, rate) = Self::try_calc_rate(self.id, &charge_m).map_err(|e| vec![e])?;
        let rescale = curr_label.amount_fraction_scale();
        let mut errors = Vec::new();
        let num_merged = charge_m
            .lines
            .into_iter()
            .filter_map(|cl| {
                // skip refund info at here
                let (pid, amt_orig, _, _) = cl.into_parts();
                if pid.store_id != self.id {
                    let e = ReportModelError::MerchantNotConsistent(self.id, pid.store_id);
                    errors.push(e);
                    return None;
                }
                let key = ReportChargeLineKey::new(pid.product_id, &curr_label);
                let entry = self.linemap.entry(key).or_default();
                rate.checked_mul(amt_orig.total)
                    .map(|amt_seller| {
                        entry.amount += amt_seller.trunc_with_scale(rescale);
                        entry.qty += amt_orig.qty;
                    })
                    .or_else(|| {
                        let e = ReportModelError::AmountOverflow(rate, amt_orig.total);
                        errors.push(e);
                        None
                    })
            })
            .count();
        if errors.is_empty() {
            Ok(num_merged)
        } else {
            Err(errors)
        }
    } // end of fn try-merge-one

    pub fn try_merge(
        &mut self,
        charge_ms: Vec<ChargeBuyerModel>,
    ) -> Result<usize, Vec<ReportModelError>> {
        let mut errors = Vec::new();
        let total_merged = charge_ms
            .into_iter()
            .filter_map(|charge_m| {
                self.try_merge_one(charge_m)
                    .map_err(|es| errors.extend(es))
                    .ok()
            })
            .sum();
        if errors.is_empty() {
            Ok(total_merged)
        } else {
            Err(errors)
        }
    } // end of fn try-merge
} // end of impl MerchantReportChargeModel

#[rustfmt::skip]
impl From<(ReportChargeLineKey, ReportChargeLineEntry)> for ReportChargeLineRespDto {
    fn from(value: (ReportChargeLineKey, ReportChargeLineEntry)) -> Self {
        let (k ,v) = value;
        Self {
            product_id: k.product_id, currency: k.currency,
            amount: v.amount.to_string(), qty: v.qty
        }
    }
}

#[rustfmt::skip]
impl From<MerchantReportChargeModel> for ReportChargeRespDto {
    fn from(value: MerchantReportChargeModel) -> Self {
        let MerchantReportChargeModel { id, time_range, linemap } = value;
        let lines = linemap.into_iter()
            .map(ReportChargeLineRespDto::from)
            .collect::<Vec<_>>();
        ReportChargeRespDto {merchant_id: id, time_range, lines}
    }
}
