use std::sync::Arc;

use chrono::{DateTime, Duration};
use order::constant::ProductType;
use order::model::OrderLineIdentity;
use order::repository::app_repo_order_return;

use crate::repository::in_mem::oorder::oline_return::{
    fetch_by_ctime_common, fetch_by_oid_ctime_common, ut_setup_fetch_by_ctime, ut_setup_ret_models,
    ut_setup_ret_models_ks2,
};

use super::super::dstore_ctx_setup;

#[cfg(feature = "mariadb")]
#[tokio::test]
async fn fetch_request_by_id_ok() {
    let ds = dstore_ctx_setup();
    let oret_repo = app_repo_order_return(ds).await.unwrap();
    let mock_oid = "bee715dead";
    let mock_time = DateTime::parse_from_rfc3339("2021-09-18T20:54:09+03:40").unwrap();
    let reqs = ut_setup_ret_models(mock_time);
    let result = oret_repo.create(mock_oid, reqs).await;
    assert!(result.is_ok());
    if let Ok(num_saved) = result {
        assert_eq!(num_saved, 6);
    }
    let pids = [
        (49, ProductType::Package, 195),
        (48, ProductType::Item, 574),
        (18u32, ProductType::Item, 465u64),
    ]
    .into_iter()
    .map(|(store_id, product_type, product_id)| OrderLineIdentity {
        store_id,
        product_type,
        product_id,
    })
    .collect::<Vec<_>>();
    let result = oret_repo.fetch_by_pid(mock_oid, pids.clone()).await;
    assert!(result.is_ok());
    if let Ok(fetched) = result {
        assert_eq!(fetched.len(), 3);
        fetched
            .iter()
            .map(|m| {
                let expect = match m.id_.store_id {
                    48 => (3, 10, 130),
                    49 => (1, 7, 112),
                    18 => (2, 6, 90),
                    _others => (0usize, 0u32, 0u32),
                };
                let total_returned = m.qty.values().map(|(q, _)| q.clone()).sum::<u32>();
                let total_refund = m.qty.values().map(|(_, refund)| refund.total).sum::<u32>();
                let actual = (m.qty.len(), total_returned, total_refund);
                assert_eq!(actual, expect);
            })
            .count();
    }
    let reqs = ut_setup_ret_models_ks2(mock_time + Duration::hours(1));
    let result = oret_repo.create(mock_oid, reqs).await;
    assert!(result.is_ok());
    if let Ok(num_saved) = result {
        assert_eq!(num_saved, 2);
    }
    let pids = [
        (49, ProductType::Package, 195),
        (18u32, ProductType::Item, 465u64),
    ]
    .into_iter()
    .map(|(store_id, product_type, product_id)| OrderLineIdentity {
        store_id,
        product_type,
        product_id,
    })
    .collect::<Vec<_>>();
    let result = oret_repo.fetch_by_pid(mock_oid, pids.clone()).await;
    assert!(result.is_ok());
    if let Ok(fetched) = result {
        assert_eq!(fetched.len(), 2);
        fetched
            .iter()
            .map(|m| {
                let expect = match m.id_.store_id {
                    49 => (2, 9, 144),
                    18 => (2, 6, 90),
                    _others => (0usize, 0u32, 0u32),
                };
                let total_returned = m.qty.values().map(|(q, _)| q.clone()).sum::<u32>();
                let total_refund = m.qty.values().map(|(_, refund)| refund.total).sum::<u32>();
                let actual = (m.qty.len(), total_returned, total_refund);
                assert_eq!(actual, expect);
            })
            .count();
    }
} // end of fn fetch_request_by_id_ok

#[cfg(feature = "mariadb")]
#[tokio::test]
async fn fetch_request_by_ctime_ok() {
    let ds = dstore_ctx_setup();
    let repo = {
        let r = app_repo_order_return(ds).await.unwrap();
        Arc::new(r)
    };
    let mock_oids = ["50ba", "beef", "0a991e"];
    let mock_time = DateTime::parse_from_rfc3339("2021-09-18T20:54:09+03:40").unwrap();
    {
        // begin setup
        let combo = ut_setup_fetch_by_ctime(mock_oids, mock_time.clone());
        for (oid, req_set) in combo {
            let result = repo.create(oid, req_set).await;
            assert!(result.is_ok());
        }
    } // end setup
    fetch_by_ctime_common(
        repo.clone(),
        mock_time + Duration::seconds(33),
        mock_time + Duration::minutes(6),
        vec![
            (
                format!("0a991e"),
                (
                    49,
                    ProductType::Package,
                    195,
                    mock_time + Duration::seconds(51),
                    3,
                    63,
                ),
            ),
            (
                format!("0a991e"),
                (
                    49,
                    ProductType::Package,
                    195,
                    mock_time + Duration::seconds(34),
                    1,
                    18,
                ),
            ),
            (
                format!("beef"),
                (
                    48,
                    ProductType::Item,
                    574,
                    mock_time + Duration::minutes(5),
                    1,
                    16,
                ),
            ),
        ],
    )
    .await;
    fetch_by_ctime_common(
        repo.clone(),
        mock_time - Duration::minutes(42),
        mock_time - Duration::minutes(9),
        vec![
            (
                format!("50ba"),
                (
                    18,
                    ProductType::Item,
                    465,
                    mock_time - Duration::minutes(41),
                    1,
                    15,
                ),
            ),
            (
                format!("beef"),
                (
                    48,
                    ProductType::Item,
                    574,
                    mock_time - Duration::minutes(10),
                    5,
                    65,
                ),
            ),
        ],
    )
    .await;
    fetch_by_oid_ctime_common(
        repo.clone(),
        "50ba",
        mock_time - Duration::seconds(70),
        mock_time - Duration::seconds(3),
        vec![
            (
                48,
                ProductType::Item,
                574,
                mock_time - Duration::seconds(3),
                3,
                39,
            ),
            (
                49,
                ProductType::Package,
                195,
                mock_time - Duration::seconds(4),
                7,
                112,
            ),
            (
                48,
                ProductType::Item,
                574,
                mock_time - Duration::seconds(55),
                2,
                26,
            ),
        ],
    )
    .await;
    fetch_by_oid_ctime_common(
        repo,
        "beef",
        mock_time - Duration::seconds(2),
        mock_time + Duration::minutes(6),
        vec![
            (
                48,
                ProductType::Item,
                574,
                mock_time + Duration::minutes(5),
                1,
                16,
            ),
            (
                18,
                ProductType::Item,
                465,
                mock_time - Duration::seconds(1),
                5,
                75,
            ),
        ],
    )
    .await;
} // end of fn fetch_request_by_ctime_ok
