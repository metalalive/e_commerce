use std::boxed::Box;
use std::cell::Cell;
use std::result::Result as DefaultResult;
use std::sync::Mutex;
use std::vec;

use async_trait::async_trait;
use chrono::DateTime;

use ecommerce_common::api::dto::CurrencyDto;
use ecommerce_common::error::AppErrorCode;

use crate::{ut_setup_share_state, MockConfidential};
use order::api::rpc::dto::{
    ProdAttrPriceSetDto, ProductPriceDeleteDto, ProductPriceDto, ProductPriceEditDto,
};
use order::error::AppError;
use order::model::{ProductPriceModel, ProductPriceModelSet};
use order::repository::AbsProductPriceRepo;
use order::usecase::EditProductPriceUseCase;

type RepoFetchCallArgType = (u32, Vec<u64>);

struct MockRepository {
    _mocked_del_all: DefaultResult<(), AppError>,
    _mocked_del_subset: DefaultResult<(), AppError>,
    _mocked_fetch: DefaultResult<ProductPriceModelSet, AppError>,
    _mocked_save: DefaultResult<(), AppError>,
    _callargs_fetch_actual: Mutex<Cell<RepoFetchCallArgType>>,
    _callargs_fetch_expect: Option<RepoFetchCallArgType>,
}

#[async_trait]
impl AbsProductPriceRepo for MockRepository {
    async fn delete_all(&self, _store_id: u32) -> DefaultResult<(), AppError> {
        self._mocked_del_all.clone()
    }

    async fn delete(
        &self,
        _store_id: u32,
        _ids: ProductPriceDeleteDto,
    ) -> DefaultResult<(), AppError> {
        self._mocked_del_subset.clone()
    }

    async fn fetch(
        &self,
        store_id: u32,
        ids: Vec<u64>,
    ) -> DefaultResult<ProductPriceModelSet, AppError> {
        if let Ok(g) = self._callargs_fetch_actual.lock() {
            g.set((store_id, ids));
        }
        match &self._mocked_fetch {
            Ok(m) => Ok(ProductPriceModelSet {
                store_id: m.store_id,
                currency: m.currency.clone(),
                items: m.items.clone(),
            }),
            Err(e) => Err(e.clone()),
        }
    }

    async fn fetch_many(
        &self,
        _ids: Vec<(u32, u64)>,
    ) -> DefaultResult<Vec<ProductPriceModelSet>, AppError> {
        Err(AppError {
            code: AppErrorCode::NotImplemented,
            detail: None,
        })
    }

    async fn save(&self, _updated: ProductPriceModelSet) -> DefaultResult<(), AppError> {
        // embed verification logic at here, the use case invokes `fetch()` first
        // then `save()`, just ensure the call arguments are correct.
        if let (Ok(guard), Some(expect)) = (
            self._callargs_fetch_actual.lock(),
            &self._callargs_fetch_expect,
        ) {
            let actual = guard.take();
            assert_eq!(expect.0, actual.0);
            assert_eq!(expect.1, actual.1);
        }
        self._mocked_save.clone()
    }
} // end of impl MockRepository

impl MockRepository {
    fn _new(
        _mocked_del_all: DefaultResult<(), AppError>,
        _mocked_del_subset: DefaultResult<(), AppError>,
        _mocked_fetch: DefaultResult<ProductPriceModelSet, AppError>,
        _mocked_save: DefaultResult<(), AppError>,
    ) -> Self {
        let fe_arg: RepoFetchCallArgType = (0, Vec::new());
        let _callargs_fetch_actual = Mutex::new(Cell::new(fe_arg));
        Self {
            _mocked_save,
            _mocked_del_all,
            _mocked_del_subset,
            _mocked_fetch,
            _callargs_fetch_expect: None,
            _callargs_fetch_actual,
        }
    }
    fn expect_callargs_fetch(&mut self, val: RepoFetchCallArgType) {
        self._callargs_fetch_expect = Some(val);
    }
} // end of impl MockRepository

#[rustfmt::skip]
#[tokio::test]
async fn create_ok() {
    let app_state = ut_setup_share_state("config_ok_no_sqldb.json", Box::new(MockConfidential {}));
    let logctx = app_state.log_context().clone();
    let (mocked_store_id, mocked_currency) = (12345, CurrencyDto::TWD);
    let mocked_ppset = ProductPriceModelSet {
        store_id: mocked_store_id,
        currency: mocked_currency.clone(),
        items: Vec::new(),
    };
    let mut repo = MockRepository::_new(Ok(()), Ok(()), Ok(mocked_ppset), Ok(()));
    let creating_products: Vec<_> = [
        (389, 2379, "2022-11-25T09:13:39+05:00", "2023-09-12T21:23:00+06:00"),
        (51, 2642, "2022-11-24T09:13:39+05:00", "2023-09-12T21:25:01+11:00"),
    ]
    .iter()
    .map(
        |&(price, product_id, start_after, end_before)| ProductPriceEditDto {
            price,
            product_id,
            start_after: DateTime::parse_from_rfc3339(start_after).unwrap(),
            end_before: DateTime::parse_from_rfc3339(end_before).unwrap(),
            attributes: ProdAttrPriceSetDto {
                extra_charge: Vec::new(),
                last_update: DateTime::parse_from_rfc3339("2022-10-09T01:03:55+08:00").unwrap(),
            },
        },
    )
    .collect();
    let data = ProductPriceDto {
        s_id: mocked_store_id,
        rm_all: false,
        currency: Some(mocked_currency),
        deleting: ProductPriceDeleteDto { items: None },
        updating: Vec::new(),
        creating: creating_products,
    };
    repo.expect_callargs_fetch((mocked_store_id, vec![]));
    let result = EditProductPriceUseCase::execute(Box::new(repo), data, logctx).await;
    assert!(result.is_ok());
} // end of fn create_ok

#[rustfmt::skip]
#[tokio::test]
async fn update_ok() {
    let app_state = ut_setup_share_state("config_ok_no_sqldb.json", Box::new(MockConfidential {}));
    let logctx = app_state.log_context().clone();
    let (mocked_store_id, mocked_currency) = (12345, CurrencyDto::USD);
    let mocked_ppset = ProductPriceModelSet {
        store_id: mocked_store_id,
        currency: mocked_currency.clone(),
        items: [
            (2613,1009,"2022-04-26T08:15:47+01:00","2024-08-28T21:35:26-08:00"),
            (3072,1010,"2021-03-20T10:58:39-06:00","2023-05-16T04:19:07+08:00"),
        ]
        .into_iter()
        .map(|d| {
            let t0 = DateTime::parse_from_rfc3339(d.2).unwrap();
            let t1 = DateTime::parse_from_rfc3339(d.3).unwrap();
            ProductPriceModel::from((d.0, d.1, [t0, t1, t0], None))
        })
        .collect(),
    };
    let mut repo = MockRepository::_new(Ok(()), Ok(()), Ok(mocked_ppset), Ok(()));
    let mut updating_products: Vec<_> = [
        (1322, 3072, "2021-03-29T10:58:39-06:00", "2023-05-15T04:18:07+08:00"),
        (1155, 2613, "2022-04-27T07:15:47+01:00", "2024-08-29T20:34:25-08:00"),
        // below for creating new objects
        (452, 8299, "2022-11-27T09:13:39+05:00", "2023-04-13T01:48:07+06:00"),
        (6066, 1712, "2022-01-19T06:05:39+05:00", "2024-08-31T21:44:25+11:00"),
    ]
    .iter()
    .map(
        |&(price, product_id, start_after, end_before)| ProductPriceEditDto {
            price,
            product_id,
            start_after: DateTime::parse_from_rfc3339(start_after).unwrap(),
            end_before: DateTime::parse_from_rfc3339(end_before).unwrap(),
            attributes: ProdAttrPriceSetDto {
                extra_charge: Vec::new(),
                last_update: DateTime::parse_from_rfc3339("2022-10-09T01:03:55+08:00").unwrap(),
            },
        },
    )
    .collect();
    let creating_products = updating_products.split_off(2);
    // product IDs here have to be consistent with the mocked fetched model set above
    let data = ProductPriceDto {
        s_id: mocked_store_id,
        rm_all: false,
        currency: Some(mocked_currency),
        deleting: ProductPriceDeleteDto { items: None },
        updating: updating_products,
        creating: creating_products,
    };
    repo.expect_callargs_fetch((mocked_store_id, vec![3072, 2613]));
    let result = EditProductPriceUseCase::execute(Box::new(repo), data, logctx).await;
    assert!(result.is_ok());
} // end of fn update_ok

#[tokio::test]
async fn fetch_error() {
    let app_state = ut_setup_share_state("config_ok_no_sqldb.json", Box::new(MockConfidential {}));
    let logctx = app_state.log_context().clone();
    let (mocked_store_id, expect_errmsg) = (12345, "unit-test-set-error-1");
    let mocked_currency = CurrencyDto::THB;
    let repo = MockRepository::_new(
        Ok(()),
        Ok(()),
        Err(AppError {
            code: AppErrorCode::DataTableNotExist,
            detail: Some(expect_errmsg.to_string()),
        }),
        Ok(()),
    );
    let creating_products = vec![ProductPriceEditDto {
        price: 389,
        product_id: 2379,
        start_after: DateTime::parse_from_rfc3339("2022-11-25T09:13:39+05:00").unwrap(),
        end_before: DateTime::parse_from_rfc3339("2023-09-12T21:23:00+06:00").unwrap(),
        attributes: ProdAttrPriceSetDto {
            extra_charge: Vec::new(),
            last_update: DateTime::parse_from_rfc3339("2022-10-09T01:03:55+08:00").unwrap(),
        },
    }];
    let data = ProductPriceDto {
        s_id: mocked_store_id,
        rm_all: false,
        currency: Some(mocked_currency),
        deleting: ProductPriceDeleteDto { items: None },
        updating: Vec::new(),
        creating: creating_products,
    };
    let result = EditProductPriceUseCase::execute(Box::new(repo), data, logctx).await;
    assert!(result.is_err());
    let actual_err = result.unwrap_err();
    assert_eq!(actual_err.code, AppErrorCode::DataTableNotExist);
    if let Some(msg) = &actual_err.detail {
        assert_eq!(msg.as_str(), expect_errmsg);
    }
} // end of fn fetch_error

#[tokio::test]
async fn save_error() {
    let app_state = ut_setup_share_state("config_ok_no_sqldb.json", Box::new(MockConfidential {}));
    let logctx = app_state.log_context().clone();
    let (mocked_store_id, expect_errmsg) = (12345, "unit-test-set-error-2");
    let mocked_currency = CurrencyDto::TWD;
    let mocked_ppset = ProductPriceModelSet {
        store_id: mocked_store_id,
        currency: mocked_currency.clone(),
        items: vec![{
            let t0 = DateTime::parse_from_rfc3339("2022-11-23T09:13:41+07:00").unwrap();
            let t1 = DateTime::parse_from_rfc3339("2023-10-12T21:23:00+08:00").unwrap();
            let args = (9914, 4810, [t0, t1, t0], None);
            ProductPriceModel::from(args)
        }],
    };
    let mut repo = MockRepository::_new(
        Ok(()),
        Ok(()),
        Ok(mocked_ppset),
        Err(AppError {
            code: AppErrorCode::DataCorruption,
            detail: Some(expect_errmsg.to_string()),
        }),
    );
    let updating_products = vec![ProductPriceEditDto {
        price: 183,
        product_id: 9914,
        start_after: DateTime::parse_from_rfc3339("2022-11-23T09:13:41+07:00").unwrap(),
        end_before: DateTime::parse_from_rfc3339("2023-10-12T21:23:00+08:00").unwrap(),
        attributes: ProdAttrPriceSetDto {
            extra_charge: Vec::new(),
            last_update: DateTime::parse_from_rfc3339("2022-10-09T01:03:55+08:00").unwrap(),
        },
    }];
    let data = ProductPriceDto {
        s_id: mocked_store_id,
        rm_all: false,
        currency: Some(mocked_currency),
        deleting: ProductPriceDeleteDto { items: None },
        updating: updating_products,
        creating: Vec::new(),
    };
    repo.expect_callargs_fetch((mocked_store_id, vec![9914]));
    let result = EditProductPriceUseCase::execute(Box::new(repo), data, logctx).await;
    assert!(result.is_err());
    let actual_err = result.unwrap_err();
    assert_eq!(actual_err.code, AppErrorCode::DataCorruption);
    if let Some(msg) = &actual_err.detail {
        assert_eq!(msg.as_str(), expect_errmsg);
    }
} // end of fn save_error

#[tokio::test]
async fn delete_subset_ok() {
    let app_state = ut_setup_share_state("config_ok_no_sqldb.json", Box::new(MockConfidential {}));
    let logctx = app_state.log_context().clone();
    let mocked_store_id = 12345;
    let mocked_ppset = ProductPriceModelSet {
        store_id: mocked_store_id,
        currency: CurrencyDto::IDR,
        items: Vec::new(),
    };
    let repo = MockRepository::_new(Ok(()), Ok(()), Ok(mocked_ppset), Ok(()));
    let data = ProductPriceDto {
        s_id: mocked_store_id,
        rm_all: false,
        currency: None,
        deleting: ProductPriceDeleteDto {
            items: Some(vec![23, 45, 67]),
        },
        updating: Vec::new(),
        creating: Vec::new(),
    };
    let result = EditProductPriceUseCase::execute(Box::new(repo), data, logctx).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn delete_subset_error() {
    let app_state = ut_setup_share_state("config_ok_no_sqldb.json", Box::new(MockConfidential {}));
    let logctx = app_state.log_context().clone();
    let (mocked_store_id, expect_errmsg) = (12345, "unit-test-set-error-1");
    let mocked_ppset = ProductPriceModelSet {
        store_id: mocked_store_id,
        currency: CurrencyDto::INR,
        items: Vec::new(),
    };
    let repo = MockRepository::_new(
        Ok(()),
        Err(AppError {
            code: AppErrorCode::DataTableNotExist,
            detail: Some(expect_errmsg.to_string()),
        }),
        Ok(mocked_ppset),
        Ok(()),
    );
    let data = ProductPriceDto {
        s_id: mocked_store_id,
        currency: None,
        rm_all: false,
        deleting: ProductPriceDeleteDto {
            items: Some(vec![23, 45, 67]),
        },
        updating: Vec::new(),
        creating: Vec::new(),
    };
    let result = EditProductPriceUseCase::execute(Box::new(repo), data, logctx).await;
    assert!(result.is_err());
    let actual_err = result.unwrap_err();
    assert_eq!(actual_err.code, AppErrorCode::DataTableNotExist);
    if let Some(msg) = &actual_err.detail {
        assert_eq!(msg.as_str(), expect_errmsg);
    }
}

#[tokio::test]
async fn delete_all_ok() {
    let app_state = ut_setup_share_state("config_ok_no_sqldb.json", Box::new(MockConfidential {}));
    let logctx = app_state.log_context().clone();
    let mocked_store_id = 12345;
    let mocked_ppset = ProductPriceModelSet {
        store_id: mocked_store_id,
        currency: CurrencyDto::USD,
        items: Vec::new(),
    };
    let repo = MockRepository::_new(Ok(()), Ok(()), Ok(mocked_ppset), Ok(()));
    let data = ProductPriceDto {
        s_id: mocked_store_id,
        rm_all: true,
        currency: None,
        deleting: ProductPriceDeleteDto { items: None },
        updating: Vec::new(),
        creating: Vec::new(),
    };
    let result = EditProductPriceUseCase::execute(Box::new(repo), data, logctx).await;
    assert!(result.is_ok());
}
