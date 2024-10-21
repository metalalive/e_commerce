use std::boxed::Box;
use std::result::Result;
use std::sync::Arc;

use chrono::{DateTime, Duration, Local, Utc};
use ecommerce_common::api::rpc::dto::{OrderReplicaRefundDto, OrderReplicaRefundReqDto};

use crate::adapter::repository::{AbstractRefundRepo, AppRepoError};
use crate::adapter::rpc::{AbstractRpcContext, AppRpcClientRequest};
use crate::model::{OrderRefundModel, RefundModelError};

#[derive(Debug)]
pub enum SyncRefundReqUcError {
    Rpc(String),
    Datastore(AppRepoError),
    CorruptedRefundReq(String),
    ModelFailure(String, Vec<RefundModelError>),
}

pub struct SyncRefundReqUseCase;

impl SyncRefundReqUseCase {
    pub async fn execute(
        repo: Box<dyn AbstractRefundRepo>,
        rpc_ctx: Arc<Box<dyn AbstractRpcContext>>,
    ) -> Result<(usize, usize), SyncRefundReqUcError> {
        let t = Local::now().to_utc();
        let map = Self::rpc_sync(repo.as_ref(), rpc_ctx, t).await?;
        let refund_ms = Self::try_convert_model(map)?;
        let num_orders = refund_ms.len();
        let num_lines = refund_ms.iter().map(|r| r.num_lines()).sum();
        if !refund_ms.is_empty() {
            repo.save_request(refund_ms)
                .await
                .map_err(SyncRefundReqUcError::Datastore)?;
        }
        repo.update_sycned_time(t)
            .await
            .map_err(SyncRefundReqUcError::Datastore)?;
        Ok((num_orders, num_lines))
    }

    #[rustfmt::skip]
    async fn rpc_sync(
        repo: &dyn AbstractRefundRepo,
        rpc_ctx: Arc<Box<dyn AbstractRpcContext>>,
        time_end: DateTime<Utc>,
    ) -> Result<OrderReplicaRefundDto, SyncRefundReqUcError> {
        let time_start = repo.last_time_synced()
            .await.map_err(SyncRefundReqUcError::Datastore)?
            .unwrap_or(time_end - Duration::hours(5)) ;
        let client = rpc_ctx.acquire().await
            .map_err(|_e| SyncRefundReqUcError::Rpc("client-conn-error".to_string()))?;
        let sync_req = OrderReplicaRefundReqDto {
            start: time_start.to_rfc3339(),
            end: time_end.to_rfc3339(),
        };
        let msgbody = serde_json::to_vec(&sync_req).unwrap();
        let req = AppRpcClientRequest {
            usr_id: 0, time: time_end, message: msgbody,
            route: "rpc.order.order_returned_replica_refund".to_string(),
        };
        let mut evt = client.send_request(req).await
            .map_err(|_e| SyncRefundReqUcError::Rpc("send-req-fail".to_string()))?;
        let reply = evt.receive_response().await
            .map_err(|_e| SyncRefundReqUcError::Rpc("recv-resp-fail".to_string()))?;
        serde_json::from_slice::<OrderReplicaRefundDto>(&reply.message)
            .map_err(|e| SyncRefundReqUcError::CorruptedRefundReq(e.to_string()))
    } // end of fn rpc_sync

    fn try_convert_model(
        map: OrderReplicaRefundDto,
    ) -> Result<Vec<OrderRefundModel>, SyncRefundReqUcError> {
        let mut errs = Vec::new();
        let refund_ms = map
            .into_iter()
            .filter_map(|(oid, refund_d)| {
                OrderRefundModel::try_from((oid.clone(), refund_d))
                    .map_err(|es| errs.push((oid, es)))
                    .ok()
            })
            .collect::<Vec<_>>();
        if errs.is_empty() {
            Ok(refund_ms)
        } else {
            let e = errs.remove(0);
            Err(SyncRefundReqUcError::ModelFailure(e.0, e.1))
        }
    }
} // end of impl SyncRefundReqUseCase
