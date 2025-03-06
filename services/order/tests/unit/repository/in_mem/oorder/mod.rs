use std::collections::HashMap;

use chrono::{DateTime, Duration, FixedOffset, Local};
use ecommerce_common::api::dto::{CountryCode, PhoneNumberDto};
use ecommerce_common::model::order::{BillingModel, ContactModel, PhyAddrModel};

use order::api::dto::ShippingMethod;
use order::datastore::AbstInMemoryDStore;
use order::model::{
    OrderLineAppliedPolicyModel, OrderLineIdentity, OrderLineModel, OrderLinePriceModel,
    OrderLineQuantityModel, ProdAttriPriceModel, ShippingModel, ShippingOptionModel,
};
use order::repository::OrderInMemRepo;

use super::in_mem_ds_ctx_setup;

mod create;
pub(crate) mod oline_return;
pub(crate) mod stock;
mod update;

async fn in_mem_repo_ds_setup<T: AbstInMemoryDStore + 'static>(
    nitems: u32,
    mut curr_time: Option<DateTime<FixedOffset>>,
) -> OrderInMemRepo {
    if curr_time.is_none() {
        curr_time = Some(Local::now().into());
    }
    let ds = in_mem_ds_ctx_setup::<T>(nitems);
    let mem = ds.in_mem.as_ref().unwrap();
    let result = OrderInMemRepo::new(mem.clone(), curr_time.unwrap()).await;
    assert_eq!(result.is_ok(), true);
    result.unwrap()
}

pub(crate) fn ut_setup_billing() -> Vec<BillingModel> {
    let item1 = {
        let (first_name, last_name) = ("Ken".to_string(), "Kabaacis".to_string());
        let emails = vec![
            "sz16@crossroad.au".to_string(),
            "hay0123@pitch.io".to_string(),
        ];
        let phones = vec![
            PhoneNumberDto {
                nation: 43,
                number: "002081264".to_string(),
            },
            PhoneNumberDto {
                nation: 43,
                number: "300801211".to_string(),
            },
        ];
        let contact = ContactModel {
            first_name,
            last_name,
            emails,
            phones,
        };
        let address = PhyAddrModel {
            country: CountryCode::TW,
            region: "PengHu".to_string(),
            city: "MaGong".to_string(),
            distinct: "xy923utn3".to_string(),
            street_name: Some("Sujaa st".to_string()),
            detail: "no limit 780".to_string(),
        };
        BillingModel {
            contact,
            address: Some(address),
        }
    };
    let item2 = {
        let (first_name, last_name) = ("Jordan".to_string(), "NormanKabboa".to_string());
        let emails = vec![
            "banker@blueocean.ic".to_string(),
            "bee@gituye.com".to_string(),
        ];
        let phones = vec![
            PhoneNumberDto {
                nation: 48,
                number: "000208126".to_string(),
            },
            PhoneNumberDto {
                nation: 49,
                number: "030001211".to_string(),
            },
        ];
        let contact = ContactModel {
            first_name,
            last_name,
            emails,
            phones,
        };
        let address = PhyAddrModel {
            country: CountryCode::US,
            region: "CA".to_string(),
            city: "i9ru24t".to_string(),
            distinct: "bliidlib".to_string(),
            street_name: Some("du iye j0y".to_string()),
            detail: "eu ur4 to4o".to_string(),
        };
        BillingModel {
            contact,
            address: Some(address),
        }
    };
    let item3 = {
        let (first_name, last_name) = ("Ben".to_string(), "MingkriokraDo".to_string());
        let emails = vec![];
        let phones = vec![PhoneNumberDto {
            nation: 886,
            number: "0900260812".to_string(),
        }];
        let contact = ContactModel {
            first_name,
            last_name,
            emails,
            phones,
        };
        BillingModel {
            contact,
            address: None,
        }
    };
    vec![item1, item2, item3]
}

pub(crate) fn ut_setup_shipping(mock_seller_ids: &[u32; 2]) -> Vec<ShippingModel> {
    let item1 = {
        let (first_name, last_name) = ("Pepek".to_string(), "LaughOutLoud".to_string());
        let emails = vec![
            "hotsauce@windows.cg".to_string(),
            "paste@shrimp.hebi".to_string(),
        ];
        let phones = vec![
            PhoneNumberDto {
                nation: 37,
                number: "950002081".to_string(),
            },
            PhoneNumberDto {
                nation: 36,
                number: "00101300802".to_string(),
            },
        ];
        let contact = ContactModel {
            first_name,
            last_name,
            emails,
            phones,
        };
        let address = PhyAddrModel {
            country: CountryCode::ID,
            region: "NusaXX".to_string(),
            city: "Heirrotyyr".to_string(),
            distinct: "d9emoss".to_string(),
            street_name: None,
            detail: "m8 warehouse 1-53-70".to_string(),
        };
        ShippingModel {
            contact,
            option: vec![],
            address: Some(address),
        }
    };
    let item2 = {
        let (first_name, last_name) = ("Johan".to_string(), "Kazzhitsch".to_string());
        let emails = vec!["high@aman.at".to_string(), "low@hunt.io".to_string()];
        let phones = vec![
            PhoneNumberDto {
                nation: 43,
                number: "500020812".to_string(),
            },
            PhoneNumberDto {
                nation: 44,
                number: "130080121".to_string(),
            },
        ];
        let contact = ContactModel {
            first_name,
            last_name,
            emails,
            phones,
        };
        let address = PhyAddrModel {
            country: CountryCode::TW,
            region: "NewTaipei".to_string(),
            city: "Yonghe".to_string(),
            distinct: "demgur".to_string(),
            street_name: None,
            detail: "postal building 1-53-70".to_string(),
        };
        let option = vec![
            ShippingOptionModel {
                seller_id: mock_seller_ids[0],
                method: ShippingMethod::FedEx,
            },
            ShippingOptionModel {
                seller_id: mock_seller_ids[1],
                method: ShippingMethod::UPS,
            },
        ];
        ShippingModel {
            contact,
            option,
            address: Some(address),
        }
    };
    let item3 = {
        let (first_name, last_name) = ("Biseakral".to_string(), "Kazzhitsch".to_string());
        let emails = ["low@hunt.io", "axl@rose.com", "steven@chou01.hk"]
            .into_iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        let phones = vec![PhoneNumberDto {
            nation: 43,
            number: "500020812".to_string(),
        }];
        let contact = ContactModel {
            first_name,
            last_name,
            emails,
            phones,
        };
        let option = vec![ShippingOptionModel {
            seller_id: mock_seller_ids[0],
            method: ShippingMethod::FedEx,
        }];
        ShippingModel {
            contact,
            option,
            address: None,
        }
    };
    vec![item1, item2, item3]
}

fn ut_setup_orderlines(seller_ids: &[u32; 2]) -> Vec<OrderLineModel> {
    let reserved_until = DateTime::parse_from_rfc3339("2023-11-15T09:23:50+02:00").unwrap();
    let warranty_until = DateTime::parse_from_rfc3339("2023-12-24T13:39:41+02:00").unwrap();
    let attr_lastupdate = reserved_until - Duration::hours(15);
    [
        (seller_ids[0], 190u64, 10, 39, 4, Some(-10)),
        (seller_ids[1], 190, 12, 60, 5, Some(11)),
        (seller_ids[0], 1190, 9, 67, 10, Some(-12)),
        (seller_ids[1], 1190, 40, 225, 6, Some(13)),
        (seller_ids[1], 192, 10, 80, 18, Some(-14)),
        (seller_ids[0], 193, 12, 320, 32, None),
        (seller_ids[1], 194, 15, 240, 16, Some(8)),
        (seller_ids[1], 198, 12, 240, 20, Some(-9)),
        (seller_ids[0], 199, 8, 264, 33, Some(10)),
        (seller_ids[0], 201, 5, 165, 33, Some(-11)),
        (seller_ids[0], 202, 23, 69, 3, None),
    ]
    .into_iter()
    .map(|d| {
        let id_ = OrderLineIdentity::from((d.0, d.1, 0));
        let price = OrderLinePriceModel::from((d.2, d.3));
        let qty = OrderLineQuantityModel {
            reserved: d.4,
            paid: 0,
            paid_last_update: None,
        };
        let policy = OrderLineAppliedPolicyModel {
            reserved_until,
            warranty_until,
        };
        let attrprice = d.5.map(|v| HashMap::from([("duglas-567".to_string(), v)]));
        let attrs_charge = ProdAttriPriceModel::from((attr_lastupdate, attrprice));
        OrderLineModel::from((id_, price, policy, qty, attrs_charge))
    })
    .collect()
} // end of ut_setup_orderlines
