use std::boxed::Box;
use std::result::Result;
use std::sync::Arc;

use crate::adapter::repository::AbstractChargeRepo;
use crate::adapter::rpc::AbstractRpcContext;

#[derive(Debug)]
pub struct SyncRefundReqUcError;

pub struct SyncRefundReqUseCase;

impl SyncRefundReqUseCase {
    pub async fn execute(
        _repo_c: Box<dyn AbstractChargeRepo>,
        _rpc_ctx: Arc<Box<dyn AbstractRpcContext>>,
    ) -> Result<(), SyncRefundReqUcError> {
        Ok(())
    }
} // end of impl SyncRefundReqUseCase
