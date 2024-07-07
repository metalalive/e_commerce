use rust_decimal::Decimal;

use ecommerce_common::api::dto::CurrencyDto;
use order::datastore::AppInMemoryDStore;
use order::model::{CurrencyModel, CurrencyModelSet};
use order::repository::{AbsCurrencyRepo, CurrencyInMemRepo};

use crate::repository::in_mem::in_mem_ds_ctx_setup;

async fn in_mem_repo_ds_setup(max_items: u32) -> CurrencyInMemRepo {
    let ds_ctx = in_mem_ds_ctx_setup::<AppInMemoryDStore>(max_items);
    let inmem = ds_ctx.in_mem.as_ref().unwrap().clone();
    let result = CurrencyInMemRepo::new(inmem).await;
    assert_eq!(result.is_ok(), true);
    result.unwrap()
}

fn ut_setup_currency_ms(data: Vec<(CurrencyDto, i64, u32)>) -> CurrencyModelSet {
    let exchange_rates = data
        .into_iter()
        .map(|(name, num, radix)| {
            let rate = Decimal::new(num, radix);
            CurrencyModel { name, rate }
        })
        .collect();
    let base = CurrencyDto::USD;
    CurrencyModelSet {
        base,
        exchange_rates,
    }
}

#[tokio::test]
async fn save_fetch_ok() {
    let repo = in_mem_repo_ds_setup(20).await;
    let mocked_data = vec![
        (CurrencyDto::INR, 840331905, 7),
        (CurrencyDto::TWD, 3009, 2),
    ];
    let ms = ut_setup_currency_ms(mocked_data);
    let result = repo.save(ms).await;
    assert!(result.is_ok());
    let keys = vec![CurrencyDto::TWD, CurrencyDto::INR];
    let result = repo.fetch(keys).await;
    assert!(result.is_ok());
    if let Ok(ms) = result {
        assert!(matches!(ms.base, CurrencyDto::USD));
        assert_eq!(ms.exchange_rates.len(), 2);
        ms.exchange_rates
            .into_iter()
            .map(|m| {
                let expect = match m.name {
                    CurrencyDto::INR => "84.0331905",
                    CurrencyDto::TWD => "30.09",
                    _others => "0.000",
                }
                .to_string();
                let actual = m.rate.to_string();
                assert_eq!(actual, expect);
            })
            .count();
    }
    // --------- subcase #2 ----------
    let mocked_data = vec![(CurrencyDto::IDR, 1350187, 2), (CurrencyDto::TWD, 32071, 3)];
    let ms = ut_setup_currency_ms(mocked_data);
    let result = repo.save(ms).await;
    assert!(result.is_ok());
    let keys = vec![CurrencyDto::TWD, CurrencyDto::IDR, CurrencyDto::INR];
    let result = repo.fetch(keys).await;
    assert!(result.is_ok());
    if let Ok(ms) = result {
        assert!(matches!(ms.base, CurrencyDto::USD));
        assert_eq!(ms.exchange_rates.len(), 3);
        ms.exchange_rates
            .into_iter()
            .map(|m| {
                let expect = match m.name {
                    CurrencyDto::INR => "84.0331905",
                    CurrencyDto::TWD => "32.071",
                    CurrencyDto::IDR => "13501.87",
                    _others => "0.000",
                }
                .to_string();
                let actual = m.rate.to_string();
                assert_eq!(actual, expect);
            })
            .count();
    }
} // end of fn save_fetch_ok
