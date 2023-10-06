use order::constant::ProductType;
use order::model::{ProductPolicyModelSet, ProductPolicyModel};
use order::api::web::dto::ProductPolicyDto;
use super::ut_clone_productpolicy;

#[test]
fn validate_newdata_ok() {
    let newdata = vec![
        ProductPolicyDto{ product_type:ProductType::Item, product_id:123,
            warranty_hours:480,  auto_cancel_secs:3600 },
        ProductPolicyDto{ product_type:ProductType::Package, product_id:124,
            warranty_hours:478,  auto_cancel_secs:3597 }
    ];
    let result = ProductPolicyModelSet::validate(&newdata);
    assert_eq!(result.is_ok(), true)
}

#[test]
fn validate_newdata_error_limit() { 
    let newdata = vec![
        ProductPolicyDto{ product_type:ProductType::Package, product_id:123,
            warranty_hours:0x7fff_ffffu32,  auto_cancel_secs:3600 },
        ProductPolicyDto{ product_type:ProductType::Item, product_id:124,
            warranty_hours:478,  auto_cancel_secs:0x7fff_ffffu32 }
    ];
    let result = ProductPolicyModelSet::validate(&newdata);
    assert_eq!(result.is_err(), true);
    let error = result.err().unwrap();
    assert_eq!(error.len(), 2);
    {
        let ce = error.iter().find(|item|{item.product_id==123}).unwrap();
        let detail = ce.warranty_hours.as_ref().unwrap();
        assert_eq!(ce.err_type, "ExceedingMaxLimit");
        assert!(detail.given > detail.limit);
        let ce = error.iter().find(|item|{item.product_id==124}).unwrap();
        let detail = ce.auto_cancel_secs.as_ref().unwrap();
        assert_eq!(ce.err_type, "ExceedingMaxLimit");
        assert!(detail.given > detail.limit);
    }
} // end of fn validate_newdata_error_limit

#[test]
fn update_instance_ok() {
    let init_data = [
        ProductPolicyModel {product_type:ProductType::Item, product_id:20903, auto_cancel_secs:731,
           warranty_hours:271, is_create:false },
        ProductPolicyModel {product_type:ProductType::Package, product_id:144, auto_cancel_secs:380,
           warranty_hours:30098, is_create:false },
        // following 2 items only for testing
        ProductPolicyModel {product_type:ProductType::Package, product_id:144, auto_cancel_secs:3597,
           warranty_hours:478, is_create:false },
        ProductPolicyModel {product_type:ProductType::Item, product_id:123, auto_cancel_secs:3600,
           warranty_hours:480, is_create:true },
    ];
    let newdata = vec![
        ProductPolicyDto{ product_type:ProductType::Item, product_id:123,
            warranty_hours:480,  auto_cancel_secs:3600 },
        ProductPolicyDto{ product_type:ProductType::Package, product_id:144,
            warranty_hours:478,  auto_cancel_secs:3597 }
    ];
    let ms = ProductPolicyModelSet {policies: vec![
        ut_clone_productpolicy(&init_data[0]),
        ut_clone_productpolicy(&init_data[1]),
    ]}; // assume these instances were stored somewhere
    let result = ms.update(&newdata);
    assert_eq!(result.is_ok(), true);
    let updated = result.unwrap();
    {
        assert_eq!(updated.policies.len(), 3);
        let actual = updated.policies.iter().find(|m| {m.product_id == 20903}).unwrap();
        assert_eq!(actual, &init_data[0]);
        let actual = updated.policies.iter().find(|m| {m.product_id == 144}).unwrap();
        assert_eq!(actual, &init_data[2]);
        let actual = updated.policies.iter().find(|m| {m.product_id == 123}).unwrap();
        assert_eq!(actual, &init_data[3]);
    }
} // end of update_instance_ok

