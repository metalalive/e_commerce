use std::cmp::min;
use std::collections::hash_map::RandomState;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use chrono::{DateTime, SubsecRound, Utc};
use ecommerce_common::constant::ProductType;
use rust_decimal::Decimal;

use ecommerce_common::api::rpc::dto::OrderLineReplicaRefundDto;
use ecommerce_common::model::BaseProductIdentity;

use super::{
    Charge3partyModel, ChargeBuyerModel, ChargeLineBuyerModel, OrderCurrencySnapshot,
    PayLineAmountError, PayLineAmountModel,
};
use crate::api::web::dto::{
    RefundCompletionOlineReqDto, RefundCompletionReqDto, RefundCompletionRespDto,
    RefundLineRejectDto, RefundRejectReasonDto,
};

#[derive(Debug)]
pub enum RefundErrorParseOline {
    CreateTime(String),
    Amount(PayLineAmountError),
}
#[derive(Debug)]
pub enum RefundModelError {
    ParseOline {
        pid: BaseProductIdentity,
        reason: RefundErrorParseOline,
    },
    QtyInsufficient {
        pid: BaseProductIdentity,
        num_avail: u32,
        num_req: u32,
    },
    AmountInsufficient {
        pid: BaseProductIdentity,
        num_avail: Decimal,
        num_req: Decimal,
    },
    MissingReqLine(BaseProductIdentity, DateTime<Utc>),
    MissingCurrency(String, u32),
    MissingMerchant,
    EmptyResolutionRequest(u32),
} // end of enum RefundModelError

// quantities of product items rejected to refund for defined reasons
pub struct RefundLineQtyRejectModel(RefundLineRejectDto);

pub struct RefundLineResolveAmountModel {
    // accumulated qty / amount against single line
    accumulated_paid: PayLineAmountModel,
    accumulated_rejected: u32, // rejected so far in previous rounds
    curr_round: PayLineAmountModel,
}
pub(super) struct RefundLineReqResolutionModel {
    pid: BaseProductIdentity,
    time_req: DateTime<Utc>,
    qty_reject: RefundLineQtyRejectModel, // rejected in current round
    // the amount should be present in buyer's currency
    amount: RefundLineResolveAmountModel,
}

pub(crate) struct RefundReqRslvInnerModel {
    buyer_usr_id: u32,
    charged_ctime: DateTime<Utc>,
    // Note
    // Stripe processor does not beed to convert the amount to other currency,
    // I still keep the currency snapshots, they might be required by other
    // 3rd-party processores in future
    currency_buyer: OrderCurrencySnapshot,
    currency_merc: OrderCurrencySnapshot,
    lines: Vec<RefundLineReqResolutionModel>,
}

pub struct RefundReqResolutionModel {
    inner: RefundReqRslvInnerModel,
    chrg3pty: Charge3partyModel,
}

pub struct OLineRefundModel {
    pid: BaseProductIdentity,
    amount_req: PayLineAmountModel,
    // the time when customer issued the refund request,
    time_req: DateTime<Utc>,
    // keep `resolution` history data along with each line
    amount_aprv: PayLineAmountModel,
    rejected: RefundLineQtyRejectModel,
}

pub struct OrderRefundModel {
    id: String, // order-id
    lines: Vec<OLineRefundModel>,
}

#[rustfmt::skip]
impl RefundModelError {
    fn qty_limit(pid: &BaseProductIdentity, num_avail:u32, num_req:u32) -> Self {
        Self::QtyInsufficient { pid: pid.clone(), num_avail, num_req }
    }
    fn amount_limit(
        pid: &BaseProductIdentity, num_avail: Decimal, num_req: Decimal
    ) -> Self {
        Self::AmountInsufficient {
            pid: pid.clone(), num_avail, num_req
        }
    }
} // end of impl RefundModelError

impl<'a> From<&'a RefundLineRejectDto> for RefundLineQtyRejectModel {
    fn from(value: &'a RefundLineRejectDto) -> Self {
        Self(value.clone())
    }
}
impl Default for RefundLineQtyRejectModel {
    fn default() -> Self {
        let iter = [
            RefundRejectReasonDto::Damaged,
            RefundRejectReasonDto::Fraudulent,
        ]
        .into_iter()
        .map(|k| (k, 0u32));
        let inner = HashMap::from_iter(iter);
        Self(inner)
    }
}
impl RefundLineQtyRejectModel {
    fn total_qty(&self) -> u32 {
        self.0.values().sum()
    }
    pub fn inner_map(&self) -> &RefundLineRejectDto {
        &self.0
    }
    fn accumulate(&self, dst: &mut Self) {
        self.0
            .iter()
            .filter_map(|(k1, v1)| {
                dst.0.get_mut(k1).map(|v2| {
                    *v2 += *v1;
                })
            })
            .count();
    }
} // end of impl RefundLineQtyRejectModel

impl RefundLineResolveAmountModel {
    pub fn curr_round(&self) -> &PayLineAmountModel {
        &self.curr_round
    }
    pub fn accumulated(&self) -> (&PayLineAmountModel, u32) {
        (&self.accumulated_paid, self.accumulated_rejected)
    }
    fn accumulate(&self, dst: &mut PayLineAmountModel) {
        assert_eq!(dst.unit, self.accumulated_paid.unit);
        let tot_qty = self.accumulated_paid.qty + self.curr_round.qty;
        let tot_amt = self.accumulated_paid.total + self.curr_round.total;
        dst.qty = tot_qty;
        dst.total = tot_amt;
    } // end of fn accumulate
} // end of impl RefundLineResolveAmountModel

#[rustfmt::skip]
impl<'a> From<(&'a PayLineAmountModel, u32, u32, Decimal)> for RefundLineResolveAmountModel {
    fn from(value: (&'a PayLineAmountModel, u32, u32, Decimal)) -> Self {
        let (prev_rfd, prev_rejected, qty, amt_tot) = value;
        let accumulated_paid = PayLineAmountModel {
            unit: prev_rfd.unit, total: prev_rfd.total, qty: prev_rfd.qty
        };
        let curr_round = PayLineAmountModel {
            unit: prev_rfd.unit, total: amt_tot, qty
        };
        Self { accumulated_paid, accumulated_rejected:prev_rejected, curr_round }
    }
}

#[rustfmt::skip]
impl TryFrom<OrderLineReplicaRefundDto> for OLineRefundModel {
    type Error = RefundModelError;
   
    #[allow(clippy::field_reassign_with_default)]
    fn try_from(value: OrderLineReplicaRefundDto) -> Result<Self, Self::Error> {
        let OrderLineReplicaRefundDto {
            seller_id, product_id, product_type, create_time, amount, qty
        } = value;
        let pid = BaseProductIdentity { store_id: seller_id, product_type, product_id };
        let time_req = DateTime::parse_from_rfc3339(create_time.as_str())
            .map_err(|e| RefundModelError::ParseOline {
                pid: pid.clone(),
                reason: RefundErrorParseOline::CreateTime(e.to_string())
            })?.to_utc();
        let unit = Decimal::from_str(amount.unit.as_str())
            .map_err(|e| RefundModelError::ParseOline {
                pid: pid.clone(),
                reason: RefundErrorParseOline::Amount(
                    PayLineAmountError::ParseUnit(amount.unit, e.to_string())
                )
            })?;
        let total = Decimal::from_str(amount.total.as_str())
            .map_err(|e| RefundModelError::ParseOline {
                pid: pid.clone(),
                reason: RefundErrorParseOline::Amount(
                    PayLineAmountError::ParseTotal(amount.total, e.to_string())
                )
            })?;
        let amount_req = PayLineAmountModel { unit, total, qty };
        let mut amount_aprv = PayLineAmountModel::default();
        amount_aprv.unit = amount_req.unit;
        let rejected = RefundLineQtyRejectModel::default();
        Ok(Self { pid, amount_req, time_req, amount_aprv, rejected })
    } // end of fn try-from
} // end of impl OLineRefundModel

type OLineRefundCvtArgs = (
    BaseProductIdentity,
    PayLineAmountModel,
    DateTime<Utc>,
    PayLineAmountModel,
    RefundLineQtyRejectModel,
);

impl From<OLineRefundCvtArgs> for OLineRefundModel {
    #[rustfmt::skip]
    fn from(value: OLineRefundCvtArgs) -> Self {
        let (pid, amount_req, time_req, amount_aprv, rejected) = value;
        Self { pid, amount_req, time_req, amount_aprv, rejected }
    }
}

impl OLineRefundModel {
    pub fn approved(&self) -> &PayLineAmountModel {
        &self.amount_aprv
    }
    pub fn rejected(&self) -> &RefundLineQtyRejectModel {
        &self.rejected
    }

    #[rustfmt::skip]
    pub(crate) fn into_parts(self) -> OLineRefundCvtArgs {
        let Self { pid, amount_req, time_req, amount_aprv, rejected } = self;
        (pid, amount_req, time_req, amount_aprv, rejected)
    }

    #[rustfmt::skip]
    fn estimate_remain_quantity(
        &self, data: &RefundCompletionOlineReqDto,
    ) -> Result<u32, RefundModelError> {
        let detail = (
            self.amount_req.qty,
            self.amount_aprv.qty,
            self.rejected.total_qty(),
        );
        let qty_avail = detail.0.checked_sub(detail.1)
            .ok_or(RefundModelError::qty_limit(&self.pid, detail.0, detail.1))?;
        let qty_avail = qty_avail.checked_sub(detail.2)
            .ok_or(RefundModelError::qty_limit(&self.pid, qty_avail, detail.2))?;
        let detail = (qty_avail, data.approval.quantity, data.total_qty_rejected());
        let qty_avail = detail.0.checked_sub(detail.1)
            .ok_or(RefundModelError::qty_limit(&self.pid, detail.0, detail.1))?;
        let qty_avail = qty_avail.checked_sub(detail.2)
            .ok_or(RefundModelError::qty_limit(&self.pid, qty_avail, detail.2))?;
        Ok(qty_avail)
    }

    fn estimate_remain_amount(
        &self,
        data: &RefundCompletionOlineReqDto,
    ) -> Result<Decimal, RefundModelError> {
        let amt_new_aprv = Decimal::from_str(data.approval.amount_total.as_str()).map_err(|e| {
            RefundModelError::ParseOline {
                pid: self.pid.clone(),
                reason: RefundErrorParseOline::Amount(PayLineAmountError::ParseTotal(
                    data.approval.amount_total.clone(),
                    e.to_string(),
                )),
            }
        })?;
        let qty_discard = data.total_qty_rejected();
        let detail = (
            self.amount_req.total,
            self.amount_aprv.total,
            amt_new_aprv,
            Decimal::new(qty_discard as i64, 0) * self.amount_req.unit,
        );
        macro_rules! check_subtract_amount {
            ($n0: expr, $n1: expr) => {{
                let out = $n0
                    .checked_sub($n1)
                    .ok_or(RefundModelError::amount_limit(&self.pid, $n0, $n1))?;
                if out.is_sign_negative() {
                    return Err(RefundModelError::amount_limit(&self.pid, $n0, $n1));
                }
                out
            }};
        }
        let amt_avail = check_subtract_amount!(detail.0, detail.1);
        let amt_avail = check_subtract_amount!(amt_avail, detail.2);
        let amt_avail = check_subtract_amount!(amt_avail, detail.3);
        Ok(amt_avail)
    } // end of fn estimate_remain_amount

    fn estimate_remains(
        &self,
        data: &RefundCompletionOlineReqDto,
    ) -> Result<(u32, Decimal), RefundModelError> {
        let qty = self.estimate_remain_quantity(data)?;
        let amt_tot = self.estimate_remain_amount(data)?;
        Ok((qty, amt_tot))
    }
} // end of impl OLineRefundModel

impl TryFrom<(String, Vec<OrderLineReplicaRefundDto>)> for OrderRefundModel {
    type Error = Vec<RefundModelError>;

    fn try_from(value: (String, Vec<OrderLineReplicaRefundDto>)) -> Result<Self, Self::Error> {
        let (oid, d_lines) = value;
        let mut errs = Vec::new();
        let lines = d_lines
            .into_iter()
            .filter_map(|d| OLineRefundModel::try_from(d).map_err(|e| errs.push(e)).ok())
            .collect::<Vec<_>>();
        if errs.is_empty() {
            Ok(Self { id: oid, lines })
        } else {
            Err(errs)
        }
    }
} // end of impl OrderRefundModel

impl From<(String, Vec<OLineRefundModel>)> for OrderRefundModel {
    fn from(value: (String, Vec<OLineRefundModel>)) -> Self {
        let (oid, lines) = value;
        Self { id: oid, lines }
    }
}

type ORefundValidateReturnType =
    Result<Vec<(ProductType, u64, DateTime<Utc>, u32, Decimal)>, Vec<RefundModelError>>;

impl OrderRefundModel {
    pub(crate) fn into_parts(self) -> (String, Vec<OLineRefundModel>) {
        let Self { id: oid, lines } = self;
        (oid, lines)
    }
    pub fn num_lines(&self) -> usize {
        self.lines.len()
    }
    pub(crate) fn merchant_ids(&self) -> Vec<u32> {
        let iter = self.lines.iter().map(|v| v.pid.store_id);
        let hset: HashSet<u32, RandomState> = HashSet::from_iter(iter);
        hset.into_iter().collect()
    }
    #[rustfmt::skip]
    pub fn get_line(
        &self, merchant_id: u32, product_type: ProductType,
        product_id: u64, time_req: DateTime<Utc>,
    ) -> Option<&OLineRefundModel> {
        let key = BaseProductIdentity {
            store_id: merchant_id, product_id,
            product_type: product_type.clone(),
        };
        self.lines.iter()
            .find(|v| v.pid == key && v.time_req.trunc_subsecs(0) == time_req.trunc_subsecs(0))
    }

    #[rustfmt::skip]
    pub fn validate(&self, merchant_id: u32, data: &RefundCompletionReqDto) -> ORefundValidateReturnType {
        let mut errors = Vec::new();
        let valid_amt_qty = data.lines.iter()
            .filter_map(|d| {
                let result = self.get_line(
                    merchant_id, d.product_type.clone(), d.product_id, d.time_issued,
                );
                if let Some(line) = result {
                    match line.estimate_remains(d) {
                        Err(e) => {
                            errors.push(e);
                            None
                        }
                        Ok((qty, amt_tot)) => Some((
                            d.product_type.clone(), d.product_id, d.time_issued, qty, amt_tot,
                        )),
                    }
                } else {
                    let e = RefundModelError::MissingReqLine(
                        BaseProductIdentity {
                            store_id: merchant_id, product_id: d.product_id,
                            product_type: d.product_type.clone(),
                        },
                        d.time_issued,
                    );
                    errors.push(e);
                    None
                }
            })
            .collect::<Vec<_>>();
        if errors.is_empty() {
            Ok(valid_amt_qty)
        } else {
            Err(errors)
        }
    } // end of fn validate

    pub fn update(&mut self, rslv_m: &RefundReqResolutionModel) -> usize {
        let num_updated = self
            .lines
            .iter_mut()
            .filter_map(|v| {
                rslv_m
                    .get_status(
                        v.pid.store_id,
                        v.pid.product_type.clone(),
                        v.pid.product_id,
                        v.time_req,
                    )
                    .map(|r| (v, r.0, r.1))
            })
            .map(|(line_req, rslv_rej, rslv_aprv)| {
                rslv_aprv.accumulate(&mut line_req.amount_aprv);
                rslv_rej.accumulate(&mut line_req.rejected);
            })
            .count();
        num_updated
    } // end of fn update
} // end of impl OrderRefundModel

impl RefundLineReqResolutionModel {
    fn to_vec(c: &ChargeLineBuyerModel, cmplt_req: &RefundCompletionReqDto) -> Vec<Self> {
        let amt_prev_refunded = c.amount_refunded();
        let num_prev_rejected = c.num_rejected();
        let mut amt_remain = c.amount_remain();
        cmplt_req
            .lines
            .iter()
            .filter(|r| r.product_id == c.pid.product_id && r.product_type == c.pid.product_type)
            .map(|r| {
                let amt_tot_req = Decimal::from_str(r.approval.amount_total.as_str()).unwrap();
                let qty_fetched = min(amt_remain.qty, r.approval.quantity);
                let tot_amt_fetched = min(amt_remain.total, amt_tot_req);
                if qty_fetched > 0 {
                    amt_remain.qty -= qty_fetched;
                    amt_remain.total -= tot_amt_fetched;
                }
                let arg = (
                    amt_prev_refunded,
                    num_prev_rejected,
                    qty_fetched,
                    tot_amt_fetched,
                );
                Self {
                    pid: c.pid.clone(),
                    time_req: r.time_issued,
                    qty_reject: RefundLineQtyRejectModel::from(&r.reject),
                    amount: RefundLineResolveAmountModel::from(arg),
                }
            })
            .filter(|m| m.total_qty_curr_round() > 0)
            .collect::<Vec<_>>()
    } // end of fn to-vec

    fn total_qty_curr_round(&self) -> u32 {
        let num_rej = self.qty_reject.total_qty();
        let num_aprv = self.amount.curr_round().qty;
        num_rej + num_aprv
    }

    pub(super) fn pid(&self) -> &BaseProductIdentity {
        &self.pid
    }
    pub(super) fn amount(&self) -> &RefundLineResolveAmountModel {
        &self.amount
    }
    pub(super) fn num_rejected(&self) -> u32 {
        self.qty_reject.total_qty()
    }
} // end of impl RefundLineReqResolutionModel

impl<'a, 'b> TryFrom<(u32, &'a ChargeBuyerModel, &'b RefundCompletionReqDto)>
    for RefundReqResolutionModel
{
    type Error = RefundModelError;
    #[rustfmt::skip]
    fn try_from(
        value: (u32, &'a ChargeBuyerModel, &'b RefundCompletionReqDto),
    ) -> Result<Self, Self::Error> {
        let (merchant_id, charge_m, cmplt_req) = value;
        let buyer_usr_id = charge_m.meta.owner();
        let currency_b = charge_m.get_buyer_currency()
            .ok_or(RefundModelError::MissingCurrency(
                "buyer-id".to_string(), buyer_usr_id,
            ))?;
        let currency_m = charge_m.get_seller_currency(merchant_id)
            .ok_or(RefundModelError::MissingCurrency(
                "merchant-id".to_string(), merchant_id,
            ))?;
        let lines = charge_m.lines.iter()
            .filter(|c| c.pid.store_id == merchant_id)
            .flat_map(|c| RefundLineReqResolutionModel::to_vec(c, cmplt_req))
            .collect::<Vec<_>>();
        let inner = RefundReqRslvInnerModel {
            buyer_usr_id, lines, charged_ctime: *charge_m.meta.create_time(),
            currency_buyer: currency_b, currency_merc: currency_m,
        };
        let chrg3pty = charge_m.meta.method_3party().clone();
        Ok(Self {chrg3pty, inner})
    }
} // end of impl RefundReqResolutionModel

impl RefundReqRslvInnerModel {
    pub(super) fn charge_id(&self) -> (u32, DateTime<Utc>) {
        (self.buyer_usr_id, self.charged_ctime)
    }
    pub(super) fn lines(&self) -> &Vec<RefundLineReqResolutionModel> {
        &self.lines
    }
    pub(crate) fn currency(&self) -> [&OrderCurrencySnapshot; 2] {
        [&self.currency_buyer, &self.currency_merc]
    }
    pub(crate) fn merchant_id(&self) -> Result<u32, RefundModelError> {
        self.lines
            .first()
            .map(|v| v.pid().store_id)
            .ok_or(RefundModelError::MissingMerchant)
    }
    pub(crate) fn total_amount_curr_round(&self) -> Decimal {
        // total amount for current round in buyer's currency
        self.lines()
            .iter()
            .map(|v| v.amount().curr_round().total)
            .sum::<Decimal>()
    }
    #[rustfmt::skip]
    fn get_status(
        &self, merchant_id: u32, product_type: ProductType,
        product_id: u64, time_req: DateTime<Utc>,
    ) -> Option<(&RefundLineQtyRejectModel, &RefundLineResolveAmountModel)> {
        let key = BaseProductIdentity {
            store_id: merchant_id ,product_type,product_id
        };
        self.lines.iter()
            .find(|v| v.pid == key && time_req.trunc_subsecs(0) == v.time_req.trunc_subsecs(0))
            .map(|v| (&v.qty_reject, &v.amount))
    }
} // end of impl RefundReqRslvInnerModel

impl RefundReqResolutionModel {
    pub(super) fn charge_id(&self) -> (u32, DateTime<Utc>) {
        self.inner.charge_id()
    }
    pub(super) fn lines(&self) -> &Vec<RefundLineReqResolutionModel> {
        self.inner.lines()
    }
    pub fn currency(&self) -> [&OrderCurrencySnapshot; 2] {
        self.inner.currency()
    }
    pub(crate) fn into_parts(self) -> (RefundReqRslvInnerModel, Charge3partyModel) {
        let Self { inner, chrg3pty } = self;
        (inner, chrg3pty)
    }
    pub(crate) fn from_parts(inner: RefundReqRslvInnerModel, chrg3pty: Charge3partyModel) -> Self {
        Self { inner, chrg3pty }
    }

    #[rustfmt::skip]
    pub fn reduce_resolved(
        &self, merchant_id: u32, req: RefundCompletionReqDto,
    ) -> RefundCompletionReqDto {
        let reduced_lines = req.lines.into_iter()
            .filter_map(|mut rline| {
                let result = self.get_status(
                    merchant_id, rline.product_type.clone(),
                    rline.product_id, rline.time_issued,
                );
                if let Some((rslv_rej, rslv_amt)) = result {
                    rline.reject.iter_mut()
                        .map(|(k, v0)| {
                            let v1 = rslv_rej.inner_map().get(k).unwrap_or(&0u32);
                            *v0 -= *v1; // TODO, verify correct number of rejected items
                        })
                        .count();
                    rline.approval.quantity -= rslv_amt.curr_round().qty;
                    rline.approval.amount_total = {
                        let s = rline.approval.amount_total.as_str();
                        let mut req_amt_tot = Decimal::from_str(s).unwrap();
                        req_amt_tot -= rslv_amt.curr_round().total;
                        assert!(req_amt_tot >= Decimal::ZERO);
                        req_amt_tot.to_string()
                    };
                    if rline.total_qty() > 0 { Some(rline) } else { None }
                } else {
                    Some(rline)
                }
            }).collect::<Vec<_>>();
        RefundCompletionReqDto { lines: reduced_lines }
    } // end of fm reduce_resolved

    /// Note the given time-req argument is truncated with all subseconds,
    /// this application does not require time precision less than one second
    /// for refund rquest recording
    #[rustfmt::skip]
    pub fn get_status(
        &self, merchant_id: u32, product_type: ProductType,
        product_id: u64, time_req: DateTime<Utc>,
    ) -> Option<(&RefundLineQtyRejectModel, &RefundLineResolveAmountModel)> {
        self.inner.get_status(merchant_id, product_type, product_id, time_req)
    }
} // end of impl RefundReqResolutionModel

impl From<Vec<RefundReqResolutionModel>> for RefundCompletionRespDto {
    fn from(_value: Vec<RefundReqResolutionModel>) -> Self {
        Self { lines: Vec::new() }
    } // TODO, finish implementation
} // end of fn RefundCompletionRespDto
