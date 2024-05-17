use chrono::{DateTime, Duration, DurationRound, FixedOffset, Local as LocalTime};
use std::collections::HashMap;
use std::result::Result as DefaultResult;
use std::vec::Vec;
use uuid::Uuid;

use ecommerce_common::api::dto::{OrderLinePayDto, PayAmountDto};
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::model::order::{ContactModel, PhyAddrModel};

use crate::api::dto::{ShippingDto, ShippingMethod, ShippingOptionDto};
use crate::api::rpc::dto::{
    InventoryEditStockLevelDto, OrderLinePaidUpdateDto, OrderLinePayUpdateErrorDto,
    OrderLinePayUpdateErrorReason, OrderLineReplicaRefundDto, OrderLineStockReservingDto,
    OrderLineStockReturningDto,
};
use crate::api::web::dto::{
    OrderLineReqDto, OrderLineReturnErrorDto, OrderLineReturnErrorReason,
    ShipOptionSellerErrorReason, ShippingErrorDto, ShippingOptionErrorDto,
};
use crate::constant::hard_limit;
use crate::error::AppError;
use crate::generate_custom_uid;

use super::{BaseProductIdentity, ProductPolicyModel, ProductPriceModel};

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
    pub lines: Vec<OrderLineModel>,
    // TODO, add currency field
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
    ) -> Vec<OrderLinePayUpdateErrorDto> {
        let dt_now = LocalTime::now();
        data.into_iter()
            .filter_map(|d| {
                let result = models.iter_mut().find(|m| {
                    (m.id_.store_id == d.seller_id)
                        && (m.id_.product_id == d.product_id)
                        && (m.id_.product_type == d.product_type)
                });
                let possible_error = if let Some(m) = result {
                    if dt_now < m.policy.reserved_until {
                        if m.qty.reserved >= d.qty {
                            if let Some(old_dt) = m.qty.paid_last_update.as_ref() {
                                if old_dt < &d.time {
                                    (m.qty.paid, m.qty.paid_last_update) = (d.qty, Some(d.time));
                                    None
                                } else {
                                    Some(OrderLinePayUpdateErrorReason::Omitted)
                                }
                            } else {
                                (m.qty.paid, m.qty.paid_last_update) = (d.qty, Some(d.time));
                                None
                            }
                        } else {
                            Some(OrderLinePayUpdateErrorReason::InvalidQuantity)
                        } // TODO, remove the quantity check, for payment failure rollback
                    } else {
                        Some(OrderLinePayUpdateErrorReason::ReservationExpired)
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
} // end of impl OrderLineModel

impl From<OrderLineModel> for OrderLinePayDto {
    fn from(value: OrderLineModel) -> OrderLinePayDto {
        OrderLinePayDto {
            seller_id: value.id_.store_id,
            product_id: value.id_.product_id,
            product_type: value.id_.product_type,
            quantity: value.qty.reserved,
            reserved_until: value.policy.reserved_until.to_rfc3339(),
            amount: PayAmountDto {
                unit: value.price.unit,
                total: value.price.total,
            },
        }
    }
}

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
impl From<OrderReturnModel> for Vec<OrderLineReplicaRefundDto> {
    fn from(value: OrderReturnModel) -> Vec<OrderLineReplicaRefundDto> {
        let (pid, map) = (value.id_, value.qty);
        map.into_iter()
            .map(|(ctime, (_q, refund))| OrderLineReplicaRefundDto {
                seller_id: pid.store_id,
                product_id: pid.product_id,
                product_type: pid.product_type.clone(),
                create_time: ctime,
                amount: PayAmountDto {
                    unit: refund.unit,
                    total: refund.total,
                },
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
} // end of impl OrderReturnModel
