use async_trait::async_trait;

use super::{AbstractOrderSyncLockCache, OrderSyncLockError};

pub(super) struct InMemOrderSyncLockCache {}

impl Default for InMemOrderSyncLockCache {
    fn default() -> Self {
        Self {}
    }
}

#[async_trait]
impl AbstractOrderSyncLockCache for InMemOrderSyncLockCache {
    async fn acquire(&self, _usr_id: u32, _oid: &str) -> Result<bool, OrderSyncLockError> {
        Ok(false)
    }

    async fn release(&self, _usr_id: u32, _oid: &str) -> Result<(), OrderSyncLockError> {
        Ok(())
    }
}
