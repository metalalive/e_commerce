use super::ut_clone_productpolicy;
use order::api::web::dto::ProductPolicyDto;
use order::model::{ProductPolicyModel, ProductPolicyModelSet};

#[test]
fn validate_newdata_ok() {
    let newdata = [
        (123u64, None, 480u32, 3600u32, None),
        (124, None, 478, 3597, None),
    ]
    .into_iter()
    .map(|d| ProductPolicyDto {
        product_id: d.0,
        min_num_rsv: d.1,
        warranty_hours: d.2,
        auto_cancel_secs: d.3,
        max_num_rsv: d.4,
    })
    .collect::<Vec<_>>();
    let result = ProductPolicyModelSet::validate(&newdata);
    assert_eq!(result.is_ok(), true)
}

#[test]
fn validate_newdata_error_limit() {
    let newdata = vec![
        (170u64, Some(5u16), 500u32, 360u32, None),
        (127, None, 500, 360, None),
        (123, None, 0x7fff_ffffu32, 3600, None),
        (124, Some(12), 478, 0x7fff_ffffu32, Some(2u16)),
    ]
    .into_iter()
    .map(|d| ProductPolicyDto {
        product_id: d.0,
        min_num_rsv: d.1,
        warranty_hours: d.2,
        auto_cancel_secs: d.3,
        max_num_rsv: d.4,
    })
    .collect::<Vec<_>>();

    let result = ProductPolicyModelSet::validate(&newdata);
    assert_eq!(result.is_err(), true);
    let error = result.err().unwrap();
    assert_eq!(error.len(), 3);
    {
        let ce = error.iter().find(|item| item.product_id == 123).unwrap();
        let detail = ce.warranty_hours.as_ref().unwrap();
        assert_eq!(ce.err_type, "ExceedingMaxLimit");
        assert!(detail.given > detail.limit);

        let ce = error.iter().find(|item| item.product_id == 124).unwrap();
        let detail = ce.auto_cancel_secs.as_ref().unwrap();
        assert_eq!(ce.err_type, "ExceedingMaxLimit");
        assert!(detail.given > detail.limit);
        let detail = ce.num_rsv.as_ref().unwrap();
        assert!(detail.min_items > detail.max_items);

        let ce = error.iter().find(|item| item.product_id == 170).unwrap();
        let detail = ce.num_rsv.as_ref().unwrap();
        assert!(detail.min_items > detail.max_items);
    }
} // end of fn validate_newdata_error_limit

#[test]
fn update_instance_ok() {
    let init_data = [
        (20903u64, 731u32, 271u32, false, 0u16, 0u16),
        (144, 380, 30098, false, 8, 0),
        (144, 3597, 478, false, 0, 0),
        (123, 3600, 480, true, 26, 15),
    ]
    .into_iter()
    .map(|d| ProductPolicyModel {
        product_id: d.0,
        auto_cancel_secs: d.1,
        warranty_hours: d.2,
        is_create: d.3,
        max_num_rsv: d.4,
        min_num_rsv: d.5,
    })
    .collect::<Vec<_>>();
    let newdata = vec![
        ProductPolicyDto {
            product_id: 123,
            warranty_hours: 480,
            auto_cancel_secs: 3600,
            max_num_rsv: Some(26),
            min_num_rsv: Some(15),
        },
        ProductPolicyDto {
            product_id: 144,
            warranty_hours: 478,
            auto_cancel_secs: 3597,
            max_num_rsv: None,
            min_num_rsv: None,
        },
    ];
    let ms = ProductPolicyModelSet {
        policies: vec![
            ut_clone_productpolicy(&init_data[0]),
            ut_clone_productpolicy(&init_data[1]),
        ],
    }; // assume these instances were stored somewhere
    let result = ms.update(newdata);
    assert_eq!(result.is_ok(), true);
    let updated = result.unwrap();
    {
        assert_eq!(updated.policies.len(), 3);
        let actual = updated
            .policies
            .iter()
            .find(|m| m.product_id == 20903)
            .unwrap();
        assert_eq!(actual, &init_data[0]);
        let actual = updated
            .policies
            .iter()
            .find(|m| m.product_id == 144)
            .unwrap();
        assert_eq!(actual, &init_data[2]);
        let actual = updated
            .policies
            .iter()
            .find(|m| m.product_id == 123)
            .unwrap();
        assert_eq!(actual, &init_data[3]);
    }
} // end of update_instance_ok
