use chrono::{Duration, Local, SubsecRound};

use super::ut_setup_db_refund_repo;
use crate::ut_setup_sharestate;

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
