use std::boxed::Box;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::result::Result as DefaultResult;
use std::sync::Arc;

use chrono::{DateTime, FixedOffset, Local as LocalTime};
use ecommerce_common::api::dto::{CurrencyDto, GenericRangeErrorDto};
use ecommerce_common::api::rpc::dto::{
    OrderLinePayUpdateErrorDto, OrderLineReplicaRefundDto, OrderPaymentUpdateDto,
    OrderPaymentUpdateErrorDto, OrderReplicaPaymentDto, OrderReplicaRefundReqDto,
};
use ecommerce_common::api::web::dto::{
    BillingErrorDto, ContactErrorDto, ContactNonFieldErrorReason, QuotaResourceErrorDto,
};

use crate::api::rpc::dto::{
    OrderLineStockReturningDto, OrderReplicaInventoryDto, OrderReplicaInventoryReqDto,
    OrderReplicaStockReservingDto, OrderReplicaStockReturningDto, StockLevelReturnDto,
    StockReturnErrorDto,
};
use crate::api::web::dto::{
    BillingReqDto, OrderCreateReqData, OrderCreateRespErrorDto, OrderCreateRespOkDto,
    OrderLineCreateErrNonExistDto, OrderLineCreateErrorDto, OrderLineCreateErrorReason,
    OrderLineReqDto, OrderLineReturnErrorDto, ShippingErrorDto, ShippingReqDto,
};

use ecommerce_common::error::AppErrorCode;
use ecommerce_common::logging::{app_log_event, AppLogContext, AppLogLevel};
use ecommerce_common::model::order::BillingModel;

use crate::constant::app_meta;
use crate::error::AppError;
use crate::model::{
    OrderCurrencyModel, OrderLineIdentity, OrderLineModel, OrderLineModelSet, OrderReturnModel,
    ProductPolicyModelSet, ProductPriceModelSet, ShippingModel, StockLevelModelSet,
};
use crate::repository::{
    AbsCurrencyRepo, AbsOrderRepo, AbsOrderReturnRepo, AbsProductPriceRepo, AbstProductPolicyRepo,
    AppStockRepoReserveReturn,
};
use crate::{AppAuthPermissionCode, AppAuthQuotaMatCode, AppAuthedClaim, AppSharedState};

pub enum CreateOrderUsKsErr {
    // use box pointer instead of concrete struct , to avoid large size types
    // passing between call stack entries
    ReqContent(Box<OrderCreateRespErrorDto>),
    Quota(Box<OrderCreateRespErrorDto>),
    Server(Vec<AppError>),
}

pub struct CreateOrderUseCase {
    pub glb_state: AppSharedState,
    pub repo_currex: Box<dyn AbsCurrencyRepo>,
    pub repo_order: Box<dyn AbsOrderRepo>,
    pub repo_price: Box<dyn AbsProductPriceRepo>,
    pub repo_policy: Box<dyn AbstProductPolicyRepo>,
    pub auth_claim: AppAuthedClaim,
}

pub struct OrderReplicaPaymentUseCase {
    pub repo: Box<dyn AbsOrderRepo>,
}
pub struct OrderReplicaRefundUseCase {
    pub o_repo: Box<dyn AbsOrderRepo>,
    pub ret_repo: Box<dyn AbsOrderReturnRepo>,
}
pub struct OrderReplicaInventoryUseCase {
    pub ret_repo: Box<dyn AbsOrderReturnRepo>,
    pub o_repo: Box<dyn AbsOrderRepo>,
    pub logctx: Arc<AppLogContext>,
}
pub struct OrderPaymentUpdateUseCase {
    pub repo: Box<dyn AbsOrderRepo>,
}
pub struct OrderDiscardUnpaidItemsUseCase {
    repo: Box<dyn AbsOrderRepo>,
    logctx: Arc<AppLogContext>,
}
pub struct ReturnLinesReqUseCase {
    pub authed_claim: AppAuthedClaim,
    pub o_repo: Box<dyn AbsOrderRepo>,
    pub or_repo: Box<dyn AbsOrderReturnRepo>,
    pub logctx: Arc<AppLogContext>,
}

impl CreateOrderUseCase {
    pub async fn execute(
        self,
        req: OrderCreateReqData,
    ) -> DefaultResult<OrderCreateRespOkDto, CreateOrderUsKsErr> {
        let logctx_p = self.glb_state.log_context().clone();
        let OrderCreateReqData {
            billing: bl_d,
            shipping: sh_d,
            order_lines: ol_d,
            currency: currency_buyer,
        } = req;
        Self::validate_quota(
            &self.auth_claim,
            sh_d.contact.emails.len(),
            sh_d.contact.phones.len(),
            bl_d.contact.emails.len(),
            bl_d.contact.phones.len(),
            ol_d.len(),
        )?;
        let (o_bl, o_sh) = Self::validate_metadata(sh_d, bl_d)?;
        let (ms_policy, ms_price) = self.load_product_properties(&ol_d).await?;
        let o_currency =
            Self::snapshot_currencies(self.repo_currex.as_ref(), currency_buyer, &ms_price)
                .await
                .map_err(|es| {
                    app_log_event!(logctx_p, AppLogLevel::ERROR, "error: {:?}", es);
                    CreateOrderUsKsErr::Server(es)
                })?;
        let o_items = Self::validate_orderline(ms_policy, ms_price, ol_d)?;
        let oid = OrderLineModel::generate_order_id(app_meta::MACHINE_CODE);
        let timenow = LocalTime::now().fixed_offset();
        let usr_id = self.auth_claim.profile;
        let ol_set = OrderLineModelSet {
            order_id: oid,
            lines: o_items,
            currency: o_currency,
            create_time: timenow, // trait `Copy` implemented, clone implicitly
            owner_id: usr_id,
        };
        // repository implementation should treat order-line reservation and
        // stock-level update as a single atomic operation
        self.try_reserve_stock(&ol_set).await?;

        // Contact info might be lost after order lines were saved, if power outage happenes
        // at here. TODO: Improve the code here
        self.repo_order
            .save_contact(ol_set.order_id.as_str(), o_bl, o_sh)
            .await
            .map_err(|e| {
                app_log_event!(logctx_p, AppLogLevel::ERROR, "repo-fail-save: {e}");
                CreateOrderUsKsErr::Server(vec![e])
            })?;
        let resp = OrderCreateRespOkDto::try_from(ol_set).map_err(CreateOrderUsKsErr::Server)?;
        Ok(resp)
    } // end of fn execute

    fn validate_quota(
        auth_claim: &AppAuthedClaim,
        num_ship_emails: usize,
        num_ship_phones: usize,
        num_bill_emails: usize,
        num_bill_phones: usize,
        num_olines: usize,
    ) -> DefaultResult<(), CreateOrderUsKsErr> {
        let mut err_obj = OrderCreateRespErrorDto::default();
        let mut quota_chk_result = [
            (AppAuthQuotaMatCode::NumPhones, num_ship_phones),
            (AppAuthQuotaMatCode::NumEmails, num_ship_emails),
            (AppAuthQuotaMatCode::NumPhones, num_bill_phones),
            (AppAuthQuotaMatCode::NumEmails, num_bill_emails),
            (AppAuthQuotaMatCode::NumOrderLines, num_olines),
        ]
        .into_iter()
        .map(|(matcode, given_num)| {
            let limit = auth_claim.quota_limit(matcode);
            if (limit as usize) < given_num {
                Some(QuotaResourceErrorDto {
                    max_: limit,
                    given: given_num,
                })
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
        if quota_chk_result.iter().any(Option::is_some) {
            let quota_phone = quota_chk_result.remove(0);
            let quota_email = quota_chk_result.remove(0);
            if quota_email.is_some() || quota_phone.is_some() {
                let contact = Some(ContactErrorDto {
                    first_name: None,
                    last_name: None,
                    emails: None,
                    phones: None,
                    quota_email,
                    quota_phone,
                    nonfield: Some(ContactNonFieldErrorReason::QuotaExceed),
                });
                err_obj.shipping = Some(ShippingErrorDto {
                    contact,
                    address: None,
                    option: None,
                });
            }
            let quota_phone = quota_chk_result.remove(0);
            let quota_email = quota_chk_result.remove(0);
            if quota_email.is_some() || quota_phone.is_some() {
                let contact = Some(ContactErrorDto {
                    first_name: None,
                    last_name: None,
                    emails: None,
                    phones: None,
                    quota_email,
                    quota_phone,
                    nonfield: Some(ContactNonFieldErrorReason::QuotaExceed),
                });
                let err_bill = BillingErrorDto {
                    contact,
                    address: None,
                };
                err_obj.billing = Some(err_bill);
            }
            let quota_olines = quota_chk_result.remove(0);
            err_obj.quota_olines = quota_olines;
            Err(CreateOrderUsKsErr::Quota(Box::new(err_obj)))
        } else {
            Ok(())
        }
    } // end of fn validate_quota

    fn validate_metadata(
        sh_d: ShippingReqDto,
        bl_d: BillingReqDto,
    ) -> DefaultResult<(BillingModel, ShippingModel), CreateOrderUsKsErr> {
        let results = (BillingModel::try_from(bl_d), ShippingModel::try_from(sh_d));
        if let (Ok(billing), Ok(shipping)) = results {
            Ok((billing, shipping))
        } else {
            let mut err_obj = OrderCreateRespErrorDto::default();
            if let Err(e) = results.0 {
                err_obj.billing = Some(e);
            }
            if let Err(e) = results.1 {
                err_obj.shipping = Some(e);
            }
            Err(CreateOrderUsKsErr::ReqContent(Box::new(err_obj)))
        }
    } // end of fn validate_metadata

    async fn load_product_properties(
        &self,
        data: &[OrderLineReqDto],
    ) -> DefaultResult<(ProductPolicyModelSet, Vec<ProductPriceModelSet>), CreateOrderUsKsErr> {
        let req_ids_policy = data.iter().map(|d| d.product_id).collect::<Vec<u64>>();
        let req_ids_price = data
            .iter()
            .map(|d| (d.seller_id, d.product_id))
            .collect::<Vec<(u32, u64)>>();
        // TODO, limit number of distinct product items to load for each order
        let rs_policy = self.repo_policy.fetch(req_ids_policy.clone()).await;
        let rs_price = self.repo_price.fetch_many(req_ids_price.clone()).await;
        if rs_policy.is_ok() && rs_price.is_ok() {
            let (ms_policy, ms_price) = (rs_policy.unwrap(), rs_price.unwrap());
            Ok((ms_policy, ms_price))
        } else {
            // repository error, internal service unavailable
            let mut errors = Vec::new();
            let logctx_p = self.glb_state.log_context().clone();
            let err_policy = if let Err(e) = rs_policy {
                let msg = e.to_string();
                errors.push(e);
                msg
            } else {
                "none".to_string()
            };
            let err_price = if let Err(e) = rs_price {
                let msg = e.to_string();
                errors.push(e);
                msg
            } else {
                "none".to_string()
            };
            app_log_event!(
                logctx_p,
                AppLogLevel::ERROR,
                "policy:{}, price:{}",
                err_policy,
                err_price
            );
            Err(CreateOrderUsKsErr::Server(errors))
        }
    } // end of load_product_properties

    pub async fn snapshot_currencies(
        repo_currex_p: &dyn AbsCurrencyRepo,
        label_buyer: CurrencyDto,
        seller_mset_price: &[ProductPriceModelSet],
    ) -> DefaultResult<OrderCurrencyModel, Vec<AppError>> {
        let mut labels = seller_mset_price
            .iter()
            .map(|ms| ms.currency.clone())
            .collect::<Vec<_>>();
        labels.push(label_buyer.clone());
        let exrate_avail = repo_currex_p.fetch(labels).await.map_err(|e| vec![e])?;
        let label_sellers = seller_mset_price
            .iter()
            .map(|ms| (ms.store_id, ms.currency.clone()))
            .collect::<Vec<_>>();
        let args = (exrate_avail, label_buyer, label_sellers);
        OrderCurrencyModel::try_from(args)
    } // end of fn snapshot_currencies

    pub fn validate_orderline(
        ms_policy: ProductPolicyModelSet,
        ms_price: Vec<ProductPriceModelSet>,
        data: Vec<OrderLineReqDto>,
    ) -> DefaultResult<Vec<OrderLineModel>, CreateOrderUsKsErr> {
        let (mut client_errors, mut server_errors) = (vec![], vec![]);
        let lines = data
            .into_iter()
            .filter_map(|d| {
                let result1 = ms_policy
                    .policies
                    .iter()
                    .find(|m| m.product_id == d.product_id);
                let result2 = ms_price.iter().find_map(|ms| {
                    if ms.store_id == d.seller_id {
                        ms.items.iter().find(|m| m.product_id == d.product_id) // TODO, validate expiry of the pricing rule
                    } else {
                        None
                    }
                });
                let (plc_nonexist, price_nonexist) = (result1.is_none(), result2.is_none());
                if let (Some(plc), Some(price)) = (result1, result2) {
                    let (seller_id, product_id, req_qty) = (d.seller_id, d.product_id, d.quantity);
                    OrderLineModel::try_from(d, plc, price)
                        .map_err(|e| {
                            if e.code == AppErrorCode::ExceedingMaxLimit {
                                let rsv_limit = GenericRangeErrorDto {
                                    max_: plc.max_num_rsv,
                                    min_: plc.min_num_rsv,
                                    given: req_qty,
                                };
                                let e = OrderLineCreateErrorDto {
                                    seller_id,
                                    product_id,
                                    rsv_limit: Some(rsv_limit),
                                    nonexist: None,
                                    shortage: None,
                                    reason: OrderLineCreateErrorReason::RsvLimitViolation,
                                };
                                client_errors.push(e);
                            } else {
                                server_errors.push(e);
                            }
                        })
                        .ok()
                } else {
                    let nonexist = OrderLineCreateErrNonExistDto {
                        product_price: price_nonexist,
                        product_policy: plc_nonexist,
                        stock_seller: false,
                    };
                    let e = OrderLineCreateErrorDto {
                        seller_id: d.seller_id,
                        product_id: d.product_id,
                        rsv_limit: None,
                        reason: OrderLineCreateErrorReason::NotExist,
                        nonexist: Some(nonexist),
                        shortage: None,
                    };
                    client_errors.push(e);
                    None
                }
            })
            .collect();
        if client_errors.is_empty() && server_errors.is_empty() {
            Ok(lines)
        } else if !server_errors.is_empty() {
            Err(CreateOrderUsKsErr::Server(server_errors))
        } else {
            let err_dto = OrderCreateRespErrorDto {
                order_lines: Some(client_errors),
                ..Default::default()
            };
            Err(CreateOrderUsKsErr::ReqContent(Box::new(err_dto)))
        }
    } // end of fn validate_orderline

    async fn try_reserve_stock(
        &self,
        req: &OrderLineModelSet,
    ) -> DefaultResult<(), CreateOrderUsKsErr> {
        let logctx_p = self.glb_state.log_context().clone();
        let repo_st = self.repo_order.stock();
        repo_st
            .try_reserve(Self::try_reserve_stock_cb, req)
            .await
            .map_err(|e| match e {
                Ok(client_e) => {
                    app_log_event!(logctx_p, AppLogLevel::WARNING, "stock reserve client error");
                    let ec = OrderCreateRespErrorDto {
                        order_lines: Some(client_e),
                        ..Default::default()
                    };
                    CreateOrderUsKsErr::ReqContent(Box::new(ec))
                }
                Err(server_e) => {
                    app_log_event!(logctx_p, AppLogLevel::ERROR, "detail:{server_e}");
                    CreateOrderUsKsErr::Server(vec![server_e])
                }
            })
    } // end of fn try_reserve_stock

    fn try_reserve_stock_cb(
        ms: &mut StockLevelModelSet,
        req: &OrderLineModelSet,
    ) -> AppStockRepoReserveReturn {
        let result = ms.try_reserve(req);
        if result.is_empty() {
            Ok(())
        } else {
            Err(Ok(result))
        }
    }
} // end of impl CreateOrderUseCase

impl OrderReplicaPaymentUseCase {
    pub(crate) async fn execute(
        self,
        oid: String,
    ) -> DefaultResult<OrderReplicaPaymentDto, AppError> {
        let olines = self.repo.fetch_all_lines(oid.clone()).await?;
        let currency_m = self.repo.currency_exrates(oid.as_str()).await?;
        // TODO, lock billing instance so customers are no longer able to update
        let usr_id = self.repo.owner_id(oid.as_str()).await?;
        let billing = self.repo.fetch_billing(oid.clone()).await?;
        OrderLineModelSet::replica_paym_dto(oid, usr_id, olines, currency_m, billing)
    }
} // end of impl OrderReplicaPaymentUseCase

impl OrderReplicaRefundUseCase {
    pub async fn execute(
        self,
        req: OrderReplicaRefundReqDto,
    ) -> DefaultResult<HashMap<String, Vec<OrderLineReplicaRefundDto>>, AppError> {
        let OrderReplicaRefundReqDto { start, end } = req;
        let start_dt = DateTime::parse_from_rfc3339(start.as_str()).map_err(|e| AppError {
            code: AppErrorCode::InvalidInput,
            detail: Some(format!("corrupted start-time :{:?} ", e)),
        })?;
        let end_dt = DateTime::parse_from_rfc3339(end.as_str()).map_err(|e| AppError {
            code: AppErrorCode::InvalidInput,
            detail: Some(format!("corrupted end-time :{:?} ", e)),
        })?;
        let combo = self
            .ret_repo
            .fetch_by_created_time(start_dt, end_dt)
            .await?;
        let ret_map = Self::reorg_return_models(combo);
        let mut out = HashMap::new();
        for (oid, ret_ms) in ret_map {
            let currency_m = self.o_repo.currency_exrates(oid.as_str()).await?;
            let refund_dtos = OrderReturnModel::to_replica_refund_dto(ret_ms, currency_m)
                .map_err(|mut es| es.remove(0))?;
            out.insert(oid, refund_dtos);
        }
        Ok(out)
    } // end of fn execute

    fn reorg_return_models(
        combo: Vec<(String, OrderReturnModel)>,
    ) -> HashMap<String, Vec<OrderReturnModel>> {
        let mut o_map: HashMap<String, Vec<OrderReturnModel>> = HashMap::new();
        combo
            .into_iter()
            .map(|(oid, ret_m)| {
                if !o_map.contains_key(oid.as_str()) {
                    let _ = o_map.insert(oid.clone(), Vec::new());
                }
                let list = o_map.get_mut(oid.as_str()).unwrap();
                list.push(ret_m);
            })
            .count();
        o_map
    }
} // end of impl OrderReplicaRefundUseCase

impl OrderReplicaInventoryUseCase {
    async fn load_reserving(
        &self,
        start: DateTime<FixedOffset>,
        end: DateTime<FixedOffset>,
    ) -> DefaultResult<Vec<OrderReplicaStockReservingDto>, AppError> {
        let result = self._load_reserving(start, end).await;
        if let Err(e) = &result {
            let logctx_p = &self.logctx;
            app_log_event!(logctx_p, AppLogLevel::ERROR, "replica: {:?}", e);
        }
        result
    }
    async fn _load_reserving(
        &self,
        start: DateTime<FixedOffset>,
        end: DateTime<FixedOffset>,
    ) -> DefaultResult<Vec<OrderReplicaStockReservingDto>, AppError> {
        let mut out = vec![];
        let order_ids = self.o_repo.fetch_ids_by_created_time(start, end).await?;
        for oid in order_ids {
            let olines = self.o_repo.fetch_all_lines(oid.clone()).await?;
            let usr_id = self.o_repo.owner_id(oid.as_str()).await?;
            let create_time = self.o_repo.created_time(oid.as_str()).await?;
            let shipping = self.o_repo.fetch_shipping(oid.clone()).await?;
            let obj = OrderReplicaStockReservingDto {
                oid,
                usr_id,
                create_time,
                shipping: shipping.into(),
                lines: olines.into_iter().map(OrderLineModel::into).collect(),
            };
            out.push(obj);
        }
        Ok(out)
    }
    async fn load_returning(
        &self,
        start: DateTime<FixedOffset>,
        end: DateTime<FixedOffset>,
    ) -> DefaultResult<Vec<OrderReplicaStockReturningDto>, AppError> {
        let logctx_p = self.logctx.as_ref();
        let combo = self.ret_repo.fetch_by_created_time(start, end).await?;
        let mut ret_intermediate = vec![];
        for (oid, ret_m) in combo {
            let usr_id = self.o_repo.owner_id(oid.as_str()).await?;
            app_log_event!(
                logctx_p,
                AppLogLevel::DEBUG,
                "oid:{}, usr:{}",
                oid.as_str(),
                usr_id
            );
            ret_intermediate.push((oid, usr_id, ret_m));
        }
        let mut ret_map: HashMap<String, OrderReplicaStockReturningDto> = HashMap::new();
        ret_intermediate
            .into_iter()
            .map(|(oid, usr_id, ret_m)| {
                if !ret_map.contains_key(oid.as_str()) {
                    let n = OrderReplicaStockReturningDto {
                        oid: oid.clone(),
                        usr_id,
                        lines: Vec::new(),
                    };
                    let _ = ret_map.insert(oid.clone(), n);
                }
                let obj = ret_map.get_mut(oid.as_str()).unwrap();
                let ret_dtos: Vec<OrderLineStockReturningDto> = ret_m.into();
                app_log_event!(
                    logctx_p,
                    AppLogLevel::DEBUG,
                    "oid :{}, dto size :{}",
                    oid.as_str(),
                    ret_dtos.len()
                );
                obj.lines.extend(ret_dtos.into_iter());
            })
            .count();
        let out = ret_map.into_values().collect();
        Ok(out)
    }
    pub async fn execute(
        self,
        req: OrderReplicaInventoryReqDto,
    ) -> DefaultResult<OrderReplicaInventoryDto, AppError> {
        // TODO, avoid loading too many order records, consider pagination
        let (start, end) = (req.start, req.end);
        #[allow(clippy::clone_on_copy)] // for learning purpose
        let reservations = self.load_reserving(start.clone(), end.clone()).await?;
        let returns = self.load_returning(start, end).await?;
        let resp = OrderReplicaInventoryDto {
            reservations,
            returns,
        };
        Ok(resp)
    } // end of fn execute
} // end of impl OrderReplicaInventoryUseCase

impl OrderPaymentUpdateUseCase {
    pub async fn execute(
        self,
        data: OrderPaymentUpdateDto,
    ) -> DefaultResult<OrderPaymentUpdateErrorDto, AppError> {
        let result = DateTime::parse_from_rfc3339(data.charge_time.as_str());
        if let Err(e) = result {
            let client_e = OrderPaymentUpdateErrorDto {
                oid: data.oid,
                charge_time: Some(e.to_string()),
                lines: Vec::new(),
            };
            Ok(client_e)
        } else {
            self.repo
                .update_lines_payment(data, Self::_update_payment)
                .await
        }
    }

    fn _update_payment(
        saved_lines: &mut Vec<OrderLineModel>,
        data: OrderPaymentUpdateDto,
    ) -> Vec<OrderLinePayUpdateErrorDto> {
        let OrderPaymentUpdateDto {
            oid: _,
            lines,
            charge_time,
        } = data;
        let ctime = DateTime::parse_from_rfc3339(charge_time.as_str()).unwrap();
        OrderLineModel::update_payments(saved_lines, lines, ctime)
    }
}

impl OrderDiscardUnpaidItemsUseCase {
    pub fn new(repo: Box<dyn AbsOrderRepo>, logctx: Arc<AppLogContext>) -> Self {
        Self { repo, logctx }
    }

    pub async fn execute(self) -> DefaultResult<(), AppError> {
        let time_start = self.repo.cancel_unpaid_last_time().await?;
        let time_end = LocalTime::now().fixed_offset();
        let result = self
            .repo
            .fetch_lines_by_rsvtime(time_start, time_end, Self::read_oline_set_cb)
            .await;
        if let Err(e) = result.as_ref() {
            let lctx = &self.logctx;
            app_log_event!(lctx, AppLogLevel::ERROR, "error: {:?}", e);
        } else {
            self.repo.cancel_unpaid_time_update().await?;
        }
        result
    }
    fn read_oline_set_cb<'a>(
        o_repo: &'a dyn AbsOrderRepo,
        ol_set: OrderLineModelSet,
    ) -> Pin<Box<dyn Future<Output = DefaultResult<(), AppError>> + Send + 'a>> {
        let fut = async move {
            let (order_id, unpaid_lines) = (
                ol_set.order_id,
                ol_set
                    .lines
                    .into_iter()
                    .filter(|m| m.qty.has_unpaid())
                    .collect::<Vec<OrderLineModel>>(),
            );
            if unpaid_lines.is_empty() {
                Ok(()) // all items have been paid, nothing to discard for now.
            } else {
                let st_repo = o_repo.stock();
                let items = unpaid_lines.into_iter().map(OrderLineModel::into).collect();
                let data = StockLevelReturnDto { items, order_id };
                let _return_result = st_repo.try_return(Self::read_stocklvl_cb, data).await?;
                Ok(()) // TODO, logging the stock-return result, the result may not be able
                       // to pass to the output of the method `fetch_lines_by_rsvtime`
            }
        }; // lifetime of the Future trait object must outlive `'static`
        Box::pin(fut)
    }
    fn read_stocklvl_cb(
        ms: &mut StockLevelModelSet,
        data: StockLevelReturnDto,
    ) -> Vec<StockReturnErrorDto> {
        ms.return_across_expiry(data)
    }
} // end of impl OrderDiscardUnpaidItemsUseCase

pub enum ReturnLinesReqUcOutput {
    Success,
    InvalidOwner,
    PermissionDeny,
    InvalidRequest(Vec<OrderLineReturnErrorDto>),
}

impl ReturnLinesReqUseCase {
    pub async fn execute(
        self,
        oid: String,
        data: Vec<OrderLineReqDto>,
    ) -> DefaultResult<ReturnLinesReqUcOutput, AppError> {
        if !self
            .authed_claim
            .contain_permission(AppAuthPermissionCode::can_create_return_req)
        {
            return Ok(ReturnLinesReqUcOutput::PermissionDeny);
        }
        let o_usr_id = self.o_repo.owner_id(oid.as_str()).await?;
        if o_usr_id != self.authed_claim.profile {
            return Ok(ReturnLinesReqUcOutput::InvalidOwner);
        }
        let pids = data
            .iter()
            .map(OrderLineIdentity::from)
            .collect::<Vec<OrderLineIdentity>>();
        let o_lines = self
            .o_repo
            .fetch_lines_by_pid(oid.as_str(), pids.clone())
            .await?;
        let o_returned = self.or_repo.fetch_by_pid(oid.as_str(), pids).await?;
        match OrderReturnModel::filter_requests(data, o_lines, o_returned) {
            Ok(modified) => {
                let _num = self.or_repo.create(oid.as_str(), modified).await?;
                Ok(ReturnLinesReqUcOutput::Success)
            }
            Err(errors) => Ok(ReturnLinesReqUcOutput::InvalidRequest(errors)),
        }
    }
} // end of impl ReturnLinesReqUseCase
