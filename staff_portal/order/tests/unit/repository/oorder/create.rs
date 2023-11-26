use order::api::dto::{CountryCode, ShippingMethod};
use order::constant::ProductType;
use order::repository::{AbsOrderRepo, OrderInMemRepo};
use order::model::{OrderLineModel, OrderLineModelSet, BillingModel, ShippingModel};

use super::{in_mem_repo_ds_setup, ut_setup_billing, ut_setup_shipping, ut_setup_orderlines};


async fn ut_verify_create_order(mock_oid:[String;2], mock_usr_ids:[u32;2],
                               o_repo :&OrderInMemRepo,
                               mut orderlines: Vec<OrderLineModel>,
                               mut billings: Vec<BillingModel>,
                               mut shippings: Vec<ShippingModel> )
{
    let ol_set = OrderLineModelSet {order_id:mock_oid[0].clone(),
        lines: orderlines.drain(0..4).collect() };
    let result = o_repo.create(mock_usr_ids[0], ol_set, billings.remove(0),
                               shippings.remove(0)).await;
    assert!(result.is_ok());
    if let Ok(dtos) = result {
        assert_eq!(dtos.len(), 4);
    };
    let ol_set = OrderLineModelSet {order_id:mock_oid[1].clone(), lines: orderlines };
    let result = o_repo.create(mock_usr_ids[1], ol_set, billings.remove(0),
                               shippings.remove(0)).await;
    assert!(result.is_ok());
    if let Ok(dtos) = result {
        assert_eq!(dtos.len(), 3);
    };
}

async fn ut_verify_fetch_olines(mock_oid:[String;2], mock_seller_ids:[u32;2],
                               o_repo :&OrderInMemRepo)
{
    let result = o_repo.fetch_all_lines(mock_oid[0].clone()).await;
    assert!(result.is_ok());
    if let Ok(mut lines) = result {
        assert_eq!(lines.len(), 4);
        lines.sort_by(|a,b| { a.qty.reserved.cmp(&b.qty.reserved) });
        assert_eq!(lines[0].qty.reserved, 4);
        assert_eq!(lines[0].id_.store_id, mock_seller_ids[0]);
        assert_eq!(lines[0].id_.product_type, ProductType::Item);
        assert_eq!(lines[0].id_.product_id, 190);
        assert_eq!(lines[2].qty.reserved, 6);
        assert_eq!(lines[2].id_.store_id, mock_seller_ids[1]);
        assert_eq!(lines[2].id_.product_type, ProductType::Package);
        assert_eq!(lines[2].id_.product_id, 190);
    }
    let result = o_repo.fetch_all_lines(mock_oid[1].clone()).await;
    assert!(result.is_ok());
    if let Ok(mut lines) = result {
        assert_eq!(lines.len(), 3);
        lines.sort_by(|a,b| { a.qty.reserved.cmp(&b.qty.reserved) });
        assert_eq!(lines[0].qty.reserved, 16);
        assert_eq!(lines[0].id_.store_id, mock_seller_ids[1]);
        assert_eq!(lines[0].id_.product_type, ProductType::Package);
        assert_eq!(lines[0].id_.product_id, 194);
    }
}
async fn ut_verify_fetch_owner_id(mock_oids:[String;2], mock_usr_ids:[u32;2],
                                  o_repo :&OrderInMemRepo)
{
    let mut uid_iter = mock_usr_ids.into_iter();
    for oid in mock_oids.into_iter() {
        let result = o_repo.owner_id(oid.as_str()).await;
        assert!(result.is_ok());
        if let Ok(fetched_usr_id) = result {
            let expect = uid_iter.next().unwrap() ;
            assert_eq!(fetched_usr_id, expect);
        }
    }
}

async fn ut_verify_fetch_billing(mock_oid:[String;2], mock_usr_ids:[u32;2],
                               o_repo :&OrderInMemRepo)
{
    let result = o_repo.fetch_billing(mock_oid[0].clone()).await;
    assert!(result.is_ok());
    if let Ok((fetched_bl, fetched_usr_id)) = result {
        assert_eq!(fetched_usr_id, mock_usr_ids[0]);
        assert_eq!(fetched_bl.contact.first_name.as_str(), "Ken");
        assert!(fetched_bl.contact.phones.iter().any(|m| m.number.as_str()=="002081264"));
        assert!(matches!(fetched_bl.address.as_ref().unwrap().country, CountryCode::TW));
    }
    let result = o_repo.fetch_billing(mock_oid[1].clone()).await;
    assert!(result.is_ok());
    if let Ok((fetched_bl, fetched_usr_id)) = result {
        assert_eq!(fetched_usr_id, mock_usr_ids[1]);
        assert_eq!(fetched_bl.contact.last_name.as_str(), "NormanKabboa");
        assert_eq!(fetched_bl.contact.emails.get(0).unwrap().as_str(), "banker@blueocean.ic");
        assert!(matches!(fetched_bl.address.as_ref().unwrap().country, CountryCode::US));
    }
}

async fn ut_verify_fetch_shipping(mock_oid:[String;2], mock_seller_ids:[u32;2],
                                  mock_usr_ids:[u32;2], o_repo :&OrderInMemRepo )
{
    let result = o_repo.fetch_shipping(mock_oid[0].clone()).await;
    assert!(result.is_ok());
    if let Ok((fetched_sh, fetched_usr_id)) = result {
        assert_eq!(fetched_usr_id, mock_usr_ids[0]);
        assert_eq!(fetched_sh.contact.last_name.as_str(), "LaughOutLoud");
        let ph = fetched_sh.contact.phones.iter().find(|m| m.nation==36).unwrap();
        assert_eq!(ph.number.as_str(), "00101300802");
        assert_eq!(fetched_sh.address.as_ref().unwrap().city.as_str(), "Heirrotyyr");
        assert!(fetched_sh.option.is_empty());
    }
    let result = o_repo.fetch_shipping(mock_oid[1].clone()).await;
    assert!(result.is_ok());
    if let Ok((fetched_sh, fetched_usr_id)) = result {
        assert_eq!(fetched_usr_id, mock_usr_ids[1]);
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

#[tokio::test]
async fn in_mem_create_ok ()
{
    let o_repo = in_mem_repo_ds_setup(30).await;
    let (mock_usr_ids, mock_seller_ids) = ([124u32, 421], [17u32,38]);
    let mock_oid = [
        OrderLineModel::generate_order_id(4),
        OrderLineModel::generate_order_id(2),
    ];
    ut_verify_create_order(mock_oid.clone(), mock_usr_ids.clone(), &o_repo,
                          ut_setup_orderlines(&mock_seller_ids),
                          ut_setup_billing(), 
                          ut_setup_shipping(&mock_seller_ids)
                        ).await ;
    ut_verify_fetch_olines(mock_oid.clone(), mock_seller_ids.clone(),  &o_repo).await;
    ut_verify_fetch_owner_id(mock_oid.clone(), mock_usr_ids.clone(), &o_repo).await ;
    ut_verify_fetch_billing(mock_oid.clone(), mock_usr_ids.clone(), &o_repo).await ;
    ut_verify_fetch_shipping(mock_oid, mock_seller_ids, mock_usr_ids, &o_repo).await;
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

