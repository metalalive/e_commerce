use serde::{Deserialize, Serialize};

use ecommerce_common::api::dto::{ContactDto, PhyAddrDto};

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
#[serde(untagged)]
pub enum ProdAttrValueDto {
    Int(i32),
    Str(String),
    Bool(bool),
}
