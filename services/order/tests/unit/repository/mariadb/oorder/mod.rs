use std::boxed::Box;
use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Duration, FixedOffset};
use rust_decimal::Decimal;

use ecommerce_common::api::dto::CurrencyDto;
use order::model::{
    CurrencyModel, OrderCurrencyModel, OrderLineAppliedPolicyModel, OrderLineIdentity,
    OrderLineModel, OrderLineModelSet, OrderLinePriceModel, OrderLineQuantityModel,
    ProdAttriPriceModel, ProductStockModel, StockLevelModelSet, StockQuantityModel,
    StoreStockModel,
};
use order::repository::AbsOrderStockRepo;

mod create;
mod line_return;
mod stock;
mod update;

async fn ut_setup_stock_product(
    stockrepo: Arc<Box<dyn AbsOrderStockRepo>>,
    mock_store_id: u32,
    mock_product_id: u64,
    init_qty: u32,
) {
    let product = ProductStockModel {
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

fn ut_default_order_currency(seller_ids: Vec<u32>) -> OrderCurrencyModel {
    let buyer = CurrencyModel {
        name: CurrencyDto::TWD,
        rate: Decimal::new(32041, 3),
    };
    let seller_c = buyer.clone();
    let kv_pairs = seller_ids
        .into_iter()
        .map(|seller_id| (seller_id, seller_c.clone()));
    OrderCurrencyModel {
        buyer,
        sellers: HashMap::from_iter(kv_pairs),
    }
}

#[rustfmt::skip]
type UtestOlineInitScalar<'a> = (
    (u32, u64), u32, u32, Option<(&'a str, i32)>, DateTime<FixedOffset>
);

fn ut_oline_init_setup(
    oid: &str,
    owner_id: u32,
    create_time: DateTime<FixedOffset>,
    currency: OrderCurrencyModel,
    lines_raw: Vec<UtestOlineInitScalar>,
) -> OrderLineModelSet {
    let olines_req = lines_raw
        .into_iter()
        .map(|d| {
            // attr-seq will be updated later in `OrderLineModelSet::try_from`
            let id_ = OrderLineIdentity::from((d.0 .0, d.0 .1, 0));
            let qty = OrderLineQuantityModel {
                reserved: d.1,
                paid: 0,
                paid_last_update: None,
            };
            let price = OrderLinePriceModel::from((d.2, d.2 * d.1));
            let policy = OrderLineAppliedPolicyModel {
                reserved_until: d.4 + Duration::minutes(2),
                warranty_until: d.4 + Duration::minutes(4),
            };
            let att_lastupdate = d.4 - Duration::minutes(35);
            let attr_price = d.3.map(|v| HashMap::from([(v.0.to_string(), v.1)]));
            let attr_chg = ProdAttriPriceModel::from((att_lastupdate, attr_price));
            OrderLineModel::from((id_, price, policy, qty, attr_chg))
        })
        .collect::<Vec<_>>();
    let args = (oid.to_string(), owner_id, create_time, currency, olines_req);
    OrderLineModelSet::try_from(args).unwrap()
} // end of fn fn ut_oline_init_setup
