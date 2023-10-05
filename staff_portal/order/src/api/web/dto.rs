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
    #[serde(deserialize_with="jsn_validate_product_type", serialize_with="jsn_serialize_product_type")]
    pub product_type: ProductType,
    pub quantity: u32
}

#[derive(Deserialize, Serialize)]
pub struct OrderLinePayDto {
    pub seller_id: u32,
    pub product_id: u64,
    #[serde(deserialize_with="jsn_validate_product_type", serialize_with="jsn_serialize_product_type")]
    pub product_type: ProductType,
    pub quantity: u32,
    pub amount: PayAmountDto
}

#[derive(Deserialize, Serialize)]
pub enum OrderLineErrorReason {
    NotExist, OutOfStock, NotEnoughToClaim 
}

#[derive(Deserialize, Serialize)]
pub struct OrderLineCreateErrorDto {
    pub seller_id: u32,
    pub product_id: u64,
    #[serde(deserialize_with="jsn_validate_product_type", serialize_with="jsn_serialize_product_type")]
    pub product_type: ProductType,
    pub reason: OrderLineErrorReason
}

#[derive(Deserialize, Serialize)]
pub struct PhoneNumberReqDto {
    pub nation: u16,
    pub number: String,
}
#[derive(Deserialize, Serialize)]
pub struct PhoneNumberErrorDto {
    pub nation: Option<PhoneNumNationErrorReason>,
    pub number: Option<PhoneNumNumberErrorReason>,
}
#[derive(Deserialize, Serialize, Debug)]
pub enum PhoneNumNationErrorReason {InvalidCode}
#[derive(Deserialize, Serialize, Debug)]
pub enum PhoneNumNumberErrorReason {Empty, InvalidChar}

#[derive(Deserialize, Serialize)]
pub struct ContactReqDto {
    pub first_name: String,
    pub last_name: String,
    pub emails: Vec<String>,
    pub phones: Vec<PhoneNumberReqDto>,
}
#[derive(Deserialize, Serialize)]
pub struct ContactErrorDto {
    pub first_name: Option<ContactNameErrorReason>,
    pub last_name: Option<ContactNameErrorReason>,
    pub emails: Option<Vec<Option<ContactEmailErrorReason>>>,
    pub phones: Option<Vec<Option<PhoneNumberErrorDto>>>,
    pub nonfield: Option<ContactNonFieldErrorReason>
}
#[derive(Deserialize, Serialize, Debug)]
pub enum ContactNameErrorReason {Empty, InvalidChar}
#[derive(Deserialize, Serialize)]
pub enum ContactEmailErrorReason {InvalidChar, InvalidCode}
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
    pub country: Option<PhyAddrRegionErrorDto>,
    pub region: Option<PhyAddrRegionErrorDto>,
    pub city:   Option<PhyAddrCityErrorDto>,
    pub distinct: Option<PhyAddrDistinctErrorDto>,
    pub street_name: Option<PhyAddrDistinctErrorDto>,
    pub detail: Option<PhyAddrDistinctErrorDto>
}
#[derive(Deserialize, Serialize)]
pub enum PhyAddrRegionErrorDto {Empty, InvalidChar, NotExist, NotSupport}
#[derive(Deserialize, Serialize)]
pub enum PhyAddrCityErrorDto {Empty, InvalidChar, NotExist}
#[derive(Deserialize, Serialize)]
pub enum PhyAddrDistinctErrorDto {Empty, InvalidChar}

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
    pub nonfield: Option<OrderNonFieldErrorReason>
}
#[derive(Deserialize, Serialize)]
pub enum OrderNonFieldErrorReason {ContractRequired}

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
    pub nonfield: Option<OrderNonFieldErrorReason>
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
    pub order_lines: Option<Vec<Option<OrderLineCreateErrorDto>>>,
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
    #[serde(serialize_with="jsn_serialize_product_type")]
    pub product_type: ProductType,
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
            if let ProductType::Unknown(uv) = typ {
                let unexp = Unexpected::Unsigned(uv as u64);
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
