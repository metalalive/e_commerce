use regex::Regex;
use std::result::Result as DefaultResult;

use crate::api::dto::{BillingDto, ContactDto, CountryCode, PhoneNumberDto, PhyAddrDto};
use crate::api::web::dto::{
    BillingErrorDto, ContactErrorDto, ContactErrorReason, ContactNonFieldErrorReason,
    PhoneNumNationErrorReason, PhoneNumberErrorDto, PhyAddrDistinctErrorReason, PhyAddrErrorDto,
    PhyAddrRegionErrorReason,
};
use crate::constant::REGEX_EMAIL_RFC5322;

pub struct ContactModel {
    pub first_name: String,
    pub last_name: String,
    pub emails: Vec<String>,
    pub phones: Vec<PhoneNumberDto>,
} // TODO, fraud check
pub struct PhyAddrModel {
    pub country: CountryCode,
    pub region: String,
    pub city: String,
    pub distinct: String,
    pub street_name: Option<String>,
    pub detail: String,
}
pub struct BillingModel {
    pub contact: ContactModel,
    pub address: Option<PhyAddrModel>,
}

impl From<ContactModel> for ContactDto {
    fn from(value: ContactModel) -> ContactDto {
        ContactDto {
            first_name: value.first_name,
            last_name: value.last_name,
            emails: value.emails,
            phones: value.phones,
        }
    }
}

impl TryFrom<ContactDto> for ContactModel {
    type Error = ContactErrorDto;
    fn try_from(value: ContactDto) -> DefaultResult<Self, Self::Error> {
        let fnam_rs = Self::check_alphabetic(value.first_name.as_str());
        let lnam_rs = Self::check_alphabetic(value.last_name.as_str());
        let (em_rs, ph_rs, nonfd_rs) = if value.emails.is_empty() {
            (None, None, Some(ContactNonFieldErrorReason::EmailMissing))
        } else if value.phones.is_empty() {
            (None, None, Some(ContactNonFieldErrorReason::PhoneMissing))
        } else {
            let rs1 = Self::check_emails(&value.emails);
            let rs2 = Self::check_phones(&value.phones);
            (rs1, rs2, None)
        };
        let error = Self::Error {
            first_name: fnam_rs,
            last_name: lnam_rs,
            emails: em_rs,
            phones: ph_rs,
            nonfield: nonfd_rs,
            quota_email: None,
            quota_phone: None,
        };
        if error.first_name.is_none()
            && error.emails.is_none()
            && error.phones.is_none()
            && error.last_name.is_none()
            && error.nonfield.is_none()
        {
            Ok(Self {
                first_name: value.first_name,
                last_name: value.last_name,
                emails: value.emails,
                phones: value.phones,
            })
        } else {
            Err(error)
        }
    } // end of fn try_from
}
impl ContactModel {
    fn check_alphabetic(value: &str) -> Option<ContactErrorReason> {
        if value.is_empty() {
            Some(ContactErrorReason::Empty)
        } else if !value.chars().all(char::is_alphabetic) {
            Some(ContactErrorReason::InvalidChar)
        } else {
            None
        }
    }
    fn check_emails(value: &[String]) -> Option<Vec<Option<ContactErrorReason>>> {
        let mut num_err: usize = 0;
        let re = Regex::new(REGEX_EMAIL_RFC5322).unwrap();
        let out = value
            .iter()
            .map(|d| {
                if d.is_empty() {
                    num_err += 1;
                    Some(ContactErrorReason::Empty)
                } else if let Some(_v) = re.find(d.as_str()) {
                    if _v.start() == 0 && d.len() == _v.end() {
                        None // given data should match the mail pattern exactly once
                    } else {
                        num_err += 1;
                        Some(ContactErrorReason::InvalidChar)
                    }
                } else {
                    num_err += 1;
                    Some(ContactErrorReason::InvalidChar)
                }
            })
            .collect();
        if num_err == 0 {
            None
        } else {
            Some(out)
        }
    }
    fn check_phones(value: &[PhoneNumberDto]) -> Option<Vec<Option<PhoneNumberErrorDto>>> {
        let mut num_err: usize = 0;
        let out = value
            .iter()
            .map(|d| {
                let nation_err = if d.nation > 0 && d.nation <= 999 {
                    None
                } else {
                    Some(PhoneNumNationErrorReason::InvalidCode)
                };
                let all_digits = d.number.chars().all(|c| c.is_ascii_digit());
                let number_err = if all_digits {
                    None
                } else {
                    Some(ContactErrorReason::InvalidChar)
                };
                if nation_err.is_some() || number_err.is_some() {
                    num_err += 1;
                    Some(PhoneNumberErrorDto {
                        nation: nation_err,
                        number: number_err,
                    })
                } else {
                    None
                }
            })
            .collect();
        if num_err == 0 {
            None
        } else {
            Some(out)
        }
    }
} // end of impl ContactModel

impl From<PhyAddrModel> for PhyAddrDto {
    fn from(value: PhyAddrModel) -> PhyAddrDto {
        PhyAddrDto {
            country: value.country,
            region: value.region,
            city: value.city,
            distinct: value.distinct,
            street_name: value.street_name,
            detail: value.detail,
        }
    }
}

impl TryFrom<PhyAddrDto> for PhyAddrModel {
    type Error = PhyAddrErrorDto;
    fn try_from(value: PhyAddrDto) -> DefaultResult<Self, Self::Error> {
        let region_rs = Self::check_region(value.region.as_str());
        let citi_rs = Self::check_region(value.city.as_str());
        let dist_rs = Self::contain_ctrl_char(value.distinct.as_str());
        let street_rs = if let Some(v) = value.street_name.as_ref() {
            Self::contain_ctrl_char(v.as_str())
        } else {
            None
        };
        let detail_rs = Self::contain_ctrl_char(value.detail.as_str());
        let error = Self::Error {
            country: None,
            region: region_rs,
            city: citi_rs,
            distinct: dist_rs,
            street_name: street_rs,
            detail: detail_rs,
        };
        if error.region.is_none()
            && error.city.is_none()
            && error.detail.is_none()
            && error.distinct.is_none()
            && error.street_name.is_none()
        {
            Ok(Self {
                country: value.country,
                region: value.region,
                city: value.city,
                distinct: value.distinct,
                street_name: value.street_name,
                detail: value.detail,
            })
        } else {
            Err(error)
        }
    }
} // end of impl PhyAddrModel
impl PhyAddrModel {
    pub fn try_from_opt(value: Option<PhyAddrDto>) -> DefaultResult<Option<Self>, PhyAddrErrorDto> {
        if let Some(d) = value {
            match PhyAddrModel::try_from(d) {
                Ok(m) => Ok(Some(m)),
                Err(e) => Err(e),
            }
        } else {
            Ok(None)
        } // client is allowed NOT to provide address with the order
    }
    fn check_region(value: &str) -> Option<PhyAddrRegionErrorReason> {
        if value.is_empty() {
            Some(PhyAddrRegionErrorReason::Empty)
        } else if !value
            .chars()
            .all(|c| c.is_alphabetic() || c.is_whitespace())
        {
            Some(PhyAddrRegionErrorReason::InvalidChar)
        } else {
            None
        }
    }
    fn contain_ctrl_char(value: &str) -> Option<PhyAddrDistinctErrorReason> {
        if value.is_empty() {
            Some(PhyAddrDistinctErrorReason::Empty)
        } else if value.chars().any(char::is_control) {
            Some(PhyAddrDistinctErrorReason::InvalidChar)
        } else {
            None
        }
    }
} // end of impl PhyAddrModel

impl From<BillingModel> for BillingDto {
    fn from(value: BillingModel) -> BillingDto {
        let (contact, pa) = (value.contact.into(), value.address);
        let address = pa.map(|v| v.into());
        BillingDto { contact, address }
    }
}
impl TryFrom<BillingDto> for BillingModel {
    type Error = BillingErrorDto;
    fn try_from(value: BillingDto) -> DefaultResult<Self, Self::Error> {
        let results = (
            ContactModel::try_from(value.contact),
            PhyAddrModel::try_from_opt(value.address),
        );
        if let (Ok(contact), Ok(maybe_addr)) = results {
            let obj = Self {
                contact,
                address: maybe_addr,
            };
            Ok(obj)
        } else {
            let mut obj = Self::Error {
                contact: None,
                address: None,
            };
            if let Err(e) = results.0 {
                obj.contact = Some(e);
            }
            if let Err(e) = results.1 {
                obj.address = Some(e);
            }
            Err(obj)
        }
    }
} // end of impl BillingModel
