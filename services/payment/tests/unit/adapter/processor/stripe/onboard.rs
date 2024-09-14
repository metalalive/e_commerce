use std::fs::File;

use chrono::Local;
use payment::AppSharedState;
use serde_json::Value as JsnVal;

use ecommerce_common::api::dto::CountryCode;
use payment::adapter::processor::AppProcessorErrorReason;
use payment::api::web::dto::StoreOnboardRespDto;
use payment::model::{Merchant3partyModel, StripeAccountCapableState};

use crate::dto::{ut_default_store_onboard_req_stripe, ut_setup_storeprofile_dto};
use crate::model::ut_default_merchant_3party_stripe;
use crate::{ut_setup_sharestate, EXAMPLE_REL_PATH};

#[actix_web::test]
async fn create_merchant_account_ok() {
    let shr_state = ut_setup_sharestate();
    let proc_ctx = shr_state.processor_context();

    let mock_shop_name = "pet finder";
    let mock_shop_owner_id = 134;
    let mock_shop_staff_ids = vec![241, 242, 248, 249];
    let common_start_time = Local::now().to_utc();
    let mock_profile = ut_setup_storeprofile_dto(
        mock_shop_name,
        mock_shop_owner_id,
        mock_shop_staff_ids,
        common_start_time,
    );
    let mock_req_3pt = ut_default_store_onboard_req_stripe();
    let result = proc_ctx.onboard_merchant(mock_profile, mock_req_3pt).await;
    assert!(result.is_ok());
    if let Ok(v) = result {
        let (respdto, m3pty) = v.into_parts();
        if let Merchant3partyModel::Stripe(s) = m3pty {
            let cond = matches!(s.country, CountryCode::TW);
            assert!(cond);
            assert!(s.id.starts_with("acct_"));
            assert!(!s.charges_enabled);
            assert!(!s.details_submitted);
            assert!(!s.payouts_enabled);
            let cond = matches!(
                s.capabilities.transfers,
                StripeAccountCapableState::inactive
            );
            assert!(cond);
            assert_eq!(s.settings.payout_interval.as_str(), "daily");
            let link_m = s.update_link.as_ref().unwrap();
            assert!(!link_m.url.is_empty());
        } else {
            assert!(false);
        }
        if let StoreOnboardRespDto::Stripe {
            fields_required,
            disabled_reason,
            url,
            expiry,
        } = respdto
        {
            assert!(!fields_required.is_empty());
            assert!(disabled_reason.is_some());
            let expiry = expiry.unwrap();
            assert!(expiry > common_start_time);
            let url = url.unwrap();
            assert!(!url.is_empty());
        } else {
            assert!(false);
        }
    }
} // end of create_merchant_account_ok

#[actix_web::test]
async fn create_merchant_profile_error() {
    let shr_state = ut_setup_sharestate();
    let proc_ctx = shr_state.processor_context();

    let mock_shop_name = "pet finder";
    let mock_shop_owner_id = 134;
    let mock_shop_staff_ids = vec![241, 242, 248, 249];
    let common_start_time = Local::now().to_utc();
    let mut mock_profile = ut_setup_storeprofile_dto(
        mock_shop_name,
        mock_shop_owner_id,
        mock_shop_staff_ids,
        common_start_time,
    );
    mock_profile.location = None;
    mock_profile.phones = None;
    let mock_req_3pt = ut_default_store_onboard_req_stripe();
    let result = proc_ctx.onboard_merchant(mock_profile, mock_req_3pt).await;
    assert!(result.is_err());
    if let Err(e) = result {
        if let AppProcessorErrorReason::InvalidStoreProfileDto(es) = e.reason {
            let cond = es.contains(&"missing-location-addr".to_string());
            assert!(cond);
            let cond = es.contains(&"missing-phone".to_string());
            assert!(cond);
        } else {
            assert!(false);
        }
    }
} // end of create_merchant_profile_error

#[rustfmt::skip]
fn ut_setup_merchant_3party_model(shr_state: AppSharedState, label: &str) -> Merchant3partyModel {
    let acfg = shr_state.config();
    let path =
        acfg.basepath.service.clone() + EXAMPLE_REL_PATH + "processor-stripe-account-ids.json";
    let f = File::open(path).unwrap();
    let account_ids = serde_json::from_reader::<File, JsnVal>(f).unwrap();
    let chosen_account = account_ids
        .as_object()
        .unwrap()
        .get(label)
        .unwrap()
        .as_str()
        .unwrap();
    let mut m = ut_default_merchant_3party_stripe();
    m.id = chosen_account.to_string();
    Merchant3partyModel::Stripe(m)
}

#[actix_web::test]
async fn refresh_merchant_status_complete() {
    let shr_state = ut_setup_sharestate();
    let proc_ctx = shr_state.processor_context();
    let mock_old_m3pty = ut_setup_merchant_3party_model(shr_state.clone(), "onboard_complete");
    let mock_req_3pt = ut_default_store_onboard_req_stripe();
    let result = proc_ctx
        .refresh_onboard_status(mock_old_m3pty, mock_req_3pt)
        .await;
    assert!(result.is_ok());
    if let Ok(v) = result {
        let (d_3pt, m_3pt) = v.into_parts();
        if let StoreOnboardRespDto::Stripe {
            fields_required,
            disabled_reason,
            url,
            expiry,
        } = d_3pt
        {
            assert!(fields_required.is_empty());
            assert!(disabled_reason.is_none());
            assert!(url.is_none());
            assert!(expiry.is_none());
        } else {
            assert!(false);
        }
        if let Merchant3partyModel::Stripe(v) = m_3pt {
            assert!(v.details_submitted);
            assert!(v.payouts_enabled);
            assert!(v.tos_accepted.is_some());
            assert!(v.update_link.is_none());
            let cond = matches!(v.capabilities.transfers, StripeAccountCapableState::active);
            assert!(cond);
        } else {
            assert!(false);
        }
    }
} // end of refresh_merchant_status_complete

#[actix_web::test]
async fn refresh_merchant_status_renew_acctlink() {
    let shr_state = ut_setup_sharestate();
    let proc_ctx = shr_state.processor_context();
    let mock_old_m3pty = ut_setup_merchant_3party_model(shr_state.clone(), "onboarding");
    let mock_req_3pt = ut_default_store_onboard_req_stripe();
    let result = proc_ctx
        .refresh_onboard_status(mock_old_m3pty, mock_req_3pt)
        .await;
    assert!(result.is_ok());
    if let Ok(v) = result {
        let (d_3pt, m_3pt) = v.into_parts();
        if let StoreOnboardRespDto::Stripe {
            fields_required,
            disabled_reason,
            url,
            expiry,
        } = d_3pt
        {
            assert!(!fields_required.is_empty());
            assert!(disabled_reason.is_some());
            assert!(url.is_some());
            assert!(expiry.is_some());
        } else {
            assert!(false);
        }
        if let Merchant3partyModel::Stripe(v) = m_3pt {
            assert!(!v.details_submitted);
            assert!(!v.payouts_enabled);
            assert!(v.tos_accepted.is_none());
            assert!(v.update_link.is_some());
            let cond = matches!(
                v.capabilities.transfers,
                StripeAccountCapableState::inactive
            );
            assert!(cond);
        } else {
            assert!(false);
        }
    }
} // end of refresh_merchant_status_renew_acctlink
