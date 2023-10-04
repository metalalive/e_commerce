use serde::{Deserialize, Serialize};
use serde::de::{Error as DeserializeError, Expected, Unexpected};

use crate::constant::ProductType;

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
pub struct OrderLineReqDto {
    pub seller_id: u32,
    pub product_id: u64,
    pub product_type: u8,
    pub quantity: u32
}

#[derive(Deserialize, Serialize)]
pub struct OrderLinePayDto {
    pub seller_id: u32,
    pub product_id: u64,
    pub product_type: u8,
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
    pub order_lines: Vec<OrderLineReqDto>,
    pub billing: BillingDto,
    pub shipping: ShippingDto
}

#[derive(Deserialize, Serialize)]
pub struct OrderCreateRespOkDto {
    pub order_id: String,
    pub usr_id: u32,
    pub time: u64,
    pub reserved_lines: Vec<OrderLinePayDto>,
}

#[derive(Deserialize, Serialize)]
pub struct OrderEditReqData {
    pub billing: BillingDto,
    pub shipping: ShippingDto
}

#[derive(Deserialize, Serialize)]
pub struct ProductPolicyDto {
    #[serde(deserialize_with="jsn_validate_product_type", serialize_with="jsn_serialize_product_type")]
    pub product_type: ProductType,
    pub product_id: u64,
    pub auto_cancel_secs: u32,
    pub warranty_hours: u32,
    pub async_stock_chk: bool,
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
    pub product_type: u8,
    pub product_id: u64,
    pub err_type: String, // convert from AppError
    pub auto_cancel_secs: Option<ProductPolicyClientLimitDto>,
    pub warranty_hours: Option<ProductPolicyClientLimitDto>,
}



struct ExpectProdTyp {
    numbers: Vec<u8>
}
impl Expected for ExpectProdTyp
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        let s:Vec<String> = self.numbers.iter().map(|n| n.to_string()).collect();
        let s = s.join(",");
        let msg = format!("accepted type number : {s}");
        formatter.write_str(msg.as_str())
    }
}

fn jsn_validate_product_type<'de, D>(raw:D) ->  Result<ProductType, D::Error>
    where D: serde::Deserializer<'de>
{
    match u8::deserialize(raw) {
        Ok(d) => {
            let typ = ProductType::from(d);
            if typ == ProductType::Unknown {
                let unexp = Unexpected::Unsigned(d as u64);
                let exp = ExpectProdTyp{ numbers: vec![
                    ProductType::Item.into(),
                    ProductType::Package.into()
                ]};
                let e = DeserializeError::invalid_value(unexp, &exp) ;
                Err(e)
            } else { Ok(typ) }
        },
        Err(e) => Err(e)
    }
}
fn jsn_serialize_product_type<S>(orig:&ProductType, ser:S)
    -> Result<S::Ok, S::Error> where S: serde::Serializer
{
    let v = orig.clone().into();
    ser.serialize_u8(v)
}
