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

use crate::api::dto::{ShippingDto, ShippingMethod, ShippingOptionDto};
use crate::api::rpc::dto::{
    InventoryEditStockLevelDto, OrderLineStockReservingDto, OrderLineStockReturningDto,
};
use crate::api::web::dto::{
    OrderCreateRespOkDto, OrderLineReqDto, OrderLineReturnErrorDto, OrderLineReturnErrorReason,
    ShipOptionSellerErrorReason, ShippingErrorDto, ShippingOptionErrorDto,
};
use crate::constant::hard_limit;
use crate::error::AppError;
use crate::generate_custom_uid;

use super::{
    BaseProductIdentity, CurrencyModel, OrderCurrencyModel, ProductPolicyModel, ProductPriceModel,
};

pub struct ShippingOptionModel {
    pub seller_id: u32,
    pub method: ShippingMethod,
}
pub struct ShippingModel {
    pub contact: ContactModel,
    pub address: Option<PhyAddrModel>,
    pub option: Vec<ShippingOptionModel>,
}

pub type OrderLineIdentity = BaseProductIdentity;

pub struct OrderLineAppliedPolicyModel {
    pub reserved_until: DateTime<FixedOffset>,
    pub warranty_until: DateTime<FixedOffset>,
}

pub struct OrderLinePriceModel {
    // the price values here are smallest unit in seller's currency.
    // In this order-processing service the price amount in this struct
    // is NOT converted with buyer's currency exchange rate
    pub unit: u32,
    pub total: u32,
} // TODO, advanced pricing model

pub struct OrderLineQuantityModel {
    pub reserved: u32,
    pub paid: u32,
    pub paid_last_update: Option<DateTime<FixedOffset>>,
} // TODO, record number of items delivered

pub struct OrderLineModel {
    pub id_: OrderLineIdentity,
    pub price: OrderLinePriceModel,
    pub qty: OrderLineQuantityModel,
    pub policy: OrderLineAppliedPolicyModel,
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
    pub order_id: String,
    pub owner_id: u32,
    pub create_time: DateTime<FixedOffset>,
    pub currency: OrderCurrencyModel,
    pub lines: Vec<OrderLineModel>,
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

impl OrderLineQuantityModel {
    pub fn has_unpaid(&self) -> bool {
        self.reserved > self.paid
    }
}

impl OrderLinePriceModel {
    fn into_paym_dto(self, curr_ex: CurrencyModel) -> PayAmountDto {
        let fraction_limit = curr_ex.name.amount_fraction_scale();
        let p_unit_seller = Decimal::new(self.unit as i64, 0u32);
        let p_total_seller = Decimal::new(self.total as i64, 0u32);
        let p_unit_buyer = p_unit_seller * curr_ex.rate;
        let p_total_buyer = p_total_seller * curr_ex.rate;
        PayAmountDto {
            unit: p_unit_buyer.trunc_with_scale(fraction_limit).to_string(),
            total: p_total_buyer.trunc_with_scale(fraction_limit).to_string(),
        }
    }
}

impl OrderLineModel {
    fn validate_id_match(
        data: &OrderLineReqDto,
        policym: &ProductPolicyModel,
        pricem: &ProductPriceModel,
    ) -> DefaultResult<(), AppError> {
        let id_mismatch = if data.product_type != policym.product_type {
            Some("product-policy, type")
        } else if data.product_id != policym.product_id {
            Some("product-policy, id")
        } else if data.product_type != pricem.product_type {
            Some("product-price, type")
        } else if data.product_id != pricem.product_id {
            Some("product-price, id")
        } else {
            None
        };
        if let Some(msg) = id_mismatch {
            Err(AppError {
                code: AppErrorCode::DataCorruption,
                detail: Some(msg.to_string()),
            })
        } else {
            Ok(())
        }
    }
    fn validate_rsv_limit(
        data: &OrderLineReqDto,
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

    pub fn try_from(
        data: OrderLineReqDto,
        policym: &ProductPolicyModel,
        pricem: &ProductPriceModel,
    ) -> DefaultResult<Self, AppError> {
        Self::validate_id_match(&data, policym, pricem)?;
        Self::validate_rsv_limit(&data, policym)?;
        let timenow = LocalTime::now().fixed_offset();
        let reserved_until = timenow + Duration::seconds(policym.auto_cancel_secs as i64);
        let warranty_until = timenow + Duration::hours(policym.warranty_hours as i64);
        let price_total = pricem.price * data.quantity;
        Ok(Self {
            id_: OrderLineIdentity {
                product_type: data.product_type,
                store_id: data.seller_id,
                product_id: data.product_id,
            },
            qty: OrderLineQuantityModel {
                reserved: data.quantity,
                paid: 0,
                paid_last_update: None,
            },
            price: OrderLinePriceModel {
                unit: pricem.price,
                total: price_total,
            },
            policy: OrderLineAppliedPolicyModel {
                reserved_until,
                warranty_until,
            },
        })
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
                    (m.id_.store_id == d.seller_id)
                        && (m.id_.product_id == d.product_id)
                        && (m.id_.product_type == d.product_type)
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
                if let Some(reason) = possible_error {
                    Some(OrderLinePayUpdateErrorDto {
                        seller_id: d.seller_id,
                        reason,
                        product_id: d.product_id,
                        product_type: d.product_type,
                    })
                } else {
                    None
                }
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

    pub fn into_paym_dto(self, curr_m: CurrencyModel) -> OrderLinePayDto {
        let OrderLineModel {
            id_,
            price,
            policy,
            qty,
        } = self;
        OrderLinePayDto {
            seller_id: id_.store_id,
            product_id: id_.product_id,
            product_type: id_.product_type,
            quantity: qty.reserved,
            reserved_until: policy.reserved_until.to_rfc3339(),
            amount: price.into_paym_dto(curr_m),
        }
    }
} // end of impl OrderLineModel

impl From<OrderLineModel> for OrderLineStockReservingDto {
    fn from(value: OrderLineModel) -> OrderLineStockReservingDto {
        OrderLineStockReservingDto {
            seller_id: value.id_.store_id,
            product_id: value.id_.product_id,
            product_type: value.id_.product_type,
            qty: value.qty.reserved,
        }
    }
}

impl From<OrderLineModel> for InventoryEditStockLevelDto {
    fn from(value: OrderLineModel) -> InventoryEditStockLevelDto {
        assert!(value.qty.reserved >= value.qty.paid);
        let num_returning = (value.qty.reserved - value.qty.paid) as i32;
        InventoryEditStockLevelDto {
            store_id: value.id_.store_id,
            product_id: value.id_.product_id,
            qty_add: num_returning,
            product_type: value.id_.product_type.clone(),
            expiry: value.policy.reserved_until,
        } // NOTE, the field `expiry` should NOT be referenced by the entire application
          // , becuase the editing data, converted from order line, does NOT really reflect
          // the expiry time of the original stock item
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
                    .to_buyer_rate(line.id_.store_id)
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
                    .to_buyer_rate(line.id_.store_id)
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
} // end of impl OrderLineModelSet

impl From<OrderReturnModel> for Vec<OrderLineStockReturningDto> {
    fn from(value: OrderReturnModel) -> Vec<OrderLineStockReturningDto> {
        let (id_, map) = (value.id_, value.qty);
        map.into_iter()
            .map(|(create_time, (qty, _refund))| OrderLineStockReturningDto {
                seller_id: id_.store_id,
                product_id: id_.product_id,
                create_time,
                qty,
                product_type: id_.product_type.clone(),
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
        data: Vec<OrderLineReqDto>,
        o_lines: Vec<OrderLineModel>,
        mut o_returns: Vec<OrderReturnModel>,
    ) -> DefaultResult<Vec<OrderReturnModel>, Vec<OrderLineReturnErrorDto>> {
        let time_now = LocalTime::now().fixed_offset();
        let time_now =
            Self::dtime_round_secs(&time_now, hard_limit::MIN_SECS_INTVL_REQ as i64).unwrap();
        let errors = data
            .iter()
            .filter_map(|d| {
                let result = o_lines.iter().find(|oline| {
                    d.seller_id == oline.id_.store_id
                        && d.product_id == oline.id_.product_id
                        && d.product_type == oline.id_.product_type
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
                        product_type: d.product_type.clone(),
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
                let result = o_returns.iter_mut().find(|ret| {
                    d.seller_id == ret.id_.store_id
                        && d.product_id == ret.id_.product_id
                        && d.product_type == ret.id_.product_type
                });
                let oline = o_lines
                    .iter()
                    .find(|item| {
                        d.seller_id == item.id_.store_id
                            && d.product_id == item.id_.product_id
                            && d.product_type == item.id_.product_type
                    })
                    .unwrap();
                let total = oline.price.unit * d.quantity;
                let refund = OrderLinePriceModel {
                    unit: oline.price.unit,
                    total,
                };
                let val = (d.quantity, refund);
                if let Some(r) = result {
                    r.qty.clear(); // no need to output saved requests
                    r.qty.insert(time_now, val);
                    None
                } else {
                    let id_ = OrderLineIdentity {
                        store_id: d.seller_id,
                        product_id: d.product_id,
                        product_type: d.product_type,
                    };
                    let qty = HashMap::from([(time_now, val)]);
                    Some(OrderReturnModel { id_, qty })
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
        let curr_ex = currency_m.to_buyer_rate(pid.store_id)?;
        let out = map
            .into_iter()
            .map(|(ctime, (_q, refund))| OrderLineReplicaRefundDto {
                seller_id: pid.store_id,
                product_id: pid.product_id,
                product_type: pid.product_type.clone(),
                create_time: ctime.to_rfc3339(),
                amount: refund.into_paym_dto(curr_ex.clone()),
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
