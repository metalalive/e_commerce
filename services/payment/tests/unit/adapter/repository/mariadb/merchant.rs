use chrono::{Duration, Local};

use payment::model::{Merchant3partyModel, MerchantProfileModel, StripeAccountCapableState};

use crate::dto::ut_setup_storeprofile_dto;
use crate::model::ut_default_merchant_3party_stripe;
use crate::ut_setup_sharestate;

use super::ut_setup_db_merchant_repo;

#[actix_web::test]
async fn create_profile_3party_ok() {
    let shr_state = ut_setup_sharestate();
    let repo = ut_setup_db_merchant_repo(shr_state).await;
    let mock_start_time = Local::now().to_utc() - Duration::minutes(2);
    let mock_store_ids = [1234u32, 6789];
    let mock_mprof = {
        let mock_storeprof_d =
            ut_setup_storeprofile_dto("homebox local", 1001, vec![1002, 1003], mock_start_time);
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
            ut_setup_storeprofile_dto("disguist master", 1005, vec![1002, 1006], mock_start_time);
        let arg = (mock_store_ids[1], &mock_storeprof_d);
        MerchantProfileModel::try_from(arg).unwrap()
    };
    let mock_m3pty = {
        let mut ms = ut_default_merchant_3party_stripe();
        ms.tos_accepted = None;
        ms.capabilities.transfers = StripeAccountCapableState::pending;
        Merchant3partyModel::Stripe(ms)
    };
    let result = repo.create(mock_mprof, mock_m3pty).await;
    assert!(result.is_ok());
    // TODO, fetch  and verify models
} // end of fn create_profile_3party_ok
