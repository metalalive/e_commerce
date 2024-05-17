use ecommerce_common::api::dto::{ContactDto, CountryCode, PhoneNumberDto, PhyAddrDto};
use ecommerce_common::api::web::dto::{PhyAddrDistinctErrorReason, PhyAddrRegionErrorReason};
use ecommerce_common::model::order::PhyAddrModel;

use order::api::dto::{ShippingDto, ShippingMethod, ShippingOptionDto};
use order::model::ShippingModel;

#[test]
fn addr_convert_dto_ok() {
    let result = PhyAddrModel::try_from_opt(None);
    assert!(result.is_ok());
    if let Ok(v) = result {
        assert!(v.is_none());
    }
    let data = PhyAddrDto {
        country: CountryCode::TW,
        region: "Yilan".to_string(),
        city: "WaiAo".to_string(),
        distinct: "shore-seaweed bay".to_string(),
        street_name: Some("Bumpy Road".to_string()),
        detail: "321-5".to_string(),
    };
    let result = PhyAddrModel::try_from_opt(Some(data));
    assert!(result.is_ok());
    if let Ok(v) = result {
        assert!(v.is_some());
        if let Some(m) = v {
            assert_eq!(m.city.as_str(), "WaiAo");
        }
    }
}

#[test]
fn addr_convert_dto_error() {
    let data = PhyAddrDto {
        country: CountryCode::TW,
        region: "Yilan".to_string(),
        city: "Wai@Ao".to_string(),
        distinct: "shore-seaweed bay".to_string(),
        street_name: Some("Bumpy Road".to_string()),
        detail: "321-5-i\x00lla".to_string(),
    };
    let result = PhyAddrModel::try_from_opt(Some(data));
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(e.city.is_some());
        if let Some(m) = e.city.as_ref() {
            assert!(matches!(m, PhyAddrRegionErrorReason::InvalidChar));
        }
        assert!(e.detail.is_some());
        if let Some(m) = e.detail.as_ref() {
            assert!(matches!(m, PhyAddrDistinctErrorReason::InvalidChar));
        }
    }
} // end of addr_convert_dto_error

#[test]
fn shipping_convert_dto_without_addr() {
    let data = ShippingDto {
        contact: ContactDto {
            first_name: "Stu".to_string(),
            last_name: "Allabom".to_string(),
            emails: vec!["myacc@domain.org".to_string()],
            phones: vec![PhoneNumberDto {
                nation: 886,
                number: "0019283".to_string(),
            }],
        },
        address: None,
        option: vec![ShippingOptionDto {
            seller_id: 87,
            method: ShippingMethod::UPS,
        }],
    };
    let result = ShippingModel::try_from(data);
    assert!(result.is_ok());
    if let Ok(v) = result {
        assert_eq!(v.contact.first_name.as_str(), "Stu");
        assert_eq!(v.contact.phones[0].nation, 886);
        assert_eq!(v.option[0].seller_id, 87);
        assert!(matches!(v.option[0].method, ShippingMethod::UPS));
    }
}

#[test]
fn shipping_opt_convert_dto_error() {
    let data = ShippingDto {
        contact: ContactDto {
            first_name: "Stu".to_string(),
            last_name: "Allabo==m".to_string(),
            emails: vec!["myacc@domain.org".to_string()],
            phones: vec![PhoneNumberDto {
                nation: 886,
                number: "0019283".to_string(),
            }],
        },
        address: None,
        option: vec![
            ShippingOptionDto {
                seller_id: 190,
                method: ShippingMethod::FedEx,
            },
            ShippingOptionDto {
                seller_id: 0,
                method: ShippingMethod::UPS,
            },
        ],
    };
    let result = ShippingModel::try_from(data);
    assert!(result.is_err());
    if let Err(e) = result {
        if let Some(v) = e.contact.as_ref() {
            assert!(v.last_name.is_some());
        }
        if let Some(v) = e.option.as_ref() {
            assert_eq!(v.len(), 2);
            assert!(v[0].is_none());
            assert!(v[1].is_some());
            if let Some(e) = v[1].as_ref() {
                assert!(e.seller_id.is_some());
            }
        }
    }
} // end of shipping_opt_convert_dto_error
