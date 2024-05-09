use ecommerce_common::constant::ProductType;
use serde::{Deserialize, Serialize};

use ecommerce_common::api::dto::PayAmountDto;

use crate::api::{jsn_serialize_product_type, jsn_validate_product_type};

// TODO, merge the 2 DTO modules in `/web` and `/rpc` package

#[derive(Deserialize, Serialize)]
pub struct PhoneNumberDto {
    pub nation: u16,
    pub number: String,
}
#[derive(Deserialize, Serialize)]
pub struct ContactDto {
    pub first_name: String,
    pub last_name: String,
    pub emails: Vec<String>,
    pub phones: Vec<PhoneNumberDto>,
}

#[derive(Deserialize, Serialize)]
pub enum CountryCode {
    TW,
    TH,
    IN,
    ID,
    US,
    Unknown,
}
impl From<CountryCode> for String {
    fn from(value: CountryCode) -> String {
        let out = match value {
            CountryCode::TW => "TW",
            CountryCode::TH => "TH",
            CountryCode::IN => "IN",
            CountryCode::ID => "ID",
            CountryCode::US => "US",
            CountryCode::Unknown => "Unknown",
        };
        out.to_string()
    }
} // implement `Into` trait, not replying on serde
impl From<String> for CountryCode {
    fn from(value: String) -> Self {
        match value.as_str() {
            "TW" => Self::TW,
            "TH" => Self::TH,
            "IN" => Self::IN,
            "ID" => Self::ID,
            "US" => Self::US,
            _others => Self::Unknown,
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct PhyAddrDto {
    pub country: CountryCode,
    pub region: String,
    pub city: String,
    pub distinct: String,
    pub street_name: Option<String>,
    pub detail: String,
}

#[derive(Deserialize, Serialize)]
pub struct ShippingOptionDto {
    pub seller_id: u32,
    // #[serde(rename_all="_")]
    pub method: ShippingMethod,
}

#[derive(Deserialize, Serialize)]
pub enum ShippingMethod {
    UPS,
    FedEx,
    BlackCatExpress,
    Unknown,
}

impl From<ShippingMethod> for String {
    fn from(value: ShippingMethod) -> String {
        let out = match value {
            ShippingMethod::UPS => "UPS",
            ShippingMethod::FedEx => "FedEx",
            ShippingMethod::BlackCatExpress => "BlackCatExpress",
            ShippingMethod::Unknown => "Unknown",
        };
        out.to_string()
    }
} // implement `Into` trait, not replying on serde
impl From<String> for ShippingMethod {
    fn from(value: String) -> Self {
        match value.as_str() {
            "UPS" => Self::UPS,
            "FedEx" => Self::FedEx,
            "BlackCatExpress" => Self::BlackCatExpress,
            _others => Self::Unknown,
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct ShippingDto {
    pub contact: ContactDto,
    pub address: Option<PhyAddrDto>,
    pub option: Vec<ShippingOptionDto>,
}

#[derive(Deserialize, Serialize)]
pub struct BillingDto {
    pub contact: ContactDto,
    pub address: Option<PhyAddrDto>,
}

#[derive(Deserialize, Serialize)]
pub struct OrderLinePayDto {
    pub seller_id: u32,
    pub product_id: u64,
    #[serde(
        deserialize_with = "jsn_validate_product_type",
        serialize_with = "jsn_serialize_product_type"
    )]
    pub product_type: ProductType,
    pub reserved_until: String, // date-time formatted in RFC3339 spec
    pub quantity: u32,
    pub amount: PayAmountDto,
}
