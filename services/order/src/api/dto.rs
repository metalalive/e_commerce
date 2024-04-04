use serde::{Deserialize, Serialize};

use crate::api::{jsn_serialize_product_type, jsn_validate_product_type};
use crate::constant::ProductType;

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
impl Into<String> for CountryCode {
    fn into(self) -> String {
        let out = match self {
            Self::TW => "TW",
            Self::TH => "TH",
            Self::IN => "IN",
            Self::ID => "ID",
            Self::US => "US",
            Self::Unknown => "Unknown",
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

impl Into<String> for ShippingMethod {
    fn into(self) -> String {
        let out = match self {
            Self::UPS => "UPS",
            Self::FedEx => "FedEx",
            Self::BlackCatExpress => "BlackCatExpress",
            Self::Unknown => "Unknown",
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
pub struct PayAmountDto {
    pub unit: u32,
    pub total: u32,
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
