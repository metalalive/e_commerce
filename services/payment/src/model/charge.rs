use std::collections::HashMap;
use std::result::Result;

use chrono::{
    DateTime, Datelike, Duration, DurationRound, FixedOffset, Local, TimeDelta, Timelike, Utc,
};

use ecommerce_common::api::dto::{CurrencyDto, GenericRangeErrorDto};
use ecommerce_common::model::BaseProductIdentity;

use super::{
    ChargeMethodStripeModel, OrderCurrencySnapshot, OrderLineModel, OrderLineModelSet,
    PayLineAmountModel,
};
use crate::api::web::dto::{
    ChargeAmountOlineDto, ChargeOlineErrorDto, ChargeReqOrderDto, ChargeRespErrorDto,
    OrderErrorReason,
};
use crate::hard_limit::{CREATE_CHARGE_SECONDS_INTERVAL, SECONDS_ORDERLINE_DISCARD_MARGIN};

pub enum ChargeMethodModel {
    Unknown,
    Stripe(ChargeMethodStripeModel),
}

const CHARGE_TOKEN_NBYTES: usize = 9;

pub struct ChargeToken(pub [u8; CHARGE_TOKEN_NBYTES]);

#[derive(Debug, Clone)]
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
    pub method: ChargeMethodModel,
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

impl TryFrom<(OrderLineModelSet, ChargeReqOrderDto)> for ChargeBuyerModel {
    type Error = ChargeRespErrorDto;

    fn try_from(value: (OrderLineModelSet, ChargeReqOrderDto)) -> Result<Self, Self::Error> {
        let (ms, req) = value;
        let ChargeReqOrderDto {
            currency: req_currency,
            lines: reqlines,
            id: req_oid,
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
            .filter_map(|req_line| {
                let args = (&valid_olines, req_line, buyer_currency.clone(), now);
                ChargeLineBuyerModel::try_from(args)
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
                method: ChargeMethodModel::Unknown,
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
        self.currency_snapshot.get(&self.owner).cloned()
    }

    pub(crate) fn update_progress(
        &mut self,
        new_state: &BuyerPayInState,
        new_method: ChargeMethodModel,
    ) {
        self.state = new_state.clone();
        self.method = new_method;
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

impl
    TryFrom<(
        &Vec<OrderLineModel>,
        ChargeAmountOlineDto,
        CurrencyDto,
        DateTime<Utc>,
    )> for ChargeLineBuyerModel
{
    type Error = ChargeOlineErrorDto;

    #[rustfmt::skip]
    fn try_from(
        value: (
            &Vec<OrderLineModel>,
            ChargeAmountOlineDto,
            CurrencyDto,
            DateTime<Utc>,
        ),
    ) -> Result<Self, Self::Error> {
        let (valid_olines, rl, currency_label, now) = value;
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
        let amount_m = match PayLineAmountModel::try_from((qty_req, amount_dto, currency_label))
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
