use chrono::{Duration, Local};

use ecommerce_common::api::dto::CountryCode;
use payment::model::{Merchant3partyModel, MerchantProfileModel, StripeAccountCapableState};

use crate::dto::{ut_default_store_onboard_req_stripe, ut_setup_storeprofile_dto};
use crate::model::ut_default_merchant_3party_stripe;
use crate::ut_setup_sharestate;

use super::ut_setup_db_merchant_repo;

#[rustfmt::skip]
#[actix_web::test]
async fn create_profile_3party_ok() {
    let shr_state = ut_setup_sharestate();
    let repo = ut_setup_db_merchant_repo(shr_state).await;
    let mock_start_time = Local::now().to_utc() - Duration::minutes(2);
    let mock_store_ids = [1234u32, 6789];
    
    // --- sub case 1 , create
    let mock_mprof = {
        let mock_storeprof_d =
            ut_setup_storeprofile_dto("homebox local", 1001, vec![1002, 1003, 1008], mock_start_time);
        let arg = (mock_store_ids[0], &mock_storeprof_d);
        MerchantProfileModel::try_from(arg).unwrap()
    };
    let mock_m3pty = {
        let ms = ut_default_merchant_3party_stripe();
        Merchant3partyModel::Stripe(ms)
    };
    let result = repo.create(mock_mprof, mock_m3pty).await;
    assert!(result.is_ok());

    let mock_mprof = {
        let mock_storeprof_d =
            ut_setup_storeprofile_dto("disguist master", 1005, vec![1002, 1006, 1007], mock_start_time);
        let arg = (mock_store_ids[1], &mock_storeprof_d);
        MerchantProfileModel::try_from(arg).unwrap()
    };
    let mock_m3pty = {
        let mut ms = ut_default_merchant_3party_stripe();
        ms.tos_accepted = None;
        ms.country = CountryCode::US;
        ms.capabilities.transfers = StripeAccountCapableState::pending;
        ms.settings.payout_delay_days = 2;
        ms.settings.payout_interval = "weekly".to_string();
        ms.details_submitted = true;
        Merchant3partyModel::Stripe(ms)
    };
    let result = repo.create(mock_mprof, mock_m3pty).await;
    assert!(result.is_ok());
    
    // --- sub case 2 , fetch  and verify
    let mock_3pty_req = ut_default_store_onboard_req_stripe();
    let expect_data = [
        (mock_store_ids[0], "homebox local", 1001u32, 1002u32, CountryCode::ID, "daily", 7i16, false),
        (mock_store_ids[1], "disguist master", 1005, 1007, CountryCode::US, "weekly", 2i16, true),
    ];
    for expect_item in expect_data {
        let result = repo.fetch(expect_item.0, &mock_3pty_req).await;
        assert!(result.is_ok());
        if let Ok(opt_v) = result {
            assert!(opt_v.is_some());
            if let Some((saved_mprof, saved_m3pty)) = opt_v {
                assert_eq!(saved_mprof.name(), expect_item.1);
                assert!(saved_mprof.valid_supervisor(expect_item.2));
                assert!(saved_mprof.valid_staff(expect_item.3));
                if let Merchant3partyModel::Stripe(ms) = saved_m3pty {
                    assert_eq!(String::from(ms.country), String::from(expect_item.4));
                    assert_eq!(ms.settings.payout_interval.as_str(), expect_item.5);
                    assert_eq!(ms.settings.payout_delay_days, expect_item.6);
                    assert_eq!(ms.details_submitted, expect_item.7);
                } else {
                    assert!(false);
                }
            }
        }
    }
} // end of fn create_profile_3party_ok
