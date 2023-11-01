use chrono::{DateTime, Local};
use order::api::dto::{PhoneNumberDto, CountryCode, ShippingMethod};
use order::constant::ProductType;
use order::datastore::AppInMemoryDStore;
use order::repository::{OrderInMemRepo, AbsOrderRepo};
use order::model::{
    BillingModel, ContactModel, PhyAddrModel, ShippingModel, ShippingOptionModel,
    OrderLineModel, OrderLinePriceModel, OrderLineAppliedPolicyModel
};

use crate::repository::in_mem_ds_ctx_setup;

async fn in_mem_repo_ds_setup (nitems:u32) -> OrderInMemRepo
{
    let ds = in_mem_ds_ctx_setup::<AppInMemoryDStore>(nitems);
    let result = OrderInMemRepo::build(ds, Local::now().into()).await;
    assert_eq!(result.is_ok(), true);
    result.unwrap()
}

fn ut_setup_billing () -> BillingModel
{
    let (first_name, last_name) = ("Ken".to_string(), "Kabaacis".to_string());
    let emails = vec!["sz16@crossroad.au".to_string(), "hay0123@pitch.io".to_string()];
    let phones = vec![
        PhoneNumberDto{nation:43, number:"002081264".to_string()},
        PhoneNumberDto{nation:43, number:"300801211".to_string()}
    ];
    let contact = ContactModel {first_name, last_name, emails, phones};
    let address = PhyAddrModel { country: CountryCode::TW, region: "PengHu".to_string(),
        city: "MaGong".to_string(), distinct: "xy923utn3".to_string(),
        street_name: Some("Sujaa st".to_string()), detail: "no limit 780".to_string() };
    BillingModel {contact, address:Some(address)}
}
fn ut_setup_shipping (mock_seller_ids:&[u32;2]) -> ShippingModel
{
    let (first_name, last_name) = ("Johan".to_string(), "Kazzhitsch".to_string());
    let emails = vec!["high@aman.at".to_string(), "low@hunt.io".to_string()];
    let phones = vec![
        PhoneNumberDto{nation:43, number:"500020812".to_string()},
        PhoneNumberDto{nation:43, number:"130080121".to_string()}
    ];
    let contact = ContactModel {first_name, last_name, emails, phones};
    let address = PhyAddrModel { country: CountryCode::TW, region: "NewTaipei".to_string(),
        city: "Yonghe".to_string(), distinct: "demgur".to_string(), street_name: None,
        detail: "postal building 1-53-70".to_string() };
    let option = vec![
        ShippingOptionModel{seller_id:mock_seller_ids[0], method:ShippingMethod::FedEx},
        ShippingOptionModel{seller_id:mock_seller_ids[1], method:ShippingMethod::UPS},
    ];
    ShippingModel {contact, option, address:Some(address)}
}

fn ut_setup_orderlines_1 (mock_seller_ids:&[u32;2])
    -> Vec<OrderLineModel>
{
    let reserved_until = DateTime::parse_from_rfc3339("2023-11-15T09:23:50+02:00").unwrap();
    let warranty_until = DateTime::parse_from_rfc3339("2023-12-24T13:39:41+02:00").unwrap();
    vec![
        OrderLineModel {seller_id:mock_seller_ids[0], product_type:ProductType::Item,
            product_id: 190, qty:4, price:OrderLinePriceModel { unit:10, total: 39 },
            policy: OrderLineAppliedPolicyModel { reserved_until, warranty_until }
        },
        OrderLineModel {seller_id:mock_seller_ids[1], product_type:ProductType::Item,
            product_id: 190, qty:5, price:OrderLinePriceModel { unit:12, total: 60 },
            policy: OrderLineAppliedPolicyModel { reserved_until, warranty_until }
        },
        OrderLineModel {seller_id:mock_seller_ids[0], product_type:ProductType::Package,
            product_id: 190, qty:10, price:OrderLinePriceModel { unit:9, total: 67 },
            policy: OrderLineAppliedPolicyModel { reserved_until, warranty_until }
        },
        OrderLineModel {seller_id:mock_seller_ids[1], product_type:ProductType::Package,
            product_id: 190, qty:6, price:OrderLinePriceModel { unit:40, total: 225 },
            policy: OrderLineAppliedPolicyModel { reserved_until, warranty_until }
        },
    ]
}

fn ut_setup_orderlines_2 (mock_seller_ids:&[u32;2])
    -> Vec<OrderLineModel>
{
    let reserved_until = DateTime::parse_from_rfc3339("2022-08-07T09:01:11+10:00").unwrap();
    let warranty_until = DateTime::parse_from_rfc3339("2023-04-25T13:38:41+02:00").unwrap();
    vec![
        OrderLineModel {seller_id:mock_seller_ids[1], product_type:ProductType::Item,
            product_id: 192, qty:18, price:OrderLinePriceModel { unit:10, total: 80 },
            policy: OrderLineAppliedPolicyModel { reserved_until, warranty_until }
        },
        OrderLineModel {seller_id:mock_seller_ids[0], product_type:ProductType::Item,
            product_id: 193, qty:32, price:OrderLinePriceModel { unit:12, total: 320 },
            policy: OrderLineAppliedPolicyModel { reserved_until, warranty_until }
        },
        OrderLineModel {seller_id:mock_seller_ids[1], product_type:ProductType::Package,
            product_id: 194, qty:16, price:OrderLinePriceModel { unit:15, total: 240 },
            policy: OrderLineAppliedPolicyModel { reserved_until, warranty_until }
        },
    ]
}

#[tokio::test]
async fn in_mem_create_ok ()
{
    let o_repo = in_mem_repo_ds_setup(30).await;
    let (mock_usr_id, mock_seller_ids) = (124, [17u32,38]);
    let mock_oid = [
        OrderLineModel::generate_order_id(4),
        OrderLineModel::generate_order_id(2),
    ];
    { // ---- subcase 1, create new order
        let billing = ut_setup_billing();
        let shipping = ut_setup_shipping (&mock_seller_ids);
        let olines = ut_setup_orderlines_1 (&mock_seller_ids);
        let result = o_repo.create(mock_oid[0].clone(), mock_usr_id,
                                   olines, billing, shipping).await;
        assert!(result.is_ok());
        if let Ok((oid, dtos)) = result {
            assert_eq!(oid, mock_oid[0]);
            assert_eq!(dtos.len(), 4);
        };
        let billing = ut_setup_billing();
        let shipping = ut_setup_shipping (&mock_seller_ids);
        let olines = ut_setup_orderlines_2 (&mock_seller_ids);
        let result = o_repo.create(mock_oid[1].clone(), mock_usr_id,
                                   olines, billing, shipping).await;
        assert!(result.is_ok());
        assert!(result.is_ok());
        if let Ok((oid, dtos)) = result {
            assert_eq!(oid, mock_oid[1]);
            assert_eq!(dtos.len(), 3);
        };
    }
    // ---- subcase 2, fetch created order-lines
    let result = o_repo.fetch_all_lines(mock_oid[0].clone()).await;
    assert!(result.is_ok());
    if let Ok(mut lines) = result {
        assert_eq!(lines.len(), 4);
        lines.sort_by(|a,b| { a.qty.cmp(&b.qty) });
        assert_eq!(lines[0].qty, 4);
        assert_eq!(lines[0].seller_id, mock_seller_ids[0]);
        assert_eq!(lines[0].product_type, ProductType::Item);
        assert_eq!(lines[0].product_id, 190);
        assert_eq!(lines[2].qty, 6);
        assert_eq!(lines[2].seller_id, mock_seller_ids[1]);
        assert_eq!(lines[2].product_type, ProductType::Package);
        assert_eq!(lines[2].product_id, 190);
    }
    let result = o_repo.fetch_all_lines(mock_oid[1].clone()).await;
    assert!(result.is_ok());
    if let Ok(mut lines) = result {
        assert_eq!(lines.len(), 3);
        lines.sort_by(|a,b| { a.qty.cmp(&b.qty) });
        assert_eq!(lines[0].qty, 16);
        assert_eq!(lines[0].seller_id, mock_seller_ids[1]);
        assert_eq!(lines[0].product_type, ProductType::Package);
        assert_eq!(lines[0].product_id, 194);
    }
} // end of in_mem_create_ok

#[tokio::test]
async fn in_mem_fetch_all_lines_empty ()
{
    let o_repo = in_mem_repo_ds_setup(30).await;
    let mock_oid = "12345".to_string();
    let result = o_repo.fetch_all_lines(mock_oid).await;
    assert!(result.is_ok());
    if let Ok(lines) = result {
        assert_eq!(lines.len(), 0);
    }
}

