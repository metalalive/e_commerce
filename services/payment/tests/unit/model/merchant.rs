use chrono::{Duration, Local};
use payment::model::{MerchantModelError, MerchantProfileModel};

use crate::dto::ut_setup_storeprofile_dto;

#[test]
fn create_ok() {
    let start_time = Local::now().to_utc() - Duration::minutes(1);
    let mock_store_id = 374u32;
    let mock_storeprof =
        ut_setup_storeprofile_dto("border guardian", 126u32, vec![573, 482], start_time);
    let arg = (mock_store_id, &mock_storeprof);
    let result = MerchantProfileModel::try_from(arg);
    if let Ok(v) = result {
        assert!(v.valid_staff(482));
        assert!(v.valid_staff(573));
    } // TODO, examine more fields
}

#[test]
fn create_err_inactive() {
    let start_time = Local::now().to_utc() - Duration::minutes(1);
    let mock_store_id = 374u32;
    let expect_staff_id = 573u32;
    let mut mock_storeprof = ut_setup_storeprofile_dto(
        "border guardian",
        126u32,
        vec![expect_staff_id, 482],
        start_time,
    );
    mock_storeprof.active = false;
    let arg = (mock_store_id, &mock_storeprof);
    let result = MerchantProfileModel::try_from(arg);
    assert!(result.is_err());
    if let Err(e) = result {
        let cond = matches!(e, MerchantModelError::InActive);
        assert!(cond);
    }
}

#[test]
fn create_err_staff_time_corrupted() {
    let start_time = Local::now().to_utc() - Duration::minutes(1);
    let mock_store_id = 374u32;
    let expect_staff_id = 573u32;
    let mut mock_storeprof = ut_setup_storeprofile_dto(
        "border guardian",
        126u32,
        vec![expect_staff_id, 482],
        start_time,
    );
    {
        let vs = mock_storeprof.staff.as_mut().unwrap();
        vs[0].start_after = "YYYY-MM-dd hh:mm:ss+gmt".to_string();
        assert_eq!(vs[0].staff_id, expect_staff_id);
    }
    let arg = (mock_store_id, &mock_storeprof);
    let result = MerchantProfileModel::try_from(arg);
    assert!(result.is_err());
    if let Err(e) = result {
        if let MerchantModelError::StaffCorruptedTime(mut es) = e {
            let (actual_staff_id, _start_after) = es.remove(0);
            assert_eq!(actual_staff_id, expect_staff_id);
        } else {
            assert!(false);
        }
    }
}

#[test]
fn create_skip_staff_time_expired() {
    let start_time = Local::now().to_utc() - Duration::minutes(1);
    let mock_store_id = 374u32;
    let expect_staff_id = 573u32;
    let mut mock_storeprof = ut_setup_storeprofile_dto(
        "border guardian",
        126u32,
        vec![expect_staff_id, 482],
        start_time,
    );
    {
        let vs = mock_storeprof.staff.as_mut().unwrap();
        let start_after = start_time - Duration::hours(2);
        let end_before = start_time - Duration::hours(1);
        vs[0].start_after = start_after.to_rfc3339();
        vs[0].end_before = end_before.to_rfc3339();
        assert_eq!(vs[0].staff_id, expect_staff_id);
    }
    let arg = (mock_store_id, &mock_storeprof);
    let result = MerchantProfileModel::try_from(arg);
    assert!(result.is_ok());
    if let Ok(v) = result {
        assert!(!v.valid_staff(expect_staff_id));
    }
}
