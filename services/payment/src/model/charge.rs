use std::result::Result;

use chrono::{DateTime, Duration, DurationRound, FixedOffset, Local, TimeDelta};
use sha2::{Digest, Sha256};

use ecommerce_common::api::dto::{GenericRangeErrorDto, OrderLinePayDto, PayAmountDto};
use ecommerce_common::model::BaseProductIdentity;

use crate::api::web::dto::{
    ChargeAmountOlineDto, ChargeOlineErrorDto, ChargeReqDto, ChargeRespErrorDto, OrderErrorReason,
    PaymentMethodReqDto,
};
use crate::hard_limit::{CREATE_CHARGE_SECONDS_INTERVAL, SECONDS_ORDERLINE_DISCARD_MARGIN};

#[derive(Debug)]
pub enum OLineModelError {
    EmptyLine,
    ZeroQuantity(BaseProductIdentity),
    RsvExpired(BaseProductIdentity),
    RsvError(BaseProductIdentity, String),
    AmountMismatch(BaseProductIdentity, PayAmountDto, u32),
}

#[derive(Default)]
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
    pub token: Vec<u8>, // idenpotency token
    pub oid: String,    // referenced order id
    pub lines: Vec<ChargeLineBuyerModel>,
    pub state: BuyerPayInState,
    pub method: PaymentMethodModel,
}

impl BuyerPayInState {
    pub fn create_time(&self) -> Option<DateTime<FixedOffset>> {
        match self {
            Self::Initialized | Self::OrderAppExpired => None,
            Self::ProcessorAccepted(t) => Some(*t), // implicit copy
            Self::ProcessorCompleted(t) => Some(*t),
            Self::OrderAppSynced(t) => Some(*t),
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
        let now = Local::now().fixed_offset();
        if oid.as_str() != req.order_id.as_str() {
            return Err(ChargeRespErrorDto {
                lines: None,
                method: None,
                order_id: Some(OrderErrorReason::InvalidOrder),
            });
        }

        let num_orig_olines = orig_olines.len();
        let valid_olines = orig_olines
            .into_iter()
            .filter(|v| {
                (v.rsv_total.qty >= v.paid_total.qty)
                    && ((v.rsv_total.qty * v.rsv_total.unit) == v.rsv_total.total)
                    && ((v.paid_total.qty * v.paid_total.unit) == v.paid_total.total)
            })
            .collect::<Vec<_>>();
        assert_eq!(valid_olines.len(), num_orig_olines);

        let mut err_lines = Vec::new();
        let lines = reqlines
            .into_iter()
            .map(|r| ChargeLineBuyerModel::try_from((&valid_olines, r, now)))
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    err_lines.push(e);
                    None
                }
            })
            .collect::<Vec<_>>();

        if err_lines.is_empty() {
            Ok(Self {
                token: Self::generate_token(owner),
                oid,
                owner,
                method,
                lines,
                state: BuyerPayInState::Initialized,
            })
        } else {
            Err(ChargeRespErrorDto {
                order_id: None,
                method: None,
                lines: Some(err_lines),
            })
        }
    } // end of fn try-from
} // end of impl TryFrom for ChargeBuyerModel

impl ChargeBuyerModel {
    fn generate_token(owner: u32) -> Vec<u8> {
        let rn = owner.to_ne_bytes();
        let td = TimeDelta::seconds(CREATE_CHARGE_SECONDS_INTERVAL as i64);
        let t0 = Local::now()
            .fixed_offset()
            .duration_round(td)
            .unwrap()
            .to_rfc3339();
        let mut hasher = Sha256::new();
        hasher.update(rn);
        hasher.update(t0.as_bytes());
        let result = hasher.finalize(); // generic-array crate
        result.to_vec()
    }
} // end of impl ChargeBuyerModel

impl
    TryFrom<(
        &Vec<OrderLineModel>,
        ChargeAmountOlineDto,
        DateTime<FixedOffset>,
    )> for ChargeLineBuyerModel
{
    type Error = ChargeOlineErrorDto;

    fn try_from(
        value: (
            &Vec<OrderLineModel>,
            ChargeAmountOlineDto,
            DateTime<FixedOffset>,
        ),
    ) -> Result<Self, Self::Error> {
        let (valid_olines, r, now) = value;
        let mut e = ChargeOlineErrorDto {
            seller_id: r.seller_id,
            product_id: r.product_id,
            product_type: r.product_type.clone(),
            quantity: None,
            amount: None,
            expired: None,
            not_exist: false,
        };
        if (r.quantity * r.amount.unit) != r.amount.total {
            let expect_qty = u16::try_from(r.amount.total / r.amount.unit).unwrap_or(u16::MAX);
            e.amount = Some(r.amount);
            e.quantity = Some(GenericRangeErrorDto {
                max_: expect_qty,
                min_: 1,
                given: r.quantity,
            });
            return Err(e);
        }
        let pid_req = BaseProductIdentity {
            store_id: r.seller_id,
            product_type: r.product_type.clone(),
            product_id: r.product_id,
        };
        let result = valid_olines.iter().find(|v| v.pid == pid_req);
        if let Some(v) = result {
            let qty_avail = v.rsv_total.qty - v.paid_total.qty;
            if now > (v.reserved_until - Duration::seconds(SECONDS_ORDERLINE_DISCARD_MARGIN as i64))
            {
                e.expired = Some(true);
                Err(e)
            } else if r.amount.unit != v.rsv_total.unit {
                e.amount = Some(r.amount);
                Err(e)
            } else if qty_avail < r.quantity {
                e.quantity = Some(GenericRangeErrorDto {
                    max_: u16::try_from(qty_avail).unwrap_or(u16::MAX),
                    given: r.quantity,
                    min_: 1,
                });
                Err(e)
            } else {
                Ok(Self {
                    pid: pid_req,
                    amount: PayLineAmountModel {
                        unit: r.amount.unit,
                        total: r.amount.total,
                        qty: r.quantity,
                    },
                })
            }
        } else {
            e.not_exist = true;
            Err(e)
        }
    } // end of fn try-from
} // end of impl TryFrom for ChargeLineBuyerModel
