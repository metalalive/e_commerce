use std::boxed::Box;
use std::future::Future;
use std::marker::Send;
use std::pin::Pin;
use std::sync::Arc;

use chrono::{DateTime, Duration, Local, SubsecRound, Utc};
use rust_decimal::Decimal;

use ecommerce_common::model::BaseProductIdentity;
use payment::adapter::processor::AbstractPaymentProcessor;
use payment::adapter::repository::{AppRefundRslvReqCbReturn, AppRepoErrorDetail};
use payment::api::web::dto::{RefundCompletionReqDto, RefundRejectReasonDto};
use payment::model::{
    BuyerPayInState, ChargeBuyerModel, OLineRefundModel, OrderRefundModel, PayLineAmountModel,
    RefundLineQtyRejectModel, RefundModelError, RefundReqResolutionModel,
};

use super::{ut_setup_currency_snapshot, ut_setup_db_refund_repo};
use crate::model::refund::ut_setup_refund_cmplt_dto;
use crate::model::{ut_default_charge_method_stripe, ut_setup_buyer_charge};
use crate::ut_setup_sharestate;

#[rustfmt::skip]
fn _ut_setup_buyer_charge(
    oid: &str,
    create_time: DateTime<Utc>,
    merchant_id : u32,
) -> ChargeBuyerModel {
    let buyer_usr_id = 1245u32;
    let state = BuyerPayInState::OrderAppSynced(create_time);
    let mthd_3pty = ut_default_charge_method_stripe(&create_time);
    let data_lines = vec![
        ((merchant_id, 25, 0), ((219,1), (21900,1), 100), ((219,1), (0,0), 0), 0),
        ((merchant_id, 25, 1), ((225,1), (22500,1), 100), ((225,1), (0,0), 0), 0),
        ((merchant_id, 25, 2), ((235,1), (23500,1), 100), ((235,1), (0,0), 0), 0),
        ((merchant_id, 902, 0), ((3040,2), (304000,2), 100), ((304,1), (0,0), 0), 0),
        ((merchant_id, 29, 0), ((200, 0), (20000, 0), 100), ((200,0), (0,0), 0), 0),
        ((merchant_id, 29, 1), ((201, 0), (12060, 0), 60), ((201,0), (0,0), 0), 0),
        ((merchant_id, 29, 2), ((203, 0), (10150, 0), 50), ((203,0), (0,0), 0), 0),
    ];
    let currency_map = ut_setup_currency_snapshot(vec![buyer_usr_id, merchant_id]);
    ut_setup_buyer_charge(
        buyer_usr_id, create_time, oid.to_string(), state,
        mthd_3pty, data_lines, currency_map,
    )
}

#[allow(clippy::field_reassign_with_default)]
#[rustfmt::skip]
fn ut_setup_refund_model(
    oid: &str,
    time_base: DateTime<Utc>,
    d_lines: Vec<((u32, u64, u16), (i64,u32), (i64,u32), u32, i64)>,
) -> OrderRefundModel {
    let lines = d_lines.into_iter().map(|d| {
        let pid = BaseProductIdentity { store_id: d.0.0, product_id: d.0.1 };
        let attr_seq = d.0.2;
        let amt_req = PayLineAmountModel {
            unit: Decimal::new(d.1.0, d.1.1),
            total: Decimal::new(d.2.0, d.2.1),
            qty: d.3
        };
        let ctime = time_base - Duration::minutes(d.4);
        let mut amt_refunded = PayLineAmountModel::default();
        amt_refunded.unit = amt_req.unit;
        let reject = RefundLineQtyRejectModel::default();
        let args = (pid, attr_seq, amt_req, ctime, amt_refunded, reject);
        OLineRefundModel::from(args)
    }).collect::<Vec<_>>();
    OrderRefundModel::from((oid.to_string(), lines))
} // end of ut_setup_refund_model

#[rustfmt::skip]
fn ut_rslv_rfnd_cb_modify_success<'a>(
    refund_m: &'a mut OrderRefundModel,
    cmplt_req: RefundCompletionReqDto,
    mut charge_ms: Vec<ChargeBuyerModel>,
    _processor: Arc<Box<dyn AbstractPaymentProcessor>>,
) -> Pin<Box<dyn Future<Output = AppRefundRslvReqCbReturn> + Send + 'a>> {
    assert_eq!(refund_m.num_lines(), 6);
    let charge_m = charge_ms.remove(0);
    let merchant_id = charge_m.lines[0].id().0;
    let time_base = DateTime::parse_from_rfc3339("2022-08-31T15:59:38+08:00").unwrap().to_utc();
    assert_eq!(merchant_id, 1066u32);
    let arg = (merchant_id, &charge_m, &cmplt_req);
    let rslv_m = RefundReqResolutionModel::try_from(arg).unwrap();
    [
        (25, 0, 15, 219, 1, 0, 3),
        (25, 0, 85, 438, 2, 0, 0),
        (25, 2, 95, 470, 2, 1, 0),
        (902, 0, 25, 608, 2, 1, 1),
        (902, 0, 11, 0, 0, 1, 0),
        (29, 0, 22, 6000, 3, 0, 0),
    ].into_iter()
        .map(|d| {
            let t_req = time_base  - Duration::minutes(d.2);
            let rline_m = refund_m
                .get_line(merchant_id, d.0, d.1, t_req).unwrap();
            let amt_aprv = rline_m.approved();
            assert_eq!(amt_aprv.total, Decimal::ZERO);
            assert_eq!(amt_aprv.qty, 0);
            let qty_rej = rline_m.rejected().inner_map();
            let n_rej = qty_rej.get(&RefundRejectReasonDto::Damaged).unwrap();
            assert_eq!(n_rej, &0u32);
            let n_rej = qty_rej.get(&RefundRejectReasonDto::Fraudulent).unwrap();
            assert_eq!(n_rej, &0u32);

            let (qty_rej, rslv_amt) = rslv_m
                .get_status(merchant_id, d.0, d.1, t_req).unwrap();
            let n_rej = qty_rej.inner_map().get(&RefundRejectReasonDto::Damaged).unwrap();
            assert_eq!(n_rej, &d.5);
            let n_rej = qty_rej.inner_map().get(&RefundRejectReasonDto::Fraudulent).unwrap();
            assert_eq!(n_rej, &d.6);
            let rslv_amt_accum = rslv_amt.accumulated();
            let rslv_amt_currround = rslv_amt.curr_round();
            assert_eq!(rslv_amt_accum.1, 0); // rejected so far
            assert_eq!(rslv_amt_accum.0.qty, 0); // approved qty so far
            assert_eq!(rslv_amt_accum.0.total, Decimal::ZERO);
            assert_eq!(rslv_amt_currround.total, Decimal::new(d.3, 1));
            assert_eq!(rslv_amt_currround.qty, d.4);
        }).count();
    let num_updated = refund_m.update(&rslv_m);
    assert_eq!(num_updated, 6);
    let fut = async move {
        Ok(vec![Ok(rslv_m)])
    };
    Box::pin(fut)
} // end of fn ut_rslv_rfnd_cb_modify_success

fn ut_rslv_rfnd_cb_verify_modified<'a>(
    refund_m: &'a mut OrderRefundModel,
    _cmplt_req: RefundCompletionReqDto,
    charge_ms: Vec<ChargeBuyerModel>,
    _processor: Arc<Box<dyn AbstractPaymentProcessor>>,
) -> Pin<Box<dyn Future<Output = AppRefundRslvReqCbReturn> + Send + 'a>> {
    assert_eq!(refund_m.num_lines(), 8);
    let merchant_id = charge_ms.get(0).unwrap().lines[0].id().0;
    let time_base = DateTime::parse_from_rfc3339("2022-08-31T15:59:38+08:00")
        .unwrap()
        .to_utc();
    [
        (25, 0, 15, 219, 1, 0, 3),
        (25, 0, 49, 0, 0, 0, 0),
        (25, 0, 85, 438, 2, 0, 0),
        (25, 1, 55, 0, 0, 0, 0),
        (25, 2, 95, 470, 2, 1, 0),
        (902, 0, 25, 608, 2, 1, 1),
        (902, 0, 11, 0, 0, 1, 0),
        (29, 0, 22, 6000, 3, 0, 0),
    ]
    .into_iter()
    .map(|d| {
        let t_req = time_base - Duration::minutes(d.2);
        let rline_m = refund_m.get_line(merchant_id, d.0, d.1, t_req).unwrap();
        let amt_aprv = rline_m.approved();
        assert_eq!(amt_aprv.total, Decimal::new(d.3, 1));
        assert_eq!(amt_aprv.qty, d.4);
        let qty_rej = rline_m.rejected().inner_map();
        let n_rej = qty_rej.get(&RefundRejectReasonDto::Damaged).unwrap();
        assert_eq!(n_rej, &d.5);
        let n_rej = qty_rej.get(&RefundRejectReasonDto::Fraudulent).unwrap();
        assert_eq!(n_rej, &d.6);
    })
    .count();
    let fut = async move { Ok(vec![]) };
    Box::pin(fut)
} // end of fn ut_rslv_rfnd_cb_verify_modified

fn ut_rslv_rfnd_cb_user_error<'a>(
    _refund_m: &'a mut OrderRefundModel,
    _cmplt_req: RefundCompletionReqDto,
    _charge_ms: Vec<ChargeBuyerModel>,
    _processor: Arc<Box<dyn AbstractPaymentProcessor>>,
) -> Pin<Box<dyn Future<Output = AppRefundRslvReqCbReturn> + Send + 'a>> {
    Box::pin(async move {
        let t = Local::now().to_utc();
        let pid = BaseProductIdentity {
            store_id: 1068,
            product_id: 168,
        };
        let attr_set_seq = 0;
        let me = vec![RefundModelError::MissingReqLine(pid, attr_set_seq, t)];
        Err(AppRepoErrorDetail::RefundResolution(me))
    })
}

#[actix_web::test]
async fn update_sync_time_ok() {
    let shr_state = ut_setup_sharestate();
    let repo = ut_setup_db_refund_repo(shr_state).await;

    let result = repo.last_time_synced().await;
    assert!(result.is_ok());
    let option_time = result.unwrap();
    assert!(option_time.is_none());

    let mock_time = Local::now().to_utc().trunc_subsecs(3) - Duration::hours(3);
    let result = repo.update_sycned_time(mock_time).await;
    assert!(result.is_ok());

    let result = repo.last_time_synced().await;
    let option_time = result.unwrap();
    assert!(option_time.is_some());
    let time_read = option_time.unwrap();
    assert_eq!(time_read, mock_time);

    let mock_newtime = mock_time + Duration::minutes(50);
    let result = repo.update_sycned_time(mock_newtime).await;
    assert!(result.is_ok());

    let result = repo.last_time_synced().await;
    let option_time = result.unwrap();
    assert!(option_time.is_some());
    let time_read = option_time.unwrap();
    assert_eq!(time_read, mock_newtime);
} // end of fn update_sync_time_ok

#[rustfmt::skip]
#[actix_web::test]
async fn save_refund_req_ok() {
    let time_now = Local::now().to_utc();
    let shr_state = ut_setup_sharestate();
    let repo = ut_setup_db_refund_repo(shr_state).await;
    let mock_rfd_ms = vec![
        ut_setup_refund_model(
            "0238b874", time_now, vec![
                ((1063, 25, 0), (219, 1), (1971, 1), 9, 15),
                ((1063, 25, 0), (219, 1), (438, 1), 2, 49),
                ((1063, 25, 1), (220, 1), (660, 1), 3, 58),
                ((1063, 2753, 0), (1005, 2), (7035, 2), 7, 15),
            ],
        ),
        ut_setup_refund_model(
            "7e80118273b7", time_now, vec![
                ((1027, 902, 0), (3040, 2), (24320, 2), 8, 20),
                ((1027, 902, 1), (3085, 2), (3085, 2), 1, 189),
                ((1027, 902, 1), (3085, 2), (6170, 2), 2, 554),
                ((1063, 409, 0), (2016, 2), (8064, 2), 4, 53),
                ((1064, 188, 0), (2009, 1), (4018, 1), 2, 36),
            ],
        ),
    ];
    let result = repo.save_request(mock_rfd_ms).await;
    assert!(result.is_ok());
} // end of fn save_refund_req_ok

#[rustfmt::skip]
#[actix_web::test]
async fn update_resolution_ok() {
    let time_base = DateTime::parse_from_rfc3339("2022-08-31T15:59:38.4455+08:00").unwrap().to_utc();
    let mock_merchant_id = 1066u32;
    let mock_oid = "d1e1723e110f";
    let shr_state = ut_setup_sharestate();
    let repo = ut_setup_db_refund_repo(shr_state.clone()).await;
    let mock_rfd_ms = vec![
        ut_setup_refund_model(
            mock_oid, time_base, vec![
                ((mock_merchant_id, 25, 0), (219, 1), (1971, 1), 9, 15),
                ((mock_merchant_id, 25, 0), (219, 1), (438, 1), 2, 49),
                ((mock_merchant_id, 25, 0), (219, 1), (2190, 1), 10, 85),
                ((mock_merchant_id, 25, 1), (225, 1), (450, 1), 2, 55),
                ((mock_merchant_id, 25, 2), (235, 1), (705, 1), 3, 95),
                ((1067, 2753, 0), (1005, 2), (7035, 2), 7, 11),
                ((1067, 2753, 0), (1005, 2), (7035, 2), 7, 24),
                ((1067, 2753, 0), (1005, 2), (5025, 2), 5, 34),
                ((mock_merchant_id, 902, 0), (3040, 2), (24320, 2), 8, 25),
                ((mock_merchant_id, 902, 0), (3040, 2), (6080, 2), 2, 11),
                ((1067, 12753, 0), (2041, 1), (20410, 1), 10, 66),
                ((mock_merchant_id, 29, 0), (200, 0), (1200, 0), 6, 22),
            ],
        ),
    ];
    let result = repo.save_request(mock_rfd_ms).await;
    assert!(result.is_ok());
    
    // the conversion between charge model, completion request DTO, and resolution model
    // , should be verified under model layer, this test case focuses on status update
    // in the database

    let mock_cmplt_req = ut_setup_refund_cmplt_dto(
        time_base, vec![
            ((25,  0), 15, 219, 1, 0, 3),
            ((25,  0), 85, 438, 2, 0, 0),
            ((25,  2), 95, 470, 2, 1, 0),
            ((902, 0), 25, 608, 2, 1, 1),
            ((902, 0), 11, 0, 0, 1, 0),
            ((29,  0), 22, 6000, 3, 0, 0),
        ]
    );
    let result = repo.resolve_request(
        mock_merchant_id,
        mock_cmplt_req,
        vec![_ut_setup_buyer_charge(mock_oid, time_base, mock_merchant_id)],
        shr_state.processor_context(),
        ut_rslv_rfnd_cb_modify_success,
    ).await;
    assert!(result.is_ok());

    let mock_cmplt_req = ut_setup_refund_cmplt_dto(
        time_base, vec![
            ((25, 0), 15, 0, 0, 0, 0),
            ((25, 0), 49, 0, 0, 0, 0),
            ((25, 0), 85, 0, 0, 0, 0),
            ((25, 1), 55, 0, 0, 0, 0),
            ((25, 2), 95, 0, 0, 0, 0),
            ((902, 0), 25, 0, 0, 0, 0),
            ((902, 0), 11, 0, 0, 0, 0),
            ((29,  0), 22, 0, 0, 0, 0),
        ]
    );
    let result = repo.resolve_request(
        mock_merchant_id,
        mock_cmplt_req,
        vec![_ut_setup_buyer_charge(mock_oid, time_base, mock_merchant_id)],
        shr_state.processor_context(),
        ut_rslv_rfnd_cb_verify_modified,
    ).await;
    assert!(result.is_ok());
} // end of fn update_resolution_ok

#[rustfmt::skip]
#[actix_web::test]
async fn update_resolution_err_usr_cb() {
    let time_base = DateTime::parse_from_rfc3339("2022-08-31T11:25:09.234+08:00").unwrap().to_utc();
    let mock_merchant_id = 1068u32;
    let mock_oid = "31e1ac1a6e";
    let shr_state = ut_setup_sharestate();
    let repo = ut_setup_db_refund_repo(shr_state.clone()).await;
    let mock_cmplt_req = ut_setup_refund_cmplt_dto(
        time_base, vec![((168, 0), 5566, 219, 2, 4, 0)]
    );
    let result = repo.resolve_request(
        mock_merchant_id,
        mock_cmplt_req,
        vec![_ut_setup_buyer_charge(mock_oid, time_base, mock_merchant_id)],
        shr_state.processor_context(),
        ut_rslv_rfnd_cb_user_error,
    ).await;
    assert!(result.is_err());
    if let Err(e) = result {
        let cond = matches!(e.detail, AppRepoErrorDetail::RefundResolution(_));
        assert!(cond);
    }
} // end of fn update_resolution_err_usr_cb

#[rustfmt::skip]
#[actix_web::test]
async fn update_resolution_err_corrupted_charge() {
    let time_base = DateTime::parse_from_rfc3339("2022-09-02T23:08:10.0049+05:00").unwrap().to_utc();
    let mock_merchant_id = 1069u32;
    let mock_oids = ["99887755", "99887766"];
    let shr_state = ut_setup_sharestate();
    let repo = ut_setup_db_refund_repo(shr_state.clone()).await;
    let mock_cmplt_req = ut_setup_refund_cmplt_dto(
        time_base, vec![((168, 0), 5566, 2190, 10, 0, 0)]
    );
    let mock_charge_ms = vec![
        _ut_setup_buyer_charge(mock_oids[0], time_base, mock_merchant_id),
        _ut_setup_buyer_charge(mock_oids[1], time_base, mock_merchant_id),
    ];
    let result = repo.resolve_request(
        mock_merchant_id,
        mock_cmplt_req,
        mock_charge_ms,
        shr_state.processor_context(),
        ut_rslv_rfnd_cb_user_error,
    ).await;
    assert!(result.is_err());
    if let Err(e) = result {
        if let AppRepoErrorDetail::OrderIDparse(s) = e.detail {
            assert_eq!(s.as_str(), "order-ids-not-consistent");
        } else {
            assert!(false);
        }
    }
} // end of fn update_resolution_err_corrupted_charge
