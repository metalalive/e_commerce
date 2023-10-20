use std::boxed::Box;
use std::sync::Arc;
use std::collections::HashMap;
use std::result::Result as DefaultResult;

use async_trait::async_trait;
use chrono::DateTime;
use rand;
use uuid::{Uuid, Builder, Timestamp, NoContext};

use crate::AppDataStoreContext;
use crate::api::web::dto::OrderLinePayDto;
use crate::constant::ProductType;
use crate::datastore::AbstInMemoryDStore;
use crate::error::{AppError, AppErrorCode};
use crate::model::{
    StockLevelModelSet, ProductStockIdentity, ProductStockModel, StoreStockModel,
    StockQuantityModel, OrderLineModel, BillingModel, ShippingModel
};

use super::{AbsOrderRepo, AbsOrderStockRepo, OrderRepoCreateErrResult};

mod _stockm {
    pub(super) const TABLE_LABEL: &'static str = "order_stock_lvl";
    pub(super) enum InMemColIdx {Expiry, QtyTotal, QtyBooked, QtyCancelled, TotNumColumns}
    impl Into<usize> for InMemColIdx {
        fn into(self) -> usize {
            match self {
                Self::Expiry => 0,
                Self::QtyTotal  => 1,
                Self::QtyBooked => 2,
                Self::QtyCancelled  => 3,
                Self::TotNumColumns => 4,
            }
        }
    }
} // end of inner module _stockm

mod _contact {
    use super::HashMap;
    use crate::model::ContactModel;

    const MULTI_VAL_COLUMN_SEPARATOR :&'static str = " ";
    pub(super) const TABLE_LABEL: &'static str = "order_contact";
    pub(super) enum InMemColIdx {UsrProfId, FirstName, LastName, Emails, Phones, TotNumColumns}
    impl Into<usize> for InMemColIdx {
        fn into(self) -> usize {
            match self {
                Self::FirstName => 0,  Self::LastName => 1,
                Self::Emails => 2,     Self::Phones => 3,
                Self::UsrProfId => 4,  Self::TotNumColumns => 5,
            }
        }
    }
    pub(super) fn to_inmem_tbl(oid:&str, usr_id:u32, pk_label:&str, data:ContactModel)
        -> (String, HashMap<String, Vec<String>>)
    { // each item in emails / phones array must NOT contain space character
        let phones_str = data.phones.iter().map(|d| {
            format!("{}-{}", d.nation.to_string(), d.number)
        }).collect::<Vec<String>>();
        let mut row = (0 .. InMemColIdx::TotNumColumns.into())
            .map(|_num| {String::new()}).collect::<Vec<String>>();
        let _ = [
            (InMemColIdx::Emails,  data.emails.join(MULTI_VAL_COLUMN_SEPARATOR)),
            (InMemColIdx::Phones,  phones_str.join(MULTI_VAL_COLUMN_SEPARATOR)),
            (InMemColIdx::UsrProfId, usr_id.to_string()),
            (InMemColIdx::FirstName, data.first_name),
            (InMemColIdx::LastName,  data.last_name),
        ].into_iter().map(|(idx, val)| {
            let idx:usize = idx.into();
            row[idx] = val;
        }).collect::<Vec<()>>();
        let pkey = format!("{}-{}", oid, pk_label);
        let table = HashMap::from([(pkey, row)]);
        (self::TABLE_LABEL.to_string(), table)
    }
} // end of inner module _contact

mod _phy_addr {
    use super::HashMap;
    use crate::model::PhyAddrModel;

    pub(super) const TABLE_LABEL: &'static str = "order_phyaddr";
    pub(super) enum InMemColIdx {Country, Region, City, Distinct, Street, Detail, TotNumColumns}
    impl Into<usize> for InMemColIdx {
        fn into(self) -> usize {
            match self {
                Self::Country => 0,    Self::Region => 1,
                Self::City => 2,       Self::Distinct => 3,
                Self::Street => 4,     Self::Detail => 5,
                Self::TotNumColumns => 6,
            }
        }
    }
    pub(super) fn to_inmem_tbl(oid:&str, pk_label:&str, data:PhyAddrModel)
        -> (String, HashMap<String, Vec<String>>)
    {
        let mut row = (0..InMemColIdx::TotNumColumns.into())
            .map(|_num| {String::new()}).collect::<Vec<String>>();
        let _ = [
            (InMemColIdx::Detail, data.detail),
            (InMemColIdx::Distinct, data.distinct),
            (InMemColIdx::Street, data.street_name.unwrap_or("".to_string())),
            (InMemColIdx::Region, data.region),
            (InMemColIdx::City,   data.city),
            (InMemColIdx::Country, data.country.into() ),
        ].into_iter().map(|(idx,val)| {
            let idx:usize = idx.into();
            row[idx] = val;
        }).collect::<()>();
        let pkey = format!("{}-{}", oid, pk_label);
        let table = HashMap::from([(pkey, row)]);
        (self::TABLE_LABEL.to_string(), table)
    }
} // end of inner module _phy_addr

mod _ship_opt {
    use super::HashMap;
    use crate::model::ShippingOptionModel;

    pub(super) enum InMemColIdx {SellerID, Method, TotNumColumns}
    impl Into<usize> for InMemColIdx {
        fn into(self) -> usize {
            match self {
                Self::SellerID => 0,    Self::Method => 1,
                Self::TotNumColumns => 2,
            }
        }
    }
    pub(super) const TABLE_LABEL: &'static str = "order_shipping_option";
    pub(super) fn to_inmem_tbl(oid:&str, data:Vec<ShippingOptionModel>)
        -> (String, HashMap<String, Vec<String>>)
    {
        let kv_iter = data.into_iter().map(|m| {
            let seller_id_s = m.seller_id.to_string();
            let pkey = format!("{oid}-{seller_id_s}");
            let row = vec![seller_id_s, m.method.into()];
            (pkey, row)
        });
        let table = HashMap::from_iter(kv_iter);
        (self::TABLE_LABEL.to_string(), table)
    }
} // end of inner module _ship_opt

mod _orderline {
    use super::{HashMap, ProductType};
    use crate::model::OrderLineModel;
    
    pub(super) const TABLE_LABEL: &'static str = "order_line_reserved";
    pub(super) enum InMemColIdx {SellerID, ProductType, ProductId, Quantity, PriceUnit,
        PriceTotal, PolicyReserved, PolicyWarranty, TotNumColumns}
    impl Into<usize> for InMemColIdx {
        fn into(self) -> usize {
            match self {
                Self::SellerID => 0,    Self::ProductType => 1,
                Self::ProductId => 2,   Self::Quantity => 3,
                Self::PriceUnit => 4,   Self::PriceTotal => 5,
                Self::PolicyReserved => 6,    Self::PolicyWarranty => 7,
                Self::TotNumColumns => 8,
            }
        }
    }
    pub(super) fn to_inmem_tbl(oid:&str, data:&Vec<OrderLineModel>)
        -> (String, HashMap<String, Vec<String>>)
    {
        let kv_iter = data.iter().map(|m| {
            let seller_id_s = m.seller_id.to_string();
            let prod_typ = <ProductType as Into<u8>>::into(m.product_type.clone()).to_string();
            let prod_id  = m.product_id.to_string();
            let pkey = format!("{oid}-{seller_id_s}-{prod_typ}-{prod_id}");
            let mut row = (0..InMemColIdx::TotNumColumns.into())
                .map(|_num| {String::new()}).collect::<Vec<String>>();
            let _ = [
                (InMemColIdx::Quantity,   m.qty.to_string()),
                (InMemColIdx::PriceUnit,  m.price.unit.to_string()),
                (InMemColIdx::PriceTotal, m.price.total.to_string()),
                (InMemColIdx::PolicyReserved, m.policy.reserved_until.to_rfc3339()),
                (InMemColIdx::PolicyWarranty, m.policy.warranty_until.to_rfc3339()),
                (InMemColIdx::ProductType, prod_typ),
                (InMemColIdx::ProductId, prod_id),
                (InMemColIdx::SellerID, seller_id_s),
            ].into_iter().map(|(idx,val)| {
                let idx:usize = idx.into();
                row[idx] = val;
            }).collect::<()>();
            (pkey, row)
        });
        let table = HashMap::from_iter(kv_iter);
        (self::TABLE_LABEL.to_string(), table)
    } // end of fn to_inmem_tbl
} // end of inner module _orderline

mod _pkey_partial_label {
    pub(super) const  BILLING:  &'static str = "billing";
    pub(super) const  SHIPPING: &'static str = "shipping";
}


// in-memory repo is unable to do concurrency test between web app
// and rpc consumer app, also it should't be deployed in production
// environment
struct StockLvlInMemRepo {
    datastore: Arc<Box<dyn AbstInMemoryDStore>>,
    _expiry_key_fmt:String,
}
pub struct OrderInMemRepo {
    datastore: Arc<Box<dyn AbstInMemoryDStore>>,
    _stock: Arc<Box<dyn AbsOrderStockRepo>>
}

#[async_trait]
impl AbsOrderStockRepo for StockLvlInMemRepo {
    async fn fetch(&self, pids:Vec<ProductStockIdentity>) -> DefaultResult<StockLevelModelSet, AppError>
    {
        let ids = pids.into_iter().map(|d| {
            let prod_typ_num:u8 = d.product_type.into();
            let exp_fmt = d.expiry.format(self._expiry_key_fmt.as_str());
            format!("{}-{}-{}-{}", d.store_id, prod_typ_num, d.product_id, exp_fmt)
        }).collect();
        let info = HashMap::from([(_stockm::TABLE_LABEL.to_string(), ids)]);
        let resultset = self.datastore.fetch(info) ?;
        if let Some((_label, rows)) = resultset.into_iter().next() {
            let mut out = StockLevelModelSet {stores:vec![]};
            let _ = rows.into_iter().map(|(key, row)| {
                let id_elms = key.split("-").collect::<Vec<&str>>();
                let prod_typ_num:u8 = id_elms[1].parse().unwrap();
                let (store_id, prod_typ, prod_id, exp_from_combo) = (
                    id_elms[0].parse().unwrap(),  ProductType::from(prod_typ_num),
                    id_elms[2].parse().unwrap(),  id_elms[3]    );
                let result = out.stores.iter_mut().find(|m| m.store_id==store_id);
                let store_rd = if let Some(m) = result {
                    m
                } else {
                    let m = StoreStockModel {store_id, products:vec![]};
                    out.stores.push(m);
                    out.stores.last_mut().unwrap()
                };
                let result = store_rd.products.iter().find(|m| {
                    let exp_fmt_verify = m.expiry.format(self._expiry_key_fmt.as_str()).to_string();
                    m.type_==prod_typ && m.id_==prod_id && exp_fmt_verify==exp_from_combo
                });
                if let Some(_product_rd) = result {
                    let _prod_typ_num:u8 = _product_rd.type_.clone().into();
                    panic!("report error, data corruption, store:{}, product: ({}, {})", 
                           store_rd.store_id, _prod_typ_num, _product_rd.id_);
                    // TODO, return error instead 
                } else {
                    let total = row.get::<usize>(_stockm::InMemColIdx::QtyTotal.into())
                        .unwrap().parse().unwrap();
                    let booked = row.get::<usize>(_stockm::InMemColIdx::QtyBooked.into())
                        .unwrap().parse().unwrap();
                    let cancelled = row.get::<usize>(_stockm::InMemColIdx::QtyCancelled.into())
                        .unwrap().parse().unwrap();
                    let expiry = row.get::<usize>(_stockm::InMemColIdx::Expiry.into()).unwrap();
                    let expiry = DateTime::parse_from_rfc3339(&expiry).unwrap();
                    let m = ProductStockModel {is_create:false, type_:prod_typ, id_:prod_id,
                        expiry, quantity: StockQuantityModel{total, booked, cancelled}
                    };
                    store_rd.products.push(m);
                }
            }).collect::<Vec<()>>();
            Ok(out)
        } else {
            Err(AppError { code:AppErrorCode::DataTableNotExist,
                detail:Some(_stockm::TABLE_LABEL.to_string())  })
        }
    } // end of fn fetch

    
    async fn save(&self, slset:StockLevelModelSet) -> DefaultResult<(), AppError>
    {
        let rows = {
            let kv_pairs = slset.stores.iter().flat_map(|m1| {
                m1.products.iter().map(|m2| {
                    let exp_fmt = m2.expiry_without_millis().format(self._expiry_key_fmt.as_str());
                    let prod_typ_num:u8 = m2.type_.clone().into();
                    let pkey = format!("{}-{}-{}-{}", m1.store_id, prod_typ_num, m2.id_, exp_fmt);
                    let mut row = (0 .. _stockm::InMemColIdx::TotNumColumns.into())
                        .map(|_n| {String::new()}).collect::<Vec<String>>();
                    let _ = [
                        (_stockm::InMemColIdx::QtyCancelled, m2.quantity.cancelled.to_string()),
                        (_stockm::InMemColIdx::QtyBooked, m2.quantity.booked.to_string()),
                        (_stockm::InMemColIdx::QtyTotal,  m2.quantity.total.to_string()),
                        (_stockm::InMemColIdx::Expiry,  m2.expiry.to_rfc3339()),
                    ].into_iter().map(|(idx, val)| {
                        let idx:usize = idx.into();
                        row[idx] = val;
                    }).collect::<Vec<()>>();
                    (pkey, row)
                }) // end of inner iter
            }); // end of outer iter
            HashMap::from_iter(kv_pairs)
        };
        let table = (_stockm::TABLE_LABEL.to_string(), rows);
        let data = HashMap::from([table]);
        let _num_saved = self.datastore.save(data)?;
        Ok(())
    } // end of fn save
} // end of impl StockLvlInMemRepo


#[async_trait]
impl AbsOrderRepo for OrderInMemRepo {
    fn new(ds:Arc<AppDataStoreContext>) -> DefaultResult<Box<dyn AbsOrderRepo>, AppError>
        where Self:Sized
    {
        match Self::build(ds) {
            Ok(obj) => Ok(Box::new(obj)),
            Err(e) => Err(e)
        }
    }
    fn stock(&self) -> Arc<Box<dyn AbsOrderStockRepo>>
    { self._stock.clone() }

    async fn create (&self, usr_id:u32, lines:Vec<OrderLineModel>,
                     bl:BillingModel, sh:ShippingModel)
        -> DefaultResult<(String, Vec<OrderLinePayDto>), OrderRepoCreateErrResult> 
    { // TODO, machine code to UUID generator should be configurable
        let machine_code = 1u8;
        let uid = generate_rand_unique_sequence(machine_code);
        let oid = Self::convert_to_order_id(uid);
        self.check_stock_level(&lines)?;
        let mut tabledata = vec![
            _contact::to_inmem_tbl(oid.as_str(), usr_id, _pkey_partial_label::BILLING, bl.contact),
            _contact::to_inmem_tbl(oid.as_str(), usr_id, _pkey_partial_label::SHIPPING, sh.contact),
            _ship_opt::to_inmem_tbl(oid.as_str(), sh.option),
            _orderline::to_inmem_tbl(oid.as_str(), &lines),
        ];
        if let Some(addr) = bl.address {
            let item = _phy_addr::to_inmem_tbl(oid.as_str(), _pkey_partial_label::BILLING, addr);
            tabledata.push(item);
        }
        if let Some(addr) = sh.address {
            let item = _phy_addr::to_inmem_tbl(oid.as_str(), _pkey_partial_label::SHIPPING, addr);
            tabledata.push(item);
        }
        let data = HashMap::from_iter(tabledata.into_iter());
        match self.datastore.save(data) {
             Ok(_num) => {
                 let paylines = lines.into_iter().map(OrderLineModel::into).collect();
                 Ok((oid, paylines))
             },
             Err(e) => Err(OrderRepoCreateErrResult::Aborted(e))
        }
    } // end of fn create
} // end of impl AbsOrderRepo


impl OrderInMemRepo {
    pub fn build(ds:Arc<AppDataStoreContext>) -> DefaultResult<Self, AppError>
    {
        if let Some(m) = &ds.in_mem {
            m.create_table(_stockm::TABLE_LABEL)?;
            m.create_table(_contact::TABLE_LABEL)?;
            m.create_table(_phy_addr::TABLE_LABEL)?;
            m.create_table(_ship_opt::TABLE_LABEL)?;
            m.create_table(_orderline::TABLE_LABEL)?;
            let stock_repo = StockLvlInMemRepo { datastore: m.clone(),
                  _expiry_key_fmt:"%Y%m%d%H%M%S".to_string() };
            let obj = Self{ _stock:Arc::new(Box::new(stock_repo)), datastore:m.clone() };
            Ok(obj)
        } else {
            Err(AppError {code:AppErrorCode::MissingDataStore,
                detail: Some(format!("in-memory"))}  )
        }
    }
    fn convert_to_order_id(uid:Uuid) -> String
    {
        let bs = uid.into_bytes();
        bs.into_iter().map(|b| format!("{:02x}",b))
            .collect::<Vec<String>>().join("")
    }

    fn check_stock_level(&self, _req:&Vec<OrderLineModel>) -> DefaultResult<(), OrderRepoCreateErrResult>
    {
        // TODO, finish the implementation
        // - call `filter_keys()` to in-memory data store, to fetch all keys
        //   in stock-level table whoss records haven't expired yet.
        // - pass the filtered keys to `fetch()`, fetch the stock records
        let op = xx;
        let tbl_label = _stockm::TABLE_LABEL.to_string();
        let result = self.datastore.filter_keys(tbl_label, op);
        Ok(())
    }
} // end of impl OrderInMemRepo

fn  generate_rand_unique_sequence (machine_code:u8) -> Uuid
{ // TODO  consider to declare this as trait method or utility.
    // UUIDv7 is for single-node application. This app needs to consider
    // scalability of multi-node environment, UUIDv8 can be utilized cuz it
    // allows custom ID layout, so few bits of the ID can be assigned to
    // represent each machine/node ID,  rest of that should be timestamp with
    // random byte sequence
    let ts_ctx = NoContext;
    let (secs, nano) = Timestamp::now(ts_ctx).to_unix();
    let millis = (secs * 1000).saturating_add((nano as u64) / 1_000_000);
    let mut node_id = rand::random::<[u8;10]>();
    node_id[0] = machine_code;
    let builder = Builder::from_unix_timestamp_millis(millis, &node_id);
    builder.into_uuid()
}

#[test]
fn test_gen_rand_unique_seq() {
    use std::collections::HashSet;
    use std::collections::hash_map::RandomState;
    let num_ids = 10;
    let machine_code = 1;
    let iter = (0 .. num_ids).into_iter().map(|_d| {
        let uid = generate_rand_unique_sequence(machine_code);
        let s = OrderInMemRepo::convert_to_order_id(uid);
        // println!("generated ID : {}", s.as_str());
        s
    });
    let hs : HashSet<String, RandomState> = HashSet::from_iter(iter);
    assert_eq!(hs.len(), num_ids);
}

