use std::boxed::Box; 
use std::result::Result as DefaultResult ; 

use crate::AppSharedState;
use crate::constant::ProductType;
use crate::api::web::dto::{
    OrderCreateRespOkDto, OrderCreateRespErrorDto, OrderLinePayDto, PayAmountDto,
    OrderCreateReqData, ShippingErrorDto, ContactErrorDto, ContactNameErrorReason,
    PhoneNumNationErrorReason, PhoneNumberErrorDto
};
use crate::repository::{AbsOrderRepo, AbsProductPriceRepo, AbstProductPolicyRepo};

pub struct CreateOrderUseCase {
    pub glb_state:AppSharedState,
    pub repo_order: Box<dyn AbsOrderRepo>,
    pub repo_price: Box<dyn AbsProductPriceRepo>,
    pub repo_policy:Box<dyn AbstProductPolicyRepo>
}
impl CreateOrderUseCase {
    pub async fn execute(self, req:OrderCreateReqData) -> DefaultResult<OrderCreateRespOkDto, OrderCreateRespErrorDto>
    { // TODO, complete implementation
        let mut error = OrderCreateRespErrorDto {
            order_lines:None, billing:None, shipping: Some(ShippingErrorDto {
                contact:None, address:None, option:None, nonfield:None})
        };
        let mut contact_err = ContactErrorDto {first_name:None,
                    last_name:None, emails:None, phones:None, nonfield:None};
        let _contact = &req.shipping.contact;
        let err_name = _contact.first_name.is_empty();
        let err_phone = (0.._contact.phones.len()).into_iter().map(
            |idx| {
                let item = &_contact.phones[idx];
                if item.nation > 0 {
                    None
                } else {
                    Some(PhoneNumberErrorDto {number:None, nation:
                        Some(PhoneNumNationErrorReason::InvalidCode)})
                }
            }
        ).collect::<Vec<Option<PhoneNumberErrorDto>>>();
        if err_name {
            contact_err.first_name = Some(ContactNameErrorReason::Empty);
        }
        if err_phone.iter().any(|d| d.is_some()) {
            contact_err.phones = Some(err_phone);
        }
        if contact_err.first_name.is_none() && contact_err.phones.is_none()
        {
            let reserved_item = OrderLinePayDto {
                seller_id: 389u32, product_id: 1018u64, product_type:ProductType::Item,
                quantity: 9u32, amount: PayAmountDto {unit:4u32, total:35u32}
            };
            let obj = OrderCreateRespOkDto { order_id: "ty033u29G".to_string(),
                usr_id: 789u32, time: 29274692u64, reserved_lines: vec![reserved_item],
            };
            Ok(obj)
        } else {
            let x = error.shipping.as_mut().unwrap();
            x.contact = Some(contact_err);
            Err(error)
        }
    } // end of fn execute
} // end of impl CreateOrderUseCase
