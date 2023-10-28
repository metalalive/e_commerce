use serde::{Deserialize, Serialize};

use crate::constant::ProductType;
use crate::api::{jsn_validate_product_type, jsn_serialize_product_type};
use crate::api::dto::OrderLinePayDto;

#[derive(Deserialize, Serialize)]
pub enum CountryCode {TW,TH,IN,ID,US}
impl Into<String> for CountryCode {
    fn into(self) -> String {
        let out = match self {
            Self::TW => "TW",  Self::TH => "TH",
            Self::IN => "IN",  Self::ID => "ID",
            Self::US => "US",
        };
        out.to_string()
    }
} // implement `Into` trait, not replying on serde 

#[derive(Deserialize, Serialize)]
pub enum ShippingMethod {UPS, FedEx, BlackCatExpress}
impl Into<String> for ShippingMethod {
    fn into(self) -> String {
        let out = match self {
            Self::UPS => "UPS",  Self::FedEx => "FedEx",
            Self::BlackCatExpress => "BlackCatExpress",
        };
        out.to_string()
    }
} // implement `Into` trait, not replying on serde 

#[derive(Deserialize, Serialize)]
pub struct OrderLineReqDto {
    pub seller_id: u32,
    pub product_id: u64,
    #[serde(deserialize_with="jsn_validate_product_type", serialize_with="jsn_serialize_product_type")]
    pub product_type: ProductType,
    pub quantity: u32
}

#[derive(Deserialize, Serialize)]
pub enum OrderLineErrorReason {
    NotExist, OutOfStock, NotEnoughToClaim 
}

#[derive(Deserialize, Serialize)]
pub struct OrderLineCreateErrNonExistDto {
    pub product_policy: bool,
    pub product_price: bool,
    pub stock_seller: bool,
}

#[derive(Deserialize, Serialize)]
pub struct OrderLineCreateErrorDto {
    pub seller_id: u32,
    pub product_id: u64,
    #[serde(deserialize_with="jsn_validate_product_type", serialize_with="jsn_serialize_product_type")]
    pub product_type: ProductType,
    pub reason: OrderLineErrorReason,
    pub nonexist: Option<OrderLineCreateErrNonExistDto>,
    pub shortage: Option<u32>
}

#[derive(Deserialize, Serialize)]
pub struct PhoneNumberReqDto {
    pub nation: u16,
    pub number: String,
}
#[derive(Deserialize, Serialize)]
pub struct PhoneNumberErrorDto {
    pub nation: Option<PhoneNumNationErrorReason>,
    pub number: Option<ContactErrorReason>,
}
#[derive(Deserialize, Serialize, Debug)]
pub enum PhoneNumNationErrorReason {InvalidCode}

#[derive(Deserialize, Serialize)]
pub struct ContactReqDto {
    pub first_name: String,
    pub last_name: String,
    pub emails: Vec<String>,
    pub phones: Vec<PhoneNumberReqDto>,
}
#[derive(Deserialize, Serialize)]
pub struct ContactErrorDto {
    pub first_name: Option<ContactErrorReason>,
    pub last_name: Option<ContactErrorReason>,
    pub emails: Option<Vec<Option<ContactErrorReason>>>,
    pub phones: Option<Vec<Option<PhoneNumberErrorDto>>>,
    pub nonfield: Option<ContactNonFieldErrorReason>
}
#[derive(Deserialize, Serialize, Debug)]
pub enum ContactErrorReason {Empty, InvalidChar}
#[derive(Deserialize, Serialize)]
pub enum ContactNonFieldErrorReason {EmailMissing, PhoneMissing}

#[derive(Deserialize, Serialize)]
pub struct PhyAddrReqDto {
    pub country: CountryCode,
    pub region: String,
    pub city: String,
    pub distinct: String,
    pub street_name: Option<String>,
    pub detail: String
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
pub struct ShippingOptionReqDto {
    pub seller_id: u32,
    // #[serde(rename_all="_")]
    pub method: ShippingMethod,
}
#[derive(Deserialize, Serialize)]
pub struct ShippingOptionErrorDto {
    pub seller_id: Option<ShipOptionSellerErrorReason>,
    pub method: Option<ShipOptionMethodErrorReason>,
}
#[derive(Deserialize, Serialize)]
pub enum ShipOptionSellerErrorReason {Empty, NotExist, NotSupport}
#[derive(Deserialize, Serialize)]
pub enum ShipOptionMethodErrorReason {Empty, NotSupport}

#[derive(Deserialize, Serialize)]
pub struct BillingReqDto {
    pub contact: ContactReqDto,
    pub address: Option<PhyAddrReqDto>,
}
#[derive(Deserialize, Serialize)]
pub struct BillingErrorDto {
    pub contact: Option<ContactErrorDto>,
    pub address: Option<PhyAddrErrorDto>,
}

#[derive(Deserialize, Serialize)]
pub struct ShippingReqDto {
    pub contact: ContactReqDto,
    pub address: Option<PhyAddrReqDto>,
    pub option: Vec<ShippingOptionReqDto>,
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
    pub shipping: Option<ShippingErrorDto>
}

#[derive(Deserialize, Serialize)]
pub struct OrderEditReqData {
    pub billing: BillingReqDto,
    pub shipping: ShippingReqDto
}

#[derive(Deserialize, Serialize)]
pub struct ProductPolicyDto {
    #[serde(deserialize_with="jsn_validate_product_type", serialize_with="jsn_serialize_product_type")]
    pub product_type: ProductType,
    pub product_id: u64,
    pub auto_cancel_secs: u32,
    pub warranty_hours: u32,
}

#[derive(Serialize)]
pub struct ProductPolicyClientLimitDto
{
    pub given:u32,
    pub limit:u32
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
}
