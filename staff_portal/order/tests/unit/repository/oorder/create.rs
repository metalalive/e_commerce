use order::api::dto::{CountryCode, ShippingMethod};
use order::constant::ProductType;
use order::repository::AbsOrderRepo;
use order::model::{OrderLineModel, OrderLineModelSet};

use super::{in_mem_repo_ds_setup, ut_setup_billing, ut_setup_shipping, ut_setup_orderlines};


#[tokio::test]
async fn in_mem_create_ok ()
{
    let o_repo = in_mem_repo_ds_setup(30).await;
    let (mock_usr_id, mock_seller_ids) = (124, [17u32,38]);
    let mock_oid = [
        OrderLineModel::generate_order_id(4),
        OrderLineModel::generate_order_id(2),
    ];
    let mut orderlines = ut_setup_orderlines(&mock_seller_ids);
    let mut billings = ut_setup_billing();
    let mut shippings = ut_setup_shipping(&mock_seller_ids);
    { // ---- subcase 1, create new order
        let ol_set = OrderLineModelSet {order_id:mock_oid[0].clone(),
            lines: orderlines.drain(0..4).collect() };
        let result = o_repo.create(mock_usr_id, ol_set, billings.remove(0),
                                   shippings.remove(0)).await;
        assert!(result.is_ok());
        if let Ok(dtos) = result {
            assert_eq!(dtos.len(), 4);
        };
        let ol_set = OrderLineModelSet {order_id:mock_oid[1].clone(), lines: orderlines };
        let result = o_repo.create(mock_usr_id, ol_set, billings.remove(0),
                                   shippings.remove(0)).await;
        assert!(result.is_ok());
        if let Ok(dtos) = result {
            assert_eq!(dtos.len(), 3);
        };
    }
    { // ---- subcase 2, fetch created order-lines
        let result = o_repo.fetch_all_lines(mock_oid[0].clone()).await;
        assert!(result.is_ok());
        if let Ok(mut lines) = result {
            assert_eq!(lines.len(), 4);
            lines.sort_by(|a,b| { a.qty.reserved.cmp(&b.qty.reserved) });
            assert_eq!(lines[0].qty.reserved, 4);
            assert_eq!(lines[0].seller_id, mock_seller_ids[0]);
            assert_eq!(lines[0].product_type, ProductType::Item);
            assert_eq!(lines[0].product_id, 190);
            assert_eq!(lines[2].qty.reserved, 6);
            assert_eq!(lines[2].seller_id, mock_seller_ids[1]);
            assert_eq!(lines[2].product_type, ProductType::Package);
            assert_eq!(lines[2].product_id, 190);
        }
        let result = o_repo.fetch_all_lines(mock_oid[1].clone()).await;
        assert!(result.is_ok());
        if let Ok(mut lines) = result {
            assert_eq!(lines.len(), 3);
            lines.sort_by(|a,b| { a.qty.reserved.cmp(&b.qty.reserved) });
            assert_eq!(lines[0].qty.reserved, 16);
            assert_eq!(lines[0].seller_id, mock_seller_ids[1]);
            assert_eq!(lines[0].product_type, ProductType::Package);
            assert_eq!(lines[0].product_id, 194);
        }
    }
    { // ---- subcase 3, fetch billings
        let result = o_repo.fetch_billing(mock_oid[0].clone()).await;
        assert!(result.is_ok());
        if let Ok((fetched_bl, fetched_usr_id)) = result {
            assert_eq!(fetched_usr_id, mock_usr_id);
            assert_eq!(fetched_bl.contact.first_name.as_str(), "Ken");
            assert!(fetched_bl.contact.phones.iter().any(|m| m.number.as_str()=="002081264"));
            assert!(matches!(fetched_bl.address.as_ref().unwrap().country, CountryCode::TW));
        }
        let result = o_repo.fetch_billing(mock_oid[1].clone()).await;
        assert!(result.is_ok());
        if let Ok((fetched_bl, fetched_usr_id)) = result {
            assert_eq!(fetched_usr_id, mock_usr_id);
            assert_eq!(fetched_bl.contact.last_name.as_str(), "NormanKabboa");
            assert_eq!(fetched_bl.contact.emails.get(0).unwrap().as_str(), "banker@blueocean.ic");
            assert!(matches!(fetched_bl.address.as_ref().unwrap().country, CountryCode::US));
        }
    }
    { // ---- subcase 4, fetch shippings
        let result = o_repo.fetch_shipping(mock_oid[0].clone()).await;
        assert!(result.is_ok());
        if let Ok((fetched_sh, fetched_usr_id)) = result {
            assert_eq!(fetched_usr_id, mock_usr_id);
            assert_eq!(fetched_sh.contact.last_name.as_str(), "LaughOutLoud");
            let ph = fetched_sh.contact.phones.iter().find(|m| m.nation==36).unwrap();
            assert_eq!(ph.number.as_str(), "00101300802");
            assert_eq!(fetched_sh.address.as_ref().unwrap().city.as_str(), "Heirrotyyr");
            assert!(fetched_sh.option.is_empty());
        }
        let result = o_repo.fetch_shipping(mock_oid[1].clone()).await;
        assert!(result.is_ok());
        if let Ok((fetched_sh, fetched_usr_id)) = result {
            assert_eq!(fetched_usr_id, mock_usr_id);
            assert_eq!(fetched_sh.contact.first_name.as_str(), "Johan");
            assert!(fetched_sh.contact.phones.iter().any(|m| m.nation==43));
            assert!(fetched_sh.contact.phones.iter().any(|m| m.nation==44));
            assert_eq!(fetched_sh.address.as_ref().unwrap().distinct.as_str(), "demgur");
            let opt = fetched_sh.option.iter().find(|m| m.seller_id == mock_seller_ids[0]).unwrap();
            assert!(matches!(opt.method, ShippingMethod::FedEx));
            let opt = fetched_sh.option.iter().find(|m| m.seller_id == mock_seller_ids[1]).unwrap();
            assert!(matches!(opt.method, ShippingMethod::UPS));
        }
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

