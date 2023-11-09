use std::vec::Vec;
use std::result::Result as DefaultResult;
use chrono::{DateTime, FixedOffset, Local as LocalTime, Duration};
use regex::Regex;
use uuid::{Uuid, Builder, Timestamp, NoContext};

use crate::api::dto::{
    ContactDto, PhyAddrDto, ShippingOptionDto, ShippingMethod, CountryCode,
    BillingDto, ShippingDto, PhoneNumberDto, OrderLinePayDto, PayAmountDto
};
use crate::api::rpc::dto::{
    OrderLineReplicaInventoryDto, OrderLinePayUpdateErrorDto, OrderLinePaidUpdateDto,
    OrderLinePayUpdateErrorReason,
};
use crate::api::web::dto::{
    BillingErrorDto, ShippingErrorDto, ContactErrorDto, PhyAddrErrorDto,
    ShipOptionSellerErrorReason, PhyAddrRegionErrorReason, PhyAddrDistinctErrorReason,
    ContactErrorReason, ContactNonFieldErrorReason, PhoneNumberErrorDto,
    PhoneNumNationErrorReason, OrderLineReqDto, ShippingOptionErrorDto 
};
use crate::constant::{REGEX_EMAIL_RFC5322, ProductType};

use super::{ProductPolicyModel, ProductPriceModel};

pub struct ContactModel {
    pub first_name: String,
    pub last_name: String,
    pub emails: Vec<String>,
    pub phones: Vec<PhoneNumberDto>,
} // TODO, fraud check

pub struct PhyAddrModel {
    pub country: CountryCode,
    pub region: String,
    pub city: String,
    pub distinct: String,
    pub street_name: Option<String>,
    pub detail: String
}
pub struct ShippingOptionModel {
    pub seller_id: u32,
    pub method: ShippingMethod,
}
pub struct BillingModel {
    pub contact: ContactModel,
    pub address: Option<PhyAddrModel>,
}
pub struct ShippingModel {
    pub contact: ContactModel,
    pub address: Option<PhyAddrModel>,
    pub option : Vec<ShippingOptionModel>
}

pub struct OrderLineAppliedPolicyModel {
    pub reserved_until: DateTime<FixedOffset>,
    pub warranty_until: DateTime<FixedOffset>
}

pub struct OrderLinePriceModel {
    pub unit:u32,
    pub total:u32
} // TODO, advanced pricing model

pub struct OrderLineQuantityModel {
    pub reserved: u32,
    pub paid: u32,
    pub paid_last_update: Option<DateTime<FixedOffset>>,
} // TODO, record number delivered, and cancelled

pub struct OrderLineModel {
    pub seller_id: u32,
    pub product_type: ProductType,
    pub product_id : u64,
    pub price: OrderLinePriceModel,
    pub qty: OrderLineQuantityModel,
    pub policy: OrderLineAppliedPolicyModel
}

pub struct OrderLineModelSet {
    pub order_id: String,
    pub lines: Vec<OrderLineModel>,
}

impl Into<ContactDto> for ContactModel {
    fn into(self) -> ContactDto {
        ContactDto { first_name: self.first_name, last_name: self.last_name,
            emails: self.emails, phones: self.phones }
    }
}

impl TryFrom<ContactDto> for ContactModel {
    type Error = ContactErrorDto;
    fn try_from(value: ContactDto) -> DefaultResult<Self, Self::Error> {
        let fnam_rs = Self::check_alphabetic(value.first_name.as_str());
        let lnam_rs = Self::check_alphabetic(value.last_name.as_str());
        let (em_rs, ph_rs, nonfd_rs) = if value.emails.is_empty() {
            (None, None, Some(ContactNonFieldErrorReason::EmailMissing))
        } else if value.phones.is_empty() {
            (None, None, Some(ContactNonFieldErrorReason::PhoneMissing))
        } else {
            let rs1 = Self::check_emails(&value.emails);
            let rs2 = Self::check_phones(&value.phones);
            (rs1, rs2, None)
        };
        let error = Self::Error { first_name: fnam_rs, last_name: lnam_rs,
                emails: em_rs, phones: ph_rs, nonfield: nonfd_rs };
        if error.first_name.is_none() && error.emails.is_none() && error.phones.is_none()
            && error.last_name.is_none() && error.nonfield.is_none()
        {
            Ok(Self{ first_name: value.first_name, last_name: value.last_name,
                emails: value.emails, phones: value.phones })
        } else { Err(error) }
    } // end of fn try_from
}
impl ContactModel {
    fn check_alphabetic (value:&str) -> Option<ContactErrorReason>
    {
        if value.is_empty() {
            Some(ContactErrorReason::Empty)
        } else if !value.chars().all(char::is_alphabetic) {
            Some(ContactErrorReason::InvalidChar)
        } else { None }
    }
    fn check_emails (value:&Vec<String>) -> Option<Vec<Option<ContactErrorReason>>>
    {
        let mut num_err:usize = 0;
        let re = Regex::new(REGEX_EMAIL_RFC5322).unwrap();
        let out = value.iter().map(|d| {
            if d.is_empty() {
                num_err += 1;
                Some(ContactErrorReason::Empty)
            } else if let Some(_v) = re.find(d.as_str()) {
                if _v.start() == 0 && d.len() == _v.end() {
                    None // given data should match the mail pattern exactly once
                } else {
                    num_err += 1;
                    Some(ContactErrorReason::InvalidChar)
                }
            } else {
                num_err += 1;
                Some(ContactErrorReason::InvalidChar)
            }
        }).collect();
        if num_err == 0 {None} else {Some(out)}
    }
    fn check_phones (value:&Vec<PhoneNumberDto>) -> Option<Vec<Option<PhoneNumberErrorDto>>>
    {
        let mut num_err:usize = 0;
        let out = value.iter().map(|d| {
            let nation_err = if d.nation > 0 && d.nation <= 999 { None }
            else { Some(PhoneNumNationErrorReason::InvalidCode) };
            let all_digits = d.number.chars().all(|c| c.is_digit(10));
            let number_err = if all_digits {None}
            else { Some(ContactErrorReason::InvalidChar) };
            if nation_err.is_some() || number_err.is_some() {
                num_err += 1;
                Some(PhoneNumberErrorDto { nation:nation_err, number:number_err })
            } else {None}
        }).collect();
        if num_err == 0 {None} else {Some(out)}
    }
} // end of impl ContactModel


impl Into<PhyAddrDto> for PhyAddrModel {
    fn into(self) -> PhyAddrDto {
        PhyAddrDto { country: self.country, region: self.region, city: self.city,
            distinct: self.distinct, street_name: self.street_name, detail: self.detail }
    }
}

impl TryFrom<PhyAddrDto> for PhyAddrModel {
    type Error = PhyAddrErrorDto;
    fn try_from(value: PhyAddrDto) -> DefaultResult<Self, Self::Error> {
        let region_rs = Self::check_region(value.region.as_str());
        let citi_rs = Self::check_region(value.city.as_str());
        let dist_rs = Self::contain_ctrl_char(value.distinct.as_str());
        let street_rs = if let Some(v) = value.street_name.as_ref() {
            Self::contain_ctrl_char(v.as_str())
        } else { None };
        let detail_rs = Self::contain_ctrl_char(value.detail.as_str());
        let error = Self::Error {country:None, region:region_rs, city:citi_rs,
            distinct:dist_rs, street_name:street_rs, detail:detail_rs };
        if error.region.is_none() && error.city.is_none() && error.detail.is_none()
            && error.distinct.is_none() && error.street_name.is_none()
        {
            Ok(Self { country: value.country, region: value.region, city: value.city,
                distinct: value.distinct, street_name: value.street_name, detail: value.detail
            })
        } else { Err(error) }
    }
} // end of impl PhyAddrModel
impl PhyAddrModel {
    pub fn try_from_opt(value: Option<PhyAddrDto>) -> DefaultResult<Option<Self>, PhyAddrErrorDto>
    {
        if let Some(d) = value {
            match PhyAddrModel::try_from(d) {
                Ok(m) => Ok(Some(m)),
                Err(e) => Err(e)
            }
        } else {
            Ok(None)
        } // client is allowed NOT to provide address with the order
    }
    fn check_region (value:&str) -> Option<PhyAddrRegionErrorReason>
    {
        if value.is_empty() {
            Some(PhyAddrRegionErrorReason::Empty)
        } else if !value.chars().all(|c| {c.is_alphabetic() || c.is_whitespace()}) {
            Some(PhyAddrRegionErrorReason::InvalidChar)
        } else { None }
    }
    fn contain_ctrl_char (value:&str) -> Option<PhyAddrDistinctErrorReason>
    {
        if value.is_empty() {
            Some(PhyAddrDistinctErrorReason::Empty)
        } else if value.chars().any(char::is_control) {
            Some(PhyAddrDistinctErrorReason::InvalidChar)
        } else { None }
    }
} // end of impl PhyAddrModel


impl Into<ShippingOptionDto> for ShippingOptionModel {
    fn into(self) -> ShippingOptionDto {
        ShippingOptionDto { seller_id: self.seller_id, method:self.method }
    }
}

impl TryFrom<ShippingOptionDto> for ShippingOptionModel {
    type Error = ShippingOptionErrorDto;
    fn try_from(value: ShippingOptionDto) -> DefaultResult<Self, Self::Error> {
        if value.seller_id == 0 {
            let e = Self::Error { method: None,
                seller_id: Some(ShipOptionSellerErrorReason::Empty) };
            Err(e)
        } else {
            Ok(Self { seller_id: value.seller_id, method: value.method })
        } // TODO, will check whether the seller supports specific delivery service
    }
}
impl ShippingOptionModel {
    pub fn try_from_vec(value :Vec<ShippingOptionDto>)
        -> DefaultResult<Vec<Self>, Vec<Option<ShippingOptionErrorDto>>>
    {
        let results = value.into_iter().map(Self::try_from).collect
                ::<  Vec<DefaultResult<Self, ShippingOptionErrorDto>>  >();
        if results.iter().any(DefaultResult::is_err) {
            let objs = results.into_iter().map(|r| {
                if let Err(e) = r { Some(e) }
                else {None} // extract all errors or return none if the item is in valid format
            }).collect();
            Err(objs)
        } else {
            let objs = results.into_iter().map(|r| {
                if let Ok(m) = r { m }
                else { panic!("failed to check results"); }
            }).collect();
            Ok(objs)
        }
    }
} // end of impl ShippingOptionModel

impl Into<BillingDto> for BillingModel {
    fn into(self) -> BillingDto {
        let (c, pa) = (self.contact.into(), self.address);
        let a = if let Some(v) = pa {
            Some(v.into())
        } else {None};
        BillingDto { contact: c, address: a }
    }
}

impl TryFrom<BillingDto> for BillingModel {
    type Error = BillingErrorDto;
    fn try_from(value: BillingDto) -> DefaultResult<Self, Self::Error>
    {
        let results = (ContactModel::try_from(value.contact),
                       PhyAddrModel::try_from_opt(value.address));
        if let (Ok(contact), Ok(maybe_addr)) = results {
            let obj = Self {contact, address:maybe_addr};
            Ok(obj)
        } else {
            let mut obj = Self::Error { contact: None, address: None };
            if let Err(e) = results.0 { obj.contact = Some(e); }
            if let Err(e) = results.1 { obj.address = Some(e); }
            Err(obj)
        }
    }
} // end of impl BillingModel

impl Into<ShippingDto> for ShippingModel {
    fn into(self) -> ShippingDto {
        let (c, pa, opt) = (self.contact.into(), self.address, self.option);
        let a = if let Some(v) = pa {
            Some(v.into())
        } else {None};
        let opt = opt.into_iter().map(ShippingOptionModel::into).collect();
        ShippingDto { contact: c, address: a, option: opt }
    }
}

impl TryFrom<ShippingDto> for ShippingModel {
    type Error = ShippingErrorDto;
    fn try_from(value: ShippingDto) -> DefaultResult<Self, Self::Error>
    {
        let results = (ContactModel::try_from(value.contact),
                       PhyAddrModel::try_from_opt(value.address),
                       ShippingOptionModel::try_from_vec(value.option) );
        if let (Ok(contact), Ok(maybe_addr), Ok(sh_opts)) = results {
            let obj = Self {contact, address:maybe_addr, option:sh_opts};
            Ok(obj)
        } else {
            let mut obj = Self::Error { contact: None, address: None,
                option: None };
            if let Err(e) = results.0 { obj.contact = Some(e); }
            if let Err(e) = results.1 { obj.address = Some(e); }
            if let Err(e) = results.2 { obj.option = Some(e); }
            Err(obj)
        }
    } // end of try_from
} // end of impl ShippingModel

impl  OrderLineModel {
    pub fn from(data:OrderLineReqDto, policym:&ProductPolicyModel, pricem:&ProductPriceModel) -> Self
    {
        assert_eq!(data.product_type, policym.product_type);
        assert_eq!(data.product_id,   policym.product_id);
        assert_eq!(data.product_type, pricem.product_type);
        assert_eq!(data.product_id,   pricem.product_id);
        let timenow = LocalTime::now().fixed_offset();
        let reserved_until = timenow + Duration::seconds(policym.auto_cancel_secs as i64);
        let warranty_until = timenow + Duration::hours(policym.warranty_hours as i64);
        let price_total = pricem.price * data.quantity;
        Self { seller_id: data.seller_id, product_type: data.product_type, product_id: data.product_id,
            qty: OrderLineQuantityModel { reserved: data.quantity, paid:0, paid_last_update:None },
            price: OrderLinePriceModel { unit: pricem.price, total: price_total } ,
            policy: OrderLineAppliedPolicyModel { reserved_until, warranty_until }
        }
    }
    pub fn generate_order_id (machine_code:u8) -> String
    { // utility for generating top-level identifier to each order
        // UUIDv7 is for single-node application. This app needs to consider
        // scalability of multi-node environment, UUIDv8 can be utilized cuz it
        // allows custom ID layout, so few bits of the ID can be assigned to
        // represent each machine/node ID,  rest of that should be timestamp with
        // random byte sequence
        let ts_ctx = NoContext;
        let (secs, nano) = Timestamp::now(ts_ctx).to_unix();
        let millis = (secs * 1000).saturating_add((nano as u64) / 1_000_000);
        let mut node_id = rand::random::<[u8;10]>();
        node_id[0] = machine_code;
        let builder = Builder::from_unix_timestamp_millis(millis, &node_id);
        let oid = builder.into_uuid();
        Self::hex_str_order_id(oid)
    }
    fn hex_str_order_id(oid:Uuid) -> String
    {
        let bs = oid.into_bytes();
        bs.into_iter().map(|b| format!("{:02x}",b))
            .collect::<Vec<String>>().join("")
    }
    pub fn update_payments(models:&mut Vec<OrderLineModel>, data:Vec<OrderLinePaidUpdateDto>)
        -> Vec<OrderLinePayUpdateErrorDto>
    {
        let dt_now = LocalTime::now();
        data.into_iter().filter_map(|d| {
            let result = models.iter_mut().find(|m| {
                (m.seller_id == d.seller_id) && (m.product_id == d.product_id) && 
                    (m.product_type == d.product_type)
            });
            let possible_error = if let Some(m) = result {
                if dt_now < m.policy.reserved_until {
                    if m.qty.reserved >= d.qty {
                        if let Some(old_dt) = m.qty.paid_last_update.as_ref() {
                            if old_dt < &d.time {
                                (m.qty.paid, m.qty.paid_last_update) = (d.qty, Some(d.time));
                                None
                            } else { Some(OrderLinePayUpdateErrorReason::Omitted) }
                        } else {
                            (m.qty.paid, m.qty.paid_last_update) = (d.qty, Some(d.time));
                            None
                        }
                    } else { Some(OrderLinePayUpdateErrorReason::InvalidQuantity) }
                } else { Some(OrderLinePayUpdateErrorReason::ReservationExpired) }
            } else { Some(OrderLinePayUpdateErrorReason::NotExist) };
            if let Some(reason) = possible_error {
                Some(OrderLinePayUpdateErrorDto { seller_id: d.seller_id, reason,
                    product_id: d.product_id, product_type: d.product_type })
            } else { None }
        }).collect()
    } // end of update_payments
} // end of impl OrderLineModel

impl Into<OrderLinePayDto> for OrderLineModel {
    fn into(self) -> OrderLinePayDto {
        OrderLinePayDto { seller_id: self.seller_id, product_id: self.product_id,
            product_type: self.product_type, quantity: self.qty.reserved,
            reserved_until:self.policy.reserved_until.to_rfc3339(),
            amount: PayAmountDto { unit: self.price.unit, total: self.price.total}
        }
    }
}

impl Into<OrderLineReplicaInventoryDto> for OrderLineModel {
    fn into(self) -> OrderLineReplicaInventoryDto {
        OrderLineReplicaInventoryDto { seller_id: self.seller_id, product_id: self.product_id,
            product_type: self.product_type, qty_booked: self.qty.reserved }
    }
}

