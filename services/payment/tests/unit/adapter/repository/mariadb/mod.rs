mod charge;
mod merchant;
mod order_replica;
mod payout;
mod refund;
mod reporting;

use std::boxed::Box;
use std::collections::HashMap;
use std::sync::Arc;

use rust_decimal::Decimal;

use ecommerce_common::api::dto::CurrencyDto;
use payment::adapter::repository::{
    app_repo_charge, app_repo_merchant, app_repo_refund, app_repo_reporting, AbstractChargeRepo,
    AbstractMerchantRepo, AbstractRefundRepo, AbstractReportingRepo,
};
use payment::model::OrderCurrencySnapshot;
use payment::AppSharedState;

use crate::model::ut_default_currency_snapshot;

async fn ut_setup_db_charge_repo(shr_state: AppSharedState) -> Arc<Box<dyn AbstractChargeRepo>> {
    let dstore = shr_state.datastore();
    let result = app_repo_charge(dstore).await;
    let repo = result.unwrap();
    Arc::new(repo)
}

async fn ut_setup_db_merchant_repo(
    shr_state: AppSharedState,
) -> Arc<Box<dyn AbstractMerchantRepo>> {
    let dstore = shr_state.datastore();
    let result = app_repo_merchant(dstore).await;
    let repo = result.unwrap();
    Arc::new(repo)
}

async fn ut_setup_db_refund_repo(shr_state: AppSharedState) -> Arc<Box<dyn AbstractRefundRepo>> {
    let dstore = shr_state.datastore();
    let result = app_repo_refund(dstore).await;
    let repo = result.unwrap();
    Arc::new(repo)
}

async fn ut_setup_db_reporting_repo(
    shr_state: AppSharedState,
) -> Arc<Box<dyn AbstractReportingRepo>> {
    let dstore = shr_state.datastore();
    let result = app_repo_reporting(dstore).await;
    let repo = result.unwrap();
    Arc::new(repo)
}

fn ut_setup_currency_snapshot(usr_ids: Vec<u32>) -> HashMap<u32, OrderCurrencySnapshot> {
    let mut out = ut_default_currency_snapshot(usr_ids);
    let mut mock_rates = [
        (CurrencyDto::INR, Decimal::new(82559, 3)),
        (CurrencyDto::THB, Decimal::new(380415, 4)),
        (CurrencyDto::IDR, Decimal::new(163082101, 4)),
        (CurrencyDto::USD, Decimal::new(10, 1)),
    ]
    .into_iter();
    let _ = out
        .iter_mut()
        .map(|(_usr_id, cs)| {
            let data = mock_rates.next().unwrap();
            cs.label = data.0;
            cs.rate = data.1;
        })
        .count();
    out
}

fn ut_verify_currency_snapshot(given: &OrderCurrencySnapshot) {
    let expect = match &given.label {
        CurrencyDto::INR => Decimal::new(82559, 3),
        CurrencyDto::THB => Decimal::new(380415, 4),
        CurrencyDto::IDR => Decimal::new(163082101, 4),
        CurrencyDto::USD => Decimal::new(10, 1),
        CurrencyDto::TWD => Decimal::new(321, 1),
        _others => Decimal::ZERO,
    };
    assert_eq!(given.rate, expect);
}
