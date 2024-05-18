use std::result::Result;

use chrono::{DateTime, FixedOffset, Local, Duration};
use ecommerce_common::api::dto::{OrderLinePayDto, PayAmountDto, GenericRangeErrorDto};
use ecommerce_common::model::BaseProductIdentity;

use crate::api::web::dto::{
    ChargeReqDto, ChargeRespErrorDto, PaymentMethodReqDto, OrderErrorReason, ChargeOlineErrorDto,
};
use crate::hard_limit::SECONDS_ORDERLINE_DISCARD_MARGIN;

#[derive(Debug)]
pub enum OLineModelError {
    EmptyLine,
    ZeroQuantity(BaseProductIdentity),
    RsvExpired(BaseProductIdentity),
    RsvError(BaseProductIdentity, String),
    AmountMismatch(BaseProductIdentity, PayAmountDto, u32),
}

pub struct PayLineAmountModel {
    pub unit: u32,
    pub total: u32,
    pub qty: u32,
}

pub struct OrderLineModel {
    pub pid: BaseProductIdentity, // product ID
    pub rsv_total: PayLineAmountModel,
    pub paid_total: PayLineAmountModel,
    pub reserved_until: DateTime<FixedOffset>,
}

pub struct OrderLineModelSet {
    pub id: String,
    pub owner: u32,
    pub lines: Vec<OrderLineModel>,
    // TODO, add following fields
    // - currency rate on initiating the charge
    // - payment-method
}

pub type PaymentMethodModel = PaymentMethodReqDto;

pub enum BuyerPayInState {
    Initialized,
    ProcessorAccepted(DateTime<FixedOffset>),
    ProcessorCompleted(DateTime<FixedOffset>),
    OrderAppSynced(DateTime<FixedOffset>),
    OrderAppExpired, // in such case, the charge should be converted to refund (TODO)
}
pub struct ChargeLineBuyerModel {
    pub pid: BaseProductIdentity, // product ID
    pub amount: PayLineAmountModel,
}
pub struct ChargeBuyerModel {
    pub owner: u32,
    pub token: String,  // idenpotency token
    pub oid: String, // referenced order id
    pub lines: Vec<ChargeLineBuyerModel>,
    pub state: BuyerPayInState,
    pub method: PaymentMethodModel,
}

impl Default for PayLineAmountModel {
    fn default() -> Self {
        Self {
            unit: 0,
            total: 0,
            qty: 0,
        }
    }
}

impl BuyerPayInState {
    pub fn create_time(&self) -> Option<DateTime<FixedOffset>> {
        match self {
            Self::Initialized | Self::OrderAppExpired => None,
            Self::ProcessorAccepted(t) => Some(t.clone()),
            Self::ProcessorCompleted(t) => Some(t.clone()),
            Self::OrderAppSynced(t) => Some(t.clone()),
        }
    }
}

impl TryFrom<OrderLinePayDto> for OrderLineModel {
    type Error = OLineModelError;
    fn try_from(value: OrderLinePayDto) -> Result<Self, Self::Error> {
        let OrderLinePayDto {
            seller_id,
            product_id,
            product_type,
            reserved_until,
            quantity,
            amount,
        } = value;
        let pid = BaseProductIdentity {
            store_id: seller_id,
            product_type,
            product_id,
        };
        let rsv_parse_result = DateTime::parse_from_rfc3339(reserved_until.as_str());
        let now = Local::now().fixed_offset();
        if quantity == 0 {
            Err(OLineModelError::ZeroQuantity(pid))
        } else if let Err(e) = rsv_parse_result.as_ref() {
            Err(OLineModelError::RsvError(pid, e.to_string()))
        } else if &now >= rsv_parse_result.as_ref().unwrap() {
            Err(OLineModelError::RsvExpired(pid))
        } else if (amount.unit * quantity) != amount.total {
            Err(OLineModelError::AmountMismatch(pid, amount, quantity))
        } else {
            let reserved_until = rsv_parse_result.unwrap();
            let rsv_total = PayLineAmountModel {
                qty: quantity,
                unit: amount.unit,
                total: amount.total,
            };
            let paid_total = PayLineAmountModel::default();
            Ok(Self {
                pid,
                paid_total,
                rsv_total,
                reserved_until,
            })
        }
    }
}

impl TryFrom<(String, u32, Vec<OrderLinePayDto>)> for OrderLineModelSet {
    type Error = Vec<OLineModelError>;
    fn try_from(value: (String, u32, Vec<OrderLinePayDto>)) -> Result<Self, Self::Error> {
        let (oid, owner, lines_dto) = value;
        let mut errors = vec![];
        if lines_dto.is_empty() {
            errors.push(OLineModelError::EmptyLine);
        }
        let lines = lines_dto
            .into_iter()
            .filter_map(|d| match OrderLineModel::try_from(d) {
                Ok(v) => Some(v),
                Err(e) => {
                    errors.push(e);
                    None
                }
            })
            .collect();
        if errors.is_empty() {
            Ok(Self {
                id: oid,
                owner,
                lines,
            })
        } else {
            Err(errors)
        }
    }
} // end of impl try-from for OrderLineModelSet

impl TryFrom<(OrderLineModelSet, ChargeReqDto)> for ChargeBuyerModel {
    type Error = ChargeRespErrorDto;

    fn try_from(value: (OrderLineModelSet, ChargeReqDto)) -> Result<Self, Self::Error> {
        let (ms, req) = value;
        let (method, reqlines) = (req.method, req.lines);
        let (oid, owner, orig_olines) = (ms.id, ms.owner, ms.lines);
        Ok(Self {
            token: String::new(),
            oid, owner, method, lines: Vec::new(),
            state: BuyerPayInState::Initialized,
        })
    } // end of fn try-from
} // end of impl TryFrom for ChargeBuyerModel
