use std::boxed::Box;
use std::future::Future;
use std::pin::Pin;
use std::result::Result as DefaultResult;

use chrono::{DateTime, Duration, Local, SubsecRound};
use order::api::dto::{CountryCode, ShippingMethod};
use order::constant::ProductType;
use order::error::AppError;
use order::model::{OrderLineModelSet, StockLevelModelSet};
use order::repository::{app_repo_order, AbsOrderRepo, AppStockRepoReserveReturn};

use super::super::super::in_mem::oorder::stock::ut_reserve_init_setup;
use super::super::super::in_mem::oorder::{ut_setup_billing, ut_setup_shipping};
use super::super::dstore_ctx_setup;
use super::{ut_oline_init_setup, ut_setup_stock_product};

pub(super) async fn ut_verify_fetch_all_olines_ok(o_repo: &Box<dyn AbsOrderRepo>) {
    let oid = "800eff40".to_string();
    let result = o_repo.fetch_all_lines(oid).await;
    assert!(result.is_ok());
    let lines = result.unwrap();
    assert_eq!(lines.len(), 4);
    lines
        .into_iter()
        .map(|o| {
            let (id_, price, qty, policy) = (o.id_, o.price, o.qty, o.policy);
            let combo = (id_.store_id, id_.product_type, id_.product_id);
            let expect = match combo {
                (1013, ProductType::Package, 9004) => (
                    2,
                    0,
                    3,
                    6,
                    true,
                    DateTime::parse_from_rfc3339("3015-11-29T15:07:30-03:00").unwrap(),
                ),
                (1013, ProductType::Item, 9006) => (
                    3,
                    0,
                    4,
                    12,
                    true,
                    DateTime::parse_from_rfc3339("3014-11-29T15:46:43-03:00").unwrap(),
                ),
                (1014, ProductType::Package, 9008) => (
                    29,
                    0,
                    20,
                    580,
                    true,
                    DateTime::parse_from_rfc3339("3015-11-29T15:09:30-03:00").unwrap(),
                ),
                (1014, ProductType::Item, 9009) => (
                    6,
                    0,
                    15,
                    90,
                    true,
                    DateTime::parse_from_rfc3339("3014-11-29T15:48:43-03:00").unwrap(),
                ),
                _others => (
                    0,
                    0,
                    0,
                    0,
                    true,
                    DateTime::parse_from_rfc3339("1989-05-30T23:57:59+00:00").unwrap(),
                ),
            };
            let actual = (
                qty.reserved,
                qty.paid,
                price.unit,
                price.total,
                qty.paid_last_update.is_none(),
                policy.warranty_until,
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

#[cfg(feature = "mariadb")]
#[tokio::test]
async fn save_contact_ok() {
    let mock_warranty = DateTime::parse_from_rfc3339("3015-11-29T15:02:32.056-03:00").unwrap();
    let ds = dstore_ctx_setup();
    let o_repo = app_repo_order(ds).await.unwrap();
    let (mock_oid, mock_store_id) = ("0e927003716a", 1021);
    ut_setup_stock_product(o_repo.stock(), mock_store_id, ProductType::Item, 9003, 15).await;
    ut_reserve_init_setup(
        o_repo.stock(),
        mock_reserve_usr_cb_0,
        mock_warranty,
        mock_store_id,
        ProductType::Item,
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

#[cfg(feature = "mariadb")]
#[tokio::test]
async fn save_contact_error() {
    let mock_warranty = DateTime::parse_from_rfc3339("3015-11-29T15:02:32.056-03:00").unwrap();
    let ds = dstore_ctx_setup();
    let o_repo = app_repo_order(ds).await.unwrap();
    let (mock_oid, mock_store_id) = ("a4190e9b4272", 1022);
    ut_setup_stock_product(o_repo.stock(), mock_store_id, ProductType::Item, 9003, 15).await;
    ut_reserve_init_setup(
        o_repo.stock(),
        mock_reserve_usr_cb_0,
        mock_warranty,
        mock_store_id,
        ProductType::Item,
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

fn ut_fetch_lines_rsvtime_usr_cb(
    _repo: &dyn AbsOrderRepo,
    mset: OrderLineModelSet,
) -> Pin<Box<dyn Future<Output = DefaultResult<(), AppError>> + Send + '_>> {
    let fut = async move {
        println!("[DEBUG] fetched oids : {}", mset.order_id.as_str());
        let expect = match mset.order_id.as_str() {
            "0e927d72" => (1usize, vec![(9013u64, 14u32)]),
            "0e927d73" => (2, vec![(9012, 15), (9013, 16)]),
            "0e927d74" => (1, vec![(9012, 17)]),
            _others => (0, vec![]),
        }; // remind `BINARY` column is right-padded with zero in MariaDB
        let mut actual_product_ids = mset
            .lines
            .iter()
            .map(|line| (line.id_.product_id, line.qty.reserved))
            .collect::<Vec<_>>();
        actual_product_ids.sort_by(|a, b| a.0.cmp(&b.0));
        let actual = (mset.lines.len(), actual_product_ids);
        assert_eq!(actual, expect);
        Ok(())
    };
    Box::pin(fut)
}

#[cfg(feature = "mariadb")]
#[tokio::test]
async fn fetch_lines_by_rsvtime_ok() {
    let ds = dstore_ctx_setup();
    let o_repo = app_repo_order(ds).await.unwrap();
    let create_time = Local::now().fixed_offset();
    let (mock_seller, mut mock_rsv_qty, mut rsv_time) = (1033, 11u32, create_time.clone());
    let mock_oids = ["0e927d71", "0e927d72", "0e927d73", "0e927d74", "0e927d75"];
    ut_setup_stock_product(o_repo.stock(), mock_seller, ProductType::Package, 9012, 500).await;
    ut_setup_stock_product(o_repo.stock(), mock_seller, ProductType::Item, 9013, 500).await;
    for mock_oid in mock_oids {
        rsv_time += Duration::days(2);
        let lines = vec![
            (
                mock_seller,
                ProductType::Package,
                9012,
                mock_rsv_qty,
                29,
                rsv_time,
            ),
            (
                mock_seller,
                ProductType::Item,
                9013,
                mock_rsv_qty + 1,
                25,
                rsv_time + Duration::days(1),
            ),
        ];
        let ol_set = ut_oline_init_setup(mock_oid, 123, create_time, lines);
        let result = o_repo
            .stock()
            .try_reserve(mock_reserve_usr_cb_0, &ol_set)
            .await;
        assert!(result.is_ok());
        mock_rsv_qty += 2;
    }
    let (time_start, time_end) = (
        create_time + Duration::days(4) + Duration::hours(1),
        create_time + Duration::days(8) + Duration::hours(1),
    );
    let result = o_repo
        .fetch_lines_by_rsvtime(time_start, time_end, ut_fetch_lines_rsvtime_usr_cb)
        .await;
    assert!(result.is_ok());
} // end of fn fetch_lines_by_rsvtime_ok

#[cfg(feature = "mariadb")]
#[tokio::test]
async fn fetch_toplvl_meta_ok() {
    let ds = dstore_ctx_setup();
    let o_repo = app_repo_order(ds).await.unwrap();
    let now = Local::now().fixed_offset();
    let mut create_time = now.clone();
    let (mock_seller, mut mock_usr_id, mock_rsv_qty) = (1033, 126u32, 1u32);
    let mock_oids = ["0e927d76", "0e927d00", "0e927d78", "0e927d79", "0e927d8a"];
    ut_setup_stock_product(o_repo.stock(), mock_seller, ProductType::Package, 9014, 50).await;
    for mock_oid in mock_oids {
        create_time += Duration::minutes(3);
        let rsv_time = now + Duration::days(1);
        let lines = vec![(
            mock_seller,
            ProductType::Package,
            9014,
            mock_rsv_qty,
            29,
            rsv_time,
        )];
        let ol_set = ut_oline_init_setup(mock_oid, mock_usr_id, create_time, lines);
        let result = o_repo
            .stock()
            .try_reserve(mock_reserve_usr_cb_0, &ol_set)
            .await;
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
