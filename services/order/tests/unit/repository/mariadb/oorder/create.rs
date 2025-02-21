use std::boxed::Box;
use std::future::Future;
use std::pin::Pin;
use std::result::Result as DefaultResult;

use chrono::{DateTime, Duration, Local, SubsecRound};
use rust_decimal::Decimal;

use ecommerce_common::api::dto::{CountryCode, CurrencyDto};

use order::api::dto::ShippingMethod;
use order::error::AppError;
use order::model::{OrderCurrencyModel, OrderLineModelSet, StockLevelModelSet};
use order::repository::{app_repo_order, AbsOrderRepo, AppStockRepoReserveReturn};

use super::super::super::in_mem::oorder::stock::ut_reserve_init_setup;
use super::super::super::in_mem::oorder::{ut_setup_billing, ut_setup_shipping};
use super::super::dstore_ctx_setup;
use super::{ut_default_order_currency, ut_oline_init_setup, ut_setup_stock_product};

#[rustfmt::skip]
pub(super) async fn ut_verify_fetch_all_olines_ok(o_repo: &Box<dyn AbsOrderRepo>) {
    let oid = "800eff40".to_string();
    let result = o_repo.fetch_all_lines(oid).await;
    assert!(result.is_ok());
    let lines = result.unwrap();
    assert_eq!(lines.len(), 6);
    lines
        .into_iter()
        .map(|o| {
            let (id_, price, qty, policy) = (o.id(), o.price(), &o.qty, &o.policy);
            let combo = (id_.store_id(), id_.product_id(), id_.attrs_seq_num());
            let expect = match combo {
                (1013, 9004, 0) => (
                    2, 0, 3, 6, true,
                    DateTime::parse_from_rfc3339("3015-11-29T15:07:30-03:00").unwrap(),
                ),
                (1013, 9006, 0) => (
                    3, 0, 4, 12, true,
                    DateTime::parse_from_rfc3339("3014-11-29T15:46:43-03:00").unwrap(),
                ),
                (1014, 9008, 0) => (
                    29, 0, 20, 580, true,
                    DateTime::parse_from_rfc3339("3015-11-29T15:09:30-03:00").unwrap(),
                ),
                (1014, 9009, 0) => (
                    6, 0, 15, 90, true,
                    DateTime::parse_from_rfc3339("3014-11-29T15:48:43-03:00").unwrap(),
                ),
                (1014, 9009, 1) => (
                    2, 0, 11, 22, true,
                    DateTime::parse_from_rfc3339("3014-11-29T15:49:43-03:00").unwrap(),
                ),
                (1014, 9009, 2) => (
                    3, 0, 14, 42, true,
                    DateTime::parse_from_rfc3339("3014-11-29T15:50:43-03:00").unwrap(),
                ),
                _others => (
                    0, 0, 0, 0, true,
                    DateTime::parse_from_rfc3339("1989-05-30T23:57:59+00:00").unwrap(),
                ),
            };
            let actual = (
                qty.reserved, qty.paid, price.unit(), price.total(),
                qty.paid_last_update.is_none(), policy.warranty_until,
            );
            assert_eq!(actual, expect);
        })
        .count();
}

fn mock_reserve_usr_cb_0(
    ms: &mut StockLevelModelSet,
    req: &OrderLineModelSet,
) -> AppStockRepoReserveReturn {
    let errors = ms.try_reserve(req);
    assert!(errors.is_empty());
    Ok(())
}

#[tokio::test]
async fn save_contact_ok() {
    let mock_warranty = DateTime::parse_from_rfc3339("3015-11-29T15:02:32.056-03:00").unwrap();
    let ds = dstore_ctx_setup();
    let o_repo = app_repo_order(ds).await.unwrap();
    let (mock_oid, mock_store_id) = ("0e927003716a", 1021);
    ut_setup_stock_product(o_repo.stock(), mock_store_id, 9003, 15).await;
    ut_reserve_init_setup(
        o_repo.stock(),
        mock_reserve_usr_cb_0,
        mock_warranty,
        mock_store_id,
        9003,
        3,
        mock_oid,
    )
    .await;
    let mut billings = ut_setup_billing();
    let mut shippings = ut_setup_shipping(&[mock_store_id, 12]);
    let billing = billings.remove(1);
    let shipping = shippings.remove(2);
    let result = o_repo.save_contact(mock_oid, billing, shipping).await;
    assert!(result.is_ok());
    let result = o_repo.fetch_billing(mock_oid.to_string()).await;
    assert!(result.is_ok());
    if let Ok(bl) = result {
        assert_eq!(bl.contact.first_name.as_str(), "Jordan");
        assert_eq!(bl.contact.last_name.as_str(), "NormanKabboa");
        assert_eq!(bl.contact.emails[0].as_str(), "banker@blueocean.ic");
        assert_eq!(bl.contact.emails[1].as_str(), "bee@gituye.com");
        assert_eq!(bl.contact.phones[0].nation, 48u16);
        assert_eq!(bl.contact.phones[0].number.as_str(), "000208126");
        assert_eq!(bl.contact.phones[1].nation, 49u16);
        assert_eq!(bl.contact.phones[1].number.as_str(), "030001211");
        let addr = bl.address.unwrap();
        assert!(matches!(addr.country, CountryCode::US));
        assert_eq!(addr.city.as_str(), "i9ru24t");
        assert_eq!(addr.street_name.as_ref().unwrap().as_str(), "du iye j0y");
        assert_eq!(addr.detail.as_str(), "eu ur4 to4o");
    }
    let result = o_repo.fetch_shipping(mock_oid.to_string()).await;
    assert!(result.is_ok());
    if let Ok(sh) = result {
        assert_eq!(sh.contact.first_name.as_str(), "Biseakral");
        assert_eq!(sh.contact.last_name.as_str(), "Kazzhitsch");
        assert_eq!(sh.contact.emails[0].as_str(), "low@hunt.io");
        assert_eq!(sh.contact.emails[1].as_str(), "axl@rose.com");
        assert_eq!(sh.contact.emails[2].as_str(), "steven@chou01.hk");
        assert_eq!(sh.contact.phones[0].nation, 43u16);
        assert_eq!(sh.contact.phones[0].number.as_str(), "500020812");
        assert!(sh.address.is_none());
        assert_eq!(sh.option[0].seller_id, mock_store_id);
        assert!(matches!(sh.option[0].method, ShippingMethod::FedEx));
    }
} // end of fn save_contact_ok

#[tokio::test]
async fn save_contact_error() {
    let mock_warranty = DateTime::parse_from_rfc3339("3015-11-29T15:02:32.056-03:00").unwrap();
    let ds = dstore_ctx_setup();
    let o_repo = app_repo_order(ds).await.unwrap();
    let (mock_oid, mock_store_id) = ("a4190e9b4272", 1022);
    ut_setup_stock_product(o_repo.stock(), mock_store_id, 9003, 15).await;
    ut_reserve_init_setup(
        o_repo.stock(),
        mock_reserve_usr_cb_0,
        mock_warranty,
        mock_store_id,
        9003,
        4,
        mock_oid,
    )
    .await;
    let mut billings = ut_setup_billing();
    let mut shippings = ut_setup_shipping(&[mock_store_id, 12]);
    let billing = billings.remove(2);
    let shipping = shippings.remove(0);
    let result = o_repo.save_contact(mock_oid, billing, shipping).await;
    assert!(result.is_err()); // no shipping option provided
}

#[rustfmt::skip]
fn ut_fetch_lines_rsvtime_usr_cb(
    _repo: &dyn AbsOrderRepo,
    mset: OrderLineModelSet,
) -> Pin<Box<dyn Future<Output = DefaultResult<(), AppError>> + Send + '_>> {
    let fut = async move {
        let mock_seller = 1033u32;
        println!("[DEBUG] fetched oids : {}", mset.id().as_str());
        let expect = match mset.id().as_str() {
            "0e927d72" => (1usize, vec![(9013u64, 14u32)], CurrencyDto::INR, "79.0045"),
            "0e927d73" => (2, vec![(9012, 15), (9013, 16)], CurrencyDto::IDR, "16298.0110"),
            "0e927d74" => (1, vec![(9012, 17)], CurrencyDto::THB, "38.7160"),
            _others => (0, vec![], CurrencyDto::Unknown, "-0.00"),
        }; // remind `BINARY` column is right-padded with zero in MariaDB
        let mut actual_product_ids = mset
            .lines()
            .iter()
            .map(|line| (line.id().product_id(), line.qty.reserved))
            .collect::<Vec<_>>();
        actual_product_ids.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(mset.lines().len(), expect.0);
        assert_eq!(actual_product_ids, expect.1);
        let result = mset.currency().sellers.get(&mock_seller)
            .map(|v| {
                assert_eq!(v.name, expect.2);
                assert_eq!(v.rate.to_string().as_str(), expect.3);
            });
        assert!(result.is_some());
        Ok(())
    };
    Box::pin(fut)
} // end of fn ut_fetch_lines_rsvtime_usr_cb

#[rustfmt::skip]
#[tokio::test]
async fn fetch_lines_by_rsvtime_ok() {
    let ds = dstore_ctx_setup();
    let o_repo = app_repo_order(ds).await.unwrap();
    let create_time = Local::now().fixed_offset();
    let (mock_seller, mut mock_rsv_qty, mut rsv_time) = (1033, 11u32, create_time.clone());
    let mock_misc = [
        ("0e927d71", CurrencyDto::THB, Decimal::new(3845, 2)),
        ("0e927d72", CurrencyDto::INR, Decimal::new(790045, 4)),
        ("0e927d73", CurrencyDto::IDR, Decimal::new(162980110, 4)),
        ("0e927d74", CurrencyDto::THB, Decimal::new(38716, 3)),
        ("0e927d75", CurrencyDto::INR, Decimal::new(8011, 2)),
    ];
    ut_setup_stock_product(o_repo.stock(), mock_seller, 9012, 500).await;
    ut_setup_stock_product(o_repo.stock(), mock_seller, 9013, 500).await;
    for (mock_oid, mock_currency_label, mock_currency_rate) in mock_misc {
        rsv_time += Duration::days(2);
        let lines = vec![
            (mock_seller, 9012, mock_rsv_qty, 29, Some(("bolu",3)), rsv_time),
            (mock_seller, 9013, mock_rsv_qty + 1, 25, None, rsv_time + Duration::days(1)),
        ];
        let mut currency = ut_default_order_currency(vec![mock_seller]);
        currency.sellers.get_mut(&mock_seller)
            .map(|v| {
                v.name = mock_currency_label;
                v.rate = mock_currency_rate;
            });
        let ol_set = ut_oline_init_setup(mock_oid, 123, create_time, currency, lines);
        let result = o_repo
            .stock()
            .try_reserve(mock_reserve_usr_cb_0, &ol_set)
            .await;
        assert!(result.is_ok());
        mock_rsv_qty += 2;
    } // end of loop
    let (time_start, time_end) = (
        create_time + Duration::days(4) + Duration::hours(1),
        create_time + Duration::days(8) + Duration::hours(1),
    );
    let result = o_repo
        .fetch_lines_by_rsvtime(time_start, time_end, ut_fetch_lines_rsvtime_usr_cb)
        .await;
    assert!(result.is_ok());
} // end of fn fetch_lines_by_rsvtime_ok

#[rustfmt::skip]
#[tokio::test]
async fn fetch_toplvl_meta_ok() {
    let ds = dstore_ctx_setup();
    let o_repo = app_repo_order(ds).await.unwrap();
    let now = Local::now().fixed_offset();
    let mut create_time = now.clone();
    let (mock_seller, mut mock_usr_id, mock_rsv_qty) = (1033, 126u32, 1u32);
    let mock_oids = ["0e927d76", "0e927d00", "0e927d78", "0e927d79", "0e927d8a"];
    ut_setup_stock_product(o_repo.stock(), mock_seller, 9014, 50).await;
    for mock_oid in mock_oids {
        create_time += Duration::minutes(3);
        let rsv_time = now + Duration::days(1);
        let lines = vec![(
            mock_seller, 9014, mock_rsv_qty, 29, Some(("bolu",5)), rsv_time,
        )];
        let currency = ut_default_order_currency(vec![mock_seller]);
        let ol_set = ut_oline_init_setup(mock_oid, mock_usr_id, create_time, currency, lines);
        let result = o_repo.stock().try_reserve(mock_reserve_usr_cb_0, &ol_set).await;
        assert!(result.is_ok());
        mock_usr_id += 10;
    }
    let (time_start, time_end) = (now + Duration::minutes(4), now + Duration::minutes(10));
    let result = o_repo.fetch_ids_by_created_time(time_start, time_end).await;
    assert!(result.is_ok());
    if let Ok(oids) = result {
        println!("[DEBUG] oids : {:?}", oids);
        assert_eq!(oids.len(), 2);
        assert!(oids.contains(&"0e927d00".to_string()));
        assert!(oids.contains(&"0e927d78".to_string()));
    }
    let result = o_repo.owner_id("0e927d8a").await;
    assert_eq!(result.unwrap(), 166);
    let result = o_repo.owner_id("0e927d78").await;
    assert_eq!(result.unwrap(), 146);
    let result = o_repo.created_time("0e927d78").await;
    assert_eq!(
        result.unwrap().round_subsecs(0),
        now.round_subsecs(0) + Duration::minutes(9)
    );
    let result = o_repo.created_time("0e927d76").await;
    assert_eq!(
        result.unwrap().round_subsecs(0),
        now.round_subsecs(0) + Duration::minutes(3)
    );
    let result = o_repo.created_time("0e927d00").await;
    assert_eq!(
        result.unwrap().round_subsecs(0),
        now.round_subsecs(0) + Duration::minutes(6)
    );
} // end of fn fetch_toplvl_meta_ok

#[rustfmt::skip]
#[tokio::test]
async fn fetch_seller_currency_ok() {
    let ds = dstore_ctx_setup();
    let o_repo = app_repo_order(ds).await.unwrap();
    let create_time = Local::now().fixed_offset();
    let rsv_time = create_time + Duration::hours(1);
    let mock_sellers = [1036u32, 1037, 1038];
    let mock_buyer_id = 126u32;
    let mock_item_price = 100u32; // price in seller's currency
    let mock_oid = "0e927d8c";
    ut_setup_stock_product(o_repo.stock(), mock_sellers[0], 1405, 14).await;
    ut_setup_stock_product(o_repo.stock(), mock_sellers[1], 554, 10).await;
    ut_setup_stock_product(o_repo.stock(), mock_sellers[2], 1492, 15).await;
    let lines = vec![
        (mock_sellers[0], 1405, 8, mock_item_price, Some(("bolu",7)), rsv_time),
        (mock_sellers[1], 554, 9, mock_item_price, None, rsv_time),
        (mock_sellers[2], 1492, 8, mock_item_price, Some(("bolu",8)), rsv_time),
    ];
    let currency = {
        let mut c = ut_default_order_currency(mock_sellers.to_vec());
        c.buyer.rate = Decimal::new(2909, 2);
        c.sellers.get_mut(&mock_sellers[0]).map(|v| {
            v.name = CurrencyDto::IDR;
            v.rate = Decimal::new(102030405, 4);
        });
        c.sellers.get_mut(&mock_sellers[2]).map(|v| {
            v.name = CurrencyDto::USD;
            v.rate = Decimal::new(1, 0);
        });
        c
    };
    let ol_set = ut_oline_init_setup(mock_oid, mock_buyer_id, create_time, currency, lines);
    let result = o_repo
        .stock()
        .try_reserve(mock_reserve_usr_cb_0, &ol_set)
        .await;
    assert!(result.is_ok());
    let result = o_repo.currency_exrates(mock_oid).await;
    assert!(result.is_ok());
    if let Ok(v) = result {
        let OrderCurrencyModel { buyer, sellers } = v;
        assert_eq!(buyer.name, CurrencyDto::TWD);
        assert_eq!(buyer.rate.to_string().as_str(), "29.0900");
        sellers.into_iter().map(|(seller_id, actual)| {
            let expect = match seller_id {
                1036 => (CurrencyDto::IDR, "10203.0405"),
                1037 => (CurrencyDto::TWD, "32.0410"),
                1038 => (CurrencyDto::USD, "1.0000"),
                _others => (CurrencyDto::Unknown, "-0.00"),
            };
            assert_eq!(actual.name, expect.0);
            assert_eq!(actual.rate.to_string().as_str(), expect.1);
        }).count();
    }
} // end of fn fetch_seller_currency_ok
