use chrono::{Duration, Local};
use rust_decimal::Decimal;

use ecommerce_common::api::dto::{CurrencyDto, OrderCurrencySnapshotDto};
use ecommerce_common::error::AppErrorCode;
use order::api::web::dto::OrderCreateRespOkDto;
use order::model::{CurrencyModel, CurrencyModelSet, OrderCurrencyModel, OrderLineModelSet};

use super::order_line::ut_setup_order_lines;

fn ut_setup_currency_mset(data: Vec<(CurrencyDto, i64, u32)>) -> CurrencyModelSet {
    let base = CurrencyDto::USD;
    let exchange_rates = data
        .into_iter()
        .map(|d| CurrencyModel {
            name: d.0,
            rate: Decimal::new(d.1, d.2),
        })
        .collect::<Vec<_>>();
    CurrencyModelSet {
        base,
        exchange_rates,
    }
}

#[test]
fn build_currency_model() {
    let search_scope = {
        let data = vec![
            (CurrencyDto::TWD, 32046, 3),
            (CurrencyDto::INR, 834094, 4),
            (CurrencyDto::IDR, 163110943, 4),
        ];
        ut_setup_currency_mset(data)
    };
    let buyer_label = CurrencyDto::TWD;
    let seller_labels = vec![
        (2603u32, CurrencyDto::TWD),
        (9442, CurrencyDto::IDR),
        (8302, CurrencyDto::INR),
        (3034, CurrencyDto::TWD),
        (8901, CurrencyDto::INR),
    ];
    let args = (search_scope, buyer_label, seller_labels);
    let result = OrderCurrencyModel::try_from(args);
    assert!(result.is_ok());
    if let Ok(v) = result {
        assert_eq!(v.buyer.name, CurrencyDto::TWD);
        assert_eq!(v.buyer.rate.to_string().as_str(), "32.046");
        let found = v.sellers.get(&8901u32).unwrap();
        assert_eq!(found.name, CurrencyDto::INR);
        assert_eq!(found.rate.to_string().as_str(), "83.4094");
        let found = v.sellers.get(&9442u32).unwrap();
        assert_eq!(found.name, CurrencyDto::IDR);
        assert_eq!(found.rate.to_string().as_str(), "16311.0943");
    }
} // end of fn build_currency_model

pub(super) fn ut_common_order_currency(seller_ids: [u32; 3]) -> OrderCurrencyModel {
    let search_scope = {
        let data = vec![
            (CurrencyDto::TWD, 32047, 3),
            (CurrencyDto::INR, 834095, 4),
            (CurrencyDto::IDR, 163019430, 4),
        ];
        ut_setup_currency_mset(data)
    };
    let buyer_label = CurrencyDto::TWD;
    let seller_labels = vec![
        (seller_ids[0], CurrencyDto::TWD),
        (seller_ids[1], CurrencyDto::IDR),
        (seller_ids[2], CurrencyDto::INR),
    ];
    let args = (search_scope, buyer_label, seller_labels);
    let result = OrderCurrencyModel::try_from(args);
    assert!(result.is_ok());
    result.unwrap()
}

#[test]
fn currency_estimate_buyer_rate_ok() {
    let mock_seller_ids = [2603u32, 9442, 8901];
    let v = ut_common_order_currency(mock_seller_ids);
    let actual = v.to_buyer_rate(mock_seller_ids[1]).unwrap();
    assert_eq!(actual.name, CurrencyDto::TWD);
    let rate2buyer_currency = actual.rate.trunc_with_scale(10).to_string();
    assert_eq!(rate2buyer_currency.as_str(), "0.0019658392");
    let actual = v.to_buyer_rate(mock_seller_ids[2]).unwrap();
    let rate2buyer_currency = actual.rate.trunc_with_scale(10).to_string();
    assert_eq!(rate2buyer_currency.as_str(), "0.3842128294");
    let actual = v.to_buyer_rate(mock_seller_ids[0]).unwrap();
    let rate2buyer_currency = actual.rate.trunc_with_scale(4).to_string();
    assert_eq!(rate2buyer_currency.as_str(), "1.0000");
} // end of fn currency_estimate_buyer_rate_ok

#[test]
fn currency_estimate_buyer_rate_err_div0() {
    let seller_id = 123;
    let search_scope = {
        let data = vec![(CurrencyDto::TWD, 32047, 3), (CurrencyDto::IDR, 0, 4)];
        ut_setup_currency_mset(data)
    };
    let buyer_label = CurrencyDto::TWD;
    let seller_labels = vec![(seller_id, CurrencyDto::IDR)];
    let args = (search_scope, buyer_label, seller_labels);
    let result = OrderCurrencyModel::try_from(args);
    assert!(result.is_ok());
    let curr_m = result.unwrap();
    let result = curr_m.to_buyer_rate(seller_id);
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(matches!(e.code, AppErrorCode::DataCorruption));
    }
} // end of fn currency_estimate_buyer_rate_err_div0

#[test]
fn currency_to_rpc_replica_dto() {
    let mock_seller_ids = [2615u32, 8299, 1031];
    let m = ut_common_order_currency(mock_seller_ids);
    let result = OrderCurrencySnapshotDto::try_from(m);
    assert!(result.is_ok());
    let v = result.unwrap();
    assert_eq!(v.buyer, CurrencyDto::TWD);
    v.sellers
        .iter()
        .map(|item| {
            let expect_label = match item.seller_id {
                2615 => CurrencyDto::TWD,
                8299 => CurrencyDto::IDR,
                1031 => CurrencyDto::INR,
                _others => CurrencyDto::Unknown,
            };
            assert_eq!(item.currency, expect_label);
        })
        .count();
    v.snapshot
        .iter()
        .map(|item| {
            let expect_rate = match &item.name {
                CurrencyDto::TWD => "32.047",
                CurrencyDto::INR => "83.4095",
                CurrencyDto::IDR => "16301.9430",
                _others => "0.000",
            };
            assert_eq!(item.rate.to_string().as_str(), expect_rate);
        })
        .count();
} // end of fn currency_to_rpc_replica_dto

#[test]
fn order_to_web_resp_dto_ok() {
    let mock_ctime = Local::now().fixed_offset();
    let mock_seller_ids = [2379u32, 8964, 9982];
    #[cfg_attr(rustfmt, rustfmt_skip)]
    let mock_olines_data = vec![
        // seller uses INR
        ((mock_seller_ids[2], 66049, 0), (35, 140), 4, 0, None,
         mock_ctime + Duration::hours(2), mock_ctime + Duration::days(14), None),
        // seller uses IDR
        ((mock_seller_ids[1], 1082, 0), (57000, 114000), 2, 0, None,
         mock_ctime + Duration::hours(1), mock_ctime + Duration::days(1), None),
        // seller uses TWD
        ((mock_seller_ids[0], 1617, 0), (215, 1075), 5, 0, None,
         mock_ctime + Duration::hours(4), mock_ctime + Duration::days(180), None),
        // seller uses INR
        ((mock_seller_ids[2], 50129, 0), (426, 2656), 6, 0, None,
         mock_ctime + Duration::hours(8), mock_ctime + Duration::days(16), None),
    ];
    let model = {
        let order_id = "extremelyInDepth".to_string();
        let owner_id = 1234;
        let currency = ut_common_order_currency(mock_seller_ids);
        let lines = ut_setup_order_lines(mock_olines_data);
        let args = (order_id, owner_id, mock_ctime, currency, lines);
        OrderLineModelSet::try_from(args).unwrap()
    };
    let result = OrderCreateRespOkDto::try_from(model);
    assert!(result.is_ok());
    if let Ok(v) = result {
        let OrderCreateRespOkDto {
            order_id: _,
            usr_id: _,
            time: _,
            currency: exrate_applied,
            reserved_lines,
        } = v;
        reserved_lines
            .into_iter()
            .map(|item| {
                let key = (item.seller_id, item.product_id);
                #[cfg_attr(rustfmt, rustfmt_skip)]
                let expect = match key {
                    // expected amount with more decimal places,
                    // "13.44744870", "53.78979480"
                    (9982, 66049) => (CurrencyDto::INR, "13.44", "53.78"),
                    // "112.05231000", "224.10462000"
                    (8964, 1082) => (CurrencyDto::IDR, "112.05", "224.10"),
                    // "215.00000000", "1075.00000000"
                    (2379, 1617) => (CurrencyDto::TWD, "215.00", "1075.00"),
                    // "163.67466132", "1020.46924992"
                    (9982, 50129) => (CurrencyDto::INR, "163.67", "1020.46"),
                    _others => (CurrencyDto::Unknown, "-0.00", "-0.00"),
                };
                let actual_currency = exrate_applied
                    .sellers
                    .iter()
                    .find(|r| r.seller_id == item.seller_id)
                    .map(|r| r.currency.clone())
                    .unwrap();
                assert_eq!(actual_currency, expect.0);
                assert_eq!(item.amount.unit.as_str(), expect.1);
                assert_eq!(item.amount.total.as_str(), expect.2);
            })
            .count();
    }
} // end of fn order_to_web_resp_dto_ok
