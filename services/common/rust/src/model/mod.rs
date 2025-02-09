pub mod order;

// apply default partial-eq implementation
#[derive(Eq, Debug, Hash, PartialEq)]
pub struct BaseProductIdentity {
    pub store_id: u32,
    pub product_id: u64, // TODO, declare type alias
}
impl Clone for BaseProductIdentity {
    fn clone(&self) -> Self {
        Self {
            store_id: self.store_id,
            product_id: self.product_id,
        }
    }
}
