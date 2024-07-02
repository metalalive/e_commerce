use std::boxed::Box;
use std::cell::Cell;
use std::result::Result as DefaultResult;
use std::sync::Mutex;
use std::vec;

use async_trait::async_trait;
use chrono::DateTime;

use ecommerce_common::api::dto::CurrencyDto;
use ecommerce_common::constant::ProductType;
use ecommerce_common::error::AppErrorCode;

use crate::{ut_setup_share_state, MockConfidential};
use order::api::rpc::dto::{ProductPriceDeleteDto, ProductPriceDto, ProductPriceEditDto};
use order::error::AppError;
use order::model::{ProductPriceModel, ProductPriceModelSet};
use order::repository::AbsProductPriceRepo;
use order::usecase::EditProductPriceUseCase;

type RepoFetchCallArgType = (u32, Vec<(ProductType, u64)>);

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
        ids: Vec<(ProductType, u64)>,
    ) -> DefaultResult<ProductPriceModelSet, AppError> {
        if let Ok(g) = self._callargs_fetch_actual.lock() {
            g.set((store_id, ids));
        }
        match &self._mocked_fetch {
            Ok(m) => Ok(ProductPriceModelSet {
                store_id: m.store_id,
                currency: m.currency.clone(),
                items: self._clone_fetched_items(&m.items),
            }),
            Err(e) => Err(e.clone()),
        }
    }

    async fn fetch_many(
        &self,
        _ids: Vec<(u32, ProductType, u64)>,
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
    fn _clone_fetched_items(&self, src: &Vec<ProductPriceModel>) -> Vec<ProductPriceModel> {
        src.iter()
            .map(|g| ProductPriceModel {
                price: g.price,
                start_after: g.start_after,
                end_before: g.end_before,
                product_id: g.product_id,
                product_type: g.product_type.clone(),
                is_create: g.is_create,
            })
            .collect()
    }
    fn expect_callargs_fetch(&mut self, val: RepoFetchCallArgType) {
        self._callargs_fetch_expect = Some(val);
    }
} // end of impl MockRepository

#[tokio::test]
async fn create_ok() {
    let app_state = ut_setup_share_state("config_ok.json", Box::new(MockConfidential {}));
    let logctx = app_state.log_context().clone();
    let (mocked_store_id, mocked_currency) = (12345, CurrencyDto::TWD);
    let mocked_ppset = ProductPriceModelSet {
        store_id: mocked_store_id,
        currency: mocked_currency.clone(),
        items: Vec::new(),
    };
    let mut repo = MockRepository::_new(Ok(()), Ok(()), Ok(mocked_ppset), Ok(()));
    let creating_products = vec![
        ProductPriceEditDto {
            price: 389,
            product_type: ProductType::Item,
            product_id: 2379,
            start_after: DateTime::parse_from_rfc3339("2022-11-25T09:13:39+05:00")
                .unwrap()
                .into(),
            end_before: DateTime::parse_from_rfc3339("2023-09-12T21:23:00+06:00")
                .unwrap()
                .into(),
        },
        ProductPriceEditDto {
            price: 51,
            product_type: ProductType::Package,
            product_id: 2642,
            start_after: DateTime::parse_from_rfc3339("2022-11-24T09:13:39+05:00")
                .unwrap()
                .into(),
            end_before: DateTime::parse_from_rfc3339("2023-09-12T21:25:01+11:00")
                .unwrap()
                .into(),
        },
    ];
    let data = ProductPriceDto {
        s_id: mocked_store_id,
        rm_all: false,
        currency: Some(mocked_currency),
        deleting: ProductPriceDeleteDto {
            items: None,
            pkgs: None,
            item_type: ProductType::Item,
            pkg_type: ProductType::Package,
        },
        updating: Vec::new(),
        creating: creating_products,
    };
    repo.expect_callargs_fetch((mocked_store_id, vec![]));
    let result = EditProductPriceUseCase::execute(Box::new(repo), data, logctx).await;
    assert!(result.is_ok());
} // end of fn create_ok

#[tokio::test]
async fn update_ok() {
    let app_state = ut_setup_share_state("config_ok.json", Box::new(MockConfidential {}));
    let logctx = app_state.log_context().clone();
    let (mocked_store_id, mocked_currency) = (12345, CurrencyDto::USD);
    let mocked_ppset = ProductPriceModelSet {
        store_id: mocked_store_id,
        currency: mocked_currency.clone(),
        items: vec![
            ProductPriceModel {
                price: 1009,
                product_type: ProductType::Package,
                product_id: 2613,
                is_create: false,
                start_after: DateTime::parse_from_rfc3339("2022-04-26T08:15:47+01:00")
                    .unwrap()
                    .into(),
                end_before: DateTime::parse_from_rfc3339("2024-08-28T21:35:26-08:00")
                    .unwrap()
                    .into(),
            },
            ProductPriceModel {
                price: 1010,
                product_type: ProductType::Item,
                product_id: 3072,
                is_create: false,
                start_after: DateTime::parse_from_rfc3339("2021-03-20T10:58:39-06:00")
                    .unwrap()
                    .into(),
                end_before: DateTime::parse_from_rfc3339("2023-05-16T04:19:07+08:00")
                    .unwrap()
                    .into(),
            },
        ],
    };
    let mut repo = MockRepository::_new(Ok(()), Ok(()), Ok(mocked_ppset), Ok(()));
    let updating_products = vec![
        ProductPriceEditDto {
            price: 1322,
            product_type: ProductType::Item,
            product_id: 3072,
            start_after: DateTime::parse_from_rfc3339("2021-03-29T10:58:39-06:00")
                .unwrap()
                .into(),
            end_before: DateTime::parse_from_rfc3339("2023-05-15T04:18:07+08:00")
                .unwrap()
                .into(),
        },
        ProductPriceEditDto {
            price: 1155,
            product_type: ProductType::Package,
            product_id: 2613,
            start_after: DateTime::parse_from_rfc3339("2022-04-27T07:15:47+01:00")
                .unwrap()
                .into(),
            end_before: DateTime::parse_from_rfc3339("2024-08-29T20:34:25-08:00")
                .unwrap()
                .into(),
        },
    ];
    let creating_products = vec![
        ProductPriceEditDto {
            price: 452,
            product_type: ProductType::Item,
            product_id: 8299,
            start_after: DateTime::parse_from_rfc3339("2022-11-27T09:13:39+05:00")
                .unwrap()
                .into(),
            end_before: DateTime::parse_from_rfc3339("2023-04-13T01:48:07+06:00")
                .unwrap()
                .into(),
        },
        ProductPriceEditDto {
            price: 6066,
            product_type: ProductType::Package,
            product_id: 1712,
            start_after: DateTime::parse_from_rfc3339("2022-01-19T06:05:39+05:00")
                .unwrap()
                .into(),
            end_before: DateTime::parse_from_rfc3339("2024-08-31T21:44:25+11:00")
                .unwrap()
                .into(),
        },
    ]; // product IDs here have to be consistent with the mocked fetched model set above
    let data = ProductPriceDto {
        s_id: mocked_store_id,
        rm_all: false,
        currency: Some(mocked_currency),
        deleting: ProductPriceDeleteDto {
            items: None,
            pkgs: None,
            item_type: ProductType::Item,
            pkg_type: ProductType::Package,
        },
        updating: updating_products,
        creating: creating_products,
    };
    repo.expect_callargs_fetch((
        mocked_store_id,
        vec![(ProductType::Item, 3072), (ProductType::Package, 2613)],
    ));
    let result = EditProductPriceUseCase::execute(Box::new(repo), data, logctx).await;
    assert!(result.is_ok());
} // end of fn update_ok

#[tokio::test]
async fn fetch_error() {
    let app_state = ut_setup_share_state("config_ok.json", Box::new(MockConfidential {}));
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
        product_type: ProductType::Item,
        product_id: 2379,
        start_after: DateTime::parse_from_rfc3339("2022-11-25T09:13:39+05:00")
            .unwrap()
            .into(),
        end_before: DateTime::parse_from_rfc3339("2023-09-12T21:23:00+06:00")
            .unwrap()
            .into(),
    }];
    let data = ProductPriceDto {
        s_id: mocked_store_id,
        rm_all: false,
        currency: Some(mocked_currency),
        deleting: ProductPriceDeleteDto {
            items: None,
            pkgs: None,
            item_type: ProductType::Item,
            pkg_type: ProductType::Package,
        },
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
    let app_state = ut_setup_share_state("config_ok.json", Box::new(MockConfidential {}));
    let logctx = app_state.log_context().clone();
    let (mocked_store_id, expect_errmsg) = (12345, "unit-test-set-error-2");
    let mocked_currency = CurrencyDto::TWD;
    let mocked_ppset = ProductPriceModelSet {
        store_id: mocked_store_id,
        currency: mocked_currency.clone(),
        items: vec![ProductPriceModel {
            is_create: false,
            price: 4810,
            product_type: ProductType::Package,
            product_id: 9914,
            start_after: DateTime::parse_from_rfc3339("2022-11-23T09:13:41+07:00")
                .unwrap()
                .into(),
            end_before: DateTime::parse_from_rfc3339("2023-10-12T21:23:00+08:00")
                .unwrap()
                .into(),
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
        product_type: ProductType::Package,
        product_id: 9914,
        start_after: DateTime::parse_from_rfc3339("2022-11-23T09:13:41+07:00")
            .unwrap()
            .into(),
        end_before: DateTime::parse_from_rfc3339("2023-10-12T21:23:00+08:00")
            .unwrap()
            .into(),
    }];
    let data = ProductPriceDto {
        s_id: mocked_store_id,
        rm_all: false,
        currency: Some(mocked_currency),
        deleting: ProductPriceDeleteDto {
            items: None,
            pkgs: None,
            item_type: ProductType::Item,
            pkg_type: ProductType::Package,
        },
        updating: updating_products,
        creating: Vec::new(),
    };
    repo.expect_callargs_fetch((mocked_store_id, vec![(ProductType::Package, 9914)]));
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
    let app_state = ut_setup_share_state("config_ok.json", Box::new(MockConfidential {}));
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
            item_type: ProductType::Item,
            pkg_type: ProductType::Package,
            items: Some(vec![23, 45, 67]),
            pkgs: Some(vec![8, 90, 123]),
        },
        updating: Vec::new(),
        creating: Vec::new(),
    };
    let result = EditProductPriceUseCase::execute(Box::new(repo), data, logctx).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn delete_subset_error() {
    let app_state = ut_setup_share_state("config_ok.json", Box::new(MockConfidential {}));
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
            item_type: ProductType::Item,
            pkg_type: ProductType::Package,
            items: Some(vec![23, 45, 67]),
            pkgs: Some(vec![8, 90, 123]),
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
    let app_state = ut_setup_share_state("config_ok.json", Box::new(MockConfidential {}));
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
        deleting: ProductPriceDeleteDto {
            items: None,
            pkgs: None,
            item_type: ProductType::Item,
            pkg_type: ProductType::Package,
        },
        updating: Vec::new(),
        creating: Vec::new(),
    };
    let result = EditProductPriceUseCase::execute(Box::new(repo), data, logctx).await;
    assert!(result.is_ok());
}
