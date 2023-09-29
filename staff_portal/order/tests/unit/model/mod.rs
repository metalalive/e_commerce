mod product_policy;
mod product_price;
mod stock_level;

use order::model::{ProductPolicyModel, ProductPriceModel};

pub(crate) fn ut_clone_productpolicy(src:&ProductPolicyModel) -> ProductPolicyModel
{
    ProductPolicyModel {
        usr_id: src.usr_id, product_id: src.product_id, auto_cancel_secs: src.auto_cancel_secs,
        warranty_hours: src.warranty_hours, async_stock_chk: src.async_stock_chk,
        is_create: src.is_create }
}
pub(crate) fn ut_clone_productprice(src:&ProductPriceModel) -> ProductPriceModel
{
    ProductPriceModel { price: src.price, product_id: src.product_id,
        product_type: src.product_type, is_create: src.is_create,
        start_after: src.start_after.clone(), end_before: src.end_before.clone() }
}
