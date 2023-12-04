use std::collections::HashMap;

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
        {
            reqs[1].qty.remove(&(mock_time - Duration::minutes(10)));
        }
        let result = repo.save("order0019286", reqs).await;
        assert!(result.is_ok());
        let mut reqs = ut_setup_ret_models(mock_time.clone());
        {
            reqs[1].qty.insert(
                mock_time + Duration::minutes(5), (1, OrderLinePriceModel {unit:16, total:16}),
            );
            reqs[0].qty.remove(&(mock_time - Duration::minutes(41)));
        }
        let result = repo.save("order00080273", reqs).await;
        assert!(result.is_ok());
        let mut reqs = ut_setup_ret_models(mock_time.clone());
        {
            reqs.drain(0..2).count();
            reqs.last_mut().unwrap().qty.insert(
                mock_time + Duration::seconds(34), (1, OrderLinePriceModel {unit:18, total:18}),
            );
        }
        let result = repo.save("order10029803", reqs).await;
        assert!(result.is_ok());
    } // end setup
    in_mem_fetch_by_ctime_common(&repo, mock_time.clone(),
         mock_time + Duration::seconds(30),
         mock_time + Duration::minutes(6),
         in_mem_fetch_by_ctime_subcase_1 ).await;
    in_mem_fetch_by_ctime_common(&repo, mock_time.clone(),
         mock_time - Duration::minutes(42),
         mock_time - Duration::minutes(9),
         in_mem_fetch_by_ctime_subcase_2 ).await;
} // end of fn in_mem_fetch_by_ctime_ok

async fn in_mem_fetch_by_ctime_common(
    repo:&OrderReturnInMemRepo, mock_time:DateTime<FixedOffset>,
    t_start:DateTime<FixedOffset>, t_end:DateTime<FixedOffset>,
    verify_data: fn(DateTime<FixedOffset>, &str) -> (DateTime<FixedOffset>,u32,u32)
)
{
    let result = repo.fetch_by_created_time(t_start, t_end).await;
    assert!(result.is_ok());
    if let Ok(fetched) = result {
        assert_eq!(fetched.len(), 2);
        fetched.into_iter().map(|(oid, mut m)| {
            assert!(m.qty.len() >= 1);
            let (key, expect_qty, expect_refund) = verify_data(mock_time.clone(), oid.as_str());
            let (actual_qty, actual_refund) = m.qty.remove(&key).unwrap();
            assert_eq!(actual_qty, expect_qty);
            assert_eq!(actual_refund.total, expect_refund);
        }).count();
    }
}
fn in_mem_fetch_by_ctime_subcase_1(mock_time:DateTime<FixedOffset>, oid:&str)
 -> (DateTime<FixedOffset>,u32,u32)
{
    match oid {
        "order00080273" => (mock_time + Duration::minutes(5), 1, 16),
        "order10029803" => (mock_time + Duration::seconds(34), 1, 18),
        _others => (mock_time, 0, 0),
    }
}
fn in_mem_fetch_by_ctime_subcase_2(mock_time:DateTime<FixedOffset>, oid:&str)
 -> (DateTime<FixedOffset>,u32,u32)
{
    match oid {
        "order0019286" => (mock_time - Duration::minutes(41), 1, 15),
        "order00080273" => (mock_time - Duration::minutes(10), 5, 65),
        _others => (mock_time, 0, 0),
    }
}

