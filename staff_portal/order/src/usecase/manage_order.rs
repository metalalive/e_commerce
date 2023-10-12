use std::boxed::Box; 
use std::result::Result as DefaultResult ; 

use crate::AppSharedState;
use crate::constant::ProductType;
use crate::api::web::dto::{
    OrderCreateRespOkDto, OrderCreateRespErrorDto, OrderLinePayDto, PayAmountDto,
    OrderCreateReqData, ShippingReqDto, BillingReqDto, OrderLineReqDto, OrderLineCreateErrorDto, OrderLineErrorReason
};
use crate::model::{BillingModel, ShippingModel, OrderLineModel, ProductPriceModelSet, ProductPolicyModelSet};
use crate::repository::{AbsOrderRepo, AbsProductPriceRepo, AbstProductPolicyRepo};
use crate::logging::{app_log_event, AppLogLevel};

pub enum CreateOrderUsKsErr {Client(OrderCreateRespErrorDto), Server}

pub struct CreateOrderUseCase {
    pub glb_state:AppSharedState,
    pub repo_order: Box<dyn AbsOrderRepo>,
    pub repo_price: Box<dyn AbsProductPriceRepo>,
    pub repo_policy:Box<dyn AbstProductPolicyRepo>
}

impl CreateOrderUseCase {
    pub async fn execute(self, req:OrderCreateReqData) -> DefaultResult<OrderCreateRespOkDto, CreateOrderUsKsErr>
    { // TODO, complete implementation
        let  (sh_d, bl_d, ol_d) = (req.shipping, req.billing, req.order_lines);
        let (_obl, _osh) = self.validate_metadata(sh_d, bl_d)?;
        let (ms_policy, ms_price) = self.load_product_properties(&ol_d).await?;
        let _oitems = self.validate_orderline(ms_policy, ms_price, ol_d)?;
        let reserved_item = OrderLinePayDto {
            seller_id: 389u32, product_id: 1018u64, product_type:ProductType::Item,
            quantity: 9u32, amount: PayAmountDto {unit:4u32, total:35u32}
        };
        let obj = OrderCreateRespOkDto { order_id: "ty033u29G".to_string(),
            usr_id: 789u32, time: 29274692u64, reserved_lines: vec![reserved_item],
        };
        Ok(obj)
    } // end of fn execute

    fn validate_metadata(&self, sh_d:ShippingReqDto, bl_d:BillingReqDto)
        -> DefaultResult<(BillingModel,ShippingModel), CreateOrderUsKsErr>
    {
        let results = (BillingModel::try_from(bl_d), ShippingModel::try_from(sh_d));
        if let (Ok(billing), Ok(shipping)) = results {
            Ok((billing, shipping))
        } else {
            let mut obj = OrderCreateRespErrorDto { order_lines: None,
                billing:None, shipping: None };
            if let Err(e) = results.0 { obj.billing = Some(e); }
            if let Err(e) = results.1 { obj.shipping = Some(e); }
            Err(CreateOrderUsKsErr::Client(obj))
        }
    }

    async fn load_product_properties (&self, data:&Vec<OrderLineReqDto>)
        -> DefaultResult<(ProductPolicyModelSet, Vec<ProductPriceModelSet>), CreateOrderUsKsErr>
    {
        let req_ids_policy = data.iter().map(|d| (d.product_type.clone(), d.product_id))
            .collect::<Vec<(ProductType, u64)>>();
        let req_ids_price = data.iter().map(|d| (d.seller_id, d.product_type.clone(), d.product_id))
            .collect::<Vec<(u32, ProductType, u64)>>();
        let rs_policy = self.repo_policy.fetch(req_ids_policy.clone()).await;
        let rs_price  = self.repo_price.fetch_many(req_ids_price.clone()).await;
        if rs_policy.is_ok() && rs_price.is_ok() {
            let (ms_policy, ms_price) = (rs_policy.unwrap(), rs_price.unwrap());
            Ok((ms_policy, ms_price))
        } else { // repository error, internal service unavailable
            let logctx_p = self.glb_state.log_context().clone();
            let err_policy = if let Err(e) = rs_policy { e.to_string() }
                             else {"none".to_string()};
            let err_price = if let Err(e) = rs_price { e.to_string() }
                             else {"none".to_string()};
            app_log_event!(logctx_p, AppLogLevel::ERROR,
                    "repository fetch error, policy:{}, price:{}",
                    err_policy, err_price);
            Err(CreateOrderUsKsErr::Server)
        }
    } // end of load_product_properties 
    
    fn validate_orderline(&self, ms_policy:ProductPolicyModelSet,
                                ms_price:Vec<ProductPriceModelSet>,
                                data:Vec<OrderLineReqDto> )
        -> DefaultResult<Vec<OrderLineModel>, CreateOrderUsKsErr>
    {
        let mut missing = vec![];
        let lines = data.into_iter().filter_map(|d| {
            let result1 = ms_policy.policies.iter().find(|m| {
                m.product_type == d.product_type && m.product_id == d.product_id
            });
            let result2 = ms_price.iter().find_map(|ms| {
                if ms.store_id == d.seller_id {
                    ms.items.iter().find(|m| {
                        m.product_type == d.product_type && m.product_id == d.product_id
                    })
                } else {None}
            });
            let (plc_nonexist, price_nonexist) = (result1.is_none(), result2.is_none());
            if let (Some(plc), Some(price)) = (result1, result2) {
                Some(OrderLineModel::from(d, plc, price))
            } else {
                let logctx_p = self.glb_state.log_context();
                let prod_typ_num:u8 = d.product_type.clone().into();
                app_log_event!(logctx_p, AppLogLevel::WARNING,
                    "product not found, {}-{}-{}, policy:{}, price:{}",
                    d.seller_id, prod_typ_num, d.product_id, plc_nonexist, price_nonexist);
                let e = OrderLineCreateErrorDto { seller_id: d.seller_id,
                    reason: OrderLineErrorReason::NotExist, product_id: d.product_id,
                    product_type: d.product_type };
                missing.push(e);
                None
            }
        }).collect();
        if missing.is_empty() {
            Ok(lines)
        } else {
            let error = OrderCreateRespErrorDto { billing: None,
                        shipping: None, order_lines: Some(missing) };
            Err(CreateOrderUsKsErr::Client(error))
        }
    } // end of fn validate_orderline
} // end of impl CreateOrderUseCase

