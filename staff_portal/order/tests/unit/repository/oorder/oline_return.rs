use std::collections::HashMap;

use chrono::{Local, Duration};
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

fn ut_setup_ret_models() -> Vec<OrderReturnModel>
{
    let now = Local::now().fixed_offset();
    vec![
        OrderReturnModel {
            id_:OrderLineIdentity {store_id:18, product_type:ProductType::Item, product_id:465},
            qty: HashMap::from([
                (now - Duration::minutes(41), (1, OrderLinePriceModel {unit:15, total:15})),
                (now - Duration::seconds(1), (5, OrderLinePriceModel {unit:15, total:75})),
            ])
        }, 
        OrderReturnModel {
            id_:OrderLineIdentity {store_id:48, product_type:ProductType::Item, product_id:574},
            qty: HashMap::from([
                (now - Duration::minutes(10), (5, OrderLinePriceModel {unit:13, total:65})),
                (now - Duration::seconds(55), (2, OrderLinePriceModel {unit:13, total:26})),
                (now - Duration::seconds(3), (3, OrderLinePriceModel {unit:13, total:39})),
            ])
        }, 
        OrderReturnModel {
            id_:OrderLineIdentity {store_id:49, product_type:ProductType::Package, product_id:195},
            qty: HashMap::from([
                (now - Duration::seconds(4), (7, OrderLinePriceModel {unit:16, total:112})),
            ])
        }, 
    ]
}

#[tokio::test]
async fn in_mem_save_fetch_ok()
{
    let oid = "order0019286";
    let repo = in_mem_repo_ds_setup(20).await;
    let reqs = ut_setup_ret_models();
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
} // end of fn in_mem_save_fetch_ok

