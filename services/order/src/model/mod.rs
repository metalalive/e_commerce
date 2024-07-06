mod cart;
mod currency;
mod order;
mod product_policy;
mod product_price;
mod stock_level;

pub use cart::{CartLineModel, CartModel};
pub use currency::{CurrencyModel, CurrencyModelSet};
pub use order::{
    OrderLineAppliedPolicyModel, OrderLineIdentity, OrderLineModel, OrderLineModelSet,
    OrderLinePriceModel, OrderLineQuantityModel, OrderReturnModel, OrderReturnQuantityModel,
    ShippingModel, ShippingOptionModel,
};
pub use product_policy::{ProductPolicyModel, ProductPolicyModelSet};
pub use product_price::{ProductPriceModel, ProductPriceModelSet};
pub use stock_level::{
    ProductStockIdentity, ProductStockIdentity2, ProductStockModel, StockLevelModelSet,
    StockQtyRsvModel, StockQuantityModel, StoreStockModel,
};

use ecommerce_common::model::BaseProductIdentity;

use crate::api::web::dto::OrderLineReqDto;

impl From<&OrderLineReqDto> for BaseProductIdentity {
    fn from(value: &OrderLineReqDto) -> Self {
        Self {
            store_id: value.seller_id,
            product_id: value.product_id,
            product_type: value.product_type.clone(),
        }
    }
}
