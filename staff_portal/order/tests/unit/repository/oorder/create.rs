use chrono::DateTime;
use order::api::dto::{CountryCode, ShippingMethod};
use order::constant::ProductType;
use order::repository::{AbsOrderRepo, OrderInMemRepo};
use order::model::{
    OrderLineModel, OrderLineModelSet, BillingModel, ShippingModel, OrderLineIdentity
};
use super::{in_mem_repo_ds_setup, ut_setup_billing, ut_setup_shipping, ut_setup_orderlines};

const ORDERS_NUM_LINES:[usize;3] = [4,5,2];

async fn ut_verify_create_order(
    mock_oid:[String;3], mock_usr_ids:[u32;3], mock_create_time:[&str;3],
    o_repo :&OrderInMemRepo, mut orderlines: Vec<OrderLineModel>,
    mut billings: Vec<BillingModel>, mut shippings: Vec<ShippingModel> )
{
    assert!(orderlines.len() >= ORDERS_NUM_LINES.iter().sum::<usize>());
    for idx in 0..3 {
        let ol_set = OrderLineModelSet {order_id:mock_oid[idx].clone(), owner_id:mock_usr_ids[idx],
            create_time: DateTime::parse_from_rfc3339(mock_create_time[idx]).unwrap(),
            lines: orderlines.drain(0..ORDERS_NUM_LINES[idx]).collect()
        };
        let result = o_repo.create(ol_set, billings.remove(0), shippings.remove(0)).await;
        assert!(result.is_ok());
        if let Ok(dtos) = result {
            assert_eq!(dtos.len(), ORDERS_NUM_LINES[idx]);
        }; 
    }
}

async fn ut_verify_fetch_all_olines(mock_oid:[String;2], mock_seller_ids:[u32;2],
                               o_repo :&OrderInMemRepo)
{
    let result = o_repo.fetch_all_lines(mock_oid[0].clone()).await;
    assert!(result.is_ok());
    if let Ok(mut lines) = result {
        assert_eq!(lines.len(), ORDERS_NUM_LINES[0]);
        lines.sort_by(|a,b| { a.qty.reserved.cmp(&b.qty.reserved) });
        assert_eq!(lines[0].qty.reserved, 4);
        assert_eq!(lines[0].id_.store_id, mock_seller_ids[0]);
        assert_eq!(lines[0].id_.product_type, ProductType::Item);
        assert_eq!(lines[0].id_.product_id, 190);
        assert_eq!(lines[0].price.unit, 10);
        assert_eq!(lines[0].price.total, 39);
        assert_eq!(lines[2].qty.reserved, 6);
        assert_eq!(lines[2].id_.store_id, mock_seller_ids[1]);
        assert_eq!(lines[2].id_.product_type, ProductType::Package);
        assert_eq!(lines[2].id_.product_id, 190);
        assert_eq!(lines[2].price.unit, 40);
        assert_eq!(lines[2].price.total, 225);
    }
    let result = o_repo.fetch_all_lines(mock_oid[1].clone()).await;
    assert!(result.is_ok());
    if let Ok(mut lines) = result {
        assert_eq!(lines.len(), ORDERS_NUM_LINES[1]);
        lines.sort_by(|a,b| { a.qty.reserved.cmp(&b.qty.reserved) });
        assert_eq!(lines[0].qty.reserved, 16);
        assert_eq!(lines[0].id_.store_id, mock_seller_ids[1]);
        assert_eq!(lines[0].id_.product_type, ProductType::Package);
        assert_eq!(lines[0].id_.product_id, 194);
        assert_eq!(lines[0].price.unit, 15);
        assert_eq!(lines[0].price.total, 240);
    }
}

async fn ut_verify_fetch_specific_olines(mock_oid:[String;2], mock_seller_ids:[u32;2],
                               o_repo :&OrderInMemRepo)
{
    let mut pids = vec![
        OrderLineIdentity {store_id:mock_seller_ids[0], product_id: 190,
            product_type:ProductType::Package},
        OrderLineIdentity {store_id:mock_seller_ids[0], product_id: 199,
            product_type:ProductType::Item}, // should not exist in the order[0]
        OrderLineIdentity {store_id: mock_seller_ids[0], product_id: 190,
            product_type:ProductType::Item},
    ];
    let result = o_repo.fetch_lines_by_pid( mock_oid[0].as_str(),
                                            pids.clone() ).await ;
    assert!(result.is_ok());
    if let Ok(mut lines) = result {
        assert_eq!(lines.len(), 2);
        lines.sort_by(|a,b| { a.qty.reserved.cmp(&b.qty.reserved) });
        assert!(lines[0].id_== pids[2]);
        assert!(lines[1].id_== pids[0]);
        assert_eq!(lines[0].qty.reserved, 4);
        assert_eq!(lines[1].qty.reserved, 10);
    }
    pids.insert(0, OrderLineIdentity {store_id:mock_seller_ids[1],
        product_id: 198, product_type:ProductType::Item});
    let result = o_repo.fetch_lines_by_pid( mock_oid[1].as_str(),
                                            pids.clone() ).await ;
    assert!(result.is_ok());
    if let Ok(mut lines) = result {
        assert_eq!(lines.len(), 2);
        lines.sort_by(|a,b| { a.qty.reserved.cmp(&b.qty.reserved) });
        assert!(lines[0].id_== pids[0]);
        assert!(lines[1].id_== pids[2]);
        assert_eq!(lines[0].qty.reserved, 20);
        assert_eq!(lines[1].qty.reserved, 33);
    }
} // end of fn ut_verify_fetch_specific_olines

async fn ut_verify_fetch_owner_id(mock_oids:[String;3], mock_usr_ids:[u32;3],
                                  o_repo :&OrderInMemRepo)
{
    let mut uid_iter = mock_usr_ids.into_iter();
    for oid in mock_oids.into_iter() {
        let result = o_repo.owner_id(oid.as_str()).await;
        assert!(result.is_ok());
        if let Ok(fetched) = result {
            let expect = uid_iter.next().unwrap() ;
            assert_eq!(fetched, expect);
        }
    }
}
async fn ut_verify_fetch_create_time(mock_oids:[String;3],
                                     verify_create_time:[&str;3],
                                     o_repo:&OrderInMemRepo )
{
    let mut ctime_iter = verify_create_time.into_iter();
    for oid in mock_oids.into_iter() {
        let result = o_repo.created_time(oid.as_str()).await;
        assert!(result.is_ok());
        if let Ok(fetched) = result {
            let expect = ctime_iter.next().unwrap();
            let expect = DateTime::parse_from_rfc3339(expect).unwrap();
            assert_eq!(fetched, expect);
        }
    }
}
async fn ut_verify_fetch_ids_by_ctime (data:[(&str, &str, Vec<String>);3],
                                       o_repo:&OrderInMemRepo )
{
    for (start, end, mut expect_ids) in data {
        let t_start = DateTime::parse_from_rfc3339(start).unwrap();
        let t_end = DateTime::parse_from_rfc3339(end).unwrap();
        let result = o_repo.fetch_ids_by_created_time(t_start, t_end).await;
        assert!(result.is_ok());
        if let Ok(mut actual) = result {
            actual.sort();
            expect_ids.sort();
            assert_eq!(actual, expect_ids);
        }
    }
}

async fn ut_verify_fetch_billing(mock_oid:[String;3], o_repo :&OrderInMemRepo)
{
    let result = o_repo.fetch_billing(mock_oid[0].clone()).await;
    assert!(result.is_ok());
    if let Ok(fetched_bl) = result {
        assert_eq!(fetched_bl.contact.first_name.as_str(), "Ken");
        assert!(fetched_bl.contact.phones.iter().any(|m| m.number.as_str()=="002081264"));
        assert!(matches!(fetched_bl.address.as_ref().unwrap().country, CountryCode::TW));
    }
    let result = o_repo.fetch_billing(mock_oid[1].clone()).await;
    assert!(result.is_ok());
    if let Ok(fetched_bl) = result {
        assert_eq!(fetched_bl.contact.last_name.as_str(), "NormanKabboa");
        assert_eq!(fetched_bl.contact.emails.get(0).unwrap().as_str(), "banker@blueocean.ic");
        assert!(matches!(fetched_bl.address.as_ref().unwrap().country, CountryCode::US));
    }
    let result = o_repo.fetch_billing(mock_oid[2].clone()).await;
    assert!(result.is_ok());
    if let Ok(fetched_bl) = result {
        assert!(fetched_bl.address.is_none());
    }
}

async fn ut_verify_fetch_shipping(mock_oid:[String;3], mock_seller_ids:[u32;2],
                                  o_repo :&OrderInMemRepo )
{
    let result = o_repo.fetch_shipping(mock_oid[0].clone()).await;
    assert!(result.is_ok());
    if let Ok(fetched_sh) = result {
        assert_eq!(fetched_sh.contact.last_name.as_str(), "LaughOutLoud");
        let ph = fetched_sh.contact.phones.iter().find(|m| m.nation==36).unwrap();
        assert_eq!(ph.number.as_str(), "00101300802");
        assert_eq!(fetched_sh.address.as_ref().unwrap().city.as_str(), "Heirrotyyr");
        assert!(fetched_sh.option.is_empty());
    }
    let result = o_repo.fetch_shipping(mock_oid[1].clone()).await;
    assert!(result.is_ok());
    if let Ok(fetched_sh) = result {
        assert_eq!(fetched_sh.contact.first_name.as_str(), "Johan");
        assert!(fetched_sh.contact.phones.iter().any(|m| m.nation==43));
        assert!(fetched_sh.contact.phones.iter().any(|m| m.nation==44));
        assert_eq!(fetched_sh.address.as_ref().unwrap().distinct.as_str(), "demgur");
        let opt = fetched_sh.option.iter().find(|m| m.seller_id == mock_seller_ids[0]).unwrap();
        assert!(matches!(opt.method, ShippingMethod::FedEx));
        let opt = fetched_sh.option.iter().find(|m| m.seller_id == mock_seller_ids[1]).unwrap();
        assert!(matches!(opt.method, ShippingMethod::UPS));
    }
    let result = o_repo.fetch_shipping(mock_oid[2].clone()).await;
    assert!(result.is_ok());
    if let Ok(fetched_sh) = result {
        assert!(fetched_sh.address.is_none());
    }
}

#[tokio::test]
async fn in_mem_create_ok ()
{
    let o_repo = in_mem_repo_ds_setup(50).await;
    let (mock_usr_ids, mock_seller_ids) = ([124u32, 421, 124], [17u32,38]);
    let mock_oid = [
        OrderLineModel::generate_order_id(4),
        OrderLineModel::generate_order_id(2),
        OrderLineModel::generate_order_id(7),
    ];
    let mock_create_time = [
        "2022-11-07T04:00:01.519-01:00",
        "2022-11-08T12:09:33.8101+04:00",
        "2022-11-09T06:07:18.150-01:00",
    ];
    ut_verify_create_order(
        mock_oid.clone(), mock_usr_ids.clone(), mock_create_time.clone(),
        &o_repo, ut_setup_orderlines(&mock_seller_ids), ut_setup_billing(), 
        ut_setup_shipping(&mock_seller_ids)
    ).await ;
    ut_verify_fetch_all_olines([mock_oid[0].clone(), mock_oid[1].clone()],
                               mock_seller_ids.clone(),  &o_repo).await;
    ut_verify_fetch_specific_olines([mock_oid[0].clone(), mock_oid[1].clone()],
                              mock_seller_ids.clone(),  &o_repo).await;
    ut_verify_fetch_owner_id(mock_oid.clone(), mock_usr_ids, &o_repo).await ;
    ut_verify_fetch_create_time(mock_oid.clone(), mock_create_time, &o_repo).await ;
    ut_verify_fetch_ids_by_ctime([
            ("2022-11-07T03:59:06.503-01:00", "2022-11-07T04:00:08.990-01:00",
             vec![mock_oid[0].clone()] ),
            ("2022-11-07T03:58:17.001-01:00", "2022-11-09T22:13:18.409+04:00",
             mock_oid.to_vec() ),
            ("2022-11-07T03:58:16.503-01:00", "2022-11-07T03:59:58.990-01:00",
             vec![] ),
        ], &o_repo ).await;
    ut_verify_fetch_billing(mock_oid.clone(), &o_repo).await ;
    ut_verify_fetch_shipping(mock_oid, mock_seller_ids, &o_repo).await;
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

