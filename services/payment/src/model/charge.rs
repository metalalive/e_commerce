use std::collections::HashMap;
use std::result::Result;

use chrono::offset::LocalResult;
use chrono::{
    DateTime, Datelike, Duration, DurationRound, FixedOffset, Local, TimeDelta, TimeZone, Timelike,
    Utc,
};
use rust_decimal::Decimal;

use ecommerce_common::api::dto::{CurrencyDto, GenericRangeErrorDto};
use ecommerce_common::api::rpc::dto::{OrderLinePaidUpdateDto, OrderPaymentUpdateDto};
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::model::BaseProductIdentity;

use super::{
    Charge3partyStripeModel, OrderCurrencySnapshot, OrderLineModel, OrderLineModelSet,
    PayLineAmountModel, PayoutAmountModel,
};
use crate::api::web::dto::{
    ChargeAmountOlineDto, ChargeOlineErrorDto, ChargeRefreshRespDto, ChargeReqOrderDto,
    ChargeRespErrorDto, ChargeStatusDto, OrderErrorReason,
};
use crate::hard_limit::{CREATE_CHARGE_SECONDS_INTERVAL, SECONDS_ORDERLINE_DISCARD_MARGIN};

pub enum Charge3partyModel {
    Unknown,
    Stripe(Charge3partyStripeModel),
}

mod token_inner {
    pub const NBYTES: usize = 9;
    pub(super) mod encoding {
        // bit length for each goven token
        pub const USR_ID: u8 = 32;
        pub const T_YEAR: u8 = 14;
        pub const T_MONTH: u8 = 4;
        pub const T_DAY: u8 = 5;
        pub const T_HOUR: u8 = 5;
        pub const T_MINUTE: u8 = 6;
        pub const T_SECOND: u8 = 6;
    }
}

pub struct ChargeToken(pub [u8; token_inner::NBYTES]);

#[derive(Debug, Clone)]
pub enum BuyerPayInState {
    Initialized,
    ProcessorAccepted(DateTime<Utc>),
    // the 3rd party has done with the payment, the payment could be
    // either successful or failed. TODO, add explicit `confirm` flag
    ProcessorCompleted(DateTime<Utc>),
    OrderAppSynced(DateTime<Utc>),
    // This model should report error when
    // - attempting to convert `charge request DTO` to `ChargeBuyerMetaModel`
    // - reservation time of an unpaid order line expires
}

pub struct ChargeLineBuyerModel {
    pub pid: BaseProductIdentity, // product ID
    // currenctly this field specifies the amount to charge in buyer's currency,
    // TODO, another column for the same purpose in seller's preferred currency
    pub amount: PayLineAmountModel,
}
pub struct ChargeBuyerMetaModel {
    _owner: u32,
    _create_time: DateTime<Utc>,
    _oid: String, // referenced order id
    _state: BuyerPayInState,
    _method: Charge3partyModel,
} // TODO, new struct function for init , instead of directly accessing the inner fields
pub struct ChargeBuyerModel {
    pub meta: ChargeBuyerMetaModel,
    pub currency_snapshot: HashMap<u32, OrderCurrencySnapshot>,
    pub lines: Vec<ChargeLineBuyerModel>,
}

impl BuyerPayInState {
    pub fn create_time(&self) -> Option<DateTime<FixedOffset>> {
        match self {
            Self::Initialized => None,
            Self::ProcessorAccepted(t) => Some((*t).into()), // implicit copy
            Self::ProcessorCompleted(t) => Some((*t).into()),
            Self::OrderAppSynced(t) => Some((*t).into()),
        }
    }
    // the method `completed` indicates whether the customer has done all
    // necessary steps in the pay-in operation, including interaction with
    // 3rd-party processor and sync with internal order app
    pub(crate) fn completed(&self) -> bool {
        matches!(self, Self::OrderAppSynced(_))
    }
    fn status_dto(&self, mthd: &Charge3partyModel) -> (ChargeStatusDto, DateTime<Utc>) {
        let now = Local::now().to_utc();
        match self {
            Self::OrderAppSynced(t) => (ChargeStatusDto::Completed, *t),
            // FIXME, it is possible that buy-in state is `completed` but the state
            // from 3rd party processor is `processing`
            Self::ProcessorCompleted(t) => (mthd.status_dto(), *t),
            Self::ProcessorAccepted(_t) => (mthd.status_dto(), now),
            Self::Initialized => (ChargeStatusDto::Initialized, now),
        }
    }
}

impl Charge3partyModel {
    // The method `pay_in_completed` indicates whether 3rd-party processor has
    // done and confirmed with the charge initiated by a client during the
    // entire pay-in flow.
    //
    // the return value could be `None` if 3rd party has not completed,
    // or `Some()` with boolean which means whether the charge has been confirmed
    // successfully by a client
    //
    // Note 3rd party might complete without confirmation, in such case the charge
    // model should be no longer valid and discarded (TODO)
    pub fn pay_in_comfirmed(&self) -> Option<bool> {
        match self {
            Self::Unknown => Some(false),
            Self::Stripe(m) => m.pay_in_comfirmed(),
        }
    }
    fn status_dto(&self) -> ChargeStatusDto {
        match self {
            Self::Unknown => ChargeStatusDto::UnknownPsp,
            Self::Stripe(m) => m.status_dto(),
        }
    }
}

impl From<&ChargeBuyerMetaModel> for ChargeRefreshRespDto {
    fn from(value: &ChargeBuyerMetaModel) -> Self {
        let arg = value.progress().status_dto(value.method_3party());
        Self {
            order_id: value.oid().clone(),
            create_time: arg.1,
            status: arg.0,
        }
    }
}

impl From<(String, u32)> for ChargeBuyerMetaModel {
    fn from(value: (String, u32)) -> Self {
        let td = TimeDelta::seconds(CREATE_CHARGE_SECONDS_INTERVAL as i64);
        let _create_time = Local::now().to_utc().duration_trunc(td).unwrap();
        Self {
            _owner: value.1,
            _create_time,
            _oid: value.0,
            _method: Charge3partyModel::Unknown,
            _state: BuyerPayInState::Initialized,
        }
    }
}
impl From<(String, u32, DateTime<Utc>)> for ChargeBuyerMetaModel {
    fn from(value: (String, u32, DateTime<Utc>)) -> Self {
        Self {
            _owner: value.1,
            _create_time: value.2,
            _oid: value.0,
            _method: Charge3partyModel::Unknown,
            _state: BuyerPayInState::Initialized,
        }
    }
}

impl ChargeBuyerMetaModel {
    pub(crate) fn token(&self) -> ChargeToken {
        // idenpotency token, derived by owner (user profile ID) and create time
        ChargeToken::encode(self._owner, self._create_time)
    }
    pub(crate) fn pay_update_dto(
        &self,
        chg_lines: Vec<ChargeLineBuyerModel>,
    ) -> OrderPaymentUpdateDto {
        let lines = chg_lines
            .into_iter()
            .map(OrderLinePaidUpdateDto::from)
            .collect::<Vec<_>>();
        OrderPaymentUpdateDto {
            oid: self._oid.clone(),
            charge_time: self._create_time.to_rfc3339(),
            lines,
        }
    }
    pub fn owner(&self) -> u32 {
        self._owner
    }
    pub fn create_time(&self) -> &DateTime<Utc> {
        &self._create_time
    }
    pub fn oid(&self) -> &String {
        &self._oid
    }
    pub fn progress(&self) -> &BuyerPayInState {
        &self._state
    }
    pub fn method_3party(&self) -> &Charge3partyModel {
        &self._method
    }
    pub fn update_progress(&mut self, new_state: &BuyerPayInState) {
        if !self._state.completed() {
            self._state = new_state.clone();
        }
    } // TODO, move to BuyerPayInState
    pub fn update_3party(&mut self, value: Charge3partyModel) {
        self._method = value;
    }
    #[rustfmt::skip]
    pub(crate) fn into_parts(self) -> (u32, DateTime<Utc>, String, BuyerPayInState, Charge3partyModel)
    {
        let Self { _owner, _create_time, _oid, _state, _method } = self;
        (_owner, _create_time, _oid, _state, _method)
    }
} // end of impl ChargeBuyerMetaModel

impl From<ChargeLineBuyerModel> for OrderLinePaidUpdateDto {
    #[rustfmt::skip]
    fn from(value: ChargeLineBuyerModel) -> Self {
        let ChargeLineBuyerModel { pid, amount } = value;
        let BaseProductIdentity { store_id, product_type, product_id } = pid;
        Self { seller_id: store_id, product_id, product_type, qty: amount.qty }
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
            Ok(Self {
                meta: ChargeBuyerMetaModel::from((oid, buyer_id)),
                currency_snapshot,
                lines,
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
        let key = self.meta.owner();
        self.currency_snapshot.get(&key).cloned()
    }
    fn get_seller_currency(&self, seller_id: u32) -> Option<OrderCurrencySnapshot> {
        self.currency_snapshot.get(&seller_id).cloned()
    }

    fn estimate_lines_amount(&self, seller_id: u32) -> Decimal {
        self.lines
            .iter()
            .filter(|line| line.pid.store_id == seller_id)
            .map(|v| v.amount.total)
            .sum::<Decimal>()
    }

    fn _capture_amount(&self, seller_id: u32) -> Result<PayoutAmountModel, String> {
        let currency_seller = self
            .get_seller_currency(seller_id)
            .ok_or("missing-currency-seller".to_string())?;
        let currency_buyer = self
            .get_buyer_currency()
            .ok_or("missing-currency-buyer".to_string())?;
        // TODO, move to top-level hard limit
        let hardlimit_rate_precision = 8u32;
        let tot_amt_buyer = self.estimate_lines_amount(seller_id);
        let target_rate = currency_seller
            .rate
            .checked_div(currency_buyer.rate)
            .ok_or("target-rate-overflow".to_string())?
            .trunc_with_scale(hardlimit_rate_precision);
        let total = tot_amt_buyer
            .checked_mul(target_rate)
            .ok_or("converting-amount-overflow".to_string())?
            .trunc_with_scale(currency_seller.label.amount_fraction_scale());

        let obj = PayoutAmountModel {
            target_rate,
            total,
            currency_seller,
            currency_buyer,
        };
        Ok(obj)
    }
    pub(super) fn capture_amount(
        &self,
        seller_id: u32,
    ) -> Result<PayoutAmountModel, (AppErrorCode, String)> {
        self._capture_amount(seller_id)
            .map_err(|detail| (AppErrorCode::DataCorruption, detail))
    }
} // end of impl ChargeBuyerModel

impl TryFrom<Vec<u8>> for ChargeToken {
    type Error = (AppErrorCode, String);
    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        let v = value
            .try_into()
            .map_err(|orig| (AppErrorCode::DataCorruption, format!("{:?}", orig)))?;
        Ok(Self(v))
    }
}
impl TryInto<(u32, DateTime<Utc>)> for ChargeToken {
    type Error = (AppErrorCode, String);
    fn try_into(self) -> Result<(u32, DateTime<Utc>), Self::Error> {
        self.decode()
            .map_err(|detail| (AppErrorCode::DataCorruption, detail))
    }
}
impl ToString for ChargeToken {
    fn to_string(&self) -> String {
        self.0.iter().fold(String::new(), |mut dst, num| {
            let hex = format!("{:02x}", num);
            dst += hex.as_str();
            dst
        })
    }
}
impl ChargeToken {
    pub fn encode(owner: u32, now: DateTime<Utc>) -> Self {
        let given = [
            (owner, token_inner::encoding::USR_ID),
            (now.year_ce().1, token_inner::encoding::T_YEAR),
            (now.month(), token_inner::encoding::T_MONTH),
            (now.day(), token_inner::encoding::T_DAY),
            (now.hour(), token_inner::encoding::T_HOUR),
            (now.minute(), token_inner::encoding::T_MINUTE),
            (now.second(), token_inner::encoding::T_SECOND),
        ];
        let inner = Self::compact_bitvec(given);
        Self(inner.try_into().unwrap())
    }
    fn compact_bitvec(data: [(u32, u8); 7]) -> Vec<u8> {
        let nbits_req = data.iter().map(|(_, sz)| *sz as usize).sum::<usize>();
        let nbits_limit = token_inner::NBYTES << 3;
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

    fn decode(self) -> Result<(u32, DateTime<Utc>), String> {
        let fields_len = [
            token_inner::encoding::USR_ID,
            token_inner::encoding::T_YEAR,
            token_inner::encoding::T_MONTH,
            token_inner::encoding::T_DAY,
            token_inner::encoding::T_HOUR,
            token_inner::encoding::T_MINUTE,
            token_inner::encoding::T_SECOND,
        ];
        let elms = Self::extract_bitvec(self.0, fields_len);
        let usr_id = elms[0];
        let result =
            Utc.with_ymd_and_hms(elms[1] as i32, elms[2], elms[3], elms[4], elms[5], elms[6]);
        match result {
            LocalResult::Single(t) => Ok((usr_id, t)),
            LocalResult::Ambiguous(_t0, _t1) => Err("ambiguous-time".to_string()),
            LocalResult::None => Err("invalid-time-serial".to_string()),
        }
    }

    fn extract_bitvec(given: [u8; token_inner::NBYTES], fields_len: [u8; 7]) -> [u32; 7] {
        let mut out = [0u32; 7];
        let mut bit_idx = 0usize;
        for (i, len) in fields_len.into_iter().enumerate() {
            let mut value = 0u32;
            let mut nbits_remaining = len as usize;
            while nbits_remaining > 0 {
                let octet_idx = bit_idx >> 3;
                let bit_offset = bit_idx & 0x7;
                let bits_in_current_octet = std::cmp::min(nbits_remaining, 8 - bit_offset);
                let mask = ((1 << bits_in_current_octet) - 1) as u8;
                let extracted_bits =
                    (given[octet_idx] >> (8 - bit_offset - bits_in_current_octet)) & mask;
                value = (value << bits_in_current_octet) | extracted_bits as u32;
                nbits_remaining -= bits_in_current_octet;
                bit_idx += bits_in_current_octet;
            }
            out[i] = value;
        }
        out
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
