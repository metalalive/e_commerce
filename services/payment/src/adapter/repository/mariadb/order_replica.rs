use std::collections::HashMap;
use std::result::Result;

use mysql_async::{Params, Value as MySqlValue};
use rust_decimal::Decimal;

use ecommerce_common::adapter::repository::OidBytes;
use ecommerce_common::constant::ProductType;
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::model::order::{BillingModel, ContactModel, PhyAddrModel};
use ecommerce_common::model::BaseProductIdentity;

use super::super::{AppRepoError, AppRepoErrorDetail, AppRepoErrorFnLabel};
use super::raw_column_to_datetime;
use crate::model::{OrderCurrencySnapshot, OrderLineModel, OrderLineModelSet, PayLineAmountModel};

const DATETIME_FMT_P0F: &str = "%Y-%m-%d %H:%M:%S";

#[rustfmt::skip]
pub(super) type OrderlineRowType = (
    u32, String, u64, Decimal, Decimal, Decimal,
    u32, u32, mysql_async::Value,
);

#[rustfmt::skip]
pub(super) type OrderCurrencyRowType = (u32, String, Decimal);

struct InsertOrderTopLvlArgs(String, Params);
struct InsertOrderLineArgs(String, Vec<Params>);
struct InsertCurrencySnapshotArgs(String, Vec<Params>);
struct InsertBillContactArgs(String, Params);
struct InsertBillPhyAddrArgs(String, Params);

pub(super) struct InsertOrderReplicaArgs(pub(super) Vec<(String, Vec<Params>)>);
pub(super) struct FetchUnpaidOlineArgs(pub(super) [(String, Params); 3]);
pub(super) struct FetchCurrencySnapshotArgs(String, Vec<MySqlValue>);

impl<'a, 'b> From<(&'a OrderLineModelSet, &'b OidBytes)> for InsertOrderTopLvlArgs {
    fn from(value: (&'a OrderLineModelSet, &'b OidBytes)) -> Self {
        let (ol_set, oid_b) = value;
        let arg = vec![
            ol_set.buyer_id.into(),
            oid_b.as_column().into(),
            ol_set
                .create_time
                .format(DATETIME_FMT_P0F)
                .to_string()
                .into(),
            ol_set.num_charges.into(),
        ];
        let params = Params::Positional(arg);
        let stmt = "INSERT INTO `order_toplvl_meta`(`buyer_id`, `o_id`, \
            `create_time`, `num_charges`) VALUES (?,?,?,?)";
        Self(stmt.to_string(), params)
    }
}
impl<'a, 'b> From<(&'a OrderLineModelSet, &'b OidBytes)> for InsertOrderLineArgs {
    fn from(value: (&'a OrderLineModelSet, &'b OidBytes)) -> Self {
        let (ol_set, oid_b) = value;
        let params = ol_set
            .lines
            .iter()
            .map(|line| {
                let prod_type_num: u8 = line.pid.product_type.clone().into();
                let arg = vec![
                    oid_b.as_column().into(),
                    line.pid.store_id.into(),
                    prod_type_num.to_string().into(),
                    line.pid.product_id.into(),
                    line.rsv_total.unit.into(),
                    line.rsv_total.total.into(),
                    line.rsv_total.qty.into(),
                    line.reserved_until
                        .to_utc()
                        .format(DATETIME_FMT_P0F)
                        .to_string()
                        .into(),
                ];
                Params::Positional(arg)
            })
            .collect::<Vec<_>>();
        let stmt = "INSERT INTO `order_line_detail`(`o_id`,`store_id`,`product_type`, \
           `product_id`,`amt_unit`,`amt_total_rsved`,`qty_rsved`,`rsved_until`) \
            VALUES (?,?,?,?,?, ?,?,?)";
        Self(stmt.to_string(), params)
    } // end of fn from
} // end of impl InsertOrderLineArgs

impl<'a, 'b> TryFrom<(&'a HashMap<u32, OrderCurrencySnapshot>, &'b OidBytes)>
    for InsertCurrencySnapshotArgs
{
    type Error = AppRepoError;

    fn try_from(
        value: (&'a HashMap<u32, OrderCurrencySnapshot>, &'b OidBytes),
    ) -> Result<Self, Self::Error> {
        const PRECISION_FRACTIONAL: u32 = 4;
        let (snapshot_map, oid_b) = value;
        let result = snapshot_map
            .iter()
            .find(|(_usr_id, s)| s.rate.scale() > PRECISION_FRACTIONAL);
        if let Some((usr_id, s)) = result {
            let actual_fraction = s.rate.scale();
            let detail = AppRepoErrorDetail::CurrencyPrecision(
                *usr_id,
                s.label.to_string(),
                s.rate.to_string(),
                PRECISION_FRACTIONAL,
                actual_fraction,
            );
            return Err(AppRepoError {
                fn_label: AppRepoErrorFnLabel::CreateOrder,
                code: AppErrorCode::ExceedingMaxLimit,
                detail,
            });
        }
        let params = snapshot_map
            .iter()
            .map(|(usr_id, s)| {
                let arg = vec![
                    oid_b.as_column().into(),
                    (*usr_id).into(),
                    s.label.to_string().into(),
                    s.rate.into(),
                ];
                Params::Positional(arg)
            })
            .collect::<Vec<_>>();
        let stmt = "INSERT INTO `order_currency_snapshot`(`o_id`,`usr_id`,\
                    `label`,`ex_rate`)  VALUES (?,?,?,?)";
        Ok(Self(stmt.to_string(), params))
    } // end of fn try-from
} // end of impl InsertCurrencySnapshotArgs

impl<'a, 'b> TryFrom<(&'a ContactModel, &'b OidBytes)> for InsertBillContactArgs {
    type Error = AppRepoError;
    fn try_from(value: (&'a ContactModel, &'b OidBytes)) -> Result<Self, Self::Error> {
        let (contact, oid_b) = value;
        let serial_mails =
            serde_json::to_string(&contact.emails).map_err(Self::map_contact_error)?;
        let serial_phones =
            serde_json::to_string(&contact.phones).map_err(Self::map_contact_error)?;
        let arg = vec![
            oid_b.0.into(),
            contact.first_name.as_str().into(),
            contact.last_name.as_str().into(),
            serial_mails.into(),
            serial_phones.into(),
        ];
        let params = Params::Positional(arg);
        let stmt = "INSERT INTO `orderbill_contact`(`o_id`,`first_name`,`last_name`, \
                    `emails`,`phones`) VALUES (?,?,?,?,?)";
        Ok(Self(stmt.to_string(), params))
    }
}
impl InsertBillContactArgs {
    fn map_contact_error(e: serde_json::Error) -> AppRepoError {
        AppRepoError {
            code: AppErrorCode::InvalidInput,
            fn_label: AppRepoErrorFnLabel::CreateOrder,
            detail: AppRepoErrorDetail::OrderContactInfo(e.to_string()),
        }
    }
}

impl<'a, 'b> From<(&'a PhyAddrModel, &'b OidBytes)> for InsertBillPhyAddrArgs {
    fn from(value: (&'a PhyAddrModel, &'b OidBytes)) -> Self {
        let (addr, oid_b) = value;
        let arg = vec![
            oid_b.as_column().into(),
            String::from(addr.country.clone()).into(),
            addr.region.as_str().into(),
            addr.city.as_str().into(),
            addr.distinct.as_str().into(),
            addr.street_name.as_deref().into(),
            addr.detail.as_str().into(),
        ];
        let params = Params::Positional(arg);
        let stmt = "INSERT INTO `orderbill_phyaddr`(`o_id`,`country`,`region`,`city`, \
                    `distinct`,`street`,`detail`) VALUES (?,?,?,?,?,?,?)";
        Self(stmt.to_string(), params)
    }
}

impl<'a, 'b> TryFrom<(&'a OrderLineModelSet, &'b BillingModel)> for InsertOrderReplicaArgs {
    type Error = AppRepoError;
    fn try_from(value: (&'a OrderLineModelSet, &'b BillingModel)) -> Result<Self, Self::Error> {
        let (ol_set, billing) = value;
        let oid_b = OidBytes::try_from(ol_set.id.as_str()).map_err(|(code, msg)| AppRepoError {
            code,
            fn_label: AppRepoErrorFnLabel::CreateOrder,
            detail: AppRepoErrorDetail::OrderIDparse(msg),
        })?;
        let toplvl_arg = InsertOrderTopLvlArgs::from((ol_set, &oid_b));
        let olines_arg = InsertOrderLineArgs::from((ol_set, &oid_b));
        let currency_arg =
            InsertCurrencySnapshotArgs::try_from((&ol_set.currency_snapshot, &oid_b))?;
        let contact_arg = InsertBillContactArgs::try_from((&billing.contact, &oid_b))?;
        let mut inner = vec![
            (toplvl_arg.0, vec![toplvl_arg.1]),
            (olines_arg.0, olines_arg.1),
            (currency_arg.0, currency_arg.1),
            (contact_arg.0, vec![contact_arg.1]),
        ];
        if let Some(a) = &billing.address {
            let phyaddr_arg = InsertBillPhyAddrArgs::from((a, &oid_b));
            inner.push((phyaddr_arg.0, vec![phyaddr_arg.1]));
        }
        Ok(Self(inner))
    }
} // end of impl InsertOrderReplicaArgs

impl<'a> TryFrom<(&'a str, Option<[u32; 2]>)> for FetchCurrencySnapshotArgs {
    type Error = (AppErrorCode, AppRepoErrorDetail);
    fn try_from(value: (&'a str, Option<[u32; 2]>)) -> Result<Self, Self::Error> {
        let (oid_hex, maybe_usr_ids) = value;
        let oid_b = OidBytes::try_from(oid_hex)
            .map_err(|(code, msg)| (code, AppRepoErrorDetail::OrderIDparse(msg)))?;
        let mut stmt = "SELECT `usr_id`,`label`,`ex_rate` FROM `order_currency_snapshot` \
             WHERE `o_id`=?"
            .to_string();
        let mut args = vec![oid_b.0.into()];
        if let Some(usr_ids) = maybe_usr_ids {
            stmt += " AND `usr_id` IN (?,?)";
            args.push(usr_ids[0].into());
            args.push(usr_ids[1].into());
        }
        Ok(Self(stmt, args))
    }
}
impl FetchCurrencySnapshotArgs {
    pub(super) fn into_parts(self) -> (String, Vec<MySqlValue>) {
        (self.0, self.1)
    }
}

impl<'a> TryFrom<(u32, &'a str)> for FetchUnpaidOlineArgs {
    type Error = AppRepoError;
    fn try_from(value: (u32, &'a str)) -> Result<Self, Self::Error> {
        let (usr_id, oid_hex) = value;
        let oid_b = OidBytes::try_from(oid_hex).map_err(|(code, msg)| AppRepoError {
            code,
            fn_label: AppRepoErrorFnLabel::GetUnpaidOlines,
            detail: AppRepoErrorDetail::OrderIDparse(msg),
        })?;
        let (stmt_currency, value_currency) = FetchCurrencySnapshotArgs::try_from((oid_hex, None))
            .map_err(|(code, detail)| AppRepoError {
                code,
                detail,
                fn_label: AppRepoErrorFnLabel::GetUnpaidOlines,
            })?
            .into_parts();
        let params = [
            value_currency,
            vec![oid_b.as_column().into(), usr_id.into()],
            vec![usr_id.into(), oid_b.0.into(), oid_b.0.into()],
        ]
        .into_iter()
        .map(Params::Positional)
        .collect::<Vec<_>>();
        let stmts = [
            stmt_currency.as_str(),
            "SELECT `create_time`,`num_charges` FROM `order_toplvl_meta` \
             WHERE `o_id`=? AND `buyer_id`=?",
            // estimate quantity and amount of paid items, by aggregating charge
            // lines which have been completed successfully
            "SELECT   `a1`.`store_id`, `a1`.`product_type`, `a1`.`product_id`, `a1`.`amt_unit`, \
             `a1`.`amt_total_rsved`, COALESCE(`a2`.`amt_total`, 0), `a1`.`qty_rsved`, COALESCE(`a2`.`qty`, 0),\
             `a1`.`rsved_until`  FROM `order_line_detail` AS `a1` LEFT JOIN (\
            SELECT  `b`.`order_id` AS `order-id`, `c`.`store_id` AS `store`, `c`.`product_type` AS `prod-typ`,\
            `c`.`product_id` AS `prod-id`, SUM(`c`.`amt_total`) AS `amt_total`, SUM(`c`.`qty`) AS `qty` \
            FROM `charge_buyer_toplvl` AS `b`  INNER JOIN `charge_line` AS `c`  ON \
            (`b`.`usr_id`=`c`.`buyer_id` AND `b`.`create_time`=`c`.`create_time`)\
            WHERE `b`.`usr_id`=? AND `b`.`order_id`=? AND `b`.`state`='OrderAppSynced' \
            GROUP BY  `c`.`store_id`, `c`.`product_type`, `c`.`product_id`)   AS `a2`  ON \
            (`a1`.`o_id`=`a2`.`order-id` AND `a1`.`store_id`=`a2`.`store` AND \
            `a1`.`product_type`=`a2`.`prod-typ` AND `a1`.`product_id`=`a2`.`prod-id`) \
            WHERE `a1`.`o_id`=? AND `a1`.`qty_rsved` > COALESCE(`a2`.`qty`, 0)",
        ]
        .into_iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
        let zipped = stmts.into_iter().zip(params).collect::<Vec<_>>();
        let inner = zipped.try_into().unwrap();
        Ok(Self(inner))
    } // end of fn try-from
} // end of impl FetchUnpaidOlineArgs

impl<'a>
    TryFrom<(
        u32,
        &'a str,
        mysql_async::Value,
        u32,
        HashMap<u32, OrderCurrencySnapshot>,
    )> for OrderLineModelSet
{
    type Error = AppRepoError;
    fn try_from(
        value: (
            u32,
            &'a str,
            mysql_async::Value,
            u32,
            HashMap<u32, OrderCurrencySnapshot>,
        ),
    ) -> Result<Self, Self::Error> {
        let (usr_id, oid, ctime, num_charges, currency_snapshot) = value;
        let create_time = raw_column_to_datetime(ctime, 0).map_err(|arg| AppRepoError {
            fn_label: AppRepoErrorFnLabel::GetUnpaidOlines,
            code: arg.0,
            detail: arg.1,
        })?;
        if currency_snapshot.is_empty() {
            let msg = "currency-snapshot-empty".to_string();
            return Err(AppRepoError {
                fn_label: AppRepoErrorFnLabel::GetUnpaidOlines,
                code: AppErrorCode::DataCorruption,
                detail: AppRepoErrorDetail::DataRowParse(msg),
            });
        }
        Ok(OrderLineModelSet {
            buyer_id: usr_id,
            id: oid.to_string(),
            create_time,
            lines: vec![],
            num_charges,
            currency_snapshot,
        })
    }
} // end of impl OrderLineModelSet

impl TryFrom<OrderlineRowType> for OrderLineModel {
    type Error = AppRepoError;
    #[rustfmt::skip]
    fn try_from(value: OrderlineRowType) -> Result<Self, Self::Error> {
        let (
            store_id, prod_typ_str, product_id,
            amount_unit, amount_total_rsved, amount_total_paid,
            qty_rsved, qty_paid, rsved_until,
        ) = value;
        let product_type = prod_typ_str
            .parse::<ProductType>()
            .map_err(|e| AppRepoError {
                fn_label: AppRepoErrorFnLabel::GetUnpaidOlines,
                code: AppErrorCode::DataCorruption,
                detail: AppRepoErrorDetail::DataRowParse(e.0.to_string()),
            })?;
        let reserved_until =
            raw_column_to_datetime(rsved_until, 0).map_err(|arg| AppRepoError {
                fn_label: AppRepoErrorFnLabel::GetUnpaidOlines,
                code: arg.0, detail: arg.1,
            })?;
        let rsv_total = PayLineAmountModel {
            unit: amount_unit, total: amount_total_rsved, qty: qty_rsved,
        };
        let paid_total = PayLineAmountModel {
            unit: amount_unit, total: amount_total_paid, qty: qty_paid,
        };
        let pid = BaseProductIdentity { store_id, product_type, product_id };
        Ok(Self { pid, rsv_total, paid_total, reserved_until })
    } // end of fn try-from
} // end of impl OrderLineModel
