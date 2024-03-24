use serde::{Deserialize, Serialize};

use crate::constant::ProductType;
use crate::api::{jsn_validate_product_type, jsn_serialize_product_type};
use crate::api::dto::{OrderLinePayDto, BillingDto, ShippingDto};

#[derive(Deserialize, Serialize)]
pub struct OrderLineReqDto {
    pub seller_id: u32,
    pub product_id: u64,
    #[serde(deserialize_with="jsn_validate_product_type", serialize_with="jsn_serialize_product_type")]
    pub product_type: ProductType,
    pub quantity: u32
}

#[derive(Deserialize, Serialize)]
pub enum OrderLineCreateErrorReason {
    NotExist, OutOfStock, NotEnoughToClaim, RsvLimitViolation 
} // TODO, rename to line-create error reason

#[derive(Serialize)]
pub enum OrderLineReturnErrorReason {
    NotExist, WarrantyExpired, QtyLimitExceed, DuplicateReturn
}

#[derive(Deserialize, Serialize)]
pub struct OrderLineCreateErrNonExistDto {
    pub product_policy: bool,
    pub product_price: bool,
    pub stock_seller: bool,
}

#[derive(Deserialize, Serialize)]
pub struct OLineCreateErrorRsvLimitDto {
    pub max_: u16,
    pub min_: u16,
    pub given: u32,
}

#[derive(Deserialize, Serialize)]
pub struct QuotaResourceErrorDto {
    pub max_: u32,
    pub given: usize,
}

#[derive(Deserialize, Serialize)]
pub struct OrderLineCreateErrorDto {
    pub seller_id: u32,
    pub product_id: u64,
    #[serde(deserialize_with="jsn_validate_product_type", serialize_with="jsn_serialize_product_type")]
    pub product_type: ProductType,
    pub reason: OrderLineCreateErrorReason,
    pub nonexist: Option<OrderLineCreateErrNonExistDto>,
    pub shortage: Option<u32>,
    pub rsv_limit: Option<OLineCreateErrorRsvLimitDto>
}

#[derive(Serialize)]
pub struct OrderLineReturnErrorDto {
    pub seller_id: u32,
    pub product_id: u64,
    #[serde(deserialize_with="jsn_validate_product_type", serialize_with="jsn_serialize_product_type")]
    pub product_type: ProductType,
    pub reason: OrderLineReturnErrorReason
}

#[derive(Deserialize, Serialize)]
pub struct PhoneNumberErrorDto {
    pub nation: Option<PhoneNumNationErrorReason>,
    pub number: Option<ContactErrorReason>,
}
#[derive(Deserialize, Serialize, Debug)]
pub enum PhoneNumNationErrorReason {InvalidCode}

#[derive(Deserialize, Serialize)]
pub struct ContactErrorDto {
    pub first_name: Option<ContactErrorReason>,
    pub last_name: Option<ContactErrorReason>,
    pub emails: Option<Vec<Option<ContactErrorReason>>>,
    pub phones: Option<Vec<Option<PhoneNumberErrorDto>>>,
    pub nonfield: Option<ContactNonFieldErrorReason>,
    pub quota_email: Option<QuotaResourceErrorDto>,
    pub quota_phone: Option<QuotaResourceErrorDto>,
}
#[derive(Deserialize, Serialize, Debug)]
pub enum ContactErrorReason {Empty, InvalidChar}

#[derive(Deserialize, Serialize)]
pub enum ContactNonFieldErrorReason {
    EmailMissing, PhoneMissing, QuotaExceed
}

#[derive(Deserialize, Serialize)]
pub struct PhyAddrErrorDto {
    pub country: Option<PhyAddrNationErrorReason>,
    pub region: Option<PhyAddrRegionErrorReason>,
    pub city:   Option<PhyAddrRegionErrorReason>,
    pub distinct: Option<PhyAddrDistinctErrorReason>,
    pub street_name: Option<PhyAddrDistinctErrorReason>,
    pub detail: Option<PhyAddrDistinctErrorReason>
}
#[derive(Deserialize, Serialize)]
pub enum PhyAddrNationErrorReason {NotSupport}
#[derive(Deserialize, Serialize)]
pub enum PhyAddrRegionErrorReason {Empty, InvalidChar, NotExist, NotSupport}
#[derive(Deserialize, Serialize)]
pub enum PhyAddrDistinctErrorReason {Empty, InvalidChar}

#[derive(Deserialize, Serialize)]
pub struct ShippingOptionErrorDto {
    pub seller_id: Option<ShipOptionSellerErrorReason>,
    pub method: Option<ShipOptionMethodErrorReason>,
}
#[derive(Deserialize, Serialize)]
pub enum ShipOptionSellerErrorReason {Empty, NotExist, NotSupport}
#[derive(Deserialize, Serialize)]
pub enum ShipOptionMethodErrorReason {Empty, NotSupport}

pub type BillingReqDto = BillingDto;
pub type ShippingReqDto = ShippingDto;

#[derive(Deserialize, Serialize)]
pub struct BillingErrorDto {
    pub contact: Option<ContactErrorDto>,
    pub address: Option<PhyAddrErrorDto>,
}

#[derive(Deserialize, Serialize)]
pub struct ShippingErrorDto {
    pub contact: Option<ContactErrorDto>,
    pub address: Option<PhyAddrErrorDto>,
    pub option: Option<Vec<Option<ShippingOptionErrorDto>>>,
}

#[derive(Deserialize, Serialize)]
pub struct OrderCreateReqData {
    pub order_lines: Vec<OrderLineReqDto>,
    pub billing: BillingReqDto,
    pub shipping: ShippingReqDto
}

#[derive(Deserialize, Serialize)]
pub struct OrderCreateRespOkDto {
    pub order_id: String,
    pub usr_id: u32,
    pub time: u64,
    pub reserved_lines: Vec<OrderLinePayDto>,
}

#[derive(Deserialize, Serialize)]
pub struct OrderCreateRespErrorDto {
    pub order_lines: Option<Vec<OrderLineCreateErrorDto>>,
    pub billing: Option<BillingErrorDto>,
    pub shipping: Option<ShippingErrorDto>,
    pub quota_olines: Option<QuotaResourceErrorDto>,
}

#[derive(Deserialize, Serialize)]
pub struct OrderEditReqData {
    pub billing: BillingReqDto,
    pub shipping: ShippingReqDto
}

#[derive(Deserialize)]
pub struct ProductPolicyDto {
    #[serde(deserialize_with="jsn_validate_product_type", serialize_with="jsn_serialize_product_type")]
    pub product_type: ProductType,
    pub product_id: u64,
    pub auto_cancel_secs: u32,
    pub warranty_hours: u32,
    pub max_num_rsv: Option<u16>,
    pub min_num_rsv: Option<u16>,
}

#[derive(Serialize)]
pub struct ProductPolicyClientLimitDto {
    pub given:u32,
    pub limit:u32
}
#[derive(Serialize)]
pub struct ProductPolicyNumRsvLimitDto {
    pub min_items: u16,
    pub max_items: u16,
}

#[derive(Serialize)]
pub struct ProductPolicyClientErrorDto
{
    #[serde(serialize_with="jsn_serialize_product_type")]
    pub product_type: ProductType,
    pub product_id: u64,
    pub err_type: String, // convert from AppError
    pub auto_cancel_secs: Option<ProductPolicyClientLimitDto>,
    pub warranty_hours: Option<ProductPolicyClientLimitDto>,
    pub num_rsv: Option<ProductPolicyNumRsvLimitDto>,
}
