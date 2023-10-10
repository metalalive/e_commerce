use std::boxed::Box; 
use std::result::Result as DefaultResult ; 

use crate::AppSharedState;
use crate::constant::ProductType;
use crate::api::web::dto::{
    OrderCreateRespOkDto, OrderCreateRespErrorDto, OrderLinePayDto, PayAmountDto,
    OrderCreateReqData, ShippingReqDto, BillingReqDto
};
use crate::model::{BillingModel, ShippingModel};
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
        let  (sh_d, bl_d, _ol_d) = (req.shipping, req.billing, req.order_lines);
        let (_b, _s) = self.try_into_models(sh_d, bl_d)?;
        let reserved_item = OrderLinePayDto {
            seller_id: 389u32, product_id: 1018u64, product_type:ProductType::Item,
            quantity: 9u32, amount: PayAmountDto {unit:4u32, total:35u32}
        };
        let obj = OrderCreateRespOkDto { order_id: "ty033u29G".to_string(),
            usr_id: 789u32, time: 29274692u64, reserved_lines: vec![reserved_item],
        };
        Ok(obj)
    } // end of fn execute

    fn try_into_models(&self, sh_d:ShippingReqDto, bl_d:BillingReqDto)
        -> DefaultResult<(BillingModel,ShippingModel), OrderCreateRespErrorDto>
    {
        let results = (BillingModel::try_from(bl_d), ShippingModel::try_from(sh_d));
        if let (Ok(billing), Ok(shipping)) = results {
            Ok((billing, shipping))
        } else {
            let mut obj = OrderCreateRespErrorDto { order_lines: None,
                billing:None, shipping: None };
            if let Err(e) = results.0 { obj.billing = Some(e); }
            if let Err(e) = results.1 { obj.shipping = Some(e); }
            Err(obj)
        }
    }
} // end of impl CreateOrderUseCase

