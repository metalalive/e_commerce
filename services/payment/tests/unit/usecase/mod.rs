mod create_charge;
mod onboard;
mod refresh_charge_status;

use std::boxed::Box;
use std::result::Result;
use std::sync::Mutex;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use ecommerce_common::api::rpc::dto::StoreProfileReplicaDto;
use ecommerce_common::model::order::BillingModel;

use payment::adapter::cache::{AbstractOrderSyncLockCache, OrderSyncLockError};
use payment::adapter::processor::{
    AbstractPaymentProcessor, AppProcessorError, AppProcessorMerchantResult,
    AppProcessorPayInResult,
};
use payment::adapter::repository::{AbstractChargeRepo, AbstractMerchantRepo, AppRepoError};
use payment::adapter::rpc::{
    AbsRpcClientContext, AbstractRpcClient, AbstractRpcContext, AbstractRpcPublishEvent,
    AppRpcClientRequest, AppRpcCtxError, AppRpcReply,
};
use payment::api::web::dto::{PaymentMethodReqDto, StoreOnboardReqDto};
use payment::model::{
    Charge3partyModel, ChargeBuyerMetaModel, ChargeBuyerModel, ChargeLineBuyerModel,
    Merchant3partyModel, MerchantProfileModel, OrderLineModelSet,
};

struct MockChargeRepo {
    _expect_unpaid_olines: Mutex<Option<Result<Option<OrderLineModelSet>, AppRepoError>>>,
    _create_order_result: Mutex<Option<Result<(), AppRepoError>>>,
    _create_charge_result: Mutex<Option<Result<(), AppRepoError>>>,
    _read_charge_meta: Mutex<Option<Result<Option<ChargeBuyerMetaModel>, AppRepoError>>>,
    _read_all_charge_lines: Mutex<Option<Result<Vec<ChargeLineBuyerModel>, AppRepoError>>>,
    _update_chargemeta_result: Mutex<Option<Result<(), AppRepoError>>>,
}

impl MockChargeRepo {
    fn build(
        unpaid_olines: Option<Result<Option<OrderLineModelSet>, AppRepoError>>,
        create_order_res: Option<Result<(), AppRepoError>>,
        create_charge_res: Option<Result<(), AppRepoError>>,
        chargemeta: Option<Result<Option<ChargeBuyerMetaModel>, AppRepoError>>,
        all_chargelines: Option<Result<Vec<ChargeLineBuyerModel>, AppRepoError>>,
        update_meta_res: Option<Result<(), AppRepoError>>,
    ) -> Box<dyn AbstractChargeRepo> {
        Box::new(Self {
            _expect_unpaid_olines: Mutex::new(unpaid_olines),
            _create_order_result: Mutex::new(create_order_res),
            _create_charge_result: Mutex::new(create_charge_res),
            _read_charge_meta: Mutex::new(chargemeta),
            _read_all_charge_lines: Mutex::new(all_chargelines),
            _update_chargemeta_result: Mutex::new(update_meta_res),
        })
    }
} // end of impl MockChargeRepo

#[async_trait]
impl AbstractChargeRepo for MockChargeRepo {
    async fn get_unpaid_olines(
        &self,
        _usr_id: u32,
        _oid: &str,
    ) -> Result<Option<OrderLineModelSet>, AppRepoError> {
        let mut g = self._expect_unpaid_olines.lock().unwrap();
        let out = g.take().unwrap();
        out
    }
    async fn create_order(
        &self,
        _olines: &OrderLineModelSet,
        _billing: &BillingModel,
    ) -> Result<(), AppRepoError> {
        let mut g = self._create_order_result.lock().unwrap();
        let out = g.take().unwrap();
        out
    }
    async fn create_charge(&self, _cline_set: ChargeBuyerModel) -> Result<(), AppRepoError> {
        let mut g = self._create_charge_result.lock().unwrap();
        let out = g.take().unwrap();
        out
    }
    async fn fetch_charge_meta(
        &self,
        _usr_id: u32,
        _create_time: DateTime<Utc>,
    ) -> Result<Option<ChargeBuyerMetaModel>, AppRepoError> {
        let mut g = self._read_charge_meta.lock().unwrap();
        let out = g.take().unwrap();
        out
    }
    async fn fetch_all_charge_lines(
        &self,
        _usr_id: u32,
        _create_time: DateTime<Utc>,
    ) -> Result<Vec<ChargeLineBuyerModel>, AppRepoError> {
        let mut g = self._read_all_charge_lines.lock().unwrap();
        let out = g.take().unwrap();
        out
    }
    async fn update_charge_progress(
        &self,
        _meta: ChargeBuyerMetaModel,
    ) -> Result<(), AppRepoError> {
        let mut g = self._update_chargemeta_result.lock().unwrap();
        let out = g.take().unwrap();
        out
    }
} // end of impl MockChargeRepo

struct MockMerchantRepo {
    _create_result: Mutex<Option<Result<(), AppRepoError>>>,
    _fetch_result: Mutex<Option<(MerchantProfileModel, Merchant3partyModel)>>,
    _update3pty_result: Mutex<Option<Result<(), AppRepoError>>>,
} // end of trait AbstractMerchantRepo

impl MockMerchantRepo {
    fn build(
        create_res: Option<Result<(), AppRepoError>>,
        fetch_res: Option<(MerchantProfileModel, Merchant3partyModel)>,
        update3pt_res: Option<Result<(), AppRepoError>>,
    ) -> Box<dyn AbstractMerchantRepo> {
        let obj = Self {
            _create_result: Mutex::new(create_res),
            _fetch_result: Mutex::new(fetch_res),
            _update3pty_result: Mutex::new(update3pt_res),
        };
        Box::new(obj)
    }
}

#[async_trait]
impl AbstractMerchantRepo for MockMerchantRepo {
    async fn create(
        &self,
        _mprof: MerchantProfileModel,
        _m3pty: Merchant3partyModel,
    ) -> Result<(), AppRepoError> {
        let mut g = self._create_result.lock().unwrap();
        let out = g.take().unwrap();
        out
    }
    async fn fetch(
        &self,
        _store_id: u32,
        _label3pty: &StoreOnboardReqDto,
    ) -> Result<Option<(MerchantProfileModel, Merchant3partyModel)>, AppRepoError> {
        let mut g = self._fetch_result.lock().unwrap();
        let out = g.take();
        Ok(out)
    }
    async fn update_3party(
        &self,
        _store_id: u32,
        _m3pty: Merchant3partyModel,
    ) -> Result<(), AppRepoError> {
        let mut g = self._update3pty_result.lock().unwrap();
        let out = g.take().unwrap();
        out
    }
} // end of impl MockMerchantRepo

struct MockOrderSyncLockCache {
    _acquire_result: Mutex<Option<Result<bool, OrderSyncLockError>>>,
    _release_result: Mutex<Option<Result<(), OrderSyncLockError>>>,
}

#[async_trait]
impl AbstractOrderSyncLockCache for MockOrderSyncLockCache {
    async fn acquire(&self, _usr_id: u32, _oid: &str) -> Result<bool, OrderSyncLockError> {
        let mut g = self._acquire_result.lock().unwrap();
        let out = g.take().unwrap();
        out
    }
    async fn release(&self, _usr_id: u32, _oid: &str) -> Result<(), OrderSyncLockError> {
        let mut g = self._release_result.lock().unwrap();
        let out = g.take().unwrap();
        out
    }
}

struct MockRpcContext {
    _acquire_result: Mutex<Option<Result<Box<dyn AbstractRpcClient>, AppRpcCtxError>>>,
}
struct MockRpcClient {
    _send_req_result: Mutex<Option<Result<Box<dyn AbstractRpcPublishEvent>, AppRpcCtxError>>>,
}
struct MockRpcPublishEvent {
    _recv_resp_result: Mutex<Option<Result<AppRpcReply, AppRpcCtxError>>>,
}

impl AbstractRpcContext for MockRpcContext {}

#[async_trait]
impl AbsRpcClientContext for MockRpcContext {
    async fn acquire(&self) -> Result<Box<dyn AbstractRpcClient>, AppRpcCtxError> {
        let mut g = self._acquire_result.lock().unwrap();
        let out = g.take().unwrap();
        out
    }
}
impl MockRpcContext {
    fn build(
        acquire_res: Option<Result<Box<dyn AbstractRpcClient>, AppRpcCtxError>>,
    ) -> Box<dyn AbstractRpcContext> {
        Box::new(Self {
            _acquire_result: Mutex::new(acquire_res),
        })
    }
}

#[async_trait]
impl AbstractRpcClient for MockRpcClient {
    async fn send_request(
        mut self: Box<Self>,
        _props: AppRpcClientRequest,
    ) -> Result<Box<dyn AbstractRpcPublishEvent>, AppRpcCtxError> {
        let mut g = self._send_req_result.lock().unwrap();
        let evt = g.take().unwrap();
        evt
    }
}
impl MockRpcClient {
    fn build(
        send_req_res: Option<Result<Box<dyn AbstractRpcPublishEvent>, AppRpcCtxError>>,
    ) -> Box<dyn AbstractRpcClient> {
        Box::new(Self {
            _send_req_result: Mutex::new(send_req_res),
        })
    }
}

#[async_trait]
impl AbstractRpcPublishEvent for MockRpcPublishEvent {
    async fn receive_response(&mut self) -> Result<AppRpcReply, AppRpcCtxError> {
        let mut g = self._recv_resp_result.lock().unwrap();
        let mock_result = g.take().unwrap();
        mock_result
    }
}
impl MockRpcPublishEvent {
    fn build(
        recv_resp: Option<Result<AppRpcReply, AppRpcCtxError>>,
    ) -> Box<dyn AbstractRpcPublishEvent> {
        Box::new(Self {
            _recv_resp_result: Mutex::new(recv_resp),
        })
    }
}

struct MockPaymentProcessor {
    _payin_start_result:
        Mutex<Option<Result<(AppProcessorPayInResult, Charge3partyModel), AppProcessorError>>>,
    _payin_progress_result: Mutex<Option<Result<Charge3partyModel, AppProcessorError>>>,
    _onboard_merchant_result: Mutex<Option<Result<AppProcessorMerchantResult, AppProcessorError>>>,
}

impl MockPaymentProcessor {
    fn build(
        payin_start: Option<
            Result<(AppProcessorPayInResult, Charge3partyModel), AppProcessorError>,
        >,
        payin_progress: Option<Result<Charge3partyModel, AppProcessorError>>,
        onboard_merchant_arg: Option<Result<AppProcessorMerchantResult, AppProcessorError>>,
    ) -> Box<dyn AbstractPaymentProcessor> {
        Box::new(Self {
            _payin_start_result: Mutex::new(payin_start),
            _payin_progress_result: Mutex::new(payin_progress),
            _onboard_merchant_result: Mutex::new(onboard_merchant_arg),
        })
    }
}

#[async_trait]
impl AbstractPaymentProcessor for MockPaymentProcessor {
    async fn pay_in_start(
        &self,
        _charge_buyer: &ChargeBuyerModel,
        _req_mthd: PaymentMethodReqDto,
    ) -> Result<(AppProcessorPayInResult, Charge3partyModel), AppProcessorError> {
        let mut g = self._payin_start_result.lock().unwrap();
        let out = g.take().unwrap();
        out
    }

    async fn pay_in_progress(
        &self,
        _meta: &ChargeBuyerMetaModel,
    ) -> Result<Charge3partyModel, AppProcessorError> {
        let mut g = self._payin_progress_result.lock().unwrap();
        let out = g.take().unwrap();
        out
    }

    async fn onboard_merchant(
        &self,
        _store_profile: StoreProfileReplicaDto,
        _req_3pt: StoreOnboardReqDto,
    ) -> Result<AppProcessorMerchantResult, AppProcessorError> {
        let mut g = self._onboard_merchant_result.lock().unwrap();
        let out = g.take().unwrap();
        out
    }

    async fn refresh_onboard_status(
        &self,
        _m3pty: Merchant3partyModel,
        _req_3pt: StoreOnboardReqDto,
    ) -> Result<AppProcessorMerchantResult, AppProcessorError> {
        let mut g = self._onboard_merchant_result.lock().unwrap();
        let out = g.take().unwrap();
        out
    }
} // end of impl MockPaymentProcessor
