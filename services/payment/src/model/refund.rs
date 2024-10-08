use std::str::FromStr;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use ecommerce_common::api::rpc::dto::OrderLineReplicaRefundDto;
use ecommerce_common::model::BaseProductIdentity;

use super::{ChargeBuyerModel, PayLineAmountError, PayLineAmountModel};
use crate::api::web::dto::{RefundCompletionReqDto, RefundCompletionRespDto};

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
}

pub struct RefundResolutionModel;

pub struct OLineRefundModel {
    pid: BaseProductIdentity,
    amount: PayLineAmountModel,
    create_time: DateTime<Utc>,
    // TODO
    // - rename `amount` to `amount_req`
    // - keep `resolution` history data, which includes
    //   - amount refunded
    //   - reject reason, if the amount above is zero
    //   - the time the merchant finalized
}

pub struct OrderRefundModel {
    id: String, // order-id
    lines: Vec<OLineRefundModel>,
}

#[rustfmt::skip]
impl TryFrom<OrderLineReplicaRefundDto> for OLineRefundModel {
    type Error = RefundModelError;
    
    fn try_from(value: OrderLineReplicaRefundDto) -> Result<Self, Self::Error> {
        let OrderLineReplicaRefundDto {
            seller_id, product_id, product_type, create_time, amount, qty
        } = value;
        let pid = BaseProductIdentity { store_id: seller_id, product_type, product_id };
        let create_time = DateTime::parse_from_rfc3339(create_time.as_str())
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
        let amount = PayLineAmountModel { unit, total, qty };
        Ok(Self { pid, amount, create_time })
    }
} // end of impl OLineRefundModel

type OLineRefundCvtArgs = (BaseProductIdentity, PayLineAmountModel, DateTime<Utc>);

impl From<OLineRefundCvtArgs> for OLineRefundModel {
    #[rustfmt::skip]
    fn from(value: OLineRefundCvtArgs) -> Self {
        let (pid, amount, create_time) = value;
        Self { pid, amount, create_time }
    }
}

impl OLineRefundModel {
    #[rustfmt::skip]
    pub(crate) fn into_parts(self) -> (BaseProductIdentity, PayLineAmountModel, DateTime<Utc>) {
        let Self { pid, amount, create_time } = self;
        (pid, amount, create_time)
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

impl OrderRefundModel {
    pub(crate) fn into_parts(self) -> (String, Vec<OLineRefundModel>) {
        let Self { id: oid, lines } = self;
        (oid, lines)
    }
    pub(crate) fn num_lines(&self) -> usize {
        self.lines.len()
    }
    pub(crate) fn validate(&self, _data: &RefundCompletionReqDto) -> Result<(), RefundModelError> {
        // FIXME, finish implementation
        Ok(())
    }
    pub(crate) fn estimate_amount(
        &self,
        _charge_m: &ChargeBuyerModel,
        _cmplt_req: &mut RefundCompletionReqDto,
    ) -> RefundResolutionModel {
        RefundResolutionModel
    }
    pub(crate) fn update(&mut self, _rslv_m: &RefundResolutionModel) {}
} // end of impl OrderRefundModel

impl From<Vec<RefundResolutionModel>> for RefundCompletionRespDto {
    fn from(_value: Vec<RefundResolutionModel>) -> Self {
        Self { lines: Vec::new() }
    } // TODO, finish implementation
} // end of fn RefundCompletionRespDto
