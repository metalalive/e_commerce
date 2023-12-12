use std::boxed::Box; 
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc; 
use std::result::Result as DefaultResult ; 

use chrono::Local as LocalTime;

use crate::AppSharedState;
use crate::constant::ProductType;
use crate::api::web::dto::{
    OrderCreateRespOkDto, OrderCreateRespErrorDto, OrderLineCreateErrorReason, OrderLineCreateErrNonExistDto,
    OrderCreateReqData, ShippingReqDto, BillingReqDto, OrderLineReqDto, OrderLineCreateErrorDto,
    OrderLineReturnErrorDto, 
};
use crate::api::rpc::dto::{
    OrderReplicaPaymentDto, OrderReplicaInventoryDto, OrderPaymentUpdateDto, OrderPaymentUpdateErrorDto,
    StockLevelReturnDto, StockReturnErrorDto, OrderReplicaInventoryReqDto, OrderReplicaStockReservingDto,
    OrderReplicaStockReturningDto, OrderLineReplicaRefundDto, OrderReplicaRefundReqDto
};
use crate::error::AppError;
use crate::model::{
    BillingModel, ShippingModel, OrderLineModel, ProductPriceModelSet, ProductPolicyModelSet,
    StockLevelModelSet, OrderLineModelSet, OrderLineIdentity, OrderReturnModel
};
use crate::repository::{
    AbsOrderRepo, AbsProductPriceRepo, AbstProductPolicyRepo, AppStockRepoReserveReturn,
    AbsOrderReturnRepo
};
use crate::logging::{app_log_event, AppLogLevel, AppLogContext};

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
pub struct OrderReplicaRefundUseCase{
    pub repo: Box<dyn AbsOrderReturnRepo>,
}
pub struct OrderReplicaInventoryUseCase {
    pub  ret_repo: Box<dyn AbsOrderReturnRepo>,
    pub  o_repo: Box<dyn AbsOrderRepo>,
}
pub struct OrderPaymentUpdateUseCase {
    pub repo: Box<dyn AbsOrderRepo>,
}
pub struct OrderDiscardUnpaidItemsUseCase {
    repo: Box<dyn AbsOrderRepo>,
    logctx: Arc<AppLogContext>
}
pub struct ReturnLinesReqUseCase {
    pub usr_prof_id: u32,
    pub o_repo: Box<dyn AbsOrderRepo>,
    pub or_repo: Box<dyn AbsOrderReturnRepo>,
    pub logctx: Arc<AppLogContext>,
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
        let timenow = LocalTime::now().fixed_offset();
        let ol_set = OrderLineModelSet { order_id:oid.clone(), lines:o_items,
                         create_time: timenow.clone(), owner_id:self.usr_id };
        self.try_reserve_stock(&ol_set).await?;
        // There might be under-booking issue if power outage happenes at here
        // before successfully saving the order lines. TODO: Improve the code here
        match self.repo_order.create(ol_set, o_bl, o_sh).await {
            Ok(lines) => {
                let obj = OrderCreateRespOkDto { order_id:oid, usr_id: self.usr_id,
                    time: timenow.timestamp() as u64, reserved_lines: lines };
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
                    reason: OrderLineCreateErrorReason::NotExist, product_type: d.product_type,
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
        // TODO, lock billing instance so customers are no longer able to update
        let usr_id = self.repo.owner_id(oid.as_str()).await?;
        let billing = self.repo.fetch_billing(oid.clone()).await ?;
        let resp = OrderReplicaPaymentDto {oid, usr_id, billing:billing.into(),
            lines: olines.into_iter().map(OrderLineModel::into).collect()
        };
        Ok(resp)
    }
}
impl OrderReplicaRefundUseCase {
    pub async fn execute(self, req:OrderReplicaRefundReqDto)
        -> DefaultResult<Vec<OrderLineReplicaRefundDto>, AppError>
    {
        let (oid, start, end) = (req.order_id, req.start, req.end);
        let ret_ms = self.repo.fetch_by_oid_ctime(oid.as_str(), start, end).await?;
        let resp = ret_ms.into_iter().flat_map::<Vec<OrderLineReplicaRefundDto>, _>
            (OrderReturnModel::into).collect();
        Ok(resp)
    }
}
impl OrderReplicaInventoryUseCase {
    pub async fn execute(self, req:OrderReplicaInventoryReqDto)
        -> DefaultResult<OrderReplicaInventoryDto, AppError>
    {
        let (start, end) = (req.start, req.end);
        let order_ids = self.o_repo.fetch_ids_by_created_time(start.clone(), end.clone()).await?;
        let mut reservations = vec![];
        let mut returns = vec![];
        for oid in order_ids {
            let olines = self.o_repo.fetch_all_lines(oid.clone()).await ?;
            let usr_id = self.o_repo.owner_id(oid.as_str()).await?;
            let create_time = self.o_repo.created_time(oid.as_str()).await?;
            let shipping = self.o_repo.fetch_shipping(oid.clone()).await ?;
            let obj = OrderReplicaStockReservingDto {
                oid, usr_id, create_time, shipping:shipping.into(),
                lines: olines.into_iter().map(OrderLineModel::into).collect()
            };
            reservations.push(obj);
        }
        let combo = self.ret_repo.fetch_by_created_time(start, end).await?;
        for (oid, ret_m) in combo {
            let usr_id = self.o_repo.owner_id(oid.as_str()).await?;
            let obj = OrderReplicaStockReturningDto { oid, usr_id, lines:ret_m.into() };
            returns.push(obj);
        }
        let resp = OrderReplicaInventoryDto { reservations, returns };
        Ok(resp)
    } // end of fn execute
} // end of impl OrderReplicaInventoryUseCase

impl OrderPaymentUpdateUseCase {
    pub async fn execute(self, data:OrderPaymentUpdateDto)
        -> DefaultResult<OrderPaymentUpdateErrorDto, AppError>
    {
        self.repo.update_lines_payment(data, OrderLineModel::update_payments).await
    }
}

impl OrderDiscardUnpaidItemsUseCase {
    pub fn new(repo: Box<dyn AbsOrderRepo>, logctx: Arc<AppLogContext>) -> Self {
        Self{ repo, logctx }
    }

    pub async fn execute(self) -> DefaultResult<(),AppError>
    {
        let time_start = self.repo.scheduled_job_last_time().await;
        let time_end = LocalTime::now().fixed_offset();
        let result = self.repo.fetch_lines_by_rsvtime( time_start,
                            time_end, Self::read_oline_set_cb ).await;
        if let Err(e) = result.as_ref() {
            let lctx = &self.logctx;
            app_log_event!(lctx, AppLogLevel::ERROR, "error: {:?}", e);
        } else {
            self.repo.scheduled_job_time_update().await;
        }
        result
    }
    fn read_oline_set_cb<'a>(o_repo: &'a dyn AbsOrderRepo, ol_set: OrderLineModelSet)
        -> Pin<Box<dyn Future<Output=DefaultResult<(),AppError>> + Send + 'a>>
    {
        let fut = async move {
            let (order_id, unpaid_lines) = (
                ol_set.order_id , ol_set.lines.into_iter().filter(
                    |m| m.qty.has_unpaid()
                ).collect::<Vec<OrderLineModel>>()
            );
            if unpaid_lines.is_empty() {
                Ok(()) // all items have been paid, nothing to discard for now.
            } else {
                let st_repo = o_repo.stock();
                let items = unpaid_lines.into_iter().map(OrderLineModel::into).collect();
                let data = StockLevelReturnDto{items, order_id};
                let _return_result = st_repo.try_return(
                    Self::read_stocklvl_cb, data).await?;
                Ok(()) // TODO, logging the stock-return result, the result may not be able
                       // to pass to the output of the method `fetch_lines_by_rsvtime`
            }
        }; // lifetime of the Future trait object must outlive `'static` 
        Box::pin(fut)
    }
    fn read_stocklvl_cb(ms: &mut StockLevelModelSet, data: StockLevelReturnDto)
        -> Vec<StockReturnErrorDto>
    { ms.return_across_expiry(data) }
} // end of impl OrderDiscardUnpaidItemsUseCase

pub enum ReturnLinesReqUcOutput {
    Success,  InvalidOwner,
    InvalidRequest(Vec<OrderLineReturnErrorDto>),
}

impl ReturnLinesReqUseCase {
    pub async fn execute(self, oid:String, data:Vec<OrderLineReqDto>)
        -> DefaultResult<ReturnLinesReqUcOutput, AppError>
    {
        let o_usr_id = self.o_repo.owner_id(oid.as_str()).await ?;
        if o_usr_id != self.usr_prof_id {
            return Ok(ReturnLinesReqUcOutput::InvalidOwner);
        }
        let pids = data.iter().map(OrderLineIdentity::from).collect::<Vec<OrderLineIdentity>>();
        let o_lines  = self.o_repo.fetch_lines_by_pid(oid.as_str(), pids.clone()).await ?;
        let o_returned = self.or_repo.fetch_by_pid(oid.as_str(), pids).await ?;
        match OrderReturnModel::filter_requests(data, o_lines, o_returned) {
            Ok(modified) => {
                let _num = self.or_repo.save(oid.as_str(), modified).await ?;
                Ok(ReturnLinesReqUcOutput::Success)
            },
            Err(errors) => Ok(ReturnLinesReqUcOutput::InvalidRequest(errors))
        }
    }
}
