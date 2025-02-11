use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct PayAmountDto {
    // represented as string , can be converted from decimal type
    pub unit: String,
    pub total: String,
}

#[derive(Deserialize, Serialize)]
pub struct GenericRangeErrorDto {
    pub max_: u16, // TODO, same type
    pub min_: u16,
    pub given: u32,
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

#[rustfmt::skip]
#[derive(Deserialize, Serialize, Clone)]
pub enum CountryCode { TW, TH, IN, ID, US, Unknown }

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
    // TODO, from literal string
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

#[rustfmt::skip]
#[allow(clippy::upper_case_acronyms)]
#[derive(Deserialize, Serialize, Debug, Clone, Hash, Eq, PartialEq)]
pub enum CurrencyDto { INR, IDR, THB, TWD, USD, Unknown }
// #[serde(rename_all = "UPPERCASE")], FIXME, the macro does not work

impl ToString for CurrencyDto {
    fn to_string(&self) -> String {
        let o = match self {
            Self::INR => "INR",
            Self::IDR => "IDR",
            Self::THB => "THB",
            Self::TWD => "TWD",
            Self::USD => "USD",
            Self::Unknown => "Unknown",
        };
        o.to_string()
    }
}

impl From<&String> for CurrencyDto {
    // TODO, from literal string
    fn from(value: &String) -> Self {
        match value.as_str() {
            "INR" => Self::INR,
            "IDR" => Self::IDR,
            "THB" => Self::THB,
            "TWD" => Self::TWD,
            "USD" => Self::USD,
            _others => Self::Unknown,
        }
    }
}

impl CurrencyDto {
    /// Number of digits in fraction part of a decimal value allowed
    /// in a given amount value. Note the decimal places should depends
    /// on the currency applied, due to the limit specified in 3rd-party
    /// payment processors such as Stripe
    pub fn amount_fraction_scale(&self) -> u32 {
        match self {
            Self::INR | Self::IDR | Self::THB | Self::TWD | Self::USD => 2,
            Self::Unknown => 0,
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
pub struct BillingDto {
    pub contact: ContactDto,
    pub address: Option<PhyAddrDto>,
}

#[derive(Deserialize, Serialize)]
pub struct OrderLinePayDto {
    pub seller_id: u32,
    pub product_id: u64,
    pub reserved_until: String, // date-time formatted in RFC3339 spec
    // TODO, add warranty time
    pub quantity: u32,
    pub amount: PayAmountDto,
}

#[derive(Deserialize, Serialize)]
pub struct CurrencySnapshotDto {
    pub name: CurrencyDto,
    pub rate: String,
}

#[derive(Deserialize, Serialize)]
pub struct OrderSellerCurrencyDto {
    pub currency: CurrencyDto,
    pub seller_id: u32,
}

#[derive(Deserialize, Serialize)]
pub struct OrderCurrencySnapshotDto {
    pub snapshot: Vec<CurrencySnapshotDto>,
    pub sellers: Vec<OrderSellerCurrencyDto>,
    pub buyer: CurrencyDto,
}
