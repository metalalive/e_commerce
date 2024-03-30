use order::constant::ProductType;
use order::model::{CartModel, CartLineModel, BaseProductIdentity};
use order::repository::app_repo_cart;

use super::dstore_ctx_setup;

fn ut_gen_line_model(value: (u32, ProductType, u64, u32))
    -> CartLineModel
{
    CartLineModel {id_:BaseProductIdentity { store_id: value.0 , product_type: value.1,
        product_id: value.2 }, qty_req: value.3 }
}

fn ut_verify_cart_model(
    actual:CartModel,
    expect: (u32, u8, &str, Vec<(u32,ProductType,u64,u32)>)
) {
    assert_eq!(actual.owner, expect.0);
    assert_eq!(actual.seq_num, expect.1);
    assert_eq!(actual.title.as_str(), expect.2);
    expect.3.into_iter().map(|expect_line| {
        let result = actual.saved_lines.iter().find(
            |obj| obj.id_.store_id == expect_line.0
                && obj.id_.product_type == expect_line.1
                && obj.id_.product_id == expect_line.2 );
        let line_found = result.unwrap();
        assert_eq!(line_found.qty_req, expect_line.3);
    }).count();
    assert!(actual.new_lines.is_empty());
}

#[cfg(feature="mariadb")]
#[tokio::test]
async fn save_different_user_carts()
{
    let ds = dstore_ctx_setup();
    let repo = app_repo_cart(ds).await.unwrap();
    let data_ins = [
        (124, 7, "next-proj purchase", vec![(3, ProductType::Item, 108u64, 24u32),
                                            (3, ProductType::Package, 91u64, 25u32)] ),
        (124, 9, "my xmax gift", vec![(3, ProductType::Item, 108, 26),
                                      (5, ProductType::Item, 108, 27),
                                      (3, ProductType::Package, 91, 28)] ),
        (127, 8, "for massion", vec![(3, ProductType::Item, 108, 29),
                                     (3, ProductType::Package, 450, 30),
                                     (3, ProductType::Item,   453, 31),
                                     (3, ProductType::Package, 91, 32)] ),
        (127, 9, "DIY kit", vec![(3, ProductType::Item, 108, 40),
                                 (3, ProductType::Item, 110, 41),
                                 (3, ProductType::Package, 91, 42)] ),
    ];
    for item in data_ins.clone() {
        let (owner, seq_num, title, lines) = (item.0, item.1, item.2.to_string(), item.3);
        let new_lines = lines.into_iter().map(ut_gen_line_model).collect();
        let obj = CartModel {owner, seq_num, title, saved_lines:Vec::new(), new_lines};
        let result = repo.update(obj).await;
        assert!(result.is_ok());
    }
    for item in data_ins.clone() {
        let result = repo.fetch_cart(item.0, item.1).await;
        let actual = result.unwrap();
        ut_verify_cart_model(actual, item);
    }
    let data_upd = [
        (127, 8, "mission essential", vec![(3, ProductType::Package, 9013, 52)] ,
                                     vec![(3, ProductType::Package, 91, 53),
                                          (3, ProductType::Item, 108, 51)] ),
        (124, 6, "boxing day shop", vec![(3, ProductType::Package, 121, 5),
                                         (3, ProductType::Item, 91, 15)] , Vec::new() ),
        (124, 7, "next-proj purchase", vec![(3, ProductType::Package, 1430, 55),
                                            (3, ProductType::Item, 169, 56)],
                                       vec![(3, ProductType::Item, 108, 54),
                                            (3, ProductType::Package, 91, 57)] ),
    ];
    for item in data_upd {
        let (owner, seq_num, title, new_lines_d, saved_lines_d) = (
            item.0, item.1, item.2.to_string(), item.3, item.4
        );
        let new_lines = new_lines_d.into_iter().map(ut_gen_line_model).collect();
        let saved_lines = saved_lines_d.into_iter().map(ut_gen_line_model).collect();
        let obj = CartModel {owner, seq_num, title, saved_lines, new_lines};
        let result = repo.update(obj).await;
        assert!(result.is_ok());
    }
    let data_verify_after_update = [
        data_ins[3].clone(),
        data_ins[1].clone(),
        (124, 6, "boxing day shop", vec![(3, ProductType::Package, 121, 5),
                                         (3, ProductType::Item, 91, 15)] ),
        (127, 8, "mission essential", vec![(3, ProductType::Item, 108, 51),
                                     (3, ProductType::Package, 9013, 52),
                                     (3, ProductType::Package, 450, 30),
                                     (3, ProductType::Item,   453, 31),
                                     (3, ProductType::Package, 91, 53)] ),
        (124, 7, "next-proj purchase", vec![(3, ProductType::Item, 108, 54),
                                            (3, ProductType::Package, 1430, 55),
                                            (3, ProductType::Item, 169, 56),
                                            (3, ProductType::Package, 91, 57)] ),
    ];
    for item in data_verify_after_update.clone() {
        let actual = repo.num_lines_saved(item.0, item.1).await.unwrap();
        assert_eq!(actual, item.3.len());
        let result = repo.fetch_cart(item.0, item.1).await;
        let actual = result.unwrap();
        ut_verify_cart_model(actual, item);
    }
} // end of fn save_different_user_carts


#[cfg(feature="mariadb")]
#[tokio::test]
async fn discard_cart_ok()
{
    let ds = dstore_ctx_setup();
    let repo = app_repo_cart(ds).await.unwrap();
    let  data_ins = [
        (128, 7, "aroma", vec![(3, ProductType::Item, 108u64, 24u32),
                               (3, ProductType::Package, 91u64, 25u32)] ),
        (128, 9, "texture combo", vec![(3, ProductType::Item, 108, 26),
                                      (5, ProductType::Item, 108, 27),
                                      (3, ProductType::Package, 91, 28)] ),
        (129, 8, "sound / unsound", vec![(3, ProductType::Item, 108, 29),
                                     (3, ProductType::Package, 450, 30),
                                     (3, ProductType::Item,   127, 31),
                                     (3, ProductType::Item, 110, 41),
                                     (3, ProductType::Package, 91, 32)] ),
        (129, 9, "lifetime", vec![(3, ProductType::Item, 108, 40),
                                 (3, ProductType::Package, 91, 42)] ),
    ];
    for item in data_ins.clone() {
        let (owner, seq_num, title, lines) = (item.0, item.1, item.2.to_string(), item.3);
        let new_lines = lines.into_iter().map(ut_gen_line_model).collect();
        let obj = CartModel {owner, seq_num, title, saved_lines:Vec::new(), new_lines};
        let result = repo.update(obj).await;
        assert!(result.is_ok());
    }
    for item in data_ins.clone() {
        let actual = repo.fetch_cart(item.0, item.1).await.unwrap();
        ut_verify_cart_model(actual, item);
    }
    {
        let result = repo.discard(128, 7).await;
        assert!(result.is_ok());
        let empty = (128, 7, "Untitled", Vec::new());
        let actual = repo.fetch_cart(128, 7).await.unwrap();
        ut_verify_cart_model(actual, empty);
    }
    let (_discarded, data_verify) = data_ins.split_first().unwrap();
    for item in data_verify {
        let item = item.clone();
        let actual = repo.fetch_cart(item.0, item.1).await.unwrap();
        ut_verify_cart_model(actual, item);
    }
} // end of fn discard_cart_ok
