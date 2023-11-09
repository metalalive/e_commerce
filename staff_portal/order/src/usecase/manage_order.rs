use std::boxed::Box; 
use std::result::Result as DefaultResult ; 

use chrono::Local;

use crate::AppSharedState;
use crate::constant::ProductType;
use crate::api::web::dto::{
    OrderCreateRespOkDto, OrderCreateRespErrorDto, OrderLineErrorReason, OrderLineCreateErrNonExistDto,
    OrderCreateReqData, ShippingReqDto, BillingReqDto, OrderLineReqDto, OrderLineCreateErrorDto,
};
use crate::api::rpc::dto::{
    OrderReplicaPaymentDto, OrderReplicaInventoryDto, OrderPaymentUpdateDto, OrderPaymentUpdateErrorDto
};
use crate::error::AppError;
use crate::model::{
    BillingModel, ShippingModel, OrderLineModel, ProductPriceModelSet, ProductPolicyModelSet,
    StockLevelModelSet, OrderLineModelSet
};
use crate::repository::{AbsOrderRepo, AbsProductPriceRepo, AbstProductPolicyRepo, AppStockRepoReserveReturn};
use crate::logging::{app_log_event, AppLogLevel};

pub enum CreateOrderUsKsErr {Client(OrderCreateRespErrorDto), Server}

pub struct CreateOrderUseCase {
    pub glb_state:AppSharedState,
    pub repo_order: Box<dyn AbsOrderRepo>,
    pub repo_price: Box<dyn AbsProductPriceRepo>,
    pub repo_policy:Box<dyn AbstProductPolicyRepo>,
    pub usr_id: u32 // TODO, switch to auth token (e.g. JWT), check user quota
}

pub struct OrderReplicaPaymentUseCase {
    pub repo: Box<dyn AbsOrderRepo>,
}
pub struct OrderReplicaInventoryUseCase {
    pub repo: Box<dyn AbsOrderRepo>,
}
pub struct OrderPaymentUpdateUseCase {
    pub repo: Box<dyn AbsOrderRepo>,
}

impl CreateOrderUseCase {
    pub async fn execute(self, req:OrderCreateReqData)
        -> DefaultResult<OrderCreateRespOkDto, CreateOrderUsKsErr>
    {
        let  (sh_d, bl_d, ol_d) = (req.shipping, req.billing, req.order_lines);
        let (o_bl, o_sh) = Self::validate_metadata(sh_d, bl_d)?;
        let (ms_policy, ms_price) = self.load_product_properties(&ol_d).await?;
        let o_items = Self::validate_orderline(ms_policy, ms_price, ol_d)?;
        // TODO, machine code to UUID generator should be configurable
        let machine_code = 1u8;
        let oid = OrderLineModel::generate_order_id(machine_code);
        let ol_set = OrderLineModelSet {order_id:oid.clone(), lines:o_items};
        self.try_reserve_stock(&ol_set).await?;
        // There might be under-booking issue if power outage happenes at here
        // before successfully saving the order lines. TODO: Improve the code here
        match self.repo_order.create(self.usr_id, ol_set, o_bl, o_sh).await {
            Ok(lines) => {
                let timenow = Local::now().fixed_offset().timestamp();
                let obj = OrderCreateRespOkDto { order_id:oid, usr_id: self.usr_id,
                    time: timenow as u64, reserved_lines: lines };
                Ok(obj)
            },
            Err(e) => {
                let logctx_p = self.glb_state.log_context().clone();
                app_log_event!(logctx_p, AppLogLevel::ERROR, "order repository error, detail:{e}");
                Err(CreateOrderUsKsErr::Server)
            }
        }
    } // end of fn execute

    fn validate_metadata(sh_d:ShippingReqDto, bl_d:BillingReqDto)
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
    

    pub fn validate_orderline(ms_policy:ProductPolicyModelSet,
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
                    }) // TODO, validate expiry of the pricing rule
                } else {None}
            });
            let (plc_nonexist, price_nonexist) = (result1.is_none(), result2.is_none());
            if let (Some(plc), Some(price)) = (result1, result2) {
                Some(OrderLineModel::from(d, plc, price))
            } else {
                let e = OrderLineCreateErrorDto { seller_id: d.seller_id, product_id: d.product_id,
                    reason: OrderLineErrorReason::NotExist, product_type: d.product_type,
                    nonexist:Some(OrderLineCreateErrNonExistDto {product_price:price_nonexist,
                        product_policy:plc_nonexist, stock_seller:false }), shortage:None
                };
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

    async fn try_reserve_stock(&self, req:&OrderLineModelSet) -> DefaultResult<(), CreateOrderUsKsErr>
    {
        let logctx_p = self.glb_state.log_context().clone();
        let repo_st = self.repo_order.stock();
        match repo_st.try_reserve(Self::try_reserve_stock_cb, req).await {
            Ok(()) =>  Ok(()),
            Err(e) => match e {
                Ok(client_e) => {
                    app_log_event!(logctx_p, AppLogLevel::WARNING, "stock reserve client error");
                    let ec = OrderCreateRespErrorDto {billing:None, shipping: None,
                                                      order_lines: Some(client_e) };
                    Err(CreateOrderUsKsErr::Client(ec))
                },
                Err(server_e) => {
                    app_log_event!(logctx_p, AppLogLevel::ERROR,
                                   "stock reserve server error, detail:{server_e}");
                    Err(CreateOrderUsKsErr::Server)
                }
            }
        }
    } // end of fn try_reserve_stock

    fn try_reserve_stock_cb (ms:&mut StockLevelModelSet, req:&OrderLineModelSet)
        -> AppStockRepoReserveReturn
    {
        let result = ms.try_reserve(req);
        if result.is_empty() {
            Ok(())
        } else {
            Err(Ok(result))
        }
    }
} // end of impl CreateOrderUseCase


impl OrderReplicaPaymentUseCase {
    pub(crate) async fn execute(self, oid:String) -> DefaultResult<OrderReplicaPaymentDto, AppError>
    {
        let olines = self.repo.fetch_all_lines(oid.clone()).await ?;
        // TODO, lock billing instance so customers are no longer able to update it
        let (billing, usr_id) = self.repo.fetch_billing(oid.clone()).await ?;
        let resp = OrderReplicaPaymentDto {oid, usr_id, billing:billing.into(),
            lines: olines.into_iter().map(OrderLineModel::into).collect()
        };
        Ok(resp)
    }
}
impl OrderReplicaInventoryUseCase {
    pub(crate) async fn execute(self, oid:String) -> DefaultResult<OrderReplicaInventoryDto, AppError>
    {
        let olines = self.repo.fetch_all_lines(oid.clone()).await ?;
        // TODO, lock shipping instance so customers are no longer able to update it
        let (shipping, usr_id) = self.repo.fetch_shipping(oid.clone()).await ?;
        let resp = OrderReplicaInventoryDto {oid, usr_id, shipping:shipping.into(),
            lines: olines.into_iter().map(OrderLineModel::into).collect()
        };
        Ok(resp)
    }
}
impl OrderPaymentUpdateUseCase {
    pub async fn execute(self, data:OrderPaymentUpdateDto)
        -> DefaultResult<OrderPaymentUpdateErrorDto, AppError>
    {
        self.repo.update_lines_payment(data, OrderLineModel::update_payments).await
    }
}
