use chrono::DateTime;

use order::constant::ProductType;
use order::error::AppErrorCode;
use order::model::{ProductPriceModelSet, ProductPriceModel};

use order::repository::{app_repo_product_price, AbsProductPriceRepo};

use crate::model::ut_clone_productprice;
use super::dstore_ctx_setup;

fn ut_pprice_data() -> [ProductPriceModel;10] {
    [
        ProductPriceModel {is_create:true, product_type:ProductType::Item, product_id:1001, price:87,
            start_after:DateTime::parse_from_rfc3339("2023-09-09T09:12:53.001985+08:00").unwrap().into(),
            end_before:DateTime::parse_from_rfc3339("2023-10-06T09:00:30.001030+08:00").unwrap().into()  },
        ProductPriceModel {is_create:true, product_type:ProductType::Package, product_id:1002, price:94555,
            start_after:DateTime::parse_from_rfc3339("2023-09-09T09:13:54+07:00").unwrap().into(),
            end_before:DateTime::parse_from_rfc3339("2023-10-07T09:01:30+06:00").unwrap().into()  },
        ProductPriceModel {is_create:true, product_type:ProductType::Item, product_id:1003, price:28379,
            start_after:DateTime::parse_from_rfc3339("2023-07-31T10:16:54+05:00").unwrap().into(),
            end_before:DateTime::parse_from_rfc3339("2023-10-10T09:01:31+02:00").unwrap().into()  },
        ProductPriceModel {is_create:true, product_type:ProductType::Package, product_id:1004, price:3008,
            start_after:DateTime::parse_from_rfc3339("2022-07-30T11:16:55.468-01:00").unwrap().into(),
            end_before:DateTime::parse_from_rfc3339("2023-10-10T09:01:31.3310+03:00").unwrap().into()  },
        ProductPriceModel {is_create:true, product_type:ProductType::Item, product_id:1005, price:1389,
            start_after:DateTime::parse_from_rfc3339("2023-07-29T10:17:54+05:00").unwrap().into(),
            end_before:DateTime::parse_from_rfc3339("2023-10-06T09:01:32+07:00").unwrap().into()  },
        ProductPriceModel {is_create:true, product_type:ProductType::Package, product_id:1006, price:183,
            start_after:DateTime::parse_from_rfc3339("2023-06-29T11:18:54.995+04:00").unwrap().into(),
            end_before:DateTime::parse_from_rfc3339("2023-10-05T08:14:05.913+09:00").unwrap().into()  },
        ProductPriceModel {is_create:true, product_type:ProductType::Item, product_id:1007, price:666,
            start_after:DateTime::parse_from_rfc3339("2022-07-28T12:24:47+08:00").unwrap().into(),
            end_before:DateTime::parse_from_rfc3339("2023-12-26T16:58:00+09:00").unwrap().into()  },
        // -------- update --------
        ProductPriceModel {is_create:false, product_type:ProductType::Item, product_id:1001, price:94,
            start_after:DateTime::parse_from_rfc3339("2023-09-09T09:12:53.001905+08:30").unwrap().into(),
            end_before:DateTime::parse_from_rfc3339("2023-10-06T09:00:30.10301+08:30").unwrap().into()  },
        ProductPriceModel {is_create:false, product_type:ProductType::Package, product_id:1002, price: 515,
            start_after:DateTime::parse_from_rfc3339("2023-09-10T11:14:54+07:00").unwrap().into(),
            end_before:DateTime::parse_from_rfc3339("2023-10-07T09:01:30.000067+06:00").unwrap().into()  },
        ProductPriceModel {is_create:false, product_type:ProductType::Item, product_id:1003, price: 28023,
            start_after:DateTime::parse_from_rfc3339("2023-07-31T10:18:54+05:00").unwrap().into(),
            end_before:DateTime::parse_from_rfc3339("2023-10-10T06:11:50+02:00").unwrap().into()  },
    ]
}

#[cfg(feature="mariadb")]
#[tokio::test]
async fn test_save_fetch_ok()
{
    let ds = dstore_ctx_setup();
    let repo = app_repo_product_price(ds).await.unwrap();
    let data = ut_pprice_data();
    let store_id = 123;
    let items = data[..4].iter().map(ut_clone_productprice).collect::<Vec<_>>();
    let mset = ProductPriceModelSet { store_id, items };
    let result = repo.save(mset).await;
    assert!(result.is_ok());
    let result = repo.fetch(store_id, vec![(ProductType::Package, 1002)]).await;
    assert!(result.is_ok());
    if let Ok(mut ms) = result {
        let m = ms.items.remove(0);
        assert!(matches!(m.product_type, ProductType::Package));
        assert_eq!(m.product_id, 1002);
        assert_eq!(m.price, 94555);
    }
    let items = data[4..].iter().map(ut_clone_productprice).collect::<Vec<_>>();
    let mset = ProductPriceModelSet { store_id, items };
    let result = repo.save(mset).await;
    assert!(result.is_ok());
    let result = repo.fetch(store_id, vec![(ProductType::Item, 1005),
                                           (ProductType::Package, 1002)]).await;
    assert!(result.is_ok());
    if let Ok(ms) = result {
        assert_eq!(ms.items.len(), 2);
        ms.items.into_iter().map(|m| {
            let expect = match &m.product_id {
                1002 => (515u32,),
                1005 => (1389,),
                _others => (0,),
            }; // TODO, verify time range
            let actual = (m.price,);
            assert_eq!(expect, actual);
        }).count();
        //assert_eq!(m.start_after, DateTime::parse_from_rfc3339("2023-09-10T11:14:54+07:00").unwrap());
        //assert_eq!(m.end_before, DateTime::parse_from_rfc3339("2023-10-07T09:01:30.000067+06:00").unwrap());
    }
} // end of fn test_save_fetch_ok


#[cfg(feature="mariadb")]
#[tokio::test]
async fn test_fetch_empty()
{
    let ds = dstore_ctx_setup();
    let repo = app_repo_product_price(ds).await.unwrap();
    let store_id = 123;
    let result = repo.fetch(store_id, vec![(ProductType::Item, 2005),
                                           (ProductType::Package, 2002)]).await;
    assert!(result.is_ok());
    if let Ok(ms) = result {
        assert_eq!(ms.items.len(), 0);
    }
}

#[cfg(feature="mariadb")]
#[tokio::test]
async fn test_save_insert_dup()
{
    let ds = dstore_ctx_setup();
    let repo = app_repo_product_price(ds).await.unwrap();
    let data = ut_pprice_data();
    let store_id = 124;
    let items = data[..2].iter().map(ut_clone_productprice).collect::<Vec<_>>();
    let mset = ProductPriceModelSet { store_id, items };
    let result = repo.save(mset).await;
    assert!(result.is_ok());
    let items = data[..2].iter().map(ut_clone_productprice).collect::<Vec<_>>();
    let mset = ProductPriceModelSet { store_id, items };
    let result = repo.save(mset).await;
    assert!(result.is_err());
    if let Err(e) = result {
        // println!("[unit-test] error : {:?}", e);
        let expect_lowlvl_err_code = "1062";
        assert_eq!(e.code, AppErrorCode::RemoteDbServerFailure);
        assert!(e.detail.as_ref().unwrap().contains(expect_lowlvl_err_code));
    }
}
