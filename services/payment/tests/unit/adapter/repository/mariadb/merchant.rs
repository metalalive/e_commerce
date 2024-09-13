use chrono::{Duration, Local};

use ecommerce_common::api::dto::CountryCode;
use payment::model::{
    Merchant3partyModel, Merchant3partyStripeModel, MerchantProfileModel, StripeAccountCapableState,
};

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
        ms.settings.payout_delay_days = 2;
        ms.settings.payout_interval = "weekly".to_string();
        ms.details_submitted = true;
        Merchant3partyModel::Stripe(ms)
    };
    let result = repo.create(mock_mprof, mock_m3pty).await;
    assert!(result.is_ok());
    
    // --- sub case 2 , fetch  and verify
    let mut saved_m3parties: Vec<(u32, Merchant3partyStripeModel)> = Vec::new();
    let mock_3pty_req = ut_default_store_onboard_req_stripe();
    let expect_data = [
        (mock_store_ids[0], "homebox local", 1001u32, 1002u32, CountryCode::ID,
         "daily", 7i16, false, false, false),
        (mock_store_ids[1], "disguist master", 1005, 1007, CountryCode::US,
         "weekly", 2i16, true, false, false),
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
                    assert_eq!(String::from(ms.country.clone()), String::from(expect_item.4));
                    assert_eq!(ms.settings.payout_interval.as_str(), expect_item.5);
                    assert_eq!(ms.settings.payout_delay_days, expect_item.6);
                    assert_eq!(ms.details_submitted, expect_item.7);
                    assert_eq!(ms.payouts_enabled, expect_item.8);
                    assert_eq!(ms.charges_enabled, expect_item.9);
                    let cond = matches!(ms.capabilities.transfers, StripeAccountCapableState::inactive);
                    assert!(cond);
                    saved_m3parties.push((expect_item.0, ms));
                } else {
                    assert!(false);
                }
            }
        }
    } // end of for loop
    
    // --- sub case 3 , fetch and update
    assert_eq!(saved_m3parties.len(), 2);
    {
        let (store_id, mut ms) = saved_m3parties.remove(0);
        assert_eq!(store_id, mock_store_ids[0]);
        ms.details_submitted = true;
        ms.payouts_enabled = true;
        ms.settings.payout_delay_days = 6;
        ms.capabilities.transfers = StripeAccountCapableState::active;
        let saved_m3pty = Merchant3partyModel::Stripe(ms);
        let result = repo.update_3party(store_id, saved_m3pty).await;
        assert!(result.is_ok());
    }
    
    // --- sub case 4 , fetch  and verify
    let result = repo.fetch(mock_store_ids[0], &mock_3pty_req).await;
    assert!(result.is_ok());
    if let Ok(opt_v) = result {
        assert!(opt_v.is_some());
        if let Some((_saved_mprof, saved_m3pty)) = opt_v {
            if let Merchant3partyModel::Stripe(ms) = saved_m3pty {
                assert_eq!(ms.settings.payout_interval.as_str(), "daily");
                assert_eq!(ms.settings.payout_delay_days, 6i16);
                assert_eq!(ms.details_submitted, true);
                assert_eq!(ms.payouts_enabled, true);
                assert_eq!(ms.charges_enabled, false);
                let cond = matches!(ms.capabilities.transfers, StripeAccountCapableState::active);
                assert!(cond);
            } else {
                assert!(false);
            }
        }
    }
} // end of fn create_profile_3party_ok
