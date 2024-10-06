use std::str::FromStr;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use ecommerce_common::api::rpc::dto::OrderLineReplicaRefundDto;
use ecommerce_common::model::BaseProductIdentity;

use super::{PayLineAmountError, PayLineAmountModel};

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
pub(crate) struct OLineRefundModel {
    pid: BaseProductIdentity,
    amount: PayLineAmountModel,
    create_time: DateTime<Utc>,
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
