pub mod order;

use crate::constant::ProductType;

#[derive(Eq, Debug)]
pub struct BaseProductIdentity {
    pub store_id: u32,
    pub product_type: ProductType,
    pub product_id: u64, // TODO, declare type alias
}
impl Clone for BaseProductIdentity {
    fn clone(&self) -> Self {
        Self {
            store_id: self.store_id,
            product_id: self.product_id,
            product_type: self.product_type.clone(),
        }
    }
}
impl PartialEq for BaseProductIdentity {
    fn eq(&self, other: &Self) -> bool {
        self.store_id == other.store_id
            && self.product_id == other.product_id
            && self.product_type == other.product_type
    }
}
