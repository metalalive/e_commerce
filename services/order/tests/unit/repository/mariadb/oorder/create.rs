use std::boxed::Box;

use chrono::DateTime;
use order::constant::ProductType;
use order::repository::AbsOrderRepo;

pub(super) async fn ut_verify_fetch_all_olines_ok(
    o_repo: &Box<dyn AbsOrderRepo> )
{
    let oid = "800eff40".to_string();
    let result = o_repo.fetch_all_lines(oid).await;
    assert!(result.is_ok());
    let lines = result.unwrap();
    assert_eq!(lines.len(), 4);
    lines.into_iter().map(|o| {
        let (id_, price, qty, policy) = (o.id_, o.price, o.qty, o.policy);
        let combo = (id_.store_id, id_.product_type, id_.product_id);
        let expect = match combo {
            (1013, ProductType::Package, 9004) => (2, 0, 3, 6,  true, DateTime::parse_from_rfc3339("3015-11-29T15:03:30-03:00").unwrap()),
            (1013, ProductType::Item, 9006)    => (3, 0, 4, 12, true, DateTime::parse_from_rfc3339("3015-11-29T15:04:30-03:00").unwrap()),
            (1014, ProductType::Package, 9008) => (29, 0, 20, 260, true, DateTime::parse_from_rfc3339("3015-11-29T15:05:30-03:00").unwrap()),
            (1014, ProductType::Item, 9009)    => (6,  0, 15, 90, true,  DateTime::parse_from_rfc3339("3015-11-29T15:06:30-03:00").unwrap()),
            _others => (0, 0, 0, 0, true, DateTime::parse_from_rfc3339("1989-05-30T23:57:59+00:00").unwrap()),
        };
        let actual = (qty.reserved, qty.paid, price.unit, price.total,
                      qty.paid_last_update.is_none(),  policy.warranty_until
                );
        assert_eq!(actual, expect);
    }).count();
}
