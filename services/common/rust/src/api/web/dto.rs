use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, PartialEq, Debug)]
pub struct QuotaResourceErrorDto {
    pub max_: u32,
    pub given: usize,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct PhoneNumberErrorDto {
    pub nation: Option<PhoneNumNationErrorReason>,
    pub number: Option<ContactErrorReason>,
}
#[derive(Deserialize, Serialize, Debug)]
pub enum PhoneNumNationErrorReason {
    InvalidCode,
}
#[derive(Deserialize, Serialize, Debug)]
pub enum ContactErrorReason {
    Empty,
    InvalidChar,
}

#[derive(Deserialize, Serialize, Debug)]
pub enum ContactNonFieldErrorReason {
    EmailMissing,
    PhoneMissing,
    QuotaExceed,
}

#[derive(Deserialize, Serialize, Debug)]
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
pub struct PhyAddrErrorDto {
    pub country: Option<PhyAddrNationErrorReason>,
    pub region: Option<PhyAddrRegionErrorReason>,
    pub city: Option<PhyAddrRegionErrorReason>,
    pub distinct: Option<PhyAddrDistinctErrorReason>,
    pub street_name: Option<PhyAddrDistinctErrorReason>,
    pub detail: Option<PhyAddrDistinctErrorReason>,
}
#[derive(Deserialize, Serialize, Debug)]
pub enum PhyAddrNationErrorReason {
    NotSupport,
}
#[derive(Deserialize, Serialize, Debug)]
pub enum PhyAddrRegionErrorReason {
    Empty,
    InvalidChar,
    NotExist,
    NotSupport,
}
#[derive(Deserialize, Serialize, Debug)]
pub enum PhyAddrDistinctErrorReason {
    Empty,
    InvalidChar,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct BillingErrorDto {
    pub contact: Option<ContactErrorDto>,
    pub address: Option<PhyAddrErrorDto>,
}
