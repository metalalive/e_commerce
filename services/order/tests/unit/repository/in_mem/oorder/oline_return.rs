use std::collections::hash_map::RandomState;
use std::collections::{HashMap, HashSet};

use chrono::{Local, Duration, DateTime, FixedOffset};
use order::constant::ProductType;
use order::datastore::AppInMemoryDStore;
use order::model::{OrderReturnModel, OrderLineIdentity, OrderLinePriceModel};
use order::repository::{OrderReturnInMemRepo, AbsOrderReturnRepo};

use crate::repository::in_mem_ds_ctx_setup;


async fn in_mem_repo_ds_setup (nitems:u32) -> OrderReturnInMemRepo
{
    let ds = in_mem_ds_ctx_setup::<AppInMemoryDStore>(nitems);
    let result = OrderReturnInMemRepo::build(ds).await;
    assert_eq!(result.is_ok(), true);
    result.unwrap()
}

fn ut_setup_ret_models(t_base:DateTime<FixedOffset>) -> Vec<OrderReturnModel>
{
    vec![
        OrderReturnModel {
            id_:OrderLineIdentity {store_id:18, product_type:ProductType::Item, product_id:465},
            qty: HashMap::from([
                (t_base - Duration::minutes(41), (1, OrderLinePriceModel {unit:15, total:15})),
                (t_base - Duration::seconds(1), (5, OrderLinePriceModel {unit:15, total:75})),
            ])
        }, 
        OrderReturnModel {
            id_:OrderLineIdentity {store_id:48, product_type:ProductType::Item, product_id:574},
            qty: HashMap::from([
                (t_base - Duration::minutes(10), (5, OrderLinePriceModel {unit:13, total:65})),
                (t_base - Duration::seconds(55), (2, OrderLinePriceModel {unit:13, total:26})),
                (t_base - Duration::seconds(3), (3, OrderLinePriceModel {unit:13, total:39})),
            ])
        }, 
        OrderReturnModel {
            id_:OrderLineIdentity {store_id:49, product_type:ProductType::Package, product_id:195},
            qty: HashMap::from([
                (t_base - Duration::seconds(4), (7, OrderLinePriceModel {unit:16, total:112})),
            ])
        }, 
    ]
}

#[tokio::test]
async fn in_mem_fetch_by_pid_ok()
{
    let oid = "order0019286";
    let now = Local::now().fixed_offset();
    let repo = in_mem_repo_ds_setup(20).await;
    let reqs = ut_setup_ret_models(now);
    let pids = reqs.iter().filter_map(|m| {
        if m.id_.store_id == 18 { None }
        else { Some(m.id_.clone()) }
    }).collect::<Vec<OrderLineIdentity>>();
    let result = repo.save(oid, reqs).await;
    assert!(result.is_ok());
    if let Ok(num_saved) = result {
        assert_eq!(num_saved, 3);
    }
    let result = repo.fetch_by_pid(oid, pids).await;
    assert!(result.is_ok());
    if let Ok(fetched) = result {
        assert_eq!(fetched.len(), 2);
        fetched.iter().map(|m| {
            let expect = match m.id_.store_id {
                48 => (3, 10, 130),
                49 => (1, 7, 112),
                _others => (0usize, 0u32, 0u32),
            };
            assert_eq!(m.qty.len(), expect.0);
            let total_returned = m.qty.values().map(|(q, _)| q.clone()).sum::<u32>();
            let total_refund = m.qty.values().map(|(_, refund)| refund.total).sum::<u32>();
            assert_eq!(total_returned, expect.1);
            assert_eq!(total_refund  , expect.2);
        }).count();
    }
} // end of in_mem_fetch_by_pid_ok


#[tokio::test]
async fn in_mem_fetch_by_ctime_ok()
{
    let repo = in_mem_repo_ds_setup(40).await;
    let mock_time = DateTime::parse_from_rfc3339("2023-01-07T19:23:50+02:00").unwrap();
    { // begin setup
        let mut reqs = ut_setup_ret_models(mock_time);
        reqs[1].qty.remove(&(mock_time - Duration::minutes(10)));
        let result = repo.save("order0019286", reqs).await;
        assert!(result.is_ok());
    } {
        let mut reqs = ut_setup_ret_models(mock_time.clone());
        reqs[1].qty.insert(
            mock_time + Duration::minutes(5), (1, OrderLinePriceModel {unit:16, total:16}),
        );
        reqs[0].qty.remove(&(mock_time - Duration::minutes(41)));
        let result = repo.save("order00080273", reqs).await;
        assert!(result.is_ok());
    } {
        let mut reqs = ut_setup_ret_models(mock_time.clone());
        reqs.drain(0..2).count();
        let ret = reqs.last_mut().unwrap();
        let prev_entry = ret.qty.insert(
            mock_time + Duration::seconds(34), (1, OrderLinePriceModel {unit:18, total:18}),
        );
        assert!(prev_entry.is_none());
        ret.qty.insert(
            mock_time + Duration::seconds(51), (3, OrderLinePriceModel {unit:21, total:63}),
        );
        let prev_entry = ret.qty.insert(
            mock_time + Duration::seconds(388), (1, OrderLinePriceModel {unit:21, total:21}),
        );
        assert!(prev_entry.is_none());
        assert_eq!(ret.qty.len(), 4);
        let result = repo.save("order10029803", reqs).await;
        assert!(result.is_ok());
    } // end setup
    in_mem_fetch_by_ctime_common( &repo,
        mock_time + Duration::seconds(30),
        mock_time + Duration::minutes(6),
        vec![
            (format!("order10029803"), (49, ProductType::Package, 195, mock_time + Duration::seconds(51), 3, 63)),
            (format!("order10029803"), (49, ProductType::Package, 195, mock_time + Duration::seconds(34), 1, 18)),
            (format!("order00080273"), (48, ProductType::Item, 574, mock_time + Duration::minutes(5), 1, 16)),
        ]
    ).await;
    in_mem_fetch_by_ctime_common( &repo,
        mock_time - Duration::minutes(42),
        mock_time - Duration::minutes(9),
        vec![
            (format!("order0019286"),  (18, ProductType::Item, 465, mock_time - Duration::minutes(41), 1, 15)),
            (format!("order00080273"), (48, ProductType::Item, 574, mock_time - Duration::minutes(10), 5, 65)),
        ]
    ).await;
    in_mem_fetch_by_oid_ctime_common( &repo, "order00080273",
        mock_time - Duration::seconds(2),
        mock_time + Duration::minutes(6),
        vec![
            (48, ProductType::Item, 574, mock_time + Duration::minutes(5), 1, 16),
            (18, ProductType::Item, 465, mock_time - Duration::seconds(1), 5, 75),
        ]
    ).await;
    in_mem_fetch_by_oid_ctime_common( &repo, "order0019286",
        mock_time - Duration::seconds(70),
        mock_time - Duration::seconds(3),
        vec![
            (48, ProductType::Item,    574, mock_time - Duration::seconds(3), 3, 39),
            (49, ProductType::Package, 195, mock_time - Duration::seconds(4), 7, 112),
            (48, ProductType::Item,    574, mock_time - Duration::seconds(55), 2, 26),
        ]
    ).await;
} // end of fn in_mem_fetch_by_ctime_ok



type UTflatReturnExpectData = (u32, ProductType, u64, DateTime<FixedOffset>, u32, u32);

async fn in_mem_fetch_by_ctime_common(
    repo:&OrderReturnInMemRepo,
    t_start:DateTime<FixedOffset>,
    t_end:DateTime<FixedOffset>,
    expect_data: Vec<(String, UTflatReturnExpectData)>
)
{
    let result = repo.fetch_by_created_time(t_start, t_end).await;
    assert!(result.is_ok());
    if let Ok(fetched) = result {
        assert!(fetched.len() <= expect_data.len());
        let actual_iter = fetched.into_iter().flat_map(
            |(oid, m)| {
                assert!(m.qty.len() >= 1);
                let (seller_id, prod_typ, prod_id) = (m.id_.store_id, m.id_.product_type, m.id_.product_id);
                m.qty.into_iter().map(move |(create_time, (q, refund))| {
                    (oid.clone(), (seller_id, prod_typ.clone(), prod_id, create_time, q, refund.total))
                })
            }
        );
        let expect: HashSet<(String,UTflatReturnExpectData), RandomState> = HashSet::from_iter(expect_data.into_iter());
        let actual: HashSet<(String,UTflatReturnExpectData), RandomState> = HashSet::from_iter(actual_iter);
        assert_eq!(actual.difference(&expect).count(), 0);
        assert_eq!(expect.difference(&actual).count(), 0);
    }
}
async fn in_mem_fetch_by_oid_ctime_common(
    repo:&OrderReturnInMemRepo, oid:&str,
    t_start:DateTime<FixedOffset>,
    t_end:DateTime<FixedOffset>,
    expect_data: Vec<UTflatReturnExpectData>
)
{
    let result = repo.fetch_by_oid_ctime(oid, t_start, t_end).await;
    assert!(result.is_ok());
    if let Ok(fetched) = result {
        assert!(fetched.len() <= expect_data.len());
        let actual_iter = fetched.into_iter().flat_map(|m| {
            assert!(m.qty.len() >= 1);
            let (seller_id, prod_typ, prod_id) = (m.id_.store_id, m.id_.product_type, m.id_.product_id);
            m.qty.into_iter().map(move |(create_time, (q, refund))| {
                (seller_id, prod_typ.clone(), prod_id, create_time, q, refund.total)
            })
        });
        let expect: HashSet<UTflatReturnExpectData, RandomState> = HashSet::from_iter(expect_data.into_iter());
        let actual: HashSet<UTflatReturnExpectData, RandomState> = HashSet::from_iter(actual_iter);
        assert_eq!(actual.difference(&expect).count(), 0);
        assert_eq!(expect.difference(&actual).count(), 0);
    }
}
