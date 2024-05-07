use std::boxed::Box;

use ecommerce_common::constant::ProductType;
use ecommerce_common::error::AppErrorCode;

use order::datastore::{AbstInMemoryDStore, AppInMemoryDStore};
use order::model::{ProductPolicyModel, ProductPolicyModelSet};
use order::repository::{AbstProductPolicyRepo, ProductPolicyInMemRepo};

use super::{in_mem_ds_ctx_setup, MockInMemDeadDataStore};
use crate::model::ut_clone_productpolicy;

const UTEST_INIT_DATA: [ProductPolicyModel; 7] = [
    ProductPolicyModel {
        product_type: ProductType::Item,
        product_id: 1556,
        min_num_rsv: 0,
        auto_cancel_secs: 309,
        warranty_hours: 7400,
        is_create: true,
        max_num_rsv: 2,
    },
    ProductPolicyModel {
        product_type: ProductType::Package,
        product_id: 9273,
        min_num_rsv: 3,
        auto_cancel_secs: 900,
        warranty_hours: 7209,
        is_create: true,
        max_num_rsv: 6,
    },
    ProductPolicyModel {
        product_type: ProductType::Item,
        product_id: 40051,
        min_num_rsv: 0,
        auto_cancel_secs: 707,
        warranty_hours: 1295,
        is_create: true,
        max_num_rsv: 0,
    },
    ProductPolicyModel {
        product_type: ProductType::Package,
        product_id: 1620,
        min_num_rsv: 3,
        auto_cancel_secs: 1645,
        warranty_hours: 1918,
        is_create: true,
        max_num_rsv: 20,
    },
    ProductPolicyModel {
        product_type: ProductType::Item,
        product_id: 14005,
        min_num_rsv: 0,
        auto_cancel_secs: 77,
        warranty_hours: 5129,
        is_create: true,
        max_num_rsv: 91,
    },
    ProductPolicyModel {
        product_type: ProductType::Item,
        product_id: 1622,
        min_num_rsv: 15,
        auto_cancel_secs: 6451,
        warranty_hours: 9181,
        is_create: true,
        max_num_rsv: 57,
    },
    ProductPolicyModel {
        product_type: ProductType::Item,
        product_id: 1622,
        min_num_rsv: 6,
        auto_cancel_secs: 1178,
        warranty_hours: 11086,
        is_create: false,
        max_num_rsv: 60,
    },
]; // end of UTEST_INIT_DATA

async fn in_mem_repo_ds_setup<T: AbstInMemoryDStore + 'static>(
    max_items: u32,
) -> Box<dyn AbstProductPolicyRepo> {
    let ds_ctx = in_mem_ds_ctx_setup::<T>(max_items);
    let in_mem_ds = ds_ctx.in_mem.as_ref().unwrap().clone();
    let result = ProductPolicyInMemRepo::new(in_mem_ds).await;
    assert_eq!(result.is_ok(), true);
    let repo = result.unwrap();
    Box::new(repo)
}

pub(crate) async fn save_fetch_ok_common(repo: Box<dyn AbstProductPolicyRepo>) {
    // ------ subcase, first bulk update
    let ppset = {
        let items = UTEST_INIT_DATA[0..3]
            .iter()
            .map(ut_clone_productpolicy)
            .collect();
        ProductPolicyModelSet { policies: items }
    };
    let result = repo.save(ppset).await;
    assert_eq!(result.is_ok(), true);
    let chosen_ids = vec![
        (ProductType::Item, 14005),
        (ProductType::Item, 1556),
        (ProductType::Item, 40051),
    ];
    let result = repo.fetch(chosen_ids).await;
    {
        assert_eq!(result.is_ok(), true);
        let modelset = result.unwrap();
        assert_eq!(modelset.policies.len(), 2);
        let exists = modelset.policies.iter().find(|m| m.product_id == 1556);
        assert_eq!(exists.unwrap(), &UTEST_INIT_DATA[0]);
        let exists = modelset.policies.iter().find(|m| m.product_id == 40051);
        assert_eq!(exists.unwrap(), &UTEST_INIT_DATA[2]);
        let exists = modelset.policies.iter().any(|m| m.product_id == 14005);
        assert_eq!(exists, false);
    }
    // ------ subcase, second bulk update
    let ppset = {
        let items = UTEST_INIT_DATA[3..6]
            .iter()
            .map(ut_clone_productpolicy)
            .collect();
        ProductPolicyModelSet { policies: items }
    };
    let result = repo.save(ppset).await;
    assert_eq!(result.is_ok(), true);
    let chosen_ids = vec![
        (ProductType::Item, 1622),
        (ProductType::Package, 1620),
        (ProductType::Package, 9273),
    ];
    let result = repo.fetch(chosen_ids).await;
    let modelset = result.unwrap();
    [
        (9273, &UTEST_INIT_DATA[1]),
        (1620, &UTEST_INIT_DATA[3]),
        (1622, &UTEST_INIT_DATA[5]),
    ]
    .into_iter()
    .map(|(given_prod_id, expect_model)| {
        let exists = modelset
            .policies
            .iter()
            .find(|m| m.product_id == given_prod_id);
        assert_eq!(exists.unwrap(), expect_model);
    })
    .count();
} // end of fn save_fetch_ok_common

#[tokio::test]
async fn save_fetch_ok_1() {
    let repo = in_mem_repo_ds_setup::<AppInMemoryDStore>(20).await;
    save_fetch_ok_common(repo).await;
} // end of fn save_fetch_ok_1

#[tokio::test]
async fn save_fetch_ok_2() {
    let repo = in_mem_repo_ds_setup::<AppInMemoryDStore>(20).await;
    let ppset = {
        let item = ut_clone_productpolicy(&UTEST_INIT_DATA[5]);
        ProductPolicyModelSet {
            policies: vec![item],
        }
    };
    let result = repo.save(ppset).await;
    assert_eq!(result.is_ok(), true);
    let ppset = {
        let item = ut_clone_productpolicy(&UTEST_INIT_DATA[6]);
        ProductPolicyModelSet {
            policies: vec![item],
        }
    };
    let result = repo.save(ppset).await;
    assert_eq!(result.is_ok(), true);

    let result = repo.fetch(vec![(ProductType::Item, 1622u64)]).await;
    {
        assert_eq!(result.is_ok(), true);
        let modelset = result.unwrap();
        let fetched = modelset
            .policies
            .iter()
            .find_map(|m| if m.product_id == 1622 { Some(m) } else { None })
            .unwrap();
        assert_eq!(fetched, &UTEST_INIT_DATA[6]);
        assert_ne!(fetched, &UTEST_INIT_DATA[5]);
    }
} // end of fn save_fetch_ok_2

#[tokio::test]
async fn save_empty_input() {
    let repo = in_mem_repo_ds_setup::<AppInMemoryDStore>(9).await;
    let ppset = ProductPolicyModelSet {
        policies: Vec::new(),
    };
    let result = repo.save(ppset).await;
    assert_eq!(result.is_err(), true);
    let error = result.err().unwrap();
    assert_eq!(error.code, AppErrorCode::EmptyInputData);
}

#[tokio::test]
async fn save_dstore_error() {
    let repo = in_mem_repo_ds_setup::<MockInMemDeadDataStore>(10).await;
    let ppset = {
        let item = ut_clone_productpolicy(&UTEST_INIT_DATA[0]);
        ProductPolicyModelSet {
            policies: vec![item],
        }
    };
    let result = repo.save(ppset).await;
    assert_eq!(result.is_err(), true);
    let error = result.err().unwrap();
    assert_eq!(error.code, AppErrorCode::DataTableNotExist);
    assert_eq!(error.detail, Some("utest".to_string()));
}

#[tokio::test]
async fn fetch_dstore_error() {
    let repo = in_mem_repo_ds_setup::<MockInMemDeadDataStore>(10).await;
    let result = repo.fetch(vec![(ProductType::Item, 1622u64)]).await;
    assert_eq!(result.is_err(), true);
    let error = result.err().unwrap();
    assert_eq!(error.code, AppErrorCode::AcquireLockFailure);
    assert_eq!(error.detail, Some("utest".to_string()));
}
