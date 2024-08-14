use std::sync::Arc;

use chrono::{DateTime, Duration, FixedOffset, Utc};

use ecommerce_common::api::rpc::dto::{
    OrderLinePayUpdateErrorDto, OrderLinePayUpdateErrorReason, OrderPaymentUpdateErrorDto,
};
use ecommerce_common::constant::ProductType;
use ecommerce_common::error::AppErrorCode;
use payment::adapter::processor::{
    AppProcessorError, AppProcessorErrorReason, AppProcessorFnLabel, BaseClientError,
    BaseClientErrorReason,
};
use payment::adapter::repository::{AppRepoError, AppRepoErrorDetail, AppRepoErrorFnLabel};
use payment::adapter::rpc::{AppRpcCtxError, AppRpcErrorFnLabel, AppRpcErrorReason, AppRpcReply};
use payment::api::web::dto::ChargeStatusDto;
use payment::model::{
    BuyerPayInState, Charge3partyModel, Charge3partyStripeModel, ChargeBuyerMetaModel,
    ChargeLineBuyerModel, StripeCheckoutPaymentStatusModel, StripeSessionStatusModel,
};
use payment::usecase::{ChargeRefreshUcError, ChargeStatusRefreshUseCase};

use super::{
    MockChargeRepo, MockPaymentProcessor, MockRpcClient, MockRpcContext, MockRpcPublishEvent,
};
use crate::model::ut_setup_buyer_charge_lines;

fn ut_setup_charge_3pty_stripe(expiry: DateTime<Utc>) -> Charge3partyModel {
    let stripe3pty = Charge3partyStripeModel {
        checkout_session_id: "mock-session-id".to_string(),
        session_state: StripeSessionStatusModel::open,
        payment_state: StripeCheckoutPaymentStatusModel::unpaid,
        payment_intent_id: "mock-payment-intent-id".to_string(),
        expiry,
    };
    Charge3partyModel::Stripe(stripe3pty)
}
fn ut_setup_buyer_meta_stripe(
    usr_id: u32,
    order_id: String,
    charge_time: DateTime<FixedOffset>,
) -> ChargeBuyerMetaModel {
    let t0 = charge_time.to_utc();
    let t1 = t0 + Duration::minutes(1);
    let t2 = t0 + Duration::minutes(3);
    let arg = (order_id, usr_id, t0);
    let mut obj = ChargeBuyerMetaModel::from(arg);
    let value = ut_setup_charge_3pty_stripe(t2);
    obj.update_3party(value);
    let value = BuyerPayInState::ProcessorAccepted(t1);
    obj.update_progress(&value);
    obj
}

fn ut_rpc_orderpay_update_err(oid: String, lines: Vec<OrderLinePayUpdateErrorDto>) -> Vec<u8> {
    let obj = OrderPaymentUpdateErrorDto {
        oid,
        charge_time: None,
        lines,
    };
    serde_json::to_vec(&obj).unwrap()
}

fn ut_common_mock_data() -> (
    u32,
    DateTime<FixedOffset>,
    String,
    String,
    ChargeBuyerMetaModel,
    Vec<ChargeLineBuyerModel>,
) {
    let usr_id = 8010095;
    let charge_time = DateTime::parse_from_rfc3339("2012-04-24T23:01:30+00:00").unwrap();
    let charge_id = "007a396f1f7131705e".to_string();
    let order_id = "shout-out".to_string();
    let d = vec![
        (8298, ProductType::Package, 471, (9028, 2), (36112, 2), 4),
        (2369, ProductType::Item, 380, (551, 1), (1102, 1), 2),
    ];
    (
        usr_id,
        charge_time,
        charge_id,
        order_id.clone(),
        ut_setup_buyer_meta_stripe(usr_id, order_id, charge_time),
        ut_setup_buyer_charge_lines(d),
    )
}

#[actix_web::test]
async fn ok_entire_pay_in_completed() {
    // with order-app synced
    let (
        mock_usr_id,
        mock_charge_time,
        mock_charge_id,
        mock_order_id,
        mock_buyer_meta,
        mock_charge_lines,
    ) = ut_common_mock_data();
    let mock_repo = MockChargeRepo::build(
        None,
        None,
        None,
        Some(Ok(Some(mock_buyer_meta))),
        Some(Ok(mock_charge_lines)),
        Some(Ok(())),
    );
    let mock3pty_refreshed = {
        let t = mock_charge_time.to_utc() + Duration::minutes(3);
        let mut m3pt = ut_setup_charge_3pty_stripe(t);
        if let Charge3partyModel::Stripe(inner) = &mut m3pt {
            inner.session_state = StripeSessionStatusModel::complete;
            inner.payment_state = StripeCheckoutPaymentStatusModel::paid;
        } // assume the client has confirmed the charge with the external 3rd party
        m3pt
    };
    let mock_3pty = MockPaymentProcessor::build(None, Some(Ok(mock3pty_refreshed)));
    let mock_reply = AppRpcReply {
        message: ut_rpc_orderpay_update_err(mock_order_id.to_string(), Vec::new()),
    };
    let rpc_pub_evt = MockRpcPublishEvent::build(Some(Ok(mock_reply)));
    let mock_rpc_client = MockRpcClient::build(Some(Ok(rpc_pub_evt)));
    let mock_rpc_ctx = MockRpcContext::build(Some(Ok(mock_rpc_client)));
    let uc = ChargeStatusRefreshUseCase {
        repo: mock_repo,
        processors: Arc::new(mock_3pty),
        rpc_ctx: Arc::new(mock_rpc_ctx),
    };
    let result = uc.execute(mock_usr_id, mock_charge_id).await;
    assert!(result.is_ok());
    if let Ok(v) = result {
        let cond = matches!(v.status, ChargeStatusDto::Completed);
        assert!(cond);
        assert_eq!(v.order_id.as_str(), mock_order_id.as_str());
    }
} // end of fn ok_entire_pay_in_completed

#[actix_web::test]
async fn ok_3party_processing() {
    let (mock_usr_id, mock_charge_time, mock_charge_id, _, mock_buyer_meta, _) =
        ut_common_mock_data();
    let mock_repo = MockChargeRepo::build(
        None,
        None,
        None,
        Some(Ok(Some(mock_buyer_meta))),
        None,
        Some(Ok(())),
    );
    let mock3pty_refreshed = {
        let t = mock_charge_time.to_utc() + Duration::minutes(3);
        // assume the client hasn't confirmed the charge yet
        ut_setup_charge_3pty_stripe(t)
    };
    let mock_3pty = MockPaymentProcessor::build(None, Some(Ok(mock3pty_refreshed)));
    let mock_rpc_ctx = MockRpcContext::build(None);
    let uc = ChargeStatusRefreshUseCase {
        repo: mock_repo,
        processors: Arc::new(mock_3pty),
        rpc_ctx: Arc::new(mock_rpc_ctx),
    };
    let result = uc.execute(mock_usr_id, mock_charge_id).await;
    assert!(result.is_ok());
    if let Ok(v) = result {
        let cond = matches!(v.status, ChargeStatusDto::PspProcessing);
        assert!(cond);
    }
} // end of fn ok_3party_processing

#[actix_web::test]
async fn ok_3party_refused() {
    let (mock_usr_id, mock_charge_time, mock_charge_id, _, mock_buyer_meta, _) =
        ut_common_mock_data();
    let mock_repo = MockChargeRepo::build(
        None,
        None,
        None,
        Some(Ok(Some(mock_buyer_meta))),
        None,
        Some(Ok(())),
    );
    let mock3pty_refreshed = {
        let t = mock_charge_time.to_utc() + Duration::minutes(3);
        let mut m3pt = ut_setup_charge_3pty_stripe(t);
        if let Charge3partyModel::Stripe(inner) = &mut m3pt {
            inner.session_state = StripeSessionStatusModel::complete;
            inner.payment_state = StripeCheckoutPaymentStatusModel::unpaid;
        }
        // this service always configures payment mode to Stripe API server,
        // so it doesn't make sense that Stripe shows a session is `completed`
        // but the corresponding payment is in `unpaid` state.
        // currently
        // TODO, find better way of validating such situation
        m3pt
    };
    let mock_3pty = MockPaymentProcessor::build(None, Some(Ok(mock3pty_refreshed)));
    let mock_rpc_ctx = MockRpcContext::build(None);
    let uc = ChargeStatusRefreshUseCase {
        repo: mock_repo,
        processors: Arc::new(mock_3pty),
        rpc_ctx: Arc::new(mock_rpc_ctx),
    };
    let result = uc.execute(mock_usr_id, mock_charge_id).await;
    assert!(result.is_ok());
    if let Ok(v) = result {
        let cond = matches!(v.status, ChargeStatusDto::PspRefused);
        assert!(cond);
    }
} // end of ok_3party_refused

#[actix_web::test]
async fn ok_3party_session_expired() {
    let (mock_usr_id, mock_charge_time, mock_charge_id, _, mock_buyer_meta, _) =
        ut_common_mock_data();
    let mock_repo = MockChargeRepo::build(
        None,
        None,
        None,
        Some(Ok(Some(mock_buyer_meta))),
        None,
        Some(Ok(())),
    );
    let mock3pty_refreshed = {
        let t = mock_charge_time.to_utc() + Duration::minutes(3);
        let mut m3pt = ut_setup_charge_3pty_stripe(t);
        if let Charge3partyModel::Stripe(inner) = &mut m3pt {
            inner.session_state = StripeSessionStatusModel::expired;
            inner.payment_state = StripeCheckoutPaymentStatusModel::unpaid;
        } // assume the client has confirmed the charge with the external 3rd party
        m3pt
    };
    let mock_3pty = MockPaymentProcessor::build(None, Some(Ok(mock3pty_refreshed)));
    let mock_rpc_ctx = MockRpcContext::build(None);
    let uc = ChargeStatusRefreshUseCase {
        repo: mock_repo,
        processors: Arc::new(mock_3pty),
        rpc_ctx: Arc::new(mock_rpc_ctx),
    };
    let result = uc.execute(mock_usr_id, mock_charge_id).await;
    assert!(result.is_ok());
    if let Ok(v) = result {
        let cond = matches!(v.status, ChargeStatusDto::SessionExpired);
        assert!(cond);
    }
} // end of fn ok_3party_session_expired

#[actix_web::test]
async fn ok_skip_3party() {
    let (
        mock_usr_id,
        mock_charge_time,
        mock_charge_id,
        mock_order_id,
        mut mock_buyer_meta,
        mock_charge_lines,
    ) = ut_common_mock_data();
    // assume 3rd party has already completed but an issue happened
    // to RPC order app for payment sync
    {
        let t = mock_charge_time.to_utc() + Duration::minutes(5);
        let new_state = BuyerPayInState::ProcessorCompleted(t);
        mock_buyer_meta.update_progress(&new_state);
        let t = mock_charge_time.to_utc() + Duration::minutes(4);
        let mut m3pty = ut_setup_charge_3pty_stripe(t);
        if let Charge3partyModel::Stripe(s) = &mut m3pty {
            s.session_state = StripeSessionStatusModel::complete;
            s.payment_state = StripeCheckoutPaymentStatusModel::paid;
        }
        mock_buyer_meta.update_3party(m3pty);
    }
    let mock_repo = MockChargeRepo::build(
        None,
        None,
        None,
        Some(Ok(Some(mock_buyer_meta))),
        Some(Ok(mock_charge_lines)),
        Some(Ok(())),
    );
    let mock_3pty = MockPaymentProcessor::build(None, None);
    let mock_reply = AppRpcReply {
        message: ut_rpc_orderpay_update_err(mock_order_id.to_string(), Vec::new()),
    };
    let rpc_pub_evt = MockRpcPublishEvent::build(Some(Ok(mock_reply)));
    let mock_rpc_client = MockRpcClient::build(Some(Ok(rpc_pub_evt)));
    let mock_rpc_ctx = MockRpcContext::build(Some(Ok(mock_rpc_client)));
    let uc = ChargeStatusRefreshUseCase {
        repo: mock_repo,
        processors: Arc::new(mock_3pty),
        rpc_ctx: Arc::new(mock_rpc_ctx),
    };
    let result = uc.execute(mock_usr_id, mock_charge_id).await;
    assert!(result.is_ok());
    if let Ok(v) = result {
        let cond = matches!(v.status, ChargeStatusDto::Completed);
        assert!(cond);
    }
} // end of ok_skip_3party

#[actix_web::test]
async fn orderapp_already_synced() {
    let (mock_usr_id, mock_charge_time, mock_charge_id, _, mut mock_buyer_meta, _) =
        ut_common_mock_data();
    {
        let t = mock_charge_time.to_utc() + Duration::minutes(6);
        let new_state = BuyerPayInState::OrderAppSynced(t);
        mock_buyer_meta.update_progress(&new_state);
        let t = mock_charge_time.to_utc() + Duration::minutes(5);
        let mut m3pty = ut_setup_charge_3pty_stripe(t);
        if let Charge3partyModel::Stripe(s) = &mut m3pty {
            s.session_state = StripeSessionStatusModel::complete;
            s.payment_state = StripeCheckoutPaymentStatusModel::paid;
        }
        mock_buyer_meta.update_3party(m3pty);
    }
    let mock_repo = MockChargeRepo::build(
        None,
        None,
        None,
        Some(Ok(Some(mock_buyer_meta))),
        None,
        Some(Ok(())),
    );
    let mock_3pty = MockPaymentProcessor::build(None, None);
    let mock_rpc_ctx = MockRpcContext::build(None);
    let uc = ChargeStatusRefreshUseCase {
        repo: mock_repo,
        processors: Arc::new(mock_3pty),
        rpc_ctx: Arc::new(mock_rpc_ctx),
    };
    let result = uc.execute(mock_usr_id, mock_charge_id).await;
    assert!(result.is_ok());
    if let Ok(v) = result {
        let cond = matches!(v.status, ChargeStatusDto::Completed);
        assert!(cond);
    }
} // end of fn orderapp_already_synced

#[actix_web::test]
async fn error_3party_lowlvl() {
    let (mock_usr_id, _, mock_charge_id, _, mock_buyer_meta, _) = ut_common_mock_data();
    let mock_repo = MockChargeRepo::build(
        None,
        None,
        None,
        Some(Ok(Some(mock_buyer_meta))),
        None,
        None,
    );
    let error3pty = {
        let reason = BaseClientErrorReason::TcpNet(
            std::io::ErrorKind::ConnectionRefused,
            "mock-unit-test".to_string(),
        );
        let client_err = BaseClientError { reason };
        let reason = AppProcessorErrorReason::LowLvlNet(client_err);
        let fn_label = AppProcessorFnLabel::PayInProgress;
        AppProcessorError { reason, fn_label }
    };
    let mock_3pty = MockPaymentProcessor::build(None, Some(Err(error3pty)));
    let mock_rpc_ctx = MockRpcContext::build(None);
    let uc = ChargeStatusRefreshUseCase {
        repo: mock_repo,
        processors: Arc::new(mock_3pty),
        rpc_ctx: Arc::new(mock_rpc_ctx),
    };
    let result = uc.execute(mock_usr_id, mock_charge_id).await;
    assert!(result.is_err());
    if let Err(ChargeRefreshUcError::ExternalProcessor(e)) = result {
        if let AppProcessorErrorReason::LowLvlNet(lowlvle) = e.reason {
            if let BaseClientErrorReason::TcpNet(ekind, msg) = lowlvle.reason {
                assert_eq!(ekind, std::io::ErrorKind::ConnectionRefused);
                assert_eq!(msg.as_str(), "mock-unit-test");
            } else {
                assert!(false);
            }
        } else {
            assert!(false);
        }
    } else {
        assert!(false);
    }
} // end of fn error_3party_lowlvl

#[actix_web::test]
async fn error_decode_charge_id() {
    let mock_usr_id = 8010095;
    let mock_charge_id = "007a396f000000000000ff".to_string();
    let mock_repo = MockChargeRepo::build(None, None, None, None, None, None);
    let mock_3pty = MockPaymentProcessor::build(None, None);
    let mock_rpc_ctx = MockRpcContext::build(None);
    let uc = ChargeStatusRefreshUseCase {
        repo: mock_repo,
        processors: Arc::new(mock_3pty),
        rpc_ctx: Arc::new(mock_rpc_ctx),
    };
    let result = uc.execute(mock_usr_id, mock_charge_id).await;
    assert!(result.is_err());
    if let Err(ChargeRefreshUcError::ChargeIdDecode(ecode, _msg)) = result {
        assert_eq!(ecode, AppErrorCode::DataCorruption);
    } else {
        assert!(false);
    }
} // end of fn error_decode_charge_id

#[actix_web::test]
async fn error_owner_mismatch() {
    let mock_usr_id = 8010095;
    let mock_charge_id = "007a00001f7131705e".to_string();
    let mock_repo = MockChargeRepo::build(None, None, None, None, None, None);
    let mock_3pty = MockPaymentProcessor::build(None, None);
    let mock_rpc_ctx = MockRpcContext::build(None);
    let uc = ChargeStatusRefreshUseCase {
        repo: mock_repo,
        processors: Arc::new(mock_3pty),
        rpc_ctx: Arc::new(mock_rpc_ctx),
    };
    let result = uc.execute(mock_usr_id, mock_charge_id).await;
    assert!(result.is_err());
    if let Err(e) = result {
        let cond = matches!(e, ChargeRefreshUcError::OwnerMismatch);
        assert!(cond);
    }
} // endmock_ of fn error_owner_mismatch

#[actix_web::test]
async fn error_repo_charge_not_exist() {
    let mock_usr_id = 8010095;
    let mock_charge_id = "007a396f1f7131705e".to_string();
    let mock_repo = MockChargeRepo::build(None, None, None, Some(Ok(None)), None, None);
    let mock_3pty = MockPaymentProcessor::build(None, None);
    let mock_rpc_ctx = MockRpcContext::build(None);
    let uc = ChargeStatusRefreshUseCase {
        repo: mock_repo,
        processors: Arc::new(mock_3pty),
        rpc_ctx: Arc::new(mock_rpc_ctx),
    };
    let result = uc.execute(mock_usr_id, mock_charge_id).await;
    assert!(result.is_err());
    if let Err(e) = result {
        let cond = matches!(e, ChargeRefreshUcError::ChargeNotExist(8010095, _));
        assert!(cond);
    }
} // end of fn error_repo_charge_not_exist

#[actix_web::test]
async fn error_repo_write_status() {
    let (mock_usr_id, mock_charge_time, mock_charge_id, _, mock_buyer_meta, _) =
        ut_common_mock_data();
    let error_wr = AppRepoError {
        fn_label: AppRepoErrorFnLabel::UpdateChargeProgress,
        code: AppErrorCode::DatabaseServerBusy,
        detail: AppRepoErrorDetail::DatabaseTxCommit("unit-test".to_string()),
    };
    let mock_repo = MockChargeRepo::build(
        None,
        None,
        None,
        Some(Ok(Some(mock_buyer_meta))),
        None,
        Some(Err(error_wr)),
    );
    let mock3pty_refreshed = {
        let t = mock_charge_time.to_utc() + Duration::minutes(3);
        let mut m3pt = ut_setup_charge_3pty_stripe(t);
        if let Charge3partyModel::Stripe(inner) = &mut m3pt {
            inner.session_state = StripeSessionStatusModel::expired;
            inner.payment_state = StripeCheckoutPaymentStatusModel::unpaid;
        }
        m3pt
    };
    let mock_3pty = MockPaymentProcessor::build(None, Some(Ok(mock3pty_refreshed)));
    let mock_rpc_ctx = MockRpcContext::build(None);
    let uc = ChargeStatusRefreshUseCase {
        repo: mock_repo,
        processors: Arc::new(mock_3pty),
        rpc_ctx: Arc::new(mock_rpc_ctx),
    };
    let result = uc.execute(mock_usr_id, mock_charge_id).await;
    assert!(result.is_err());
    if let Err(ChargeRefreshUcError::DataStore(e)) = result {
        let cond = matches!(e.fn_label, AppRepoErrorFnLabel::UpdateChargeProgress);
        assert!(cond);
        let cond = matches!(e.code, AppErrorCode::DatabaseServerBusy);
        assert!(cond);
        let cond = matches!(e.detail, AppRepoErrorDetail::DatabaseTxCommit(_));
        assert!(cond);
    } else {
        assert!(false);
    }
} // end of fn error_repo_write_status

#[actix_web::test]
async fn error_rpc_lowlvl() {
    let (mock_usr_id, mock_charge_time, mock_charge_id, _, mock_buyer_meta, _) =
        ut_common_mock_data();
    let mock_repo = MockChargeRepo::build(
        None,
        None,
        None,
        Some(Ok(Some(mock_buyer_meta))),
        None,
        Some(Ok(())),
    );
    let mock3pty_refreshed = {
        let t = mock_charge_time.to_utc() + Duration::minutes(3);
        let mut m3pt = ut_setup_charge_3pty_stripe(t);
        if let Charge3partyModel::Stripe(inner) = &mut m3pt {
            inner.session_state = StripeSessionStatusModel::complete;
            inner.payment_state = StripeCheckoutPaymentStatusModel::paid;
        } // assume the client has confirmed the charge with the external 3rd party
        m3pt
    };
    let mock_3pty = MockPaymentProcessor::build(None, Some(Ok(mock3pty_refreshed)));
    let error_rpc = AppRpcCtxError {
        fn_label: AppRpcErrorFnLabel::AcquireClientConn,
        reason: AppRpcErrorReason::LowLevelConn("unit-test".to_string()),
    };
    let mock_rpc_ctx = MockRpcContext::build(Some(Err(error_rpc)));
    let uc = ChargeStatusRefreshUseCase {
        repo: mock_repo,
        processors: Arc::new(mock_3pty),
        rpc_ctx: Arc::new(mock_rpc_ctx),
    };
    let result = uc.execute(mock_usr_id, mock_charge_id).await;
    assert!(result.is_err());
    if let Err(ChargeRefreshUcError::RpcContext(e)) = result {
        let cond = matches!(e.fn_label, AppRpcErrorFnLabel::AcquireClientConn);
        assert!(cond);
        if let AppRpcErrorReason::LowLevelConn(msg) = e.reason {
            assert_eq!(msg.as_str(), "unit-test");
        } else {
            assert!(false);
        }
    } else {
        assert!(false);
    }
} // end of fn error_rpc_lowlvl

// Consider over-charging situation, could this test case shows that
// my payment service can block overcharge request ? (TODO)
#[actix_web::test]
async fn error_rpc_reply_sync_orderapp() {
    let (
        mock_usr_id,
        mock_charge_time,
        mock_charge_id,
        mock_order_id,
        mock_buyer_meta,
        mock_charge_lines,
    ) = ut_common_mock_data();
    let mock_repo = MockChargeRepo::build(
        None,
        None,
        None,
        Some(Ok(Some(mock_buyer_meta))),
        Some(Ok(mock_charge_lines)),
        Some(Ok(())),
    );
    let mock3pty_refreshed = {
        let t = mock_charge_time.to_utc() + Duration::minutes(3);
        let mut m3pt = ut_setup_charge_3pty_stripe(t);
        if let Charge3partyModel::Stripe(inner) = &mut m3pt {
            inner.session_state = StripeSessionStatusModel::complete;
            inner.payment_state = StripeCheckoutPaymentStatusModel::paid;
        } // assume the client has confirmed the charge with the external 3rd party
        m3pt
    };
    let mock_3pty = MockPaymentProcessor::build(None, Some(Ok(mock3pty_refreshed)));
    let mock_reply = {
        let e = vec![OrderLinePayUpdateErrorDto {
            seller_id: 8298,
            product_type: ProductType::Package,
            product_id: 471,
            reason: OrderLinePayUpdateErrorReason::InvalidQuantity,
        }];
        AppRpcReply {
            message: ut_rpc_orderpay_update_err(mock_order_id.clone(), e),
        }
    };
    let rpc_pub_evt = MockRpcPublishEvent::build(Some(Ok(mock_reply)));
    let mock_rpc_client = MockRpcClient::build(Some(Ok(rpc_pub_evt)));
    let mock_rpc_ctx = MockRpcContext::build(Some(Ok(mock_rpc_client)));
    let uc = ChargeStatusRefreshUseCase {
        repo: mock_repo,
        processors: Arc::new(mock_3pty),
        rpc_ctx: Arc::new(mock_rpc_ctx),
    };
    let result = uc.execute(mock_usr_id, mock_charge_id).await;
    assert!(result.is_err());
    if let Err(ChargeRefreshUcError::RpcUpdateOrder(mut e)) = result {
        assert_eq!(e.oid.as_str(), mock_order_id.as_str());
        assert!(e.charge_time.is_none());
        assert_eq!(e.lines.len(), 1);
        let e_line = e.lines.remove(0);
        assert_eq!(e_line.seller_id, 8298);
        assert_eq!(e_line.product_id, 471);
        let cond = matches!(
            e_line.reason,
            OrderLinePayUpdateErrorReason::InvalidQuantity
        );
        assert!(cond);
    } else {
        assert!(false);
    }
} // end of fn error_rpc_reply_sync_orderapp
