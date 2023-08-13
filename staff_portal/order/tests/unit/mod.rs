mod config;
mod logging;
mod usecase;
mod adapter;
mod repository;
mod model;

use order::model::ProductPolicyModel;

pub(crate) const EXAMPLE_REL_PATH : &'static str = "/tests/unit/examples/";

pub(crate) fn ut_clone_productpolicy_model(src:&ProductPolicyModel) -> ProductPolicyModel
{
    ProductPolicyModel {
        usr_id: src.usr_id, product_id: src.product_id, auto_cancel_secs: src.auto_cancel_secs,
        warranty_hours: src.warranty_hours, async_stock_chk: src.async_stock_chk,
        is_create: src.is_create }
}

