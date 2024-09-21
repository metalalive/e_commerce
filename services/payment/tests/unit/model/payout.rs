use std::collections::HashMap;

use chrono::{DateTime, Duration, Local, Utc};
use rust_decimal::Decimal;

use ecommerce_common::{api::dto::CurrencyDto, constant::ProductType, error::AppErrorCode};
use payment::model::{
    BuyerPayInState, Charge3partyModel, ChargeBuyerModel, Merchant3partyModel,
    MerchantProfileModel, OrderCurrencySnapshot, PayoutModel, PayoutModelError,
    StripeAccountCapableState, StripeCheckoutPaymentStatusModel,
};

use super::{
    ut_default_charge_method_stripe, ut_default_merchant_3party_stripe, ut_setup_buyer_charge,
};
use crate::dto::ut_setup_storeprofile_dto;

#[rustfmt::skip]
pub(crate) fn ut_setup_buyer_charge_inner(
    buyer_usr_id: u32,
    charge_ctime: DateTime<Utc>,
    pay_in_state : BuyerPayInState,
) -> ChargeBuyerModel {
    let merchant_ids = [1008u32, 1009];
    let order_id = "overlapping-camera-calibrate".to_string();
    let charge_lines = vec![
        (merchant_ids[0], ProductType::Item, 19030u64, (502i64, 1u32), (2510i64, 1u32), 5u32,),
        (merchant_ids[1], ProductType::Item, 9451, (1700, 1), (12600, 1), 8,),
        (merchant_ids[0], ProductType::Package, 6763, (9900, 2), (49500, 2), 5,),
        (merchant_ids[1], ProductType::Package, 8454, (3760, 1), (37600, 1), 10,),
        (merchant_ids[0], ProductType::Item, 9925, (411, 1), (3699, 1), 9,),
        (merchant_ids[1], ProductType::Item, 9914, (226, 0), (2486, 0), 11,),
    ];
    let currency_snapshot = {
        let iter = [
            (buyer_usr_id, CurrencyDto::TWD, (3185i64, 2u32)),
            (merchant_ids[1], CurrencyDto::IDR, (123451, 1)),
            (merchant_ids[0], CurrencyDto::THB, (393, 1)),
        ]
        .map(|(usr_id, label, ratescalar)| {
            let rate = Decimal::new(ratescalar.0, ratescalar.1);
            let obj = OrderCurrencySnapshot { label, rate };
            (usr_id, obj)
        });
        HashMap::from_iter(iter)
    };
    let paymethod = {
        let mut mthd = ut_default_charge_method_stripe(&charge_ctime);
        if let Charge3partyModel::Stripe(s) = &mut mthd {
            s.payment_state = StripeCheckoutPaymentStatusModel::unpaid;
        }
        mthd
    };
    ut_setup_buyer_charge(
        buyer_usr_id,
        charge_ctime,
        order_id,
        pay_in_state,
        paymethod,
        charge_lines,
        currency_snapshot,
    )
} // end of fn ut_setup_buyer_charge_inner

pub(crate) fn ut_setup_merchant_profile(
    mock_store_id: u32,
    storestaff_id: u32,
) -> MerchantProfileModel {
    let start_time = Local::now().to_utc() - Duration::minutes(2);
    let mock_storeprof = ut_setup_storeprofile_dto(
        "cement tile",
        126u32,
        vec![storestaff_id, 573, 482],
        start_time,
    );
    let arg = (mock_store_id, &mock_storeprof);
    let result = MerchantProfileModel::try_from(arg);
    result.unwrap()
}

pub(crate) fn ut_setup_merchant_3party_stripe() -> Merchant3partyModel {
    let mut ms = ut_default_merchant_3party_stripe();
    ms.capabilities.transfers = StripeAccountCapableState::active;
    ms.payouts_enabled = true;
    ms.charges_enabled = true;
    ms.details_submitted = true;
    Merchant3partyModel::Stripe(ms)
}

pub(crate) fn ut_common_create_first_payout(
    buyer_usr_id: u32,
    mock_store_id: u32,
    staff_usr_id: u32,
    charge_ctime: DateTime<Utc>,
) -> Result<PayoutModel, PayoutModelError> {
    let done_time = charge_ctime + Duration::minutes(15);
    let payin_state = BuyerPayInState::OrderAppSynced(done_time);
    let mock_charge_m = ut_setup_buyer_charge_inner(buyer_usr_id, charge_ctime, payin_state);
    let mock_merchant_prof = ut_setup_merchant_profile(mock_store_id, staff_usr_id);
    let mock_merchant_3pty = ut_setup_merchant_3party_stripe();
    let arg = (
        mock_charge_m,
        mock_merchant_prof,
        mock_merchant_3pty,
        staff_usr_id,
        None,
    );
    PayoutModel::try_from(arg)
}

#[test]
fn create_ok() {
    let mock_buyer_id = 518u32;
    let mock_store_id = 1009u32;
    let staff_usr_id = 2074u32;
    let charge_ctime = Local::now().to_utc() - Duration::minutes(96);
    let result =
        ut_common_create_first_payout(mock_buyer_id, mock_store_id, staff_usr_id, charge_ctime);
    assert!(result.is_ok());
    if let Ok(v) = result {
        let readback_store_id = v.merchant_id();
        assert_eq!(readback_store_id, mock_store_id);
        let (total, exrate, currency) = v.amount_merchant();
        assert_eq!(currency.label, CurrencyDto::IDR);
        assert_eq!(currency.rate.to_string().as_str(), "12345.1");
        assert_eq!(exrate.to_string().as_str(), "387.60125588");
        assert_eq!(total.to_string().as_str(), "2909335.02");
    }
}

#[rustfmt::skip]
#[test]
fn create_merchant_id_mismatch() {
    let (mock_buyer_id, orig_store_id, staff_usr_id) = (518u32, 1009u32, 2074u32);
    let charge_ctime = Local::now().to_utc() - Duration::minutes(96);
    let result = ut_common_create_first_payout(
        mock_buyer_id, orig_store_id, staff_usr_id, charge_ctime
    );
    let valid_payout = result.unwrap();
    let wrong_store_id = 1008u32;
    let payin_state = BuyerPayInState::OrderAppSynced(charge_ctime + Duration::minutes(5));
    let mock_charge_m = ut_setup_buyer_charge_inner(mock_buyer_id, charge_ctime, payin_state);
    let mock_merchant_prof = ut_setup_merchant_profile(wrong_store_id, staff_usr_id);
    let mock_merchant_3pty = ut_setup_merchant_3party_stripe();
    let arg = (
        mock_charge_m, mock_merchant_prof, mock_merchant_3pty,
        staff_usr_id, Some(valid_payout),
    );
    let result = PayoutModel::try_from(arg);
    assert!(result.is_err());
    if let Err(e) = result {
        if let PayoutModelError::MerchantInconsistent(sid0, sid1) = e {
            assert_eq!(sid0, orig_store_id);
            assert_eq!(sid1, wrong_store_id);
        } else {
            assert!(false);
        }
    }
}

#[rustfmt::skip]
#[test]
fn create_charge_id_mismatch() {
    let (orig_buyer_id, mock_store_id, staff_usr_id) = (518u32, 1009u32, 2074u32);
    let charge_ctime = Local::now().to_utc() - Duration::minutes(96);
    let result = ut_common_create_first_payout(
        orig_buyer_id, mock_store_id, staff_usr_id, charge_ctime
    );
    let valid_payout = result.unwrap();
    let wrong_buyer_id = 5118u32;
    let payin_state = BuyerPayInState::OrderAppSynced(charge_ctime + Duration::minutes(5));
    let mock_charge_m = ut_setup_buyer_charge_inner(wrong_buyer_id, charge_ctime, payin_state);
    let mock_merchant_prof = ut_setup_merchant_profile(mock_store_id, staff_usr_id);
    let mock_merchant_3pty = ut_setup_merchant_3party_stripe();
    let arg = (
        mock_charge_m, mock_merchant_prof, mock_merchant_3pty,
        staff_usr_id, Some(valid_payout),
    );
    let result = PayoutModel::try_from(arg);
    assert!(result.is_err());
    if let Err(e) = result {
        if let PayoutModelError::BuyerInconsistent(cid0, cid1) = e {
            assert_eq!(cid0, orig_buyer_id);
            assert_eq!(cid1, wrong_buyer_id);
        } else {
            assert!(false);
        }
    }
}

#[rustfmt::skip]
#[test]
fn create_err_merchant_no_permit() {
    let (mock_buyer_id, mock_store_id, staff_usr_id) = (518u32, 1009u32, 2074u32);
    let charge_ctime = Local::now().to_utc() - Duration::minutes(96);
    let payin_state = BuyerPayInState::OrderAppSynced(charge_ctime + Duration::minutes(5));
    let mock_charge_m = ut_setup_buyer_charge_inner(mock_buyer_id, charge_ctime, payin_state);
    let mock_merchant_prof = ut_setup_merchant_profile(mock_store_id, staff_usr_id);
    let mock_merchant_3pty = {
        let ms = ut_default_merchant_3party_stripe();
        Merchant3partyModel::Stripe(ms)
    }; // assume 3rd-party Stripe hasn't enabled the payout uet
    let arg = (
        mock_charge_m, mock_merchant_prof, mock_merchant_3pty,
        staff_usr_id, None,
    );
    let result = PayoutModel::try_from(arg);
    assert!(result.is_err());
    if let Err(e) = result {
        if let PayoutModelError::MerchantPermissionDenied(sid) = e {
            assert_eq!(sid, mock_store_id);
        } else {
            assert!(false);
        }
    }
}

#[rustfmt::skip]
#[test]
fn create_missing_currency_snapshot() {
    let (mock_buyer_id, wrong_store_id, staff_usr_id) = (518u32, 9999u32, 2074u32);
    let charge_ctime = Local::now().to_utc() - Duration::minutes(96);
    let payin_state = BuyerPayInState::OrderAppSynced(charge_ctime + Duration::minutes(5));
    let mock_charge_m = ut_setup_buyer_charge_inner(mock_buyer_id, charge_ctime, payin_state);
    let mock_merchant_prof = ut_setup_merchant_profile(wrong_store_id, staff_usr_id);
    let mock_merchant_3pty = ut_setup_merchant_3party_stripe();
    let arg = (
        mock_charge_m, mock_merchant_prof, mock_merchant_3pty,
        staff_usr_id, None,
    );
    let result = PayoutModel::try_from(arg);
    assert!(result.is_err());
    if let Err(e) = result {
        if let PayoutModelError::AmountEstimate(code, detail) = e {
            assert_eq!(code, AppErrorCode::DataCorruption);
            assert_eq!(detail.as_str(), "missing-currency-seller");
        } else {
            assert!(false);
        }
    }
}

#[rustfmt::skip]
#[test]
fn create_err_invalid_amount() {
    let (mock_buyer_id, mock_store_id, staff_usr_id) = (518u32, 1009u32, 2074u32);
    let charge_ctime = Local::now().to_utc() - Duration::minutes(96);
    let result = ut_common_create_first_payout(
        mock_buyer_id, mock_store_id, staff_usr_id, charge_ctime
    );
    let valid_payout = result.unwrap();
    let payin_state = BuyerPayInState::OrderAppSynced(charge_ctime + Duration::minutes(5));
    let mock_charge_m = ut_setup_buyer_charge_inner(mock_buyer_id, charge_ctime, payin_state);
    let mock_merchant_prof = ut_setup_merchant_profile(mock_store_id, staff_usr_id);
    let mock_merchant_3pty = ut_setup_merchant_3party_stripe();
    let arg = (
        mock_charge_m, mock_merchant_prof, mock_merchant_3pty,
        staff_usr_id, Some(valid_payout),
    );
    let result = PayoutModel::try_from(arg);
    assert!(result.is_err());
    if let Err(e) = result {
        if let PayoutModelError::AmountNotEnough(amt_done, amt_req) = e {
            assert_eq!(amt_done.to_string().as_str(), "2909335.02");
            assert_eq!(amt_req.to_string().as_str(), "2909335.02");
        } else {
            assert!(false);
        }
    }
}

#[rustfmt::skip]
#[test]
fn create_err_3party_mismatch() {
    // charge 3party and merchant 3party mismatch
    let (mock_buyer_id, mock_store_id, staff_usr_id) = (518u32, 1009u32, 2074u32);
    let charge_ctime = Local::now().to_utc() - Duration::minutes(88);
    let mock_charge_m = {
        let payin_state = BuyerPayInState::OrderAppSynced(charge_ctime + Duration::minutes(5));
        let mut cm = ut_setup_buyer_charge_inner(mock_buyer_id, charge_ctime, payin_state);
        cm.meta.update_3party(Charge3partyModel::Unknown);
        cm
    };
    let mock_merchant_prof = ut_setup_merchant_profile(mock_store_id, staff_usr_id);
    let mock_merchant_3pty = ut_setup_merchant_3party_stripe();
    let arg = (
        mock_charge_m, mock_merchant_prof, mock_merchant_3pty,
        staff_usr_id, None,
    );
    let result = PayoutModel::try_from(arg);
    assert!(result.is_err());
    if let Err(e) = result {
        let cond = matches!(e, PayoutModelError::Init3partyFailure);
        assert!(cond);
    }
}

#[test]
fn create_err_3party_stripe_tx_grp_mismatch() {
    // exising payout 3party and charge 3party mismatch
    let (mock_buyer_id, mock_store_id, staff_usr_id) = (518u32, 1009u32, 2074u32);
    let charge_ctime = Local::now().to_utc() - Duration::minutes(96);
    let result =
        ut_common_create_first_payout(mock_buyer_id, mock_store_id, staff_usr_id, charge_ctime);
    let valid_payout = result.unwrap();
    let mock_charge_m = {
        let payin_state = BuyerPayInState::OrderAppSynced(charge_ctime + Duration::minutes(5));
        let mut cm = ut_setup_buyer_charge_inner(mock_buyer_id, charge_ctime, payin_state);
        let paymethod = {
            let mut mthd = ut_default_charge_method_stripe(&charge_ctime);
            if let Charge3partyModel::Stripe(s) = &mut mthd {
                s.transfer_group = "fake-another-transfer-group".to_string();
                s.payment_state = StripeCheckoutPaymentStatusModel::unpaid;
            }
            mthd
        };
        cm.meta.update_3party(paymethod);
        cm
    };
    let mock_merchant_prof = ut_setup_merchant_profile(mock_store_id, staff_usr_id);
    let mock_merchant_3pty = ut_setup_merchant_3party_stripe();
    let arg = (
        mock_charge_m,
        mock_merchant_prof,
        mock_merchant_3pty,
        staff_usr_id,
        Some(valid_payout),
    );
    let result = PayoutModel::try_from(arg);
    assert!(result.is_err());
    if let Err(e) = result {
        let cond = matches!(e, PayoutModelError::Invalid3partyParams(_));
        assert!(cond);
    }
} // end of fn create_err_3party_stripe_tx_grp_mismatch
