use std::collections::HashMap;
use std::result::Result as DefaultResult;
use std::vec::Vec;

use chrono::{DateTime, Duration, DurationRound, FixedOffset, Local as LocalTime};
use rust_decimal::Decimal;
use uuid::Uuid;

use ecommerce_common::api::dto::{OrderLinePayDto, PayAmountDto};
use ecommerce_common::api::rpc::dto::{
    OrderLinePaidUpdateDto, OrderLinePayUpdateErrorDto, OrderLinePayUpdateErrorReason,
    OrderLineReplicaRefundDto, OrderReplicaPaymentDto,
};
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::model::order::{BillingModel, ContactModel, PhyAddrModel};
use ecommerce_common::model::BaseProductIdentity;

use crate::api::dto::{ShippingDto, ShippingMethod, ShippingOptionDto};
use crate::api::rpc::dto::{
    InventoryEditStockLevelDto, OrderLineStockReservingDto, OrderLineStockReturningDto,
};
use crate::api::web::dto::{
    OrderCreateRespOkDto, OrderLineCreateErrorDto, OrderLineCreateErrorReason,
    OrderLineReturnErrorDto, OrderLineReturnErrorReason, OrderLineReturnReqDto, OrderLineRsvReqDto,
    ShipOptionSellerErrorReason, ShippingErrorDto, ShippingOptionErrorDto,
};

use crate::constant::hard_limit;
use crate::error::AppError;
use crate::generate_custom_uid;

use super::product_price::ProdAttriPriceModel;
use super::{CurrencyModel, OrderCurrencyModel, ProductPolicyModel, ProductPriceModel};

pub struct ShippingOptionModel {
    pub seller_id: u32,
    pub method: ShippingMethod,
}
pub struct ShippingModel {
    pub contact: ContactModel,
    pub address: Option<PhyAddrModel>,
    pub option: Vec<ShippingOptionModel>,
}

#[derive(Clone, Debug)]
pub struct OrderLineIdentity {
    _base: BaseProductIdentity,
    // `attr_set_req` is a sequence number which refers to specific set of
    // product attributes chosen by customer in an order line, this sequence number
    // could be shared with other applications e.g. payment or inventory,
    _attr_set_req: u16,
}

pub struct OrderLineAppliedPolicyModel {
    pub reserved_until: DateTime<FixedOffset>,
    pub warranty_until: DateTime<FixedOffset>,
}

pub struct OrderLinePriceModel {
    // the price values here are smallest unit in seller's currency.
    // In this order-processing service the price amount in this struct
    // is NOT converted with buyer's currency exchange rate
    _unit: u32,
    _total: u32,
}

pub struct OrderLineQuantityModel {
    pub reserved: u32,
    pub paid: u32,
    pub paid_last_update: Option<DateTime<FixedOffset>>,
} // TODO, record number of items delivered

pub struct OrderLineModel {
    id_: OrderLineIdentity,
    price: OrderLinePriceModel,
    attrs_charge: ProdAttriPriceModel,
    // TODO, new field `attr-combo-seq` for shortening identification of the
    // combination of attributes selected in the order line
    pub policy: OrderLineAppliedPolicyModel,
    pub qty: OrderLineQuantityModel,
}

// TODO, new struct for hash-map value, including :
// - number of cancelled
// - expected amount of refund  corresponding to the return
// - reason
pub type OrderReturnQuantityModel = HashMap<DateTime<FixedOffset>, (u32, OrderLinePriceModel)>;

pub struct OrderReturnModel {
    pub id_: OrderLineIdentity,
    pub qty: OrderReturnQuantityModel,
} // TODO, declare new struct which collects the hash entry
  // , add different shipping address for each return

pub struct OrderLineModelSet {
    order_id: String,
    owner_id: u32,
    create_time: DateTime<FixedOffset>,
    lines: Vec<OrderLineModel>,
    currency: OrderCurrencyModel,
}

impl From<ShippingOptionModel> for ShippingOptionDto {
    fn from(value: ShippingOptionModel) -> ShippingOptionDto {
        ShippingOptionDto {
            seller_id: value.seller_id,
            method: value.method,
        }
    }
}

impl TryFrom<ShippingOptionDto> for ShippingOptionModel {
    type Error = ShippingOptionErrorDto;
    fn try_from(value: ShippingOptionDto) -> DefaultResult<Self, Self::Error> {
        if value.seller_id == 0 {
            let e = Self::Error {
                method: None,
                seller_id: Some(ShipOptionSellerErrorReason::Empty),
            };
            Err(e)
        } else {
            Ok(Self {
                seller_id: value.seller_id,
                method: value.method,
            })
        } // TODO, will check whether the seller supports specific delivery service
    }
}
impl ShippingOptionModel {
    pub fn try_from_vec(
        value: Vec<ShippingOptionDto>,
    ) -> DefaultResult<Vec<Self>, Vec<Option<ShippingOptionErrorDto>>> {
        let results = value
            .into_iter()
            .map(Self::try_from)
            .collect::<Vec<DefaultResult<Self, ShippingOptionErrorDto>>>();
        if results.iter().any(DefaultResult::is_err) {
            let objs = results
                .into_iter()
                .map(|r| {
                    if let Err(e) = r {
                        Some(e)
                    } else {
                        None
                    } // extract all errors or return none if the item is in valid format
                })
                .collect();
            Err(objs)
        } else {
            let objs = results
                .into_iter()
                .map(|r| {
                    if let Ok(m) = r {
                        m
                    } else {
                        panic!("failed to check results");
                    }
                })
                .collect();
            Ok(objs)
        }
    }
} // end of impl ShippingOptionModel

impl From<ShippingModel> for ShippingDto {
    fn from(value: ShippingModel) -> ShippingDto {
        let (contact, pa, opt) = (value.contact.into(), value.address, value.option);
        let address = pa.map(|v| v.into());
        let option = opt.into_iter().map(ShippingOptionModel::into).collect();
        ShippingDto {
            contact,
            address,
            option,
        }
    }
}

impl TryFrom<ShippingDto> for ShippingModel {
    type Error = ShippingErrorDto;
    fn try_from(value: ShippingDto) -> DefaultResult<Self, Self::Error> {
        let results = (
            ContactModel::try_from(value.contact),
            PhyAddrModel::try_from_opt(value.address),
            ShippingOptionModel::try_from_vec(value.option),
        );
        if let (Ok(contact), Ok(maybe_addr), Ok(sh_opts)) = results {
            let obj = Self {
                contact,
                address: maybe_addr,
                option: sh_opts,
            };
            Ok(obj)
        } else {
            let mut obj = Self::Error {
                contact: None,
                address: None,
                option: None,
            };
            if let Err(e) = results.0 {
                obj.contact = Some(e);
            }
            if let Err(e) = results.1 {
                obj.address = Some(e);
            }
            if let Err(e) = results.2 {
                obj.option = Some(e);
            }
            Err(obj)
        }
    } // end of try_from
} // end of impl ShippingModel

impl From<&OrderLineReturnReqDto> for OrderLineIdentity {
    fn from(d: &OrderLineReturnReqDto) -> Self {
        let args = (d.seller_id, d.product_id, d.attr_set_seq);
        Self::from(args)
    }
}
impl From<(u32, u64, u16)> for OrderLineIdentity {
    fn from(d: (u32, u64, u16)) -> Self {
        Self {
            _base: BaseProductIdentity {
                store_id: d.0,
                product_id: d.1,
            },
            _attr_set_req: d.2,
        }
    }
}
impl PartialEq for OrderLineIdentity {
    fn eq(&self, other: &Self) -> bool {
        (self._base == other._base) && (self._attr_set_req == other._attr_set_req)
    }
}
impl OrderLineIdentity {
    pub fn store_id(&self) -> u32 {
        self._base.store_id
    }
    pub fn product_id(&self) -> u64 {
        self._base.product_id
    }
    pub fn attrs_seq_num(&self) -> u16 {
        self._attr_set_req
    }
    fn compare_raw(&self, d: (u32, u64, u16)) -> bool {
        (self.store_id() == d.0) && (self.product_id() == d.1) && (self.attrs_seq_num() == d.2)
    }
}

impl OrderLineQuantityModel {
    pub fn has_unpaid(&self) -> bool {
        self.reserved > self.paid
    }
}

impl From<(u32, u32)> for OrderLinePriceModel {
    fn from((_unit, _total): (u32, u32)) -> Self {
        Self { _unit, _total }
    }
}

impl OrderLinePriceModel {
    fn finalize_price(
        data: &OrderLineRsvReqDto,
        pricem: &ProductPriceModel,
    ) -> DefaultResult<(Self, ProdAttriPriceModel), AppError> {
        let attrprice = pricem.extract_attributes(data)?;
        let baseprice = i32::try_from(pricem.base_price()).map_err(|e| AppError {
            code: AppErrorCode::DataCorruption,
            detail: Some(format!("oline-calc-baseprice: {:?}", e)),
        })?;
        let total_attr_amount = attrprice.total_amount()?;
        let final_unit_price = baseprice.checked_add(total_attr_amount).ok_or(AppError {
            code: AppErrorCode::DataCorruption,
            detail: Some("oline-calc-final-unitprice: overflow".to_string()),
        })?;
        let final_unit_price = u32::try_from(final_unit_price).map_err(|e| AppError {
            code: AppErrorCode::DataCorruption,
            detail: Some(format!("oline-calc-final-unitprice: {:?}", e)),
        })?;
        let price_total = final_unit_price * data.quantity;
        let obj = Self::from((final_unit_price, price_total));
        Ok((obj, attrprice))
    }

    fn into_paym_dto(self, curr_ex: CurrencyModel) -> PayAmountDto {
        let fraction_limit = curr_ex.name.amount_fraction_scale();
        let p_unit_seller = Decimal::new(self.unit() as i64, 0u32);
        let p_total_seller = Decimal::new(self.total() as i64, 0u32);
        let p_unit_buyer = p_unit_seller * curr_ex.rate;
        let p_total_buyer = p_total_seller * curr_ex.rate;
        PayAmountDto {
            unit: p_unit_buyer.trunc_with_scale(fraction_limit).to_string(),
            total: p_total_buyer.trunc_with_scale(fraction_limit).to_string(),
        }
    }

    pub fn unit(&self) -> u32 {
        self._unit
    }
    pub fn total(&self) -> u32 {
        self._total
    }
} // end of impl OrderLinePriceModel

#[rustfmt::skip]
type OLineModelCvtArgs = (
    OrderLineIdentity, OrderLinePriceModel, OrderLineAppliedPolicyModel, OrderLineQuantityModel,
    ProdAttriPriceModel,
);

impl From<OLineModelCvtArgs> for OrderLineModel {
    fn from(value: OLineModelCvtArgs) -> Self {
        Self {
            id_: value.0,
            price: value.1,
            policy: value.2,
            qty: value.3,
            attrs_charge: value.4,
        }
    }
}

impl OrderLineModel {
    fn validate_id_match(
        data: &OrderLineRsvReqDto,
        policym: &ProductPolicyModel,
        pricem: &ProductPriceModel,
    ) -> DefaultResult<(), AppError> {
        let result = if data.product_id != policym.product_id {
            Err("product-policy, id")
        } else if data.product_id != pricem.product_id() {
            Err("product-price, id")
        } else {
            Ok(())
        };
        result.map_err(|msg| AppError {
            code: AppErrorCode::DataCorruption,
            detail: Some(msg.to_string()),
        })
    }
    fn validate_rsv_limit(
        data: &OrderLineRsvReqDto,
        policym: &ProductPolicyModel,
    ) -> DefaultResult<(), AppError> {
        let max_rsv = policym.max_num_rsv as u32;
        let min_rsv = policym.min_num_rsv as u32;
        // note the zero value in max/min rsv means omitting the limit check
        let cond1 = (max_rsv > 0) && (data.quantity > max_rsv);
        let cond2 = (min_rsv > 0) && (min_rsv > data.quantity);
        if cond1 || cond2 {
            let detail = format!(
                "rsv-limit, max:{max_rsv}, min:{min_rsv}, \
                                 given:{}",
                data.quantity
            );
            Err(AppError {
                code: AppErrorCode::ExceedingMaxLimit,
                detail: Some(detail),
            })
        } else {
            Ok(())
        }
    }

    fn update_attr_seqs(lines: &mut Vec<Self>) {
        let mut grps: HashMap<BaseProductIdentity, u16> = HashMap::new();
        lines
            .iter_mut()
            .map(|line| {
                let k = line.id()._base.clone();
                let curr_seq: u16 = *grps.entry(k.clone()).or_default();
                line.id_._attr_set_req = curr_seq;
                grps.insert(k, curr_seq + 1);
            })
            .count();
    }

    pub fn try_from(
        data: OrderLineRsvReqDto,
        policym: &ProductPolicyModel,
        pricem: &ProductPriceModel,
    ) -> DefaultResult<Self, AppError> {
        Self::validate_id_match(&data, policym, pricem)?;
        Self::validate_rsv_limit(&data, policym)?;
        // TODO, move the code below into applied policy model
        let timenow = LocalTime::now().fixed_offset();
        let reserved_until = timenow + Duration::seconds(policym.auto_cancel_secs as i64);
        let warranty_until = timenow + Duration::hours(policym.warranty_hours as i64);
        let (lineprice, attrs_charge) = OrderLinePriceModel::finalize_price(&data, pricem)?;
        let id_ = OrderLineIdentity::from((data.seller_id, data.product_id, 0));
        let qty = OrderLineQuantityModel {
            reserved: data.quantity,
            paid: 0,
            paid_last_update: None,
        };
        let policy = OrderLineAppliedPolicyModel {
            reserved_until,
            warranty_until,
        };
        let args = (id_, lineprice, policy, qty, attrs_charge);
        Ok(Self::from(args))
    } // end of fn try_from

    pub fn generate_order_id(machine_code: u8) -> String {
        // utility for generating top-level identifier to each order
        let oid = generate_custom_uid(machine_code);
        Self::hex_str_order_id(oid)
    }
    fn hex_str_order_id(oid: Uuid) -> String {
        let bs = oid.into_bytes();
        bs.into_iter()
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<String>>()
            .join("")
    }
    pub fn update_payments(
        models: &mut Vec<OrderLineModel>,
        data: Vec<OrderLinePaidUpdateDto>,
        charge_time: DateTime<FixedOffset>,
    ) -> Vec<OrderLinePayUpdateErrorDto> {
        data.into_iter()
            .filter_map(|d| {
                let result = models.iter_mut().find(|m| {
                    let args = (d.seller_id, d.product_id, d.attr_set_seq);
                    m.id().compare_raw(args)
                });
                let possible_error = if let Some(m) = result {
                    let new_paid_qty = m.qty.paid + d.qty;
                    if m.qty.reserved >= new_paid_qty {
                        if let Some(old_dt) = m.qty.paid_last_update.as_ref() {
                            if old_dt < &charge_time {
                                m.qty.paid = new_paid_qty;
                                m.qty.paid_last_update = Some(charge_time);
                                None
                            } else {
                                Some(OrderLinePayUpdateErrorReason::Omitted)
                            }
                        } else {
                            m.qty.paid = new_paid_qty;
                            m.qty.paid_last_update = Some(charge_time);
                            None
                        }
                    } else {
                        Some(OrderLinePayUpdateErrorReason::InvalidQuantity)
                    }
                } else {
                    Some(OrderLinePayUpdateErrorReason::NotExist)
                };
                possible_error.map(|reason| OrderLinePayUpdateErrorDto {
                    seller_id: d.seller_id,
                    product_id: d.product_id,
                    attr_set_seq: d.attr_set_seq,
                    reason,
                })
            })
            .collect()
    } // end of update_payments

    pub(crate) fn num_reserved(&self, time_now: DateTime<FixedOffset>) -> u32 {
        if time_now < self.policy.reserved_until {
            self.qty.reserved
        } else {
            self.qty.paid
        }
    }
    pub fn id(&self) -> &OrderLineIdentity {
        &self.id_
    }
    pub fn price(&self) -> &OrderLinePriceModel {
        &self.price
    }
    pub(crate) fn attrs_charge(&self) -> &ProdAttriPriceModel {
        &self.attrs_charge
    }

    fn into_paym_dto(self, curr_m: CurrencyModel) -> OrderLinePayDto {
        let Self {
            id_,
            price,
            policy,
            qty,
            attrs_charge: _,
        } = self;
        OrderLinePayDto {
            seller_id: id_.store_id(),
            product_id: id_.product_id(),
            attr_set_seq: id_.attrs_seq_num(),
            quantity: qty.reserved,
            reserved_until: policy.reserved_until.to_rfc3339(),
            amount: price.into_paym_dto(curr_m),
        } // TODO, add attribute pricing, and attr-set-seq-num to this dto object
    }
} // end of impl OrderLineModel

impl From<OrderLineModel> for OrderLineStockReservingDto {
    fn from(value: OrderLineModel) -> OrderLineStockReservingDto {
        OrderLineStockReservingDto {
            seller_id: value.id_.store_id(),
            product_id: value.id_.product_id(),
            qty: value.qty.reserved,
        }
    }
}

impl<'a> From<&'a OrderLineModel> for InventoryEditStockLevelDto {
    fn from(value: &'a OrderLineModel) -> InventoryEditStockLevelDto {
        assert!(value.qty.reserved >= value.qty.paid);
        let num_returning = (value.qty.reserved - value.qty.paid) as i32;
        InventoryEditStockLevelDto {
            store_id: value.id_.store_id(),
            product_id: value.id_.product_id(),
            qty_add: num_returning,
            expiry: value.policy.reserved_until,
        } // NOTE, the field `expiry` should NOT be referenced by the entire application
          // , becuase the editing data, converted from order line, does NOT really reflect
          // the expiry time of the original stock item
    }
}

#[derive(Hash, PartialEq, Eq, Clone, Debug)]
pub struct OlineDupError {
    base_id: BaseProductIdentity,
    attr_vals: Vec<String>,
}

impl OlineDupError {
    fn find_duplicate(given: &[OrderLineModel]) -> Vec<Self> {
        let mut grps: HashMap<Self, Vec<&OrderLineModel>> = HashMap::new();
        for line in given {
            let mut attr_vals = line.attrs_charge().applied_attributes();
            attr_vals.sort(); // ensure hash key consistent in hashmap
            let k = Self {
                base_id: line.id()._base.clone(),
                attr_vals,
            };
            grps.entry(k).or_default().push(line);
        }
        grps.into_iter()
            .filter_map(
                |(key, lines)| {
                    if lines.len() > 1 {
                        Some(key)
                    } else {
                        None
                    }
                },
            )
            .collect()
    }
}

impl ToString for OlineDupError {
    fn to_string(&self) -> String {
        format!(
            "order-line-dup, id: {:?}, attributes: {:?}",
            self.base_id, self.attr_vals
        )
    }
}

impl From<OlineDupError> for OrderLineCreateErrorDto {
    fn from(e: OlineDupError) -> Self {
        let OlineDupError { attr_vals, base_id } = e;
        let attr_vals = attr_vals.into_iter().collect::<Vec<_>>();
        Self {
            seller_id: base_id.store_id,
            product_id: base_id.product_id,
            reason: OrderLineCreateErrorReason::DuplicateLines,
            attr_vals: Some(attr_vals),
            nonexist: None,
            shortage: None,
            rsv_limit: None,
        }
    }
}

#[rustfmt::skip]
type OrderTopLvlCvtArgs = (
    String, u32, DateTime<FixedOffset>, OrderCurrencyModel, Vec<OrderLineModel>
);

impl TryFrom<OrderTopLvlCvtArgs> for OrderLineModelSet {
    type Error = Vec<OlineDupError>;
    fn try_from(d: OrderTopLvlCvtArgs) -> DefaultResult<Self, Self::Error> {
        Self::try_from_inner(d, true)
    }
}

impl TryFrom<OrderLineModelSet> for OrderCreateRespOkDto {
    type Error = Vec<AppError>;

    fn try_from(value: OrderLineModelSet) -> Result<Self, Self::Error> {
        let OrderLineModelSet {
            order_id,
            owner_id,
            create_time,
            currency,
            lines,
        } = value;
        let mut errors = Vec::new();
        let reserved_lines = lines
            .into_iter()
            .filter_map(|line| {
                currency
                    .to_buyer_rate(line.id_.store_id())
                    .map_err(|e| errors.push(e))
                    .ok()
                    .map(|rate| line.into_paym_dto(rate))
            })
            .collect::<Vec<_>>();
        if errors.is_empty() {
            Ok(Self {
                order_id,
                usr_id: owner_id,
                currency: currency.into(),
                reserved_lines,
                time: create_time.timestamp() as u64,
            })
        } else {
            Err(errors)
        }
    } // end of fn try-from
} // end of impl OrderLineModelSet

impl OrderLineModelSet {
    pub(crate) fn replica_paym_dto(
        oid: String,
        usr_id: u32,
        olines: Vec<OrderLineModel>,
        currency_m: OrderCurrencyModel,
        billing: BillingModel,
    ) -> DefaultResult<OrderReplicaPaymentDto, AppError> {
        let mut errors = Vec::new();
        let lines = olines
            .into_iter()
            .filter_map(|line| {
                currency_m
                    .to_buyer_rate(line.id_.store_id())
                    .map_err(|e| errors.push(e))
                    .ok()
                    .map(|rate| line.into_paym_dto(rate))
            })
            .collect::<Vec<_>>();
        if errors.is_empty() {
            Ok(OrderReplicaPaymentDto {
                oid,
                usr_id,
                lines,
                billing: billing.into(),
                currency: currency_m.into(),
            })
        } else {
            Err(errors.remove(0))
        }
    }
    pub fn id(&self) -> &String {
        &self.order_id
    }
    pub fn owner(&self) -> u32 {
        self.owner_id
    }
    pub(crate) fn create_time(&self) -> DateTime<FixedOffset> {
        self.create_time
    }
    pub fn currency(&self) -> &OrderCurrencyModel {
        &self.currency
    }
    pub fn lines(&self) -> &[OrderLineModel] {
        &self.lines
    }
    pub(crate) fn append_lines(&mut self, new: Vec<OrderLineModel>) {
        self.lines.extend(new);
    }
    pub(crate) fn unpaid_lines(&self) -> Vec<&OrderLineModel> {
        self.lines
            .iter()
            .filter(|m| m.qty.has_unpaid())
            .collect::<Vec<_>>()
    }

    fn try_from_inner(
        d: OrderTopLvlCvtArgs,
        update_attr_seqs: bool,
    ) -> DefaultResult<Self, Vec<OlineDupError>> {
        let (oid, owner, ctime, currency, mut lines) = d;
        let dup_errs = OlineDupError::find_duplicate(&lines);
        if !dup_errs.is_empty() {
            return Err(dup_errs);
        }
        if update_attr_seqs {
            OrderLineModel::update_attr_seqs(&mut lines);
        }
        Ok(Self {
            order_id: oid,
            owner_id: owner,
            create_time: ctime,
            currency,
            lines,
        })
    }
    pub(crate) fn try_from_repo(d: OrderTopLvlCvtArgs) -> DefaultResult<Self, Vec<OlineDupError>> {
        Self::try_from_inner(d, false)
    }
} // end of impl OrderLineModelSet

impl From<OrderReturnModel> for Vec<OrderLineStockReturningDto> {
    fn from(value: OrderReturnModel) -> Vec<OrderLineStockReturningDto> {
        let (id_, map) = (value.id_, value.qty);
        map.into_iter()
            .map(|(create_time, (qty, _refund))| OrderLineStockReturningDto {
                seller_id: id_.store_id(),
                product_id: id_.product_id(),
                create_time,
                qty,
            })
            .collect()
    }
}

impl OrderReturnModel {
    pub fn num_returned(&self) -> u32 {
        self.qty.values().map(|q| q.0).sum::<u32>()
    }

    pub fn dtime_round_secs(
        time: &DateTime<FixedOffset>,
        n_secs: i64,
    ) -> DefaultResult<DateTime<FixedOffset>, AppError> {
        let dr = Duration::seconds(n_secs);
        match time.duration_trunc(dr) {
            Ok(t) => Ok(t),
            Err(e) => Err(AppError {
                code: AppErrorCode::ExceedingMaxLimit,
                detail: Some(e.to_string()),
            }),
        }
    }

    pub fn filter_requests(
        data: Vec<OrderLineReturnReqDto>,
        o_lines: Vec<OrderLineModel>,
        mut o_returns: Vec<Self>,
    ) -> DefaultResult<Vec<Self>, Vec<OrderLineReturnErrorDto>> {
        let time_now = LocalTime::now().fixed_offset();
        let time_now =
            Self::dtime_round_secs(&time_now, hard_limit::MIN_SECS_INTVL_REQ as i64).unwrap();
        let errors = data
            .iter()
            .filter_map(|d| {
                let result = o_lines.iter().find(|oline| {
                    let args = (d.seller_id, d.product_id, d.attr_set_seq);
                    oline.id_.compare_raw(args)
                });
                let opt = if let Some(oline) = result {
                    if oline.policy.warranty_until > time_now {
                        let result = o_returns.iter().find(|r| r.id_ == oline.id_);
                        let num_returned = if let Some(r) = result.as_ref() {
                            r.num_returned()
                        } else {
                            0u32
                        };
                        let tot_num_return = num_returned + d.quantity;
                        if tot_num_return > oline.num_reserved(time_now) {
                            Some(OrderLineReturnErrorReason::QtyLimitExceed)
                        } else if let Some(r) = result {
                            if r.qty.contains_key(&time_now) {
                                Some(OrderLineReturnErrorReason::DuplicateReturn)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        Some(OrderLineReturnErrorReason::WarrantyExpired)
                    }
                } else {
                    Some(OrderLineReturnErrorReason::NotExist)
                };
                if let Some(reason) = opt {
                    let e = OrderLineReturnErrorDto {
                        seller_id: d.seller_id,
                        reason,
                        product_id: d.product_id,
                        attr_set_seq: d.attr_set_seq,
                    };
                    Some(e)
                } else {
                    None
                }
            })
            .collect::<Vec<OrderLineReturnErrorDto>>();
        if !errors.is_empty() {
            //println!("filter-return-request : {:?}", errors[0].reason);
            return Err(errors);
        }
        let new_returns = data
            .into_iter()
            .filter_map(|d| {
                let req_id_combo = (d.seller_id, d.product_id, d.attr_set_seq);
                let result = o_returns
                    .iter_mut()
                    .find(|ret| ret.id_.compare_raw(req_id_combo));
                let oline = o_lines
                    .iter()
                    .find(|item| item.id_.compare_raw(req_id_combo))
                    .unwrap();
                let total = oline.price.unit() * d.quantity;
                let refund = OrderLinePriceModel::from((oline.price.unit(), total));
                let val = (d.quantity, refund);
                if let Some(r) = result {
                    r.qty.clear(); // no need to output saved requests
                    r.qty.insert(time_now, val);
                    None
                } else {
                    let id_ = OrderLineIdentity::from(req_id_combo);
                    let qty = HashMap::from([(time_now, val)]);
                    Some(Self { id_, qty })
                }
            })
            .collect::<Vec<_>>();
        o_returns.extend(new_returns);
        Ok(o_returns)
    } // end of fn filter_requests

    fn _to_replica_refund_dto(
        self,
        currency_m: &OrderCurrencyModel,
    ) -> DefaultResult<Vec<OrderLineReplicaRefundDto>, AppError> {
        let (pid, map) = (self.id_, self.qty);
        let curr_ex = currency_m.to_buyer_rate(pid.store_id())?;
        let out = map
            .into_iter()
            .map(|(ctime, (q, refund))| OrderLineReplicaRefundDto {
                seller_id: pid.store_id(),
                product_id: pid.product_id(),
                attr_set_seq: pid.attrs_seq_num(),
                create_time: ctime.to_rfc3339(),
                amount: refund.into_paym_dto(curr_ex.clone()),
                qty: q,
            })
            .collect();
        Ok(out)
    }

    pub(crate) fn to_replica_refund_dto(
        o_rets: Vec<Self>,
        currency_m: OrderCurrencyModel,
    ) -> DefaultResult<Vec<OrderLineReplicaRefundDto>, Vec<AppError>> {
        let mut errors = Vec::new();
        let resp = o_rets
            .into_iter()
            .filter_map(|v| {
                v._to_replica_refund_dto(&currency_m)
                    .map_err(|e| errors.push(e))
                    .ok()
            })
            .flatten()
            .collect();
        if errors.is_empty() {
            Ok(resp)
        } else {
            Err(errors)
        }
    }
} // end of impl OrderReturnModel
