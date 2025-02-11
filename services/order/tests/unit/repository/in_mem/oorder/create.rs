use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Duration, FixedOffset};
use rust_decimal::Decimal;

use ecommerce_common::api::dto::{CountryCode, CurrencyDto};
use ecommerce_common::model::order::BillingModel;

use order::api::dto::ShippingMethod;
use order::datastore::AppInMemoryDStore;
use order::model::{
    CurrencyModel, OrderCurrencyModel, OrderLineIdentity, OrderLineModel, OrderLineModelSet,
    ProductStockModel, ShippingModel, StockLevelModelSet, StockQuantityModel, StoreStockModel,
};
use order::repository::{
    AbsOrderRepo, AbsOrderStockRepo, AppStockRepoReserveReturn, OrderInMemRepo,
};

use super::{in_mem_repo_ds_setup, ut_setup_billing, ut_setup_orderlines, ut_setup_shipping};

const ORDERS_NUM_LINES: [usize; 3] = [4, 5, 2];

fn ut_setup_olines_gen_stock(
    olines: &Vec<OrderLineModel>,
    mock_expiry: DateTime<FixedOffset>,
) -> StockLevelModelSet {
    let mut stores = HashMap::new();
    assert!(olines.len() > 0);
    olines
        .iter()
        .map(|ol| {
            let store_id = ol.id_.store_id;
            if stores.get_mut(&store_id).is_none() {
                let value = StoreStockModel {
                    store_id,
                    products: vec![],
                };
                stores.insert(store_id, value);
            }
            let store = stores.get_mut(&store_id).unwrap();
            let value = ProductStockModel {
                id_: ol.id_.product_id,
                is_create: true,
                expiry: mock_expiry.into(),
                quantity: StockQuantityModel {
                    total: ol.qty.reserved,
                    cancelled: 0,
                    booked: 0,
                    rsv_detail: None,
                },
            };
            store.products.push(value);
        })
        .count();
    let stores = stores.into_values().collect::<Vec<_>>();
    StockLevelModelSet { stores }
} // end of fn ut_setup_olines_gen_stock

pub(super) async fn ut_setup_save_stock(
    stockrepo: Arc<Box<dyn AbsOrderStockRepo>>,
    mock_repo_time: DateTime<FixedOffset>,
    orderlines: &Vec<OrderLineModel>,
) {
    let mock_expiry = mock_repo_time + Duration::minutes(2);
    let slset = ut_setup_olines_gen_stock(orderlines, mock_expiry);
    let result = stockrepo.save(slset).await;
    assert!(result.is_ok());
}

pub(super) fn ut_setup_stock_rsv_cb(
    sl_set: &mut StockLevelModelSet,
    ol_set: &OrderLineModelSet,
) -> AppStockRepoReserveReturn {
    let errors = sl_set.try_reserve(ol_set);
    // for e1 in errors.iter() {
    //     println!("[utest][ERROR] stock reserve {:?}", e1);
    // }
    assert!(errors.is_empty());
    Ok(())
}

async fn ut_verify_create_order(
    mock_oid: [String; 3],
    mut mock_buyer_meta: Vec<(u32, CurrencyDto, Decimal)>,
    mock_create_time: [&str; 3],
    o_repo: &OrderInMemRepo,
    mut seller_currency_data: Vec<Vec<(u32, CurrencyDto, Decimal)>>,
    mut orderlines: Vec<OrderLineModel>,
    mut billings: Vec<BillingModel>,
    mut shippings: Vec<ShippingModel>,
) {
    assert!(orderlines.len() >= ORDERS_NUM_LINES.iter().sum::<usize>());
    let stockrepo = o_repo.stock();
    for idx in 0..3 {
        let lines = orderlines
            .drain(0..ORDERS_NUM_LINES[idx])
            .collect::<Vec<_>>();
        let seller_currencies = {
            let scdata = seller_currency_data.remove(0);
            let iter = scdata.into_iter().map(|v| {
                let m = CurrencyModel {
                    name: v.1,
                    rate: v.2,
                };
                (v.0, m)
            });
            HashMap::from_iter(iter)
        };
        let bcdata = mock_buyer_meta.remove(0);
        let currency = OrderCurrencyModel {
            buyer: CurrencyModel {
                name: bcdata.1,
                rate: bcdata.2,
            },
            sellers: seller_currencies,
        };
        let ol_set = OrderLineModelSet {
            order_id: mock_oid[idx].clone(),
            owner_id: bcdata.0,
            create_time: DateTime::parse_from_rfc3339(mock_create_time[idx]).unwrap(),
            lines,
            currency,
        };
        let result = stockrepo.try_reserve(ut_setup_stock_rsv_cb, &ol_set).await;
        assert!(result.is_ok());
        let result = o_repo
            .save_contact(
                ol_set.order_id.as_str(),
                billings.remove(0),
                shippings.remove(0),
            )
            .await;
        assert!(result.is_ok());
    } // end of loop
} // end of fn ut_verify_create_order

async fn ut_verify_fetch_all_olines(
    mock_oid: [String; 2],
    mock_seller_ids: [u32; 2],
    o_repo: &OrderInMemRepo,
) {
    let result = o_repo.fetch_all_lines(mock_oid[0].clone()).await;
    assert!(result.is_ok());
    if let Ok(mut lines) = result {
        assert_eq!(lines.len(), ORDERS_NUM_LINES[0]);
        lines.sort_by(|a, b| a.qty.reserved.cmp(&b.qty.reserved));
        assert_eq!(lines[0].qty.reserved, 4);
        assert_eq!(lines[0].id_.store_id, mock_seller_ids[0]);
        assert_eq!(lines[0].id_.product_id, 190);
        assert_eq!(lines[0].price.unit, 10);
        assert_eq!(lines[0].price.total, 39);
        assert_eq!(lines[2].qty.reserved, 6);
        assert_eq!(lines[2].id_.store_id, mock_seller_ids[1]);
        assert_eq!(lines[2].id_.product_id, 1190);
        assert_eq!(lines[2].price.unit, 40);
        assert_eq!(lines[2].price.total, 225);
    }
    let result = o_repo.fetch_all_lines(mock_oid[1].clone()).await;
    assert!(result.is_ok());
    if let Ok(mut lines) = result {
        assert_eq!(lines.len(), ORDERS_NUM_LINES[1]);
        lines.sort_by(|a, b| a.qty.reserved.cmp(&b.qty.reserved));
        assert_eq!(lines[0].qty.reserved, 16);
        assert_eq!(lines[0].id_.store_id, mock_seller_ids[1]);
        assert_eq!(lines[0].id_.product_id, 194);
        assert_eq!(lines[0].price.unit, 15);
        assert_eq!(lines[0].price.total, 240);
    }
}

async fn ut_verify_fetch_specific_olines(
    mock_oid: [String; 2],
    mock_seller_ids: [u32; 2],
    o_repo: &OrderInMemRepo,
) {
    let mut pids = vec![
        OrderLineIdentity {
            store_id: mock_seller_ids[0],
            product_id: 1190,
        },
        OrderLineIdentity {
            store_id: mock_seller_ids[0],
            product_id: 199,
        }, // should not exist in the order[0]
        OrderLineIdentity {
            store_id: mock_seller_ids[0],
            product_id: 190,
        },
    ];
    let result = o_repo
        .fetch_lines_by_pid(mock_oid[0].as_str(), pids.clone())
        .await;
    assert!(result.is_ok());
    if let Ok(mut lines) = result {
        assert_eq!(lines.len(), 2);
        lines.sort_by(|a, b| a.qty.reserved.cmp(&b.qty.reserved));
        assert!(lines[0].id_ == pids[2]);
        assert!(lines[1].id_ == pids[0]);
        assert_eq!(lines[0].qty.reserved, 4);
        assert_eq!(lines[1].qty.reserved, 10);
    }
    pids.insert(
        0,
        OrderLineIdentity {
            store_id: mock_seller_ids[1],
            product_id: 198,
        },
    );
    let result = o_repo
        .fetch_lines_by_pid(mock_oid[1].as_str(), pids.clone())
        .await;
    assert!(result.is_ok());
    if let Ok(mut lines) = result {
        assert_eq!(lines.len(), 2);
        lines.sort_by(|a, b| a.qty.reserved.cmp(&b.qty.reserved));
        assert!(lines[0].id_ == pids[0]);
        assert!(lines[1].id_ == pids[2]);
        assert_eq!(lines[0].qty.reserved, 20);
        assert_eq!(lines[1].qty.reserved, 33);
    }
} // end of fn ut_verify_fetch_specific_olines

async fn ut_verify_fetch_owner_id(
    mock_oids: [String; 3],
    mock_usr_ids: [u32; 3],
    o_repo: &OrderInMemRepo,
) {
    let mut uid_iter = mock_usr_ids.into_iter();
    for oid in mock_oids.into_iter() {
        let result = o_repo.owner_id(oid.as_str()).await;
        assert!(result.is_ok());
        if let Ok(fetched) = result {
            let expect = uid_iter.next().unwrap();
            assert_eq!(fetched, expect);
        }
    }
}
async fn ut_verify_fetch_create_time(
    mock_oids: [String; 3],
    verify_create_time: [&str; 3],
    o_repo: &OrderInMemRepo,
) {
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
async fn ut_verify_fetch_ids_by_ctime(
    data: [(&str, &str, Vec<String>); 3],
    o_repo: &OrderInMemRepo,
) {
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

async fn ut_verify_fetch_billing(mock_oid: [String; 3], o_repo: &OrderInMemRepo) {
    let result = o_repo.fetch_billing(mock_oid[0].clone()).await;
    assert!(result.is_ok());
    if let Ok(fetched_bl) = result {
        assert_eq!(fetched_bl.contact.first_name.as_str(), "Ken");
        assert!(fetched_bl
            .contact
            .phones
            .iter()
            .any(|m| m.number.as_str() == "002081264"));
        assert!(matches!(
            fetched_bl.address.as_ref().unwrap().country,
            CountryCode::TW
        ));
    }
    let result = o_repo.fetch_billing(mock_oid[1].clone()).await;
    assert!(result.is_ok());
    if let Ok(fetched_bl) = result {
        assert_eq!(fetched_bl.contact.last_name.as_str(), "NormanKabboa");
        assert_eq!(
            fetched_bl.contact.emails.get(0).unwrap().as_str(),
            "banker@blueocean.ic"
        );
        assert!(matches!(
            fetched_bl.address.as_ref().unwrap().country,
            CountryCode::US
        ));
    }
    let result = o_repo.fetch_billing(mock_oid[2].clone()).await;
    assert!(result.is_ok());
    if let Ok(fetched_bl) = result {
        assert!(fetched_bl.address.is_none());
    }
}

async fn ut_verify_fetch_shipping(
    mock_oid: [String; 3],
    mock_seller_ids: [u32; 2],
    o_repo: &OrderInMemRepo,
) {
    let result = o_repo.fetch_shipping(mock_oid[0].clone()).await;
    assert!(result.is_ok());
    if let Ok(fetched_sh) = result {
        assert_eq!(fetched_sh.contact.last_name.as_str(), "LaughOutLoud");
        let ph = fetched_sh
            .contact
            .phones
            .iter()
            .find(|m| m.nation == 36)
            .unwrap();
        assert_eq!(ph.number.as_str(), "00101300802");
        assert_eq!(
            fetched_sh.address.as_ref().unwrap().city.as_str(),
            "Heirrotyyr"
        );
        assert!(fetched_sh.option.is_empty());
    }
    let result = o_repo.fetch_shipping(mock_oid[1].clone()).await;
    assert!(result.is_ok());
    if let Ok(fetched_sh) = result {
        assert_eq!(fetched_sh.contact.first_name.as_str(), "Johan");
        assert!(fetched_sh.contact.phones.iter().any(|m| m.nation == 43));
        assert!(fetched_sh.contact.phones.iter().any(|m| m.nation == 44));
        assert_eq!(
            fetched_sh.address.as_ref().unwrap().distinct.as_str(),
            "demgur"
        );
        let opt = fetched_sh
            .option
            .iter()
            .find(|m| m.seller_id == mock_seller_ids[0])
            .unwrap();
        assert!(matches!(opt.method, ShippingMethod::FedEx));
        let opt = fetched_sh
            .option
            .iter()
            .find(|m| m.seller_id == mock_seller_ids[1])
            .unwrap();
        assert!(matches!(opt.method, ShippingMethod::UPS));
    }
    let result = o_repo.fetch_shipping(mock_oid[2].clone()).await;
    assert!(result.is_ok());
    if let Ok(fetched_sh) = result {
        assert!(fetched_sh.address.is_none());
    }
}

async fn ut_verify_seller_currencies(
    mock_oids: [String; 3],
    mut mock_buyer_data: Vec<(u32, CurrencyDto, Decimal)>,
    mut mock_seller_data: Vec<Vec<(u32, CurrencyDto, Decimal)>>,
    o_repo: &OrderInMemRepo,
) {
    for oid in mock_oids {
        let result = o_repo.currency_exrates(oid.as_str()).await;
        assert!(result.is_ok());
        let actual = result.unwrap();
        let expect_buyer = mock_buyer_data.remove(0);
        let expect_sellers = mock_seller_data.remove(0);
        assert_eq!(actual.buyer.name, expect_buyer.1);
        assert_eq!(actual.buyer.rate, expect_buyer.2);
        expect_sellers
            .into_iter()
            .map(|(seller_id, curr_label, ex_rate)| {
                let seller_exist = actual.sellers.get(&seller_id).unwrap();
                assert_eq!(seller_exist.name, curr_label);
                assert_eq!(seller_exist.rate, ex_rate);
            })
            .count();
    }
}

#[cfg_attr(rustfmt, rustfmt_skip)]
#[tokio::test]
async fn in_mem_create_ok() {
    let mock_buyer_ids = [124u32, 421, 124];
    let mock_buyer_meta = vec![
        (mock_buyer_ids[0], CurrencyDto::IDR, Decimal::new(163217709, 4)),
        (mock_buyer_ids[1], CurrencyDto::TWD, Decimal::new(3201, 2)),
        (mock_buyer_ids[2], CurrencyDto::INR, Decimal::new(843029, 4)),
    ];
    let mock_seller_ids = [17u32, 38];
    let mock_seller_currency = vec![
        vec![
            (mock_seller_ids[0], CurrencyDto::TWD, Decimal::new(320256, 4)),
            (mock_seller_ids[1], CurrencyDto::USD, Decimal::new(10, 1)),
        ],
        vec![
            (mock_seller_ids[0], CurrencyDto::THB, Decimal::new(3874, 2)),
            (mock_seller_ids[1], CurrencyDto::IDR, Decimal::new(1590293, 2)),
        ],
        vec![
            (mock_seller_ids[0], CurrencyDto::IDR, Decimal::new(158930029, 4)),
            (mock_seller_ids[1], CurrencyDto::TWD, Decimal::new(3196, 2)),
        ],
    ];
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
    let mock_repo_time = DateTime::parse_from_rfc3339("2022-11-11T12:30:51.150-02:00").unwrap();
    let o_repo = in_mem_repo_ds_setup::<AppInMemoryDStore>(50, Some(mock_repo_time)).await;
    let orderlines = ut_setup_orderlines(&mock_seller_ids);
    ut_setup_save_stock(o_repo.stock(), mock_repo_time, &orderlines).await;
    ut_verify_create_order(
        mock_oid.clone(),
        mock_buyer_meta.clone(),
        mock_create_time.clone(),
        &o_repo,
        mock_seller_currency.clone(),
        orderlines,
        ut_setup_billing(),
        ut_setup_shipping(&mock_seller_ids),
    )
    .await;
    ut_verify_fetch_all_olines(
        [mock_oid[0].clone(), mock_oid[1].clone()],
        mock_seller_ids.clone(),
        &o_repo,
    )
    .await;
    ut_verify_fetch_specific_olines(
        [mock_oid[0].clone(), mock_oid[1].clone()],
        mock_seller_ids.clone(),
        &o_repo,
    )
    .await;
    ut_verify_fetch_owner_id(mock_oid.clone(), mock_buyer_ids, &o_repo).await;
    ut_verify_fetch_create_time(mock_oid.clone(), mock_create_time, &o_repo).await;
    ut_verify_fetch_ids_by_ctime(
        [
            (
                "2022-11-07T03:59:06.503-01:00",
                "2022-11-07T04:00:08.990-01:00",
                vec![mock_oid[0].clone()],
            ),
            (
                "2022-11-07T03:58:17.001-01:00",
                "2022-11-09T22:13:18.409+04:00",
                mock_oid.to_vec(),
            ),
            (
                "2022-11-07T03:58:16.503-01:00",
                "2022-11-07T03:59:58.990-01:00",
                vec![],
            ),
        ],
        &o_repo,
    )
    .await;
    ut_verify_seller_currencies(
        mock_oid.clone(),
        mock_buyer_meta,
        mock_seller_currency,
        &o_repo
    ).await;
    ut_verify_fetch_billing(mock_oid.clone(), &o_repo).await;
    ut_verify_fetch_shipping(mock_oid, mock_seller_ids, &o_repo).await;
} // end of in_mem_create_ok

#[tokio::test]
async fn in_mem_fetch_all_lines_empty() {
    let o_repo = in_mem_repo_ds_setup::<AppInMemoryDStore>(30, None).await;
    let mock_oid = "12345".to_string();
    let result = o_repo.fetch_all_lines(mock_oid).await;
    assert!(result.is_ok());
    if let Ok(lines) = result {
        assert_eq!(lines.len(), 0);
    }
}
