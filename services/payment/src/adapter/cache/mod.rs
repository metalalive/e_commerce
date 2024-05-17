mod in_mem;

use std::boxed::Box;
use std::marker::{Send, Sync};
use std::result::Result;

use async_trait::async_trait;

use in_mem::InMemOrderSyncLockCache;

pub struct OrderSyncLockError;

#[async_trait]
pub trait AbstractOrderSyncLockCache: Send + Sync {
    async fn acquire(&self, usr_id: u32, oid: &str) -> Result<bool, OrderSyncLockError>;

    async fn release(&self, usr_id: u32, oid: &str) -> Result<(), OrderSyncLockError>;
}

// TODO, pass config object that allows users to switch between
// different caches e.g. Redis in the future
pub fn app_cache_order_sync_lock() -> Box<dyn AbstractOrderSyncLockCache> {
    let cch = InMemOrderSyncLockCache::default();
    Box::new(cch)
}
