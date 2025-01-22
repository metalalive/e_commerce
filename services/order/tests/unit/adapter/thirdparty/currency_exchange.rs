use std::boxed::Box;
use std::env;

use rust_decimal::Decimal;

use ecommerce_common::api::dto::CurrencyDto;
use ecommerce_common::confidentiality::UserSpaceConfidentiality;
use ecommerce_common::constant::env_vars::SYS_BASEPATH;
use order::AppSharedState;

use crate::ut_setup_share_state;

fn ut_appstate_setup() -> AppSharedState {
    let cfdntl = {
        let sys_basepath = env::var(SYS_BASEPATH).unwrap();
        let path = sys_basepath.clone() + "/common/data/secrets.json";
        UserSpaceConfidentiality::build(path)
    };
    ut_setup_share_state("config_ok_no_sqldb.json", Box::new(cfdntl))
}

/// Note I use free plan in `openexchangerates.org` service, that means this project
/// (including all test cases always have USD as the base currency.
#[tokio::test]
async fn refresh_ok() {
    let shrstate = ut_appstate_setup();
    let currency_ctx = shrstate.currency();
    let chosen = vec![
        CurrencyDto::TWD,
        CurrencyDto::INR,
        CurrencyDto::THB,
        CurrencyDto::USD,
        CurrencyDto::IDR,
    ];
    let result = currency_ctx.refresh(chosen).await;
    if let Err(e) = &result {
        println!("[error] after-3rdparty-currency-ex : {:?}", e);
    }
    assert!(result.is_ok());
    let ms = result.unwrap();
    assert!(matches!(ms.base, CurrencyDto::USD));
    assert_eq!(ms.exchange_rates.len(), 5);
    ms.exchange_rates
        .into_iter()
        .map(|c| {
            println!(
                "[debug] name:{} , rate: {}",
                c.name.to_string(),
                c.rate.to_string()
            );
            match c.name {
                CurrencyDto::TWD => {
                    let hi = Decimal::new(359i64, 1u32);
                    let lo = Decimal::new(273i64, 1u32);
                    assert!(c.rate < hi);
                    assert!(c.rate > lo);
                }
                CurrencyDto::INR => {
                    let hi = Decimal::new(90i64, 0u32);
                    let lo = Decimal::new(68i64, 0u32);
                    assert!(c.rate < hi);
                    assert!(c.rate > lo);
                }
                CurrencyDto::THB => {
                    let hi = Decimal::new(5298i64, 2u32);
                    let lo = Decimal::new(2020i64, 2u32);
                    assert!(c.rate < hi);
                    assert!(c.rate > lo);
                }
                CurrencyDto::USD => {
                    let expect = Decimal::new(10i64, 1u32);
                    assert_eq!(c.rate, expect);
                }
                CurrencyDto::IDR => {
                    let hi = Decimal::new(17800i64, 0u32);
                    let lo = Decimal::new(12990i64, 0u32);
                    assert!(c.rate < hi);
                    assert!(c.rate > lo);
                }
                CurrencyDto::Unknown => assert!(false),
            };
        })
        .count();
} // end of fn refresh_ok
