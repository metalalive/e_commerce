use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub enum CountryCode {TW,TH,IN,ID,US}

#[derive(Deserialize, Serialize)]
pub enum ShippingMethod {UPS, FedEx, BlackCatExpress}

#[derive(Deserialize, Serialize)]
pub struct PayAmountDto {
    pub unit: u32,
    pub total: u32
}

#[derive(Deserialize, Serialize)]
pub struct OrderLinePendingDto {
    pub seller_id: u32,
    pub product_id: u32,
    pub quantity: u32
}

#[derive(Deserialize, Serialize)]
pub struct OrderLinePayDto {
    pub seller_id: u32,
    pub product_id: u32,
    pub quantity: u32,
    pub amount: PayAmountDto
}

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
pub struct PhyAddrDto {
    pub country: CountryCode,
    pub region: String,
    pub city: String,
    pub distinct: String,
    pub street_name: Option<String>,
    pub detail: String
}

#[derive(Deserialize, Serialize)]
pub struct ShippingOptionDto {
    pub seller_id: u32,
    // #[serde(rename_all="_")]
    pub method: ShippingMethod,
}

#[derive(Deserialize, Serialize)]
pub struct BillingDto {
    pub contact: ContactDto,
    pub address: Option<PhyAddrDto>,
}

#[derive(Deserialize, Serialize)]
pub struct ShippingDto {
    pub contact: ContactDto,
    pub address: Option<PhyAddrDto>,
    pub option: Vec<ShippingOptionDto>,
}

#[derive(Deserialize, Serialize)]
pub struct OrderCreateReqData {
    pub order_lines: Vec<OrderLinePendingDto>,
    pub billing: BillingDto,
    pub shipping: ShippingDto
}

#[derive(Deserialize, Serialize)]
pub struct OrderCreateRespAsyncData {
    pub order_id: String,
    pub usr_id: u32,
    pub time: u64,
    pub reserved_lines: Vec<OrderLinePayDto>,
    pub async_stock_chk: Vec<OrderLinePendingDto> 
}

#[derive(Deserialize, Serialize)]
pub struct OrderEditReqData {
    pub billing: BillingDto,
    pub shipping: ShippingDto
}

#[derive(Deserialize, Serialize)]
pub struct ProductPolicyDto {
    pub product_id: u64,
    pub auto_cancel_secs: u32,
    pub warranty_hours: u32,
    pub async_stock_chk: bool,
}

