use std::collections::hash_map::RandomState;
use std::collections::{HashMap, HashSet};
use std::result::Result;
use std::str::FromStr;

use chrono::{
    DateTime, Datelike, Duration, DurationRound, FixedOffset, Local, TimeDelta, Timelike, Utc,
};
use rust_decimal::Decimal;

use ecommerce_common::api::dto::{
    CurrencyDto, CurrencySnapshotDto, GenericRangeErrorDto, OrderCurrencySnapshotDto,
    OrderLinePayDto, OrderSellerCurrencyDto, PayAmountDto,
};
use ecommerce_common::model::BaseProductIdentity;

use crate::api::web::dto::{
    ChargeAmountOlineDto, ChargeOlineErrorDto, ChargeReqDto, ChargeRespErrorDto, OrderErrorReason,
    PaymentMethodReqDto,
};
use crate::hard_limit::{CREATE_CHARGE_SECONDS_INTERVAL, SECONDS_ORDERLINE_DISCARD_MARGIN};

#[derive(Debug)]
pub enum PayLineAmountError {
    // the first argument indicates stringified `amount per unit`
    Overflow(String, u32),
    Mismatch(PayAmountDto, u32),
    // the 2 fields indicate `stringified value` and `detail reason`
    ParseUnit(String, String),
    ParseTotal(String, String),
}

#[derive(Debug)]
pub enum OrderModelError {
    EmptyLine,
    ZeroQuantity(BaseProductIdentity),
    RsvExpired(BaseProductIdentity),
    RsvError(BaseProductIdentity, String),
    InvalidAmount(BaseProductIdentity, PayLineAmountError),
    MissingActorsCurrency(Vec<u32>),
    MissingExRate(CurrencyDto),
    CorruptedExRate(CurrencyDto, String),
}

/// this type does not contain the currency of the amount,
/// such currency is defined by upper structure
#[derive(Default)]
pub struct PayLineAmountModel {
    pub unit: Decimal,
    pub total: Decimal,
    pub qty: u32,
}

pub struct OrderLineModel {
    pub pid: BaseProductIdentity, // product ID
    pub rsv_total: PayLineAmountModel,
    pub paid_total: PayLineAmountModel,
    pub reserved_until: DateTime<Utc>,
}

#[derive(Clone)]
pub struct OrderCurrencySnapshot {
    pub label: CurrencyDto,
    pub rate: Decimal,
}

pub struct OrderLineModelSet {
    pub id: String,
    pub buyer_id: u32, // buyer's profile ID in user-management service
    pub lines: Vec<OrderLineModel>,
    pub create_time: DateTime<Utc>,
    pub num_charges: u32,
    // - the map indicates currencies and locked exchange rate applied
    //   in buyer or sellers business.
    // - note current base currency in this project defaults to USD
    pub currency_snapshot: HashMap<u32, OrderCurrencySnapshot>,
}

pub type PaymentMethodModel = PaymentMethodReqDto;

const CHARGE_TOKEN_NBYTES: usize = 9;

pub struct ChargeToken(pub [u8; CHARGE_TOKEN_NBYTES]);

#[derive(Debug)]
pub enum BuyerPayInState {
    Initialized,
    ProcessorAccepted(DateTime<Utc>),
    ProcessorCompleted(DateTime<Utc>),
    OrderAppSynced(DateTime<Utc>),
    OrderAppExpired, // in such case, the charge should be converted to refund (TODO)
}

pub struct ChargeLineBuyerModel {
    pub pid: BaseProductIdentity, // product ID
    // currenctly this field specifies the amount to charge in buyer's currency,
    // TODO, another column for the same purpose in seller's preferred currency
    pub amount: PayLineAmountModel,
}
pub struct ChargeBuyerModel {
    pub owner: u32,
    pub create_time: DateTime<Utc>,
    // idenpotency token, derived by owner (user profile ID) and create time
    pub token: ChargeToken,
    pub oid: String, // referenced order id
    pub currency_snapshot: HashMap<u32, OrderCurrencySnapshot>,
    pub lines: Vec<ChargeLineBuyerModel>,
    pub state: BuyerPayInState,
    pub method: PaymentMethodModel,
}

impl BuyerPayInState {
    pub fn create_time(&self) -> Option<DateTime<FixedOffset>> {
        match self {
            Self::Initialized | Self::OrderAppExpired => None,
            Self::ProcessorAccepted(t) => Some((*t).into()), // implicit copy
            Self::ProcessorCompleted(t) => Some((*t).into()),
            Self::OrderAppSynced(t) => Some((*t).into()),
        }
    }
}

impl TryFrom<(u32, PayAmountDto)> for PayLineAmountModel {
    type Error = PayLineAmountError;
    fn try_from(value: (u32, PayAmountDto)) -> Result<Self, Self::Error> {
        let (quantity, amount_dto) = value;
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
            if m.total_amount_eq()? {
                Ok(m)
            } else {
                Err(Self::Error::Mismatch(amount_dto, quantity))
            }
        }
    }
} // end of impl TryFrom for PayLineAmountModel

impl PayLineAmountModel {
    fn total_amount_eq(&self) -> Result<bool, PayLineAmountError> {
        let qty_d = Decimal::new(self.qty as i64, 0);
        let tot_actual =
            qty_d
                .checked_mul(self.unit.clone())
                .ok_or(PayLineAmountError::Overflow(
                    self.unit.to_string(),
                    self.qty,
                ))?;
        Ok(tot_actual == self.total)
    }
} // end of impl TryFrom for PayLineAmountModel

#[rustfmt::skip]
impl TryFrom<OrderLinePayDto> for OrderLineModel {
    type Error = OrderModelError;
    fn try_from(value: OrderLinePayDto) -> Result<Self, Self::Error> {
        let OrderLinePayDto {
            seller_id, product_id, product_type,
            reserved_until, quantity, amount: amount_dto,
        } = value;
        let pid = BaseProductIdentity {store_id: seller_id, product_type, product_id};
        let rsv_parse_result = DateTime::parse_from_rfc3339(reserved_until.as_str());
        let now = Local::now().fixed_offset();

        if quantity == 0 {
            Err(Self::Error::ZeroQuantity(pid))
        } else if let Err(e) = rsv_parse_result.as_ref() {
            Err(Self::Error::RsvError(pid, e.to_string()))
        } else if &now >= rsv_parse_result.as_ref().unwrap() {
            Err(Self::Error::RsvExpired(pid))
        } else {
            let reserved_until = rsv_parse_result.unwrap().to_utc();
            let rsv_total = PayLineAmountModel::try_from((quantity, amount_dto))
                .map_err(|e| Self::Error::InvalidAmount(pid.clone(), e)) ?;
            let paid_total = PayLineAmountModel::default();
            Ok(Self {pid, paid_total, rsv_total, reserved_until})
        }
    } // end of fn try-from
} // end of impl TryFrom for OrderLineModel

impl TryFrom<(CurrencyDto, &Vec<CurrencySnapshotDto>)> for OrderCurrencySnapshot {
    type Error = OrderModelError;
    fn try_from(value: (CurrencyDto, &Vec<CurrencySnapshotDto>)) -> Result<Self, Self::Error> {
        let (label, search_src) = value;
        let raw_rate = search_src
            .iter()
            .find(|s| s.name == label)
            .map(|s| s.rate.to_string())
            .ok_or(OrderModelError::MissingExRate(label.clone()))?;
        let rate = Decimal::from_str(raw_rate.as_str())
            .map_err(|_e| OrderModelError::CorruptedExRate(label.clone(), raw_rate.to_string()))?;
        Ok(Self { label, rate })
    }
} // end of impl OrderCurrencySnapshot

impl OrderLineModelSet {
    fn verify_seller_currency_integrity(
        required: &Vec<OrderLinePayDto>,
        provided: &Vec<OrderSellerCurrencyDto>,
    ) -> Result<(), OrderModelError> {
        let iter0 = required.iter().map(|v| v.seller_id);
        let iter1 = provided.iter().map(|v| v.seller_id);
        let required_ids: HashSet<u32, RandomState> = HashSet::from_iter(iter0);
        let provided_ids = HashSet::from_iter(iter1);
        let uncovered = &required_ids - &provided_ids;
        if uncovered.is_empty() {
            Ok(())
        } else {
            let ids = uncovered.into_iter().collect::<Vec<_>>();
            Err(OrderModelError::MissingActorsCurrency(ids))
        }
    }
    fn try_build_currency_snapshot(
        buyer_id: u32,
        data: OrderCurrencySnapshotDto,
    ) -> Result<HashMap<u32, OrderCurrencySnapshot>, Vec<OrderModelError>> {
        let OrderCurrencySnapshotDto {
            snapshot,
            sellers: sellers_label,
            buyer: buyer_label,
        } = data;
        let mut errors = Vec::new();
        let map_iter = sellers_label.into_iter().filter_map(|d| {
            let OrderSellerCurrencyDto {
                currency: s_label,
                seller_id,
            } = d;
            OrderCurrencySnapshot::try_from((s_label, &snapshot))
                .map(|m| (seller_id, m))
                .map_err(|e| errors.push(e))
                .ok()
        });
        let mut map = HashMap::from_iter(map_iter);
        if errors.is_empty() {
            let m =
                OrderCurrencySnapshot::try_from((buyer_label, &snapshot)).map_err(|e| vec![e])?;
            map.insert(buyer_id, m);
            Ok(map)
        } else {
            Err(errors)
        }
    } // end of fn try_build_currency_snapshot
} // end of impl OrderLineModelSet

impl TryFrom<(String, u32, Vec<OrderLinePayDto>, OrderCurrencySnapshotDto)> for OrderLineModelSet {
    type Error = Vec<OrderModelError>;

    fn try_from(
        value: (String, u32, Vec<OrderLinePayDto>, OrderCurrencySnapshotDto),
    ) -> Result<Self, Self::Error> {
        let (oid, buyer_id, lines_dto, currency_d) = value;
        let mut errors = vec![];
        if lines_dto.is_empty() {
            errors.push(OrderModelError::EmptyLine);
        }
        let _ = Self::verify_seller_currency_integrity(&lines_dto, &currency_d.sellers)
            .map_err(|e| errors.push(e));
        let currency_m =
            Self::try_build_currency_snapshot(buyer_id, currency_d).map_err(|e| errors.extend(e));
        let lines = lines_dto
            .into_iter()
            .filter_map(|d| OrderLineModel::try_from(d).map_err(|e| errors.push(e)).ok())
            .collect();
        if errors.is_empty() {
            Ok(Self {
                id: oid,
                buyer_id,
                lines,
                create_time: Local::now().to_utc(),
                num_charges: 0,
                currency_snapshot: currency_m.unwrap(),
            })
        } else {
            Err(errors)
        }
    } // end of fn try-from
} // end of impl try-from for OrderLineModelSet

impl TryFrom<(OrderLineModelSet, ChargeReqDto)> for ChargeBuyerModel {
    type Error = ChargeRespErrorDto;

    fn try_from(value: (OrderLineModelSet, ChargeReqDto)) -> Result<Self, Self::Error> {
        let (ms, req) = value;
        let ChargeReqDto {
            method,
            currency: req_currency,
            lines: reqlines,
            order_id: req_oid,
        } = req;
        let OrderLineModelSet {
            id: oid,
            buyer_id,
            lines: valid_olines,
            create_time: _,
            num_charges: _,
            currency_snapshot,
        } = ms;
        let now = Local::now().to_utc();
        if oid.as_str() != req_oid.as_str() {
            return Err(ChargeRespErrorDto {
                order_id: Some(OrderErrorReason::InvalidOrder),
                ..Default::default()
            });
        }
        let buyer_currency = {
            let s = currency_snapshot.get(&buyer_id).ok_or(ChargeRespErrorDto {
                currency: Some(CurrencyDto::Unknown),
                ..Default::default()
            })?;
            if s.label == req_currency {
                s.label.clone()
            } else {
                return Err(ChargeRespErrorDto {
                    currency: Some(s.label.clone()),
                    ..Default::default()
                });
            }
        };

        let mut err_lines = Vec::new();
        let lines = reqlines
            .into_iter()
            .filter_map(|r| {
                ChargeLineBuyerModel::try_from((&valid_olines, r, now))
                    .map_err(|e| err_lines.push(e))
                    .ok()
            })
            .collect::<Vec<_>>();

        if err_lines.is_empty() {
            let now = Local::now().to_utc();
            Ok(Self {
                oid,
                create_time: now,
                token: ChargeToken::encode(buyer_id, now),
                owner: buyer_id,
                currency_snapshot,
                method,
                lines,
                state: BuyerPayInState::Initialized,
            })
        } else {
            Err(ChargeRespErrorDto {
                lines: Some(err_lines),
                currency: Some(buyer_currency),
                ..Default::default()
            })
        }
    } // end of fn try-from
} // end of impl TryFrom for ChargeBuyerModel

impl ChargeBuyerModel {
    pub(crate) fn get_buyer_currency(&self) -> Option<OrderCurrencySnapshot> {
        self.currency_snapshot.get(&self.owner).map(|v| v.clone())
    }
}

impl ChargeToken {
    pub fn encode(owner: u32, now: DateTime<Utc>) -> Self {
        let td = TimeDelta::seconds(CREATE_CHARGE_SECONDS_INTERVAL as i64);
        let now = now.duration_round(td).unwrap();
        let given = [
            (owner, 32u8),
            (now.year_ce().1, 14),
            (now.month(), 4),
            (now.day(), 5),
            (now.hour(), 5),
            (now.minute(), 6),
            (now.second(), 6),
        ];
        let inner = Self::compact_bitvec(given);
        Self(inner.try_into().unwrap())
    }
    fn compact_bitvec(data: [(u32, u8); 7]) -> Vec<u8> {
        let nbits_req = data.iter().map(|(_, sz)| *sz as usize).sum::<usize>();
        let nbits_limit = CHARGE_TOKEN_NBYTES << 3;
        assert!(nbits_limit >= nbits_req);
        let mut out: Vec<u8> = Vec::new();
        let mut nbit_avail_last = 0u8; // range 0 to 7
        data.into_iter()
            .map(|(mut v, mut sz)| {
                assert!(32u8 >= sz);
                assert!(8 > nbit_avail_last);
                v <<= 32u8.saturating_sub(sz);
                // println!("[compact-bitvec] v:{v:#x}, sz:{sz}, \
                //          nbit_avail_last:{nbit_avail_last}");
                if nbit_avail_last > 0 {
                    let nbit_shift = nbit_avail_last.min(sz);
                    let nbit_rsv_last = 32u8.saturating_sub(nbit_avail_last);
                    let v0 = (v >> nbit_rsv_last) as u8;
                    v <<= nbit_shift;
                    let mut last = out.pop().unwrap();
                    last = (last & Self::bitmask_msb8(nbit_avail_last)) | v0;
                    out.push(last);
                    sz = if nbit_shift == sz {
                        nbit_avail_last = nbit_avail_last.saturating_sub(sz);
                        0
                    } else {
                        sz.saturating_sub(nbit_avail_last)
                    };
                }
                let lastbyte_incomplete = (sz & 0x7u8) != 0;
                let nbytes_add = (sz >> 3) + (lastbyte_incomplete as u8);
                let v_bytes = v.to_be_bytes(); // always convert to big-endian value
                                               // println!("[compact-bitvec] v_bytes :{:?}", v_bytes);
                let (adding, _discarding) = v_bytes.split_at(nbytes_add as usize);
                out.extend(adding);
                if sz > 0 {
                    nbit_avail_last = ((lastbyte_incomplete as u8) << 3).saturating_sub(sz & 0x7u8);
                }
            })
            .count();
        out
    } // end of fn compact_bitvec

    fn bitmask_msb8(n: u8) -> u8 {
        0xffu8 << n
    }
} // end of impl ChargeToken

impl TryFrom<(&Vec<OrderLineModel>, ChargeAmountOlineDto, DateTime<Utc>)> for ChargeLineBuyerModel {
    type Error = ChargeOlineErrorDto;

    #[rustfmt::skip]
    fn try_from(
        value: (
            &Vec<OrderLineModel>,
            ChargeAmountOlineDto,
            DateTime<Utc>,
        ),
    ) -> Result<Self, Self::Error> {
        let (valid_olines, rl, now) = value;
        let ChargeAmountOlineDto {
            seller_id, product_id, product_type,
            quantity: qty_req, amount: amount_dto,
        } = rl;
        let mut e = ChargeOlineErrorDto {
            seller_id,
            product_id,
            product_type: product_type.clone(),
            quantity: None,
            amount: None,
            expired: None,
            not_exist: false,
        };
        let amount_dto_bak = amount_dto.clone();
        let amount_m = match PayLineAmountModel::try_from((qty_req, amount_dto))
        {
            Ok(v) => v,
            Err(_e) => {
                e.amount = Some(amount_dto_bak);
                e.quantity = Some(GenericRangeErrorDto {
                    max_: u16::try_from(qty_req).unwrap_or(u16::MAX),
                    given: qty_req,
                    min_: 1,
                });
                return Err(e);
            }, // TODO, improve error detail
        };
        let pid_req = BaseProductIdentity {
            store_id: seller_id,  product_id, product_type
        };
        let result = valid_olines.iter().find(|v| v.pid == pid_req);
        if let Some(v) = result {
            let qty_avail = v.rsv_total.qty - v.paid_total.qty;
            if now > (v.reserved_until - Duration::seconds(SECONDS_ORDERLINE_DISCARD_MARGIN as i64))
            {
                e.expired = Some(true);
                Err(e)
            } else if amount_m.unit != v.rsv_total.unit {
                e.amount = Some(amount_dto_bak);
                Err(e)
            } else if qty_avail < qty_req {
                e.quantity = Some(GenericRangeErrorDto {
                    max_: u16::try_from(qty_avail).unwrap_or(u16::MAX),
                    given: qty_req,
                    min_: 1,
                });
                Err(e)
            } else {
                Ok(Self {pid: pid_req, amount: amount_m})
            }
        } else {
            e.not_exist = true;
            Err(e)
        }
    } // end of fn try-from
} // end of impl TryFrom for ChargeLineBuyerModel
