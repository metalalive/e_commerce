use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub enum CountryCode {TW,TH,IN,ID,US}

#[derive(Deserialize, Serialize)]
pub enum ShippingMethod {UPS, FedEx, BlackCatExpress}

#[derive(Deserialize, Serialize)]
pub struct PayAmountModel {
    pub unit: u32,
    pub total: u32
}

#[derive(Deserialize, Serialize)]
pub struct OrderLinePendingModel {
    pub seller_id: u32,
    pub product_id: u32,
    pub quantity: u32
}

#[derive(Deserialize, Serialize)]
pub struct OrderLinePayModel {
    pub seller_id: u32,
    pub product_id: u32,
    pub quantity: u32,
    pub amount: PayAmountModel
}

#[derive(Deserialize, Serialize)]
pub struct PhoneNumberModel {
    pub nation: u16,
    pub number: String,
}

#[derive(Deserialize, Serialize)]
pub struct ContactModel {
    pub first_name: String,
    pub last_name: String,
    pub emails: Vec<String>,
    pub phones: Vec<PhoneNumberModel>,
}

#[derive(Deserialize, Serialize)]
pub struct PhyAddrModel {
    pub country: CountryCode,
    pub region: String,
    pub city: String,
    pub distinct: String,
    pub street_name: Option<String>,
    pub detail: String
}

#[derive(Deserialize, Serialize)]
pub struct ShippingOptionModel {
    pub seller_id: u32,
    // #[serde(rename_all="_")]
    pub method: ShippingMethod,
}

#[derive(Deserialize, Serialize)]
pub struct BillingModel {
    pub contact: ContactModel,
    pub address: Option<PhyAddrModel>,
}

#[derive(Deserialize, Serialize)]
pub struct ShippingModel {
    pub contact: ContactModel,
    pub address: Option<PhyAddrModel>,
    pub option: Vec<ShippingOptionModel>,
}

#[derive(Deserialize, Serialize)]
pub struct OrderCreateReqData {
    pub order_lines: Vec<OrderLinePendingModel>,
    pub billing: BillingModel,
    pub shipping: ShippingModel
}

#[derive(Deserialize, Serialize)]
pub struct OrderCreateRespAsyncData {
    pub order_id: String,
    pub usr_id: u32,
    pub time: u64,
    pub reserved_lines: Vec<OrderLinePayModel>,
    pub async_stock_chk: Vec<OrderLinePendingModel> 
}

#[derive(Deserialize, Serialize)]
pub struct OrderEditReqData {
    pub billing: BillingModel,
    pub shipping: ShippingModel
}

#[derive(Deserialize, Serialize)]
pub struct ProductPolicyData {
    pub product_id: u64,
    pub auto_cancel_secs: u32,
    pub warranty_hours: u32,
    pub async_stock_chk: bool,
}

