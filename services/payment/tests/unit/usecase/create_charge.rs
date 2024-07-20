use std::boxed::Box;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::{Duration, Local};
use ecommerce_common::api::dto::{
    BillingDto, ContactDto, CurrencyDto, CurrencySnapshotDto, OrderCurrencySnapshotDto,
    OrderLinePayDto, OrderSellerCurrencyDto, PayAmountDto, PhoneNumberDto,
};
use ecommerce_common::api::rpc::dto::OrderReplicaPaymentDto;
use ecommerce_common::constant::ProductType;

use ecommerce_common::error::AppErrorCode;
use ecommerce_common::model::BaseProductIdentity;
use payment::adapter::cache::OrderSyncLockError;
use payment::adapter::processor::{
    AppProcessorError, AppProcessorErrorReason, AppProcessorPayInResult,
};
use payment::adapter::repository::{AppRepoError, AppRepoErrorDetail, AppRepoErrorFnLabel};
use payment::adapter::rpc::{AppRpcCtxError, AppRpcErrorFnLabel, AppRpcErrorReason, AppRpcReply};
use payment::api::web::dto::{
    ChargeAmountOlineDto, ChargeReqDto, PaymentMethodErrorReason, PaymentMethodReqDto,
    PaymentMethodRespDto, StripeCheckoutSessionReqDto, StripeCheckoutSessionRespDto,
    StripeCheckoutUImodeDto,
};
use payment::model::{
    BuyerPayInState, OrderCurrencySnapshot, OrderLineModel, OrderLineModelSet, PayLineAmountModel,
};
use payment::usecase::{ChargeCreateUcError, ChargeCreateUseCase};
use rust_decimal::Decimal;

use super::{
    MockChargeRepo, MockOrderSyncLockCache, MockPaymentProcessor, MockRpcClient, MockRpcContext,
    MockRpcPublishEvent,
};

fn ut_saved_oline_set(mock_order_id: String, mock_usr_id: u32) -> OrderLineModelSet {
    let mock_seller_id = 379u32;
    let now = Local::now();
    let reserved_until = (now + Duration::minutes(2i64)).to_utc();
    let line = OrderLineModel {
        pid: BaseProductIdentity {
            store_id: mock_seller_id,
            product_type: ProductType::Item,
            product_id: 6741,
        },
        rsv_total: PayLineAmountModel {
            unit: Decimal::new(30001, 2),
            total: Decimal::new(180006, 2),
            qty: 6,
        },
        paid_total: PayLineAmountModel::default(),
        reserved_until,
    };
    let currency_snapshot = {
        let s = [
            (
                mock_usr_id,
                OrderCurrencySnapshot {
                    label: CurrencyDto::TWD,
                    rate: Decimal::new(321, 1),
                },
            ),
            (
                mock_seller_id,
                OrderCurrencySnapshot {
                    label: CurrencyDto::IDR,
                    rate: Decimal::new(16208, 0),
                },
            ),
        ];
        HashMap::from(s)
    };
    OrderLineModelSet {
        id: mock_order_id,
        buyer_id: mock_usr_id,
        currency_snapshot,
        num_charges: 0,
        create_time: now.to_utc(),
        lines: vec![line],
    }
} // end of fn ut_saved_oline_set

fn ut_charge_req_dto(mock_order_id: String) -> ChargeReqDto {
    let mock_finish_url = "https://mysite.io/products".to_string();
    ChargeReqDto {
        order_id: mock_order_id,
        method: PaymentMethodReqDto::Stripe(StripeCheckoutSessionReqDto {
            customer_id: Some("ut-stripe-mock-id".to_string()),
            success_url: Some(mock_finish_url.clone()),
            return_url: Some(mock_finish_url.clone()),
            cancel_url: None,
            ui_mode: StripeCheckoutUImodeDto::RedirectPage,
        }),
        lines: vec![ChargeAmountOlineDto {
            seller_id: 379,
            product_id: 6741,
            product_type: ProductType::Item,
            quantity: 6,
            amount: PayAmountDto {
                unit: "300.01".to_string(),
                total: "1800.06".to_string(),
            },
        }],
        currency: CurrencyDto::TWD,
    }
}

fn ut_orderpay_replica(mock_usr_id: u32, mock_order_id: String) -> Vec<u8> {
    let mock_seller_id = 379u32;
    let reserved_until = (Local::now() + Duration::minutes(2i64))
        .fixed_offset()
        .to_rfc3339();
    let replica = OrderReplicaPaymentDto {
        usr_id: mock_usr_id,
        oid: mock_order_id,
        lines: vec![OrderLinePayDto {
            seller_id: mock_seller_id,
            product_id: 6741,
            product_type: ProductType::Item,
            reserved_until,
            quantity: 6,
            amount: PayAmountDto {
                unit: "300.01".to_string(),
                total: "1800.06".to_string(),
            },
        }],
        currency: OrderCurrencySnapshotDto {
            snapshot: vec![
                CurrencySnapshotDto {
                    name: CurrencyDto::TWD,
                    rate: "31.8042".to_string(),
                },
                CurrencySnapshotDto {
                    name: CurrencyDto::IDR,
                    rate: "16250.91".to_string(),
                },
            ],
            sellers: vec![OrderSellerCurrencyDto {
                seller_id: mock_seller_id,
                currency: CurrencyDto::IDR,
            }],
            buyer: CurrencyDto::TWD,
        },
        billing: BillingDto {
            contact: ContactDto {
                first_name: "Zim".to_string(),
                last_name: "EverGreen".to_string(),
                emails: vec!["nobody@gohome.org".to_string()],
                phones: vec![PhoneNumberDto {
                    nation: 123,
                    number: "10740149".to_string(),
                }],
            },
            address: None,
        },
    };
    serde_json::to_vec(&replica).unwrap()
} // end of fn ut_orderpay_replica

fn ut_processor_pay_in_result() -> AppProcessorPayInResult {
    let detail = StripeCheckoutSessionRespDto {
        id: String::new(),
        redirect_url: Some(String::new()),
        client_session: Some(String::new()),
    };
    AppProcessorPayInResult {
        charge_id: Vec::new(),
        method: PaymentMethodRespDto::Stripe(detail),
        state: BuyerPayInState::Initialized,
        completed: false,
    }
}

#[actix_web::test]
async fn ok_with_existing_order_replica() {
    let mock_usr_id = 1234u32;
    let mock_order_id = "ut-origin-order-id".to_string();
    let mock_oline_set = ut_saved_oline_set(mock_order_id.clone(), mock_usr_id);
    let mock_repo = MockChargeRepo {
        _expect_unpaid_olines: Mutex::new(Some(Ok(Some(mock_oline_set)))),
        _create_order_result: Mutex::new(None),
        _create_charge_result: Mutex::new(Some(Ok(()))),
    };
    let mock_sync_cache = MockOrderSyncLockCache {
        _acquire_result: Mutex::new(None),
        _release_result: Mutex::new(None),
    };
    let mock_rpc_ctx = MockRpcContext {
        _acquire_result: Mutex::new(None),
    };
    let mock_payin_result = ut_processor_pay_in_result();
    let mock_processor = MockPaymentProcessor {
        _payin_start_result: Mutex::new(Some(Ok(mock_payin_result))),
    };
    let uc = ChargeCreateUseCase {
        processors: Arc::new(Box::new(mock_processor)),
        rpc_ctx: Arc::new(Box::new(mock_rpc_ctx)),
        ordersync_lockset: Arc::new(Box::new(mock_sync_cache)),
        repo: Box::new(mock_repo),
    };
    let mock_req = ut_charge_req_dto(mock_order_id.clone());
    let result = uc.execute(mock_usr_id, mock_req).await;
    assert!(result.is_ok());
    if let Ok(_resp) = result {
        // TODO, examine response detail
    }
} // end of ok_with_existing_order_replica

#[actix_web::test]
async fn ok_with_rpc_replica_order() {
    let mock_usr_id = 1234u32;
    let mock_order_id = "ut-origin-order-id".to_string();
    let mock_repo = MockChargeRepo {
        _expect_unpaid_olines: Mutex::new(Some(Ok(None))),
        _create_order_result: Mutex::new(Some(Ok(()))),
        _create_charge_result: Mutex::new(Some(Ok(()))),
    };
    let mock_sync_cache = MockOrderSyncLockCache {
        _acquire_result: Mutex::new(Some(Ok(true))),
        _release_result: Mutex::new(Some(Ok(()))),
    };
    let mock_reply = AppRpcReply {
        message: ut_orderpay_replica(mock_usr_id, mock_order_id.clone()),
    };
    let rpc_pub_evt = MockRpcPublishEvent {
        _recv_resp_result: Mutex::new(Some(Ok(mock_reply))),
    };
    let mock_rpc_client = MockRpcClient {
        _send_req_result: Mutex::new(Some(Ok(Box::new(rpc_pub_evt)))),
    };
    let mock_rpc_ctx = MockRpcContext {
        _acquire_result: Mutex::new(Some(Ok(Box::new(mock_rpc_client)))),
    };
    let mock_payin_result = ut_processor_pay_in_result();
    let mock_processor = MockPaymentProcessor {
        _payin_start_result: Mutex::new(Some(Ok(mock_payin_result))),
    };
    let uc = ChargeCreateUseCase {
        processors: Arc::new(Box::new(mock_processor)),
        rpc_ctx: Arc::new(Box::new(mock_rpc_ctx)),
        ordersync_lockset: Arc::new(Box::new(mock_sync_cache)),
        repo: Box::new(mock_repo),
    };
    let mock_req = ut_charge_req_dto(mock_order_id.clone());
    let result = uc.execute(mock_usr_id, mock_req).await;
    assert!(result.is_ok());
    if let Ok(_resp) = result {
        // TODO, examine response detail
    }
} // end of fn ok_with_rpc_replica_order

#[actix_web::test]
async fn load_unpaid_order_failure() {
    let mock_usr_id = 1234u32;
    let mock_order_id = "ut-origin-order-id".to_string();
    let repo_expect_error = AppRepoError {
        fn_label: AppRepoErrorFnLabel::GetUnpaidOlines,
        code: AppErrorCode::Unknown,
        detail: AppRepoErrorDetail::Unknown,
    };
    let mock_repo = MockChargeRepo {
        _expect_unpaid_olines: Mutex::new(Some(Err(repo_expect_error))),
        _create_order_result: Mutex::new(None),
        _create_charge_result: Mutex::new(None),
    };
    let mock_sync_cache = MockOrderSyncLockCache {
        _acquire_result: Mutex::new(None),
        _release_result: Mutex::new(None),
    };
    let mock_rpc_ctx = MockRpcContext {
        _acquire_result: Mutex::new(None),
    };
    let mock_processor = MockPaymentProcessor {
        _payin_start_result: Mutex::new(None),
    };
    let uc = ChargeCreateUseCase {
        processors: Arc::new(Box::new(mock_processor)),
        rpc_ctx: Arc::new(Box::new(mock_rpc_ctx)),
        ordersync_lockset: Arc::new(Box::new(mock_sync_cache)),
        repo: Box::new(mock_repo),
    };
    let mock_req = ut_charge_req_dto(mock_order_id.clone());
    let result = uc.execute(mock_usr_id, mock_req).await;
    assert!(result.is_err());
    if let Err(e) = result {
        if let ChargeCreateUcError::DataStoreError(actual_error) = e {
            let cond = matches!(actual_error.fn_label, AppRepoErrorFnLabel::GetUnpaidOlines);
            assert!(cond);
        } else {
            assert!(false);
        }
    }
} // end of fn load_unpaid_order_failure

#[actix_web::test]
async fn sync_order_get_lock_failure() {
    let mock_usr_id = 1234u32;
    let mock_order_id = "ut-origin-order-id".to_string();
    let mock_repo = MockChargeRepo {
        _expect_unpaid_olines: Mutex::new(Some(Ok(None))),
        _create_order_result: Mutex::new(None),
        _create_charge_result: Mutex::new(None),
    };
    let mock_sync_cache = MockOrderSyncLockCache {
        _acquire_result: Mutex::new(Some(Ok(false))),
        _release_result: Mutex::new(None),
    };
    let mock_rpc_ctx = MockRpcContext {
        _acquire_result: Mutex::new(None),
    };
    let mock_processor = MockPaymentProcessor {
        _payin_start_result: Mutex::new(None),
    };
    let uc = ChargeCreateUseCase {
        processors: Arc::new(Box::new(mock_processor)),
        rpc_ctx: Arc::new(Box::new(mock_rpc_ctx)),
        ordersync_lockset: Arc::new(Box::new(mock_sync_cache)),
        repo: Box::new(mock_repo),
    };
    let mock_req = ut_charge_req_dto(mock_order_id.clone());
    let result = uc.execute(mock_usr_id, mock_req).await;
    assert!(result.is_err());
    if let Err(e) = result {
        let cond = matches!(e, ChargeCreateUcError::LoadOrderConflict);
        assert!(cond);
    }
} // end of fn sync_order_get_lock_failure

#[actix_web::test]
async fn sync_order_release_lock_failure() {
    let mock_usr_id = 1234u32;
    let mock_order_id = "ut-origin-order-id".to_string();
    let mock_repo = MockChargeRepo {
        _expect_unpaid_olines: Mutex::new(Some(Ok(None))),
        _create_order_result: Mutex::new(None),
        _create_charge_result: Mutex::new(None),
    };
    let mock_sync_cache = MockOrderSyncLockCache {
        _acquire_result: Mutex::new(Some(Ok(true))),
        _release_result: Mutex::new(Some(Err(OrderSyncLockError))),
    };
    let mock_reply = AppRpcReply {
        message: ut_orderpay_replica(mock_usr_id, mock_order_id.clone()),
    };
    let rpc_pub_evt = MockRpcPublishEvent {
        _recv_resp_result: Mutex::new(Some(Ok(mock_reply))),
    };
    let mock_rpc_client = MockRpcClient {
        _send_req_result: Mutex::new(Some(Ok(Box::new(rpc_pub_evt)))),
    };
    let mock_rpc_ctx = MockRpcContext {
        _acquire_result: Mutex::new(Some(Ok(Box::new(mock_rpc_client)))),
    };
    let mock_processor = MockPaymentProcessor {
        _payin_start_result: Mutex::new(None),
    };
    let uc = ChargeCreateUseCase {
        processors: Arc::new(Box::new(mock_processor)),
        rpc_ctx: Arc::new(Box::new(mock_rpc_ctx)),
        ordersync_lockset: Arc::new(Box::new(mock_sync_cache)),
        repo: Box::new(mock_repo),
    };
    let mock_req = ut_charge_req_dto(mock_order_id.clone());
    let result = uc.execute(mock_usr_id, mock_req).await;
    assert!(result.is_err());
    if let Err(e) = result {
        let cond = matches!(e, ChargeCreateUcError::LockCacheError);
        assert!(cond);
    }
} // end of fn sync_order_release_lock_failure

#[actix_web::test]
async fn rpc_acquire_conn_error() {
    let mock_usr_id = 1234u32;
    let mock_order_id = "ut-origin-order-id".to_string();
    let mock_repo = MockChargeRepo {
        _expect_unpaid_olines: Mutex::new(Some(Ok(None))),
        _create_order_result: Mutex::new(None),
        _create_charge_result: Mutex::new(None),
    };
    let mock_sync_cache = MockOrderSyncLockCache {
        _acquire_result: Mutex::new(Some(Ok(true))),
        _release_result: Mutex::new(Some(Ok(()))),
    };
    let rpc_expect_error = AppRpcCtxError {
        fn_label: AppRpcErrorFnLabel::AcquireClientConn,
        reason: AppRpcErrorReason::CorruptedCredential,
    };
    let mock_rpc_ctx = MockRpcContext {
        _acquire_result: Mutex::new(Some(Err(rpc_expect_error))),
    };
    let mock_processor = MockPaymentProcessor {
        _payin_start_result: Mutex::new(None),
    };
    let uc = ChargeCreateUseCase {
        processors: Arc::new(Box::new(mock_processor)),
        rpc_ctx: Arc::new(Box::new(mock_rpc_ctx)),
        ordersync_lockset: Arc::new(Box::new(mock_sync_cache)),
        repo: Box::new(mock_repo),
    };
    let mock_req = ut_charge_req_dto(mock_order_id.clone());
    let result = uc.execute(mock_usr_id, mock_req).await;
    assert!(result.is_err());
    if let Err(e) = result {
        if let ChargeCreateUcError::LoadOrderInternalError(actual_error) = e {
            let cond = matches!(actual_error.fn_label, AppRpcErrorFnLabel::AcquireClientConn);
            assert!(cond);
            let cond = matches!(actual_error.reason, AppRpcErrorReason::CorruptedCredential);
            assert!(cond);
        } else {
            assert!(false);
        }
    }
} // end of fn rpc_acquire_conn_error

#[actix_web::test]
async fn rpc_publish_error_replica_order() {
    let mock_usr_id = 1234u32;
    let mock_order_id = "ut-origin-order-id".to_string();
    let mock_repo = MockChargeRepo {
        _expect_unpaid_olines: Mutex::new(Some(Ok(None))),
        _create_order_result: Mutex::new(None),
        _create_charge_result: Mutex::new(None),
    };
    let mock_sync_cache = MockOrderSyncLockCache {
        _acquire_result: Mutex::new(Some(Ok(true))),
        _release_result: Mutex::new(Some(Ok(()))),
    };
    let rpc_expect_error = AppRpcCtxError {
        fn_label: AppRpcErrorFnLabel::ClientSendReq,
        reason: AppRpcErrorReason::LowLevelConn("unit-test".to_string()),
    };
    let mock_rpc_client = MockRpcClient {
        _send_req_result: Mutex::new(Some(Err(rpc_expect_error))),
    };
    let mock_rpc_ctx = MockRpcContext {
        _acquire_result: Mutex::new(Some(Ok(Box::new(mock_rpc_client)))),
    };
    let mock_processor = MockPaymentProcessor {
        _payin_start_result: Mutex::new(None),
    };
    let uc = ChargeCreateUseCase {
        processors: Arc::new(Box::new(mock_processor)),
        rpc_ctx: Arc::new(Box::new(mock_rpc_ctx)),
        ordersync_lockset: Arc::new(Box::new(mock_sync_cache)),
        repo: Box::new(mock_repo),
    };
    let mock_req = ut_charge_req_dto(mock_order_id.clone());
    let result = uc.execute(mock_usr_id, mock_req).await;
    assert!(result.is_err());
    if let Err(e) = result {
        if let ChargeCreateUcError::LoadOrderInternalError(actual_error) = e {
            let cond = matches!(actual_error.fn_label, AppRpcErrorFnLabel::ClientSendReq);
            assert!(cond);
            let cond = matches!(actual_error.reason, AppRpcErrorReason::LowLevelConn(_));
            assert!(cond);
        } else {
            assert!(false);
        }
    }
} // end of fn rpc_publish_error_replica_order

#[actix_web::test]
async fn rpc_reply_error_replica_order() {
    let mock_usr_id = 1234u32;
    let mock_order_id = "ut-origin-order-id".to_string();
    let mock_repo = MockChargeRepo {
        _expect_unpaid_olines: Mutex::new(Some(Ok(None))),
        _create_order_result: Mutex::new(None),
        _create_charge_result: Mutex::new(None),
    };
    let mock_sync_cache = MockOrderSyncLockCache {
        _acquire_result: Mutex::new(Some(Ok(true))),
        _release_result: Mutex::new(Some(Ok(()))),
    };
    let rpc_expect_error = AppRpcCtxError {
        fn_label: AppRpcErrorFnLabel::ClientRecvResp,
        reason: AppRpcErrorReason::NotSupport,
    };
    let rpc_pub_evt = MockRpcPublishEvent {
        _recv_resp_result: Mutex::new(Some(Err(rpc_expect_error))),
    };
    let mock_rpc_client = MockRpcClient {
        _send_req_result: Mutex::new(Some(Ok(Box::new(rpc_pub_evt)))),
    };
    let mock_rpc_ctx = MockRpcContext {
        _acquire_result: Mutex::new(Some(Ok(Box::new(mock_rpc_client)))),
    };
    let mock_processor = MockPaymentProcessor {
        _payin_start_result: Mutex::new(None),
    };
    let uc = ChargeCreateUseCase {
        processors: Arc::new(Box::new(mock_processor)),
        rpc_ctx: Arc::new(Box::new(mock_rpc_ctx)),
        ordersync_lockset: Arc::new(Box::new(mock_sync_cache)),
        repo: Box::new(mock_repo),
    };
    let mock_req = ut_charge_req_dto(mock_order_id.clone());
    let result = uc.execute(mock_usr_id, mock_req).await;
    assert!(result.is_err());
    if let Err(e) = result {
        if let ChargeCreateUcError::LoadOrderInternalError(actual_error) = e {
            let cond = matches!(actual_error.fn_label, AppRpcErrorFnLabel::ClientRecvResp);
            assert!(cond);
            let cond = matches!(actual_error.reason, AppRpcErrorReason::NotSupport);
            assert!(cond);
        } else {
            assert!(false);
        }
    }
} // end of rpc_reply_error_replica_order

#[actix_web::test]
async fn save_replica_order_failure() {
    let mock_usr_id = 1234u32;
    let mock_order_id = "ut-origin-order-id".to_string();
    let repo_expect_error = AppRepoError {
        fn_label: AppRepoErrorFnLabel::CreateOrder,
        code: AppErrorCode::DataTableNotExist,
        detail: AppRepoErrorDetail::DatabaseExec("unit-test".to_string()),
    };
    let mock_repo = MockChargeRepo {
        _expect_unpaid_olines: Mutex::new(Some(Ok(None))),
        _create_order_result: Mutex::new(Some(Err(repo_expect_error))),
        _create_charge_result: Mutex::new(None),
    };
    let mock_sync_cache = MockOrderSyncLockCache {
        _acquire_result: Mutex::new(Some(Ok(true))),
        _release_result: Mutex::new(Some(Ok(()))),
    };
    let mock_reply = AppRpcReply {
        message: ut_orderpay_replica(mock_usr_id, mock_order_id.clone()),
    };
    let rpc_pub_evt = MockRpcPublishEvent {
        _recv_resp_result: Mutex::new(Some(Ok(mock_reply))),
    };
    let mock_rpc_client = MockRpcClient {
        _send_req_result: Mutex::new(Some(Ok(Box::new(rpc_pub_evt)))),
    };
    let mock_rpc_ctx = MockRpcContext {
        _acquire_result: Mutex::new(Some(Ok(Box::new(mock_rpc_client)))),
    };
    let mock_processor = MockPaymentProcessor {
        _payin_start_result: Mutex::new(None),
    };
    let uc = ChargeCreateUseCase {
        processors: Arc::new(Box::new(mock_processor)),
        rpc_ctx: Arc::new(Box::new(mock_rpc_ctx)),
        ordersync_lockset: Arc::new(Box::new(mock_sync_cache)),
        repo: Box::new(mock_repo),
    };
    let mock_req = ut_charge_req_dto(mock_order_id.clone());
    let result = uc.execute(mock_usr_id, mock_req).await;
    assert!(result.is_err());
    if let Err(e) = result {
        if let ChargeCreateUcError::DataStoreError(actual_error) = e {
            let cond = matches!(actual_error.fn_label, AppRepoErrorFnLabel::CreateOrder);
            assert!(cond);
        } else {
            assert!(false);
        }
    }
} // end of fn save_replica_order_failure

#[actix_web::test]
async fn processor_start_payin_failure() {
    let mock_usr_id = 1234u32;
    let mock_order_id = "ut-origin-order-id".to_string();
    let mock_repo = MockChargeRepo {
        _expect_unpaid_olines: Mutex::new(Some(Ok(None))),
        _create_order_result: Mutex::new(Some(Ok(()))),
        _create_charge_result: Mutex::new(Some(Ok(()))),
    };
    let mock_sync_cache = MockOrderSyncLockCache {
        _acquire_result: Mutex::new(Some(Ok(true))),
        _release_result: Mutex::new(Some(Ok(()))),
    };
    let mock_reply = AppRpcReply {
        message: ut_orderpay_replica(mock_usr_id, mock_order_id.clone()),
    };
    let rpc_pub_evt = MockRpcPublishEvent {
        _recv_resp_result: Mutex::new(Some(Ok(mock_reply))),
    };
    let mock_rpc_client = MockRpcClient {
        _send_req_result: Mutex::new(Some(Ok(Box::new(rpc_pub_evt)))),
    };
    let mock_rpc_ctx = MockRpcContext {
        _acquire_result: Mutex::new(Some(Ok(Box::new(mock_rpc_client)))),
    };
    let expect_proc_error = AppProcessorError {
        reason: AppProcessorErrorReason::CredentialCorrupted,
    };
    let mock_processor = MockPaymentProcessor {
        _payin_start_result: Mutex::new(Some(Err(expect_proc_error))),
    };
    let uc = ChargeCreateUseCase {
        processors: Arc::new(Box::new(mock_processor)),
        rpc_ctx: Arc::new(Box::new(mock_rpc_ctx)),
        ordersync_lockset: Arc::new(Box::new(mock_sync_cache)),
        repo: Box::new(mock_repo),
    };
    let mock_req = ut_charge_req_dto(mock_order_id.clone());
    let result = uc.execute(mock_usr_id, mock_req).await;
    assert!(result.is_err());
    if let Err(e) = result {
        if let ChargeCreateUcError::ExternalProcessorError(actual_error) = e {
            let cond = matches!(actual_error, PaymentMethodErrorReason::ProcessorFailure);
            assert!(cond);
        } else {
            assert!(false);
        }
    }
} // end of fn processor_start_payin_failure

#[actix_web::test]
async fn save_new_chargeline_failure() {
    let mock_usr_id = 1234u32;
    let mock_order_id = "ut-origin-order-id".to_string();
    let repo_expect_error = AppRepoError {
        fn_label: AppRepoErrorFnLabel::CreateCharge,
        code: AppErrorCode::DataCorruption,
        detail: AppRepoErrorDetail::DatabaseTxCommit("unit-test".to_string()),
    };
    let mock_repo = MockChargeRepo {
        _expect_unpaid_olines: Mutex::new(Some(Ok(None))),
        _create_order_result: Mutex::new(Some(Ok(()))),
        _create_charge_result: Mutex::new(Some(Err(repo_expect_error))),
    };
    let mock_sync_cache = MockOrderSyncLockCache {
        _acquire_result: Mutex::new(Some(Ok(true))),
        _release_result: Mutex::new(Some(Ok(()))),
    };
    let mock_reply = AppRpcReply {
        message: ut_orderpay_replica(mock_usr_id, mock_order_id.clone()),
    };
    let rpc_pub_evt = MockRpcPublishEvent {
        _recv_resp_result: Mutex::new(Some(Ok(mock_reply))),
    };
    let mock_rpc_client = MockRpcClient {
        _send_req_result: Mutex::new(Some(Ok(Box::new(rpc_pub_evt)))),
    };
    let mock_rpc_ctx = MockRpcContext {
        _acquire_result: Mutex::new(Some(Ok(Box::new(mock_rpc_client)))),
    };
    let mock_payin_result = ut_processor_pay_in_result();
    let mock_processor = MockPaymentProcessor {
        _payin_start_result: Mutex::new(Some(Ok(mock_payin_result))),
    };
    let uc = ChargeCreateUseCase {
        processors: Arc::new(Box::new(mock_processor)),
        rpc_ctx: Arc::new(Box::new(mock_rpc_ctx)),
        ordersync_lockset: Arc::new(Box::new(mock_sync_cache)),
        repo: Box::new(mock_repo),
    };
    let mock_req = ut_charge_req_dto(mock_order_id.clone());
    let result = uc.execute(mock_usr_id, mock_req).await;
    assert!(result.is_err());
    if let Err(e) = result {
        if let ChargeCreateUcError::DataStoreError(actual_error) = e {
            let cond = matches!(actual_error.fn_label, AppRepoErrorFnLabel::CreateCharge);
            assert!(cond);
        } else {
            assert!(false);
        }
    }
} // end of fn save_new_chargeline_failure
