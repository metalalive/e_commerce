use ecommerce_common::api::dto::CurrencyDto;
use ecommerce_common::error::AppErrorCode;

use order::repository::app_repo_currency;

use super::super::in_mem::currency::ut_setup_currency_ms;
use super::dstore_ctx_setup;

#[cfg(feature = "mariadb")]
#[tokio::test]
async fn save_fetch_ok() {
    let ds = dstore_ctx_setup();
    let repo = app_repo_currency(ds).await.unwrap();
    let mocked_ms = ut_setup_currency_ms(vec![
        (CurrencyDto::INR, 184095, 3),
        (CurrencyDto::TWD, 3090, 2),
    ]);
    let result = repo.save(mocked_ms).await;
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
                    CurrencyDto::INR => ("184.095".to_string(), 3u32),
                    CurrencyDto::TWD => ("30.90".to_string(), 2),
                    _others => ("99999".to_string(), 0),
                };
                let actual = m.rate.trunc_with_scale(expect.1).to_string();
                assert_eq!(actual, expect.0);
            })
            .count();
    }
    // ---- subcase #2 ----
    let mocked_ms = ut_setup_currency_ms(vec![
        (CurrencyDto::IDR, 135028787, 4),
        (CurrencyDto::TWD, 31072, 3),
    ]);
    let result = repo.save(mocked_ms).await;
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
                    CurrencyDto::INR => ("184.095".to_string(), 3u32),
                    CurrencyDto::TWD => ("31.072".to_string(), 3),
                    CurrencyDto::IDR => ("13502.8787".to_string(), 4),
                    _others => ("99999".to_string(), 0),
                };
                let actual = m.rate.trunc_with_scale(expect.1).to_string();
                assert_eq!(actual, expect.0);
            })
            .count();
    }
} // end of fn save_fetch_ok

#[cfg(feature = "mariadb")]
#[tokio::test]
async fn save_error_range() {
    let ds = dstore_ctx_setup();
    let repo = app_repo_currency(ds).await.unwrap();
    let mocked_ms = ut_setup_currency_ms(vec![
        (CurrencyDto::INR, 1840954, 5),
        (CurrencyDto::IDR, 30050080099, 2),
    ]);
    let result = repo.save(mocked_ms).await;
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.code, AppErrorCode::ExceedingMaxLimit);
    }
} // end of fn save_error_range
