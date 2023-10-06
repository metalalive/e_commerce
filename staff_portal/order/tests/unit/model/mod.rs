mod product_policy;
mod product_price;
mod stock_level;

use order::model::{ProductPolicyModel, ProductPriceModel, StockLevelModelSet};

pub(crate) fn ut_clone_productpolicy(src:&ProductPolicyModel) -> ProductPolicyModel
{
    ProductPolicyModel {
        product_id: src.product_id, auto_cancel_secs: src.auto_cancel_secs,
        warranty_hours: src.warranty_hours, is_create: src.is_create,
        product_type:src.product_type.clone() }
}

pub(crate) fn ut_clone_productprice(src:&ProductPriceModel) -> ProductPriceModel
{
    ProductPriceModel { price: src.price, product_id: src.product_id,
        product_type: src.product_type, is_create: src.is_create,
        start_after: src.start_after.clone(), end_before: src.end_before.clone() }
}

pub(crate) fn verify_stocklvl_model(actual:&StockLevelModelSet,
                         expect:&StockLevelModelSet,
                         idx:[usize;2] ,
                         use_eq_op:bool )
{
    let rand_chosen_store = &expect.stores[idx[0]];
    let result = actual.stores.iter().find(|m| {m.store_id == rand_chosen_store.store_id});
    assert!(result.is_some());
    if let Some(actual_st) = result {
        let rand_chosen_product = &rand_chosen_store.products[idx[1]];
        let result = actual_st.products.iter().find(|m| {
            m.type_ == rand_chosen_product.type_ &&  m.id_ == rand_chosen_product.id_
                && m.expiry_without_millis() == rand_chosen_product.expiry_without_millis()
        });
        assert!(result.is_some());
        if let Some(actual_prod) = result {
            if use_eq_op {
                assert_eq!(actual_prod, rand_chosen_product);
            } else {
                assert_ne!(actual_prod, rand_chosen_product);
            }
        }
    }
} // end of verify_stocklvl_model
