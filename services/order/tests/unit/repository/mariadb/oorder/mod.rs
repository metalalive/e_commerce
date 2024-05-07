use std::boxed::Box;
use std::sync::Arc;

use chrono::{DateTime, Duration, FixedOffset};
use ecommerce_common::constant::ProductType;
use order::model::{
    OrderLineAppliedPolicyModel, OrderLineIdentity, OrderLineModel, OrderLineModelSet,
    OrderLinePriceModel, OrderLineQuantityModel, ProductStockModel, StockLevelModelSet,
    StockQuantityModel, StoreStockModel,
};
use order::repository::AbsOrderStockRepo;

mod create;
mod line_return;
mod stock;
mod update;

async fn ut_setup_stock_product(
    stockrepo: Arc<Box<dyn AbsOrderStockRepo>>,
    mock_store_id: u32,
    mock_product_type: ProductType,
    mock_product_id: u64,
    init_qty: u32,
) {
    let product = ProductStockModel {
        type_: mock_product_type,
        id_: mock_product_id,
        expiry: DateTime::parse_from_rfc3339("2023-11-07T08:12:05.008+02:00")
            .unwrap()
            .into(),
        quantity: StockQuantityModel::new(init_qty, 0, 0, None),
        is_create: true,
    };
    let store = StoreStockModel {
        store_id: mock_store_id,
        products: vec![product],
    };
    let slset = StockLevelModelSet {
        stores: vec![store],
    };
    let result = stockrepo.save(slset).await;
    assert!(result.is_ok());
}

fn ut_oline_init_setup(
    oid: &str,
    owner_id: u32,
    create_time: DateTime<FixedOffset>,
    lines: Vec<(u32, ProductType, u64, u32, u32, DateTime<FixedOffset>)>,
) -> OrderLineModelSet {
    let order_req = lines
        .into_iter()
        .map(|d| OrderLineModel {
            id_: OrderLineIdentity {
                store_id: d.0,
                product_type: d.1,
                product_id: d.2,
            },
            qty: OrderLineQuantityModel {
                reserved: d.3,
                paid: 0,
                paid_last_update: None,
            },
            policy: OrderLineAppliedPolicyModel {
                reserved_until: d.5 + Duration::minutes(2),
                warranty_until: d.5 + Duration::minutes(4),
            },
            price: OrderLinePriceModel {
                unit: d.4,
                total: d.4 * d.3,
            },
        })
        .collect::<Vec<_>>();
    OrderLineModelSet {
        order_id: oid.to_string(),
        owner_id,
        create_time,
        lines: order_req,
    }
}
