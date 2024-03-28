mod product_policy;
mod product_price;
mod stock_level;
mod order;
mod cart;

pub use product_policy::{ProductPolicyModel, ProductPolicyModelSet};
pub use product_price::{ProductPriceModel, ProductPriceModelSet};
pub use stock_level::{
    StockLevelModelSet, StoreStockModel, ProductStockModel, StockQuantityModel, ProductStockIdentity,
    ProductStockIdentity2, StockQtyRsvModel
};
pub use order::{
    BillingModel, ShippingModel, PhyAddrModel, ContactModel, OrderLinePriceModel,
    OrderLineAppliedPolicyModel, ShippingOptionModel, OrderLineModel, OrderLineQuantityModel,
    OrderLineModelSet, OrderLineIdentity, OrderReturnModel, OrderReturnQuantityModel
};
pub use cart::{CartLineModel, CartModel};

use crate::constant::ProductType;
use crate::api::web::dto::OrderLineReqDto;

#[derive(Eq)]
pub struct BaseProductIdentity {
    pub store_id: u32,
    pub product_type: ProductType,
    pub product_id: u64, // TODO, declare type alias
}
impl Clone for BaseProductIdentity {
    fn clone(&self) -> Self {
        Self { store_id: self.store_id, product_id: self.product_id,
               product_type: self.product_type.clone() }
    }
}
impl From<&OrderLineReqDto> for BaseProductIdentity {
    fn from(value: &OrderLineReqDto) -> Self {
        Self { store_id: value.seller_id, product_id: value.product_id,
               product_type: value.product_type.clone() }
    }
}
impl PartialEq for BaseProductIdentity {
    fn eq(&self, other: &Self) -> bool {
        self.store_id == other.store_id && self.product_id == other.product_id
            && self.product_type == other.product_type
    }
    fn ne(&self, other: &Self) -> bool {
        !self.eq(other)
    }
}
