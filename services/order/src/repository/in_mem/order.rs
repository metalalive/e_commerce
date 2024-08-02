use std::boxed::Box;
use std::collections::HashMap;
use std::result::Result as DefaultResult;
use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset, Local as LocalTime};
use rust_decimal::Decimal;
use tokio::sync::Mutex;

use ecommerce_common::api::dto::{CurrencyDto, PhoneNumberDto};
use ecommerce_common::api::rpc::dto::{OrderPaymentUpdateDto, OrderPaymentUpdateErrorDto};
use ecommerce_common::constant::ProductType;
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::model::order::{BillingModel, ContactModel, PhyAddrModel};

use crate::api::dto::ShippingMethod;
use crate::datastore::{AbstInMemoryDStore, AppInMemFetchedSingleRow, AppInMemFetchedSingleTable};
use crate::error::AppError;
use crate::model::{
    CurrencyModel, OrderCurrencyModel, OrderLineAppliedPolicyModel, OrderLineIdentity,
    OrderLineModel, OrderLineModelSet, OrderLinePriceModel, OrderLineQuantityModel, ShippingModel,
    ShippingOptionModel,
};

use super::super::{
    AbsOrderRepo, AbsOrderStockRepo, AppOrderFetchRangeCallback, AppOrderRepoUpdateLinesUserFunc,
};
use super::StockLvlInMemRepo;

struct InnerTopLvlWrapper(u32, DateTime<FixedOffset>, CurrencyDto, Decimal);
struct ContactModelWrapper(ContactModel);
struct PhyAddrModelWrapper(PhyAddrModel);
struct SellerCurrencyWrapper(HashMap<u32, CurrencyModel>);

mod _contact {
    use super::{AppInMemFetchedSingleRow, ContactModel, ContactModelWrapper, HashMap};

    pub(super) const MULTI_VAL_COLUMN_SEPARATOR: &str = " ";
    pub(super) const TABLE_LABEL: &str = "order_contact";
    #[rustfmt::skip]
    pub(super) enum InMemColIdx {FirstName, LastName, Emails, Phones, TotNumColumns}
    impl From<InMemColIdx> for usize {
        fn from(value: InMemColIdx) -> usize {
            match value {
                InMemColIdx::FirstName => 0,
                InMemColIdx::LastName => 1,
                InMemColIdx::Emails => 2,
                InMemColIdx::Phones => 3,
                InMemColIdx::TotNumColumns => 4,
            }
        }
    }
    pub(super) fn to_inmem_tbl(
        oid: &str,
        pk_label: &str,
        data: ContactModel,
    ) -> HashMap<String, AppInMemFetchedSingleRow> {
        // each item in emails / phones array must NOT contain space character
        let row = AppInMemFetchedSingleRow::from(ContactModelWrapper(data));
        let pkey = format!("{}-{}", oid, pk_label);
        HashMap::from([(pkey, row)])
    }
} // end of inner module _contact

mod _phy_addr {
    use super::{AppInMemFetchedSingleRow, HashMap, PhyAddrModel, PhyAddrModelWrapper};

    pub(super) const TABLE_LABEL: &str = "order_phyaddr";
    #[rustfmt::skip]
    pub(super) enum InMemColIdx {
        Country, Region, City, Distinct,
        Street, Detail, TotNumColumns,
    }
    impl From<InMemColIdx> for usize {
        fn from(value: InMemColIdx) -> usize {
            match value {
                InMemColIdx::Country => 0,
                InMemColIdx::Region => 1,
                InMemColIdx::City => 2,
                InMemColIdx::Distinct => 3,
                InMemColIdx::Street => 4,
                InMemColIdx::Detail => 5,
                InMemColIdx::TotNumColumns => 6,
            }
        }
    }
    pub(super) fn to_inmem_tbl(
        oid: &str,
        pk_label: &str,
        data: PhyAddrModel,
    ) -> HashMap<String, AppInMemFetchedSingleRow> {
        let row = PhyAddrModelWrapper(data).into();
        let pkey = format!("{}-{}", oid, pk_label);
        HashMap::from([(pkey, row)])
    }
} // end of inner module _phy_addr

mod _ship_opt {
    use super::{HashMap, ShippingOptionModel};

    #[rustfmt::skip]
    pub(super) enum InMemColIdx {SellerID, Method, TotNumColumns}
    impl From<InMemColIdx> for usize {
        fn from(value: InMemColIdx) -> usize {
            match value {
                InMemColIdx::SellerID => 0,
                InMemColIdx::Method => 1,
                InMemColIdx::TotNumColumns => 2,
            }
        }
    }
    pub(super) const TABLE_LABEL: &str = "order_shipping_option";
    pub(super) fn to_inmem_tbl(
        oid: &str,
        data: Vec<ShippingOptionModel>,
    ) -> HashMap<String, Vec<String>> {
        let kv_iter = data.into_iter().map(|m| {
            let pkey = format!("{}-{}", oid, m.seller_id);
            (pkey, m.into())
        });
        HashMap::from_iter(kv_iter)
    }
} // end of inner module _ship_opt

mod _orderline {
    use super::{AppInMemFetchedSingleRow, HashMap, ProductType};
    use crate::model::OrderLineModel;

    pub(super) const TABLE_LABEL: &str = "order_line_reserved";
    #[rustfmt::skip]
    pub(super) enum InMemColIdx {
        SellerID, ProductType, ProductId, QtyReserved,
        PriceUnit, PriceTotal, PolicyReserved, PolicyWarranty,
        QtyPaid, QtyPaidLastUpdate, TotNumColumns,
    }
    impl From<InMemColIdx> for usize {
        fn from(value: InMemColIdx) -> usize {
            match value {
                InMemColIdx::SellerID => 0,
                InMemColIdx::ProductType => 1,
                InMemColIdx::ProductId => 2,
                InMemColIdx::QtyReserved => 3,
                InMemColIdx::QtyPaid => 4,
                InMemColIdx::QtyPaidLastUpdate => 5,
                InMemColIdx::PriceUnit => 6,
                InMemColIdx::PriceTotal => 7,
                InMemColIdx::PolicyReserved => 8,
                InMemColIdx::PolicyWarranty => 9,
                InMemColIdx::TotNumColumns => 10,
            }
        }
    }
    pub(super) fn inmem_pkey(
        oid: &str,
        seller_id: u32,
        prod_typ: ProductType,
        prod_id: u64,
    ) -> String {
        let prod_typ = <ProductType as Into<u8>>::into(prod_typ);
        format!("{oid}-{seller_id}-{prod_typ}-{prod_id}")
    }
    pub(super) fn to_inmem_tbl(
        oid: &str,
        data: &[OrderLineModel],
    ) -> HashMap<String, AppInMemFetchedSingleRow> {
        let kv_iter = data.iter().map(|m| {
            let pkey = inmem_pkey(
                oid,
                m.id_.store_id,
                m.id_.product_type.clone(),
                m.id_.product_id,
            );
            (pkey, m.into())
        });
        HashMap::from_iter(kv_iter)
    } // end of fn to_inmem_tbl
    pub(super) fn pk_group_by_oid(flattened: Vec<String>) -> HashMap<String, Vec<String>> {
        let mut out: HashMap<String, Vec<String>> = HashMap::new();
        flattened
            .into_iter()
            .map(|key| {
                let oid = key.split('-').next().unwrap();
                if let Some(v) = out.get_mut(oid) {
                    v.push(key);
                } else {
                    out.insert(oid.to_string(), vec![key]);
                }
            })
            .count();
        out
    }
} // end of inner module _orderline

mod _seller_currencies {
    use super::{AppInMemFetchedSingleRow, CurrencyModel, HashMap};

    pub(super) const TABLE_LABEL: &str = "order_seller_currency";
    #[rustfmt::skip]
    pub(super) enum InMemColIdx {SellerID, CurrencyLabel, ExchangeRate}
    impl From<InMemColIdx> for usize {
        fn from(value: InMemColIdx) -> usize {
            match value {
                InMemColIdx::SellerID => 0,
                InMemColIdx::CurrencyLabel => 1,
                InMemColIdx::ExchangeRate => 2,
            }
        }
    }
    pub(super) fn to_inmem_tbl(
        oid: &str,
        data: &HashMap<u32, CurrencyModel>,
    ) -> HashMap<String, AppInMemFetchedSingleRow> {
        let iter = data.iter().map(|(seller_id, curr_m)| {
            let key = format!("{oid}-{seller_id}");
            let row = vec![
                seller_id.to_string(),
                curr_m.name.to_string(),
                curr_m.rate.to_string(),
            ];
            (key, row)
        });
        HashMap::from_iter(iter)
    }
} // end of mode _seller_currencies

mod _order_toplvl_meta {
    use super::{AppInMemFetchedSingleRow, HashMap, OrderLineModelSet};

    pub(super) const TABLE_LABEL: &str = "order_toplvl_meta";
    #[rustfmt::skip]
    pub(super) enum InMemColIdx {
        OwnerUsrID, CreateTime, BuyerCurrencyLabel,
        BuyerExchangeRate,
    }
    impl From<InMemColIdx> for usize {
        fn from(value: InMemColIdx) -> usize {
            match value {
                InMemColIdx::OwnerUsrID => 0,
                InMemColIdx::CreateTime => 1,
                InMemColIdx::BuyerCurrencyLabel => 2,
                InMemColIdx::BuyerExchangeRate => 3,
            }
        }
    }
    pub(super) fn to_inmem_tbl(
        data: &OrderLineModelSet,
    ) -> HashMap<String, AppInMemFetchedSingleRow> {
        let pkey = data.order_id.clone();
        let value = vec![
            data.owner_id.to_string(),
            data.create_time.to_rfc3339(),
            data.currency.buyer.name.to_string(),
            data.currency.buyer.rate.to_string(),
        ];
        HashMap::from([(pkey, value)])
    } // end of fn to_inmem_tbl
} // end of inner module _order_toplvl_meta

mod _pkey_partial_label {
    use super::{DateTime, FixedOffset};
    use crate::datastore::AbsDStoreFilterKeyOp;

    pub(super) const BILLING: &str = "billing";
    pub(super) const SHIPPING: &str = "shipping";
    pub(super) struct InMemDStoreFiltKeyOID<'a> {
        pub oid: &'a str,
        pub label: Option<&'a str>,
    }
    impl<'a> AbsDStoreFilterKeyOp for InMemDStoreFiltKeyOID<'a> {
        fn filter(&self, k: &String, _v: &Vec<String>) -> bool {
            let mut id_elms = k.split('-');
            let oid_rd = id_elms.next().unwrap();
            let label_rd = id_elms.next().unwrap();
            let mut cond = self.oid == oid_rd;
            if let Some(l) = self.label {
                cond = cond && (l == label_rd);
            }
            cond
        }
    }
    pub(super) struct InMemDStoreFilterTimeRangeOp {
        pub t0: DateTime<FixedOffset>,
        pub t1: DateTime<FixedOffset>,
        pub col_idx: usize, // column which stores the time to compare with
    }
    impl AbsDStoreFilterKeyOp for InMemDStoreFilterTimeRangeOp {
        fn filter(&self, _k: &String, row: &Vec<String>) -> bool {
            let time_mid = row.get(self.col_idx).unwrap();
            let time_mid = DateTime::parse_from_rfc3339(time_mid.as_str()).unwrap();
            (self.t0 < time_mid) && (time_mid < self.t1)
        }
    }
} // end of mod _pkey_partial_label

pub struct OrderInMemRepo {
    datastore: Arc<Box<dyn AbstInMemoryDStore>>,
    _stock: Arc<Box<dyn AbsOrderStockRepo>>,
    _sched_job_last_launched: Mutex<DateTime<FixedOffset>>,
}

impl From<&OrderLineModel> for AppInMemFetchedSingleRow {
    fn from(value: &OrderLineModel) -> Self {
        let seller_id_s = value.id_.store_id.to_string();
        let prod_typ = <ProductType as Into<u8>>::into(value.id_.product_type.clone()).to_string();
        let prod_id = value.id_.product_id.to_string();
        let _paid_last_update = if let Some(v) = value.qty.paid_last_update.as_ref() {
            v.to_rfc3339()
        } else {
            assert_eq!(value.qty.paid, 0);
            String::new()
        };
        let mut row = (0.._orderline::InMemColIdx::TotNumColumns.into())
            .map(|_num| String::new())
            .collect::<Self>();
        [
            (
                _orderline::InMemColIdx::QtyReserved,
                value.qty.reserved.to_string(),
            ),
            (_orderline::InMemColIdx::QtyPaid, value.qty.paid.to_string()),
            (
                _orderline::InMemColIdx::QtyPaidLastUpdate,
                _paid_last_update,
            ),
            (
                _orderline::InMemColIdx::PriceUnit,
                value.price.unit.to_string(),
            ),
            (
                _orderline::InMemColIdx::PriceTotal,
                value.price.total.to_string(),
            ),
            (
                _orderline::InMemColIdx::PolicyReserved,
                value.policy.reserved_until.to_rfc3339(),
            ),
            (
                _orderline::InMemColIdx::PolicyWarranty,
                value.policy.warranty_until.to_rfc3339(),
            ),
            (_orderline::InMemColIdx::ProductType, prod_typ),
            (_orderline::InMemColIdx::ProductId, prod_id),
            (_orderline::InMemColIdx::SellerID, seller_id_s),
        ]
        .into_iter()
        .map(|(idx, val)| {
            let idx: usize = idx.into();
            row[idx] = val;
        })
        .count();
        row
    }
} // end of impl From for OrderLineModel reference
impl From<AppInMemFetchedSingleRow> for OrderLineModel {
    #[rustfmt::skip]
    fn from(value: AppInMemFetchedSingleRow) -> OrderLineModel {
        let row = value;
        let seller_id = row
            .get::<usize>(_orderline::InMemColIdx::SellerID.into())
            .unwrap().parse().unwrap();
        let prod_typ = row
            .get::<usize>(_orderline::InMemColIdx::ProductType.into())
            .unwrap().parse::<u8>().unwrap();
        let product_id = row
            .get::<usize>(_orderline::InMemColIdx::ProductId.into())
            .unwrap().parse().unwrap();
        let price = OrderLinePriceModel {
            unit: row
                .get::<usize>(_orderline::InMemColIdx::PriceUnit.into())
                .unwrap().parse().unwrap(),
            total: row
                .get::<usize>(_orderline::InMemColIdx::PriceTotal.into())
                .unwrap().parse().unwrap(),
        };
        let qty_paid_last_update = {
            let p = row.get::<usize>(_orderline::InMemColIdx::QtyPaidLastUpdate.into());
            let p = p.unwrap().as_str();
            DateTime::parse_from_rfc3339(p).ok()
        };
        let qty = OrderLineQuantityModel {
            reserved: row
                .get::<usize>(_orderline::InMemColIdx::QtyReserved.into())
                .unwrap().parse().unwrap(),
            paid: row
                .get::<usize>(_orderline::InMemColIdx::QtyPaid.into())
                .unwrap().parse().unwrap(),
            paid_last_update: qty_paid_last_update,
        };
        if qty.paid_last_update.is_none() {
            assert_eq!(qty.paid, 0);
        }
        let reserved_until = {
            let s = row
                .get::<usize>(_orderline::InMemColIdx::PolicyReserved.into())
                .unwrap();
            DateTime::parse_from_rfc3339(s.as_str()).unwrap()
        };
        let warranty_until = {
            let s = row
                .get::<usize>(_orderline::InMemColIdx::PolicyReserved.into())
                .unwrap();
            DateTime::parse_from_rfc3339(s.as_str()).unwrap()
        };
        let policy = OrderLineAppliedPolicyModel {reserved_until, warranty_until};
        OrderLineModel {
            id_: OrderLineIdentity {
                store_id: seller_id,
                product_id,
                product_type: ProductType::from(prod_typ),
            },
            price, policy, qty,
        }
    }
} // end of impl into OrderLineModel

impl From<ContactModelWrapper> for AppInMemFetchedSingleRow {
    fn from(value: ContactModelWrapper) -> Self {
        let value = value.0;
        let phones_str = value
            .phones
            .iter()
            .map(|d| format!("{}-{}", d.nation, d.number))
            .collect::<Vec<String>>();
        let mut row = (0.._contact::InMemColIdx::TotNumColumns.into())
            .map(|_num| String::new())
            .collect::<Self>();
        let _ = [
            (
                _contact::InMemColIdx::Emails,
                value.emails.join(_contact::MULTI_VAL_COLUMN_SEPARATOR),
            ),
            (
                _contact::InMemColIdx::Phones,
                phones_str.join(_contact::MULTI_VAL_COLUMN_SEPARATOR),
            ),
            (_contact::InMemColIdx::FirstName, value.first_name),
            (_contact::InMemColIdx::LastName, value.last_name),
        ]
        .into_iter()
        .map(|(idx, val)| {
            let idx: usize = idx.into();
            row[idx] = val;
        })
        .collect::<Vec<()>>();
        row
    }
}
impl From<AppInMemFetchedSingleRow> for ContactModelWrapper {
    fn from(value: AppInMemFetchedSingleRow) -> ContactModelWrapper {
        let emails = value
            .get::<usize>(_contact::InMemColIdx::Emails.into())
            .unwrap()
            .split(_contact::MULTI_VAL_COLUMN_SEPARATOR)
            .map(|s| s.to_string())
            .collect();
        let phones = value
            .get::<usize>(_contact::InMemColIdx::Phones.into())
            .unwrap()
            .split(_contact::MULTI_VAL_COLUMN_SEPARATOR)
            .map(|s| {
                let mut s = s.split('-');
                let nation = s.next().unwrap().parse().unwrap();
                let number = s.next().unwrap().to_string();
                PhoneNumberDto { nation, number }
            })
            .collect();
        let (first_name, last_name) = (
            value
                .get::<usize>(_contact::InMemColIdx::FirstName.into())
                .unwrap()
                .to_owned(),
            value
                .get::<usize>(_contact::InMemColIdx::LastName.into())
                .unwrap()
                .to_owned(),
        );
        ContactModelWrapper(ContactModel {
            first_name,
            last_name,
            emails,
            phones,
        })
    }
}

impl From<PhyAddrModelWrapper> for AppInMemFetchedSingleRow {
    fn from(value: PhyAddrModelWrapper) -> Self {
        let value = value.0;
        let mut row = (0.._phy_addr::InMemColIdx::TotNumColumns.into())
            .map(|_num| String::new())
            .collect::<Self>();
        [
            (_phy_addr::InMemColIdx::Detail, value.detail),
            (_phy_addr::InMemColIdx::Distinct, value.distinct),
            (
                _phy_addr::InMemColIdx::Street,
                value.street_name.unwrap_or("".to_string()),
            ),
            (_phy_addr::InMemColIdx::Region, value.region),
            (_phy_addr::InMemColIdx::City, value.city),
            (_phy_addr::InMemColIdx::Country, value.country.into()),
        ]
        .into_iter()
        .map(|(idx, val)| {
            let idx: usize = idx.into();
            row[idx] = val;
        })
        .count();
        row
    } // end of fn from
}
impl From<AppInMemFetchedSingleRow> for PhyAddrModelWrapper {
    #[rustfmt::skip]
    fn from(value: AppInMemFetchedSingleRow) -> PhyAddrModelWrapper {
        let (country, region, city, distinct, street, detail) = (
            value
                .get::<usize>(_phy_addr::InMemColIdx::Country.into())
                .unwrap().to_owned().into(),
            value
                .get::<usize>(_phy_addr::InMemColIdx::Region.into())
                .unwrap().to_owned(),
            value
                .get::<usize>(_phy_addr::InMemColIdx::City.into())
                .unwrap().to_owned(),
            value
                .get::<usize>(_phy_addr::InMemColIdx::Distinct.into())
                .unwrap().to_owned(),
            value
                .get::<usize>(_phy_addr::InMemColIdx::Street.into())
                .unwrap().to_owned(),
            value
                .get::<usize>(_phy_addr::InMemColIdx::Detail.into())
                .unwrap().to_owned(),
        );
        let street_name = if street.is_empty() {
            None
        } else {
            Some(street)
        };
        PhyAddrModelWrapper(PhyAddrModel {
            country, region, city,
            distinct, street_name, detail,
        })
    } // end of fn from
}

impl From<ShippingOptionModel> for AppInMemFetchedSingleRow {
    fn from(value: ShippingOptionModel) -> Self {
        let mut row = (0.._ship_opt::InMemColIdx::TotNumColumns.into())
            .map(|_num| String::new())
            .collect::<Self>();
        [
            (
                _ship_opt::InMemColIdx::SellerID,
                value.seller_id.to_string(),
            ),
            (_ship_opt::InMemColIdx::Method, value.method.into()),
        ]
        .into_iter()
        .map(|(idx, val)| {
            let idx: usize = idx.into();
            row[idx] = val;
        })
        .count();
        row
    }
}
impl From<AppInMemFetchedSingleRow> for ShippingOptionModel {
    #[rustfmt::skip]
    fn from(value: AppInMemFetchedSingleRow) -> ShippingOptionModel {
        let (seller_id, method) = (
            value
                .get::<usize>(_ship_opt::InMemColIdx::SellerID.into())
                .unwrap().parse().unwrap(),
            value
                .get::<usize>(_ship_opt::InMemColIdx::Method.into())
                .unwrap().to_owned(),
        );
        ShippingOptionModel {
            seller_id, method: ShippingMethod::from(method),
        }
    }
}

impl From<AppInMemFetchedSingleRow> for InnerTopLvlWrapper {
    fn from(value: AppInMemFetchedSingleRow) -> Self {
        let usr_id = value
            .get::<usize>(_order_toplvl_meta::InMemColIdx::OwnerUsrID.into())
            .unwrap()
            .parse()
            .unwrap();
        let idx: usize = _order_toplvl_meta::InMemColIdx::CreateTime.into();
        let create_time = DateTime::parse_from_rfc3339(value[idx].as_str()).unwrap();
        let buyer_currency = value
            .get::<usize>(_order_toplvl_meta::InMemColIdx::BuyerCurrencyLabel.into())
            .unwrap()
            .into();
        let buyer_exrate = value
            .get::<usize>(_order_toplvl_meta::InMemColIdx::BuyerExchangeRate.into())
            .map(|v| Decimal::from_str(v.as_str()))
            .unwrap()
            .unwrap();
        Self(usr_id, create_time, buyer_currency, buyer_exrate)
    }
}

impl From<AppInMemFetchedSingleTable> for SellerCurrencyWrapper {
    #[rustfmt::skip]
    fn from(value : AppInMemFetchedSingleTable) -> Self {
        let iter = value.into_values()
            .map(|row| {
                let seller_id = row
                    .get::<usize>(_seller_currencies::InMemColIdx::SellerID.into())
                    .unwrap().parse().unwrap();
                let cname_raw = row
                    .get::<usize>(_seller_currencies::InMemColIdx::CurrencyLabel.into())
                    .unwrap();
                let rate_raw = row
                    .get::<usize>(_seller_currencies::InMemColIdx::ExchangeRate.into())
                    .unwrap().as_str();
                let m = CurrencyModel {
                    name: CurrencyDto::from(cname_raw),
                    rate: Decimal::from_str(rate_raw).unwrap()
                };
                (seller_id, m)
            });
        Self(HashMap::from_iter(iter))
    }
} // end of impl SellerCurrencyWrapper

#[async_trait]
impl AbsOrderRepo for OrderInMemRepo {
    fn stock(&self) -> Arc<Box<dyn AbsOrderStockRepo>> {
        self._stock.clone()
    }

    async fn save_contact(
        &self,
        oid: &str,
        bl: BillingModel,
        sh: ShippingModel,
    ) -> DefaultResult<(), AppError> {
        let mut tabledata: [(String, AppInMemFetchedSingleTable); 3] = [
            (_contact::TABLE_LABEL.to_string(), HashMap::new()),
            (_phy_addr::TABLE_LABEL.to_string(), HashMap::new()),
            (
                _ship_opt::TABLE_LABEL.to_string(),
                _ship_opt::to_inmem_tbl(oid, sh.option),
            ),
        ];
        {
            let items = _contact::to_inmem_tbl(oid, _pkey_partial_label::SHIPPING, sh.contact);
            items
                .into_iter()
                .map(|(k, v)| {
                    tabledata[0].1.insert(k, v);
                })
                .count();
            let items = _contact::to_inmem_tbl(oid, _pkey_partial_label::BILLING, bl.contact);
            items
                .into_iter()
                .map(|(k, v)| {
                    tabledata[0].1.insert(k, v);
                })
                .count();
        }
        if let Some(addr) = bl.address {
            let items = _phy_addr::to_inmem_tbl(oid, _pkey_partial_label::BILLING, addr);
            items
                .into_iter()
                .map(|(k, v)| {
                    tabledata[1].1.insert(k, v);
                })
                .count();
        }
        if let Some(addr) = sh.address {
            let items = _phy_addr::to_inmem_tbl(oid, _pkey_partial_label::SHIPPING, addr);
            items
                .into_iter()
                .map(|(k, v)| {
                    tabledata[1].1.insert(k, v);
                })
                .count();
        }
        let data = HashMap::from_iter(tabledata);
        let _num = self.datastore.save(data).await?;
        Ok(())
    } // end of fn save_contact

    async fn fetch_all_lines(&self, oid: String) -> DefaultResult<Vec<OrderLineModel>, AppError> {
        let op = _pkey_partial_label::InMemDStoreFiltKeyOID {
            oid: oid.as_str(),
            label: None,
        };
        let tbl_label = _orderline::TABLE_LABEL.to_string();
        let keys = self.datastore.filter_keys(tbl_label, &op).await?;
        self.fetch_lines_common(keys).await
    }

    async fn fetch_billing(&self, oid: String) -> DefaultResult<BillingModel, AppError> {
        let op = _pkey_partial_label::InMemDStoreFiltKeyOID {
            oid: oid.as_str(),
            label: Some(_pkey_partial_label::BILLING),
        };
        let tbl_labels = [_contact::TABLE_LABEL, _phy_addr::TABLE_LABEL];
        let mut info = vec![];
        for table_name in tbl_labels.iter() {
            let keys = self
                .datastore
                .filter_keys(table_name.to_string(), &op)
                .await?;
            info.push((table_name.to_string(), keys));
        }
        let info = HashMap::from_iter(info);
        let mut data = self.datastore.fetch(info).await?;
        let (result1, result2) = (
            data.remove(tbl_labels[0]).unwrap().into_values().next(),
            data.remove(tbl_labels[1]).unwrap().into_values().next(),
        );
        if let Some(raw_cta) = result1 {
            let contact: ContactModelWrapper = raw_cta.into();
            let address: Option<PhyAddrModelWrapper> = result2.map(|raw_pa| raw_pa.into());
            Ok(BillingModel {
                contact: contact.0,
                address: address.map(|a| a.0),
            })
        } else {
            let e = AppError {
                code: AppErrorCode::IOerror(std::io::ErrorKind::NotFound),
                detail: Some("no-contact-data".to_string()),
            };
            Err(e)
        }
    } // end of fn fetch_billing

    async fn fetch_shipping(&self, oid: String) -> DefaultResult<ShippingModel, AppError> {
        let ops = [
            _pkey_partial_label::InMemDStoreFiltKeyOID {
                oid: oid.as_str(),
                label: Some(_pkey_partial_label::SHIPPING),
            },
            _pkey_partial_label::InMemDStoreFiltKeyOID {
                oid: oid.as_str(),
                label: None,
            },
        ];
        let data = [
            (_contact::TABLE_LABEL, &ops[0]),
            (_phy_addr::TABLE_LABEL, &ops[0]),
            (_ship_opt::TABLE_LABEL, &ops[1]),
        ];
        let mut info = vec![];
        for (table_name, op) in data.into_iter() {
            let keys = self
                .datastore
                .filter_keys(table_name.to_string(), op)
                .await?;
            info.push((table_name.to_string(), keys));
        }
        let info = HashMap::from_iter(info);
        let mut data = self.datastore.fetch(info).await?;
        let (result1, result2, result3) = (
            data.remove(_contact::TABLE_LABEL)
                .unwrap()
                .into_values()
                .next(),
            data.remove(_phy_addr::TABLE_LABEL)
                .unwrap()
                .into_values()
                .next(),
            data.remove(_ship_opt::TABLE_LABEL).unwrap().into_values(),
        );
        if let Some(raw_cta) = result1 {
            let contact = ContactModelWrapper::from(raw_cta).0;
            let address: Option<PhyAddrModelWrapper> = result2.map(|raw_pa| raw_pa.into());
            // shipping option can be empty
            let option = result3.map(AppInMemFetchedSingleRow::into).collect();
            Ok(ShippingModel {
                contact,
                address: address.map(|a| a.0),
                option,
            })
        } else {
            let e = AppError {
                code: AppErrorCode::IOerror(std::io::ErrorKind::NotFound),
                detail: Some("no-contact-data".to_string()),
            };
            Err(e)
        }
    } // end of fetch_shipping

    async fn update_lines_payment(
        &self,
        data: OrderPaymentUpdateDto,
        usr_cb: AppOrderRepoUpdateLinesUserFunc,
    ) -> DefaultResult<OrderPaymentUpdateErrorDto, AppError> {
        let table_name = _orderline::TABLE_LABEL;
        let oid = data.oid.clone();
        let num_data_items = data.lines.len();
        let (mut models, g_lock) = {
            let pids = data
                .lines
                .iter()
                .map(|d| {
                    _orderline::inmem_pkey(
                        oid.as_ref(),
                        d.seller_id,
                        d.product_type.clone(),
                        d.product_id,
                    )
                })
                .collect();
            let info = HashMap::from([(table_name.to_string(), pids)]);
            let (mut rawdata, lock) = self.datastore.fetch_acquire(info).await?;
            let rawdata = rawdata.remove(table_name).unwrap();
            let ms = rawdata
                .into_values()
                .map(AppInMemFetchedSingleRow::into)
                .collect();
            (ms, lock)
        };
        let errors = usr_cb(&mut models, data);
        if errors.len() < num_data_items {
            let rows = _orderline::to_inmem_tbl(oid.as_str(), &models);
            let info = HashMap::from([(table_name.to_string(), rows)]);
            let _num = self.datastore.save_release(info, g_lock)?;
        } // no need to save if all data items cause errors
        Ok(OrderPaymentUpdateErrorDto {
            oid,
            charge_time: None,
            lines: errors,
        })
    } // end of fn update_lines_payment

    async fn fetch_lines_by_rsvtime(
        &self,
        time_start: DateTime<FixedOffset>,
        time_end: DateTime<FixedOffset>,
        usr_cb: AppOrderFetchRangeCallback,
    ) -> DefaultResult<(), AppError> {
        // fetch lines by range of reserved time
        let table_name = _orderline::TABLE_LABEL;
        let op = _pkey_partial_label::InMemDStoreFilterTimeRangeOp {
            col_idx: _orderline::InMemColIdx::PolicyReserved.into(),
            t0: time_start,
            t1: time_end,
        };
        let keys_flattened = self
            .datastore
            .filter_keys(table_name.to_string(), &op)
            .await?;
        let key_grps = _orderline::pk_group_by_oid(keys_flattened);
        for (oid, keys) in key_grps.into_iter() {
            let InnerTopLvlWrapper(owner_id, create_time, buyer_currency, buyer_exrate) =
                self.fetch_toplvl_meta(oid.as_str()).await?;
            let ms = self.fetch_lines_common(keys).await?;
            let mset = OrderLineModelSet {
                order_id: oid,
                owner_id,
                create_time,
                lines: ms,
                currency: OrderCurrencyModel {
                    buyer: CurrencyModel {
                        name: buyer_currency,
                        rate: buyer_exrate,
                    },
                    sellers: std::collections::HashMap::new(),
                }, // TODO, will be deleted once refactored
            };
            usr_cb(self, mset).await?;
        }
        Ok(())
    } // end of fn fetch_lines_by_rsvtime

    async fn fetch_lines_by_pid(
        &self,
        oid: &str,
        pids: Vec<OrderLineIdentity>,
    ) -> DefaultResult<Vec<OrderLineModel>, AppError> {
        let keys = pids
            .into_iter()
            .map(|d| _orderline::inmem_pkey(oid, d.store_id, d.product_type, d.product_id))
            .collect();
        self.fetch_lines_common(keys).await
    }

    async fn fetch_ids_by_created_time(
        &self,
        start: DateTime<FixedOffset>,
        end: DateTime<FixedOffset>,
    ) -> DefaultResult<Vec<String>, AppError> {
        let table_name = _order_toplvl_meta::TABLE_LABEL;
        let op = _pkey_partial_label::InMemDStoreFilterTimeRangeOp {
            col_idx: _order_toplvl_meta::InMemColIdx::CreateTime.into(),
            t0: start,
            t1: end,
        };
        let keys = self
            .datastore
            .filter_keys(table_name.to_string(), &op)
            .await?;
        Ok(keys)
    }

    async fn owner_id(&self, order_id: &str) -> DefaultResult<u32, AppError> {
        let inner = self.fetch_toplvl_meta(order_id).await?;
        Ok(inner.0)
    }
    async fn created_time(&self, order_id: &str) -> DefaultResult<DateTime<FixedOffset>, AppError> {
        let inner = self.fetch_toplvl_meta(order_id).await?;
        Ok(inner.1)
    }

    async fn currency_exrates(&self, oid: &str) -> DefaultResult<OrderCurrencyModel, AppError> {
        let toplvl_m = self.fetch_toplvl_meta(oid).await?;
        let op = _pkey_partial_label::InMemDStoreFiltKeyOID { oid, label: None };
        let tbl_label = _seller_currencies::TABLE_LABEL;
        let keys = self
            .datastore
            .filter_keys(tbl_label.to_string(), &op)
            .await?;
        let info = HashMap::from([(tbl_label.to_string(), keys)]);
        let mut resultset = self.datastore.fetch(info).await?;
        let data = resultset.remove(tbl_label).ok_or(AppError {
            code: AppErrorCode::DataTableNotExist,
            detail: Some(tbl_label.to_string()),
        })?;
        let sellers_c = SellerCurrencyWrapper::from(data);
        Ok(OrderCurrencyModel {
            buyer: CurrencyModel {
                name: toplvl_m.2,
                rate: toplvl_m.3,
            },
            sellers: sellers_c.0,
        })
    } // end of fn currency_exrates

    async fn cancel_unpaid_last_time(&self) -> DefaultResult<DateTime<FixedOffset>, AppError> {
        let guard = self._sched_job_last_launched.lock().await;
        let t = *guard; // copy by de-ref
        Ok(t)
    }

    async fn cancel_unpaid_time_update(&self) -> DefaultResult<(), AppError> {
        let mut guard = self._sched_job_last_launched.lock().await;
        let t = LocalTime::now().fixed_offset();
        *guard = t;
        Ok(())
    }
} // end of impl AbsOrderRepo

impl OrderInMemRepo {
    pub async fn new(
        m: Arc<Box<dyn AbstInMemoryDStore>>,
        timenow: DateTime<FixedOffset>,
    ) -> DefaultResult<Self, AppError> {
        m.create_table(_contact::TABLE_LABEL).await?;
        m.create_table(_phy_addr::TABLE_LABEL).await?;
        m.create_table(_ship_opt::TABLE_LABEL).await?;
        m.create_table(_orderline::TABLE_LABEL).await?;
        m.create_table(_seller_currencies::TABLE_LABEL).await?;
        m.create_table(_order_toplvl_meta::TABLE_LABEL).await?;
        let stock_repo = StockLvlInMemRepo::build(m.clone(), timenow).await?;
        let job_time = DateTime::parse_from_rfc3339("2019-03-13T12:59:54+08:00").unwrap();
        let obj = Self {
            _sched_job_last_launched: Mutex::new(job_time),
            _stock: Arc::new(Box::new(stock_repo)),
            datastore: m,
        };
        Ok(obj)
    }
    pub(super) fn gen_lowlvl_tablerows(
        lineset: &OrderLineModelSet,
    ) -> Vec<(String, AppInMemFetchedSingleTable)> {
        let oid = lineset.order_id.as_str();
        vec![
            (
                _orderline::TABLE_LABEL.to_string(),
                _orderline::to_inmem_tbl(oid, &lineset.lines),
            ),
            (
                _seller_currencies::TABLE_LABEL.to_string(),
                _seller_currencies::to_inmem_tbl(oid, &lineset.currency.sellers),
            ),
            (
                _order_toplvl_meta::TABLE_LABEL.to_string(),
                _order_toplvl_meta::to_inmem_tbl(lineset),
            ),
        ] // TODO, add seller-currency table
    }
    async fn fetch_lines_common(
        &self,
        keys: Vec<String>,
    ) -> DefaultResult<Vec<OrderLineModel>, AppError> {
        let tbl_label = _orderline::TABLE_LABEL;
        let info = HashMap::from([(tbl_label.to_string(), keys)]);
        let mut data = self.datastore.fetch(info).await?;
        let data = data.remove(tbl_label).unwrap();
        let olines = data
            .into_values()
            .map(AppInMemFetchedSingleRow::into)
            .collect();
        Ok(olines)
    }
    async fn fetch_toplvl_meta(
        &self,
        order_id: &str,
    ) -> DefaultResult<InnerTopLvlWrapper, AppError> {
        let tbl_label = _order_toplvl_meta::TABLE_LABEL;
        let keys = vec![order_id.to_string()];
        let info = HashMap::from([(tbl_label.to_string(), keys)]);
        let mut data = self.datastore.fetch(info).await?;
        let result = data.remove(tbl_label).unwrap().into_values().next();
        if let Some(row) = result {
            Ok(InnerTopLvlWrapper::from(row))
        } else {
            let detail = order_id.to_string();
            Err(AppError {
                code: AppErrorCode::InvalidInput,
                detail: Some(detail),
            })
        }
    }
} // end of impl OrderInMemRepo
