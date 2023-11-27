use std::boxed::Box;
use std::sync::Arc;
use std::collections::HashMap;
use std::result::Result as DefaultResult;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset, Local as LocalTime};
use tokio::sync::Mutex;

use crate::AppDataStoreContext;
use crate::api::dto::{OrderLinePayDto, PhoneNumberDto, ShippingMethod};
use crate::api::rpc::dto::{OrderPaymentUpdateDto, OrderPaymentUpdateErrorDto};
use crate::constant::ProductType;
use crate::datastore::{AbstInMemoryDStore, AppInMemFetchedSingleTable, AppInMemFetchedSingleRow};
use crate::error::{AppError, AppErrorCode};
use crate::model::{
    OrderLineModel, BillingModel, ShippingModel, ContactModel, OrderLinePriceModel,
    OrderLineAppliedPolicyModel, PhyAddrModel, ShippingOptionModel, OrderLineQuantityModel,
    OrderLineModelSet, OrderLineIdentity
};

use super::{
    AbsOrderRepo, AbsOrderStockRepo, StockLvlInMemRepo, AppOrderRepoUpdateLinesUserFunc,
    AppOrderFetchRangeCallback
};

mod _contact {
    use super::{HashMap, AppInMemFetchedSingleRow, ContactModel};

    pub(super) const MULTI_VAL_COLUMN_SEPARATOR :&'static str = " ";
    pub(super) const TABLE_LABEL: &'static str = "order_contact";
    pub(super) enum InMemColIdx {FirstName, LastName, Emails, Phones, TotNumColumns}
    impl Into<usize> for InMemColIdx {
        fn into(self) -> usize {
            match self {
                Self::FirstName => 0,  Self::LastName => 1,  Self::Emails => 2,
                Self::Phones => 3,  Self::TotNumColumns => 4,
            }
        }
    }
    pub(super) fn to_inmem_tbl(oid:&str, usr_id:u32, pk_label:&str, data:ContactModel)
        -> HashMap<String, Vec<String>>
    { // each item in emails / phones array must NOT contain space character
        let row = AppInMemFetchedSingleRow::from(data);
        let pkey = format!("{}-{}-{}", oid, pk_label, usr_id);
        HashMap::from([(pkey, row)])
    }
    pub(super) fn inmem_parse_usr_id (pkey:&str) -> u32 {
        let mut id_elms = pkey.split("-");
        let (_oid, _label, usr_id) = (
            id_elms.next().unwrap(), id_elms.next().unwrap(),
            id_elms.next().unwrap().parse::<u32>().unwrap(),
        );
        usr_id
    }
} // end of inner module _contact

mod _phy_addr {
    use super::{HashMap, PhyAddrModel};

    pub(super) const TABLE_LABEL: &'static str = "order_phyaddr";
    pub(super) enum InMemColIdx {Country, Region, City, Distinct, Street, Detail, TotNumColumns}
    impl Into<usize> for InMemColIdx {
        fn into(self) -> usize {
            match self {
                Self::Country => 0,    Self::Region => 1,   Self::City => 2,
                Self::Distinct => 3,   Self::Street => 4,   Self::Detail => 5,
                Self::TotNumColumns => 6,
            }
        }
    }
    pub(super) fn to_inmem_tbl(oid:&str, pk_label:&str, data:PhyAddrModel)
        -> HashMap<String, Vec<String>>
    {
        let row = data.into();
        let pkey = format!("{}-{}", oid, pk_label);
        HashMap::from([(pkey, row)])
    }
} // end of inner module _phy_addr

mod _ship_opt {
    use super::{HashMap, ShippingOptionModel};

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
        -> HashMap<String, Vec<String>>
    {
        let kv_iter = data.into_iter().map(|m| {
            let pkey = format!("{}-{}", oid, m.seller_id);
            (pkey, m.into())
        });
        HashMap::from_iter(kv_iter)
    }
} // end of inner module _ship_opt

mod _orderline {
    use super::{HashMap, ProductType};
    use crate::model::OrderLineModel;
    
    pub(super) const TABLE_LABEL: &'static str = "order_line_reserved";
    pub(super) enum InMemColIdx { SellerID, ProductType, ProductId, QtyReserved, PriceUnit,
        PriceTotal, PolicyReserved, PolicyWarranty, QtyPaid, QtyPaidLastUpdate, TotNumColumns}
    impl Into<usize> for InMemColIdx {
        fn into(self) -> usize {
            match self {
                Self::SellerID => 0,    Self::ProductType => 1,
                Self::ProductId => 2,   Self::QtyReserved => 3,
                Self::QtyPaid => 4,     Self::QtyPaidLastUpdate => 5,
                Self::PriceUnit => 6,   Self::PriceTotal => 7,
                Self::PolicyReserved => 8,    Self::PolicyWarranty => 9,
                Self::TotNumColumns => 10,
            }
        }
    }
    pub(super) fn inmem_pkey(oid:&str, seller_id:u32, prod_typ:ProductType,
                             prod_id:u64) -> String
    {
        let prod_typ = <ProductType as Into<u8>>::into(prod_typ);
        format!("{oid}-{seller_id}-{prod_typ}-{prod_id}")
    }
    pub(super) fn to_inmem_tbl(oid:&str, data:&Vec<OrderLineModel>)
        ->  HashMap<String, Vec<String>>
    {
        let kv_iter = data.iter().map(|m| {
            let pkey = inmem_pkey(oid, m.id_.store_id, m.id_.product_type.clone(),
                                  m.id_.product_id);
            (pkey, m.into())
        });
        HashMap::from_iter(kv_iter)
    } // end of fn to_inmem_tbl
    pub(super) fn pk_group_by_oid(flattened:Vec<String>) -> HashMap<String, Vec<String>>
    {
        let mut out: HashMap<String, Vec<String>> = HashMap::new();
        flattened.into_iter().map(|key| {
            let oid = key.split("-").next().unwrap();
            if let Some(v) = out.get_mut(oid) {
                v.push(key);
            } else {
                out.insert(oid.to_string(), vec![key]);
            }
        }).count();
        out
    }
} // end of inner module _orderline

mod _pkey_partial_label {
    use crate::datastore::AbsDStoreFilterKeyOp;
    use super::{DateTime, FixedOffset};

    pub(super) const  BILLING:  &'static str = "billing";
    pub(super) const  SHIPPING: &'static str = "shipping";
    pub(super) struct InMemDStoreFiltKeyOID<'a> {
        pub oid: &'a str,
        pub label: Option<&'a str>,
    }
    impl<'a> AbsDStoreFilterKeyOp for InMemDStoreFiltKeyOID<'a> {
        fn filter(&self, k:&String, _v:&Vec<String>) -> bool {
            let mut id_elms = k.split("-");
            let oid_rd   = id_elms.next().unwrap();
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
        fn filter(&self, _k:&String, row:&Vec<String>) -> bool {
            let rsv_time = row.get(self.col_idx).unwrap();
            let rsv_time = DateTime::parse_from_rfc3339(rsv_time.as_str()).unwrap();
            (self.t0 < rsv_time) && (rsv_time < self.t1)
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
        let prod_id  = value.id_.product_id.to_string();
        let _paid_last_update = if let Some(v) = value.qty.paid_last_update.as_ref()
        { v.to_rfc3339() }
        else {
            assert_eq!(value.qty.paid, 0);
            String::new()
        };
        let mut row = (0.. _orderline::InMemColIdx::TotNumColumns.into())
            .map(|_num| {String::new()}).collect::<Self>();
        let _ = [
            (_orderline::InMemColIdx::QtyReserved,  value.qty.reserved.to_string()),
            (_orderline::InMemColIdx::QtyPaid,      value.qty.paid.to_string()),
            (_orderline::InMemColIdx::QtyPaidLastUpdate,  _paid_last_update),
            (_orderline::InMemColIdx::PriceUnit,  value.price.unit.to_string()),
            (_orderline::InMemColIdx::PriceTotal, value.price.total.to_string()),
            (_orderline::InMemColIdx::PolicyReserved, value.policy.reserved_until.to_rfc3339()),
            (_orderline::InMemColIdx::PolicyWarranty, value.policy.warranty_until.to_rfc3339()),
            (_orderline::InMemColIdx::ProductType, prod_typ),
            (_orderline::InMemColIdx::ProductId, prod_id),
            (_orderline::InMemColIdx::SellerID, seller_id_s),
        ].into_iter().map(|(idx,val)| {
            let idx:usize = idx.into();
            row[idx] = val;
        }).collect::<()>();
        row
    }
} // end of impl From for OrderLineModel reference
impl Into<OrderLineModel> for AppInMemFetchedSingleRow {
    fn into(self) -> OrderLineModel {
        let row = self;
        let seller_id = row.get::<usize>(_orderline::InMemColIdx::SellerID.into()).unwrap().parse().unwrap();
        let prod_typ = row.get::<usize>(_orderline::InMemColIdx::ProductType.into()).unwrap().parse::<u8>().unwrap();
        let product_id = row.get::<usize>(_orderline::InMemColIdx::ProductId.into()).unwrap().parse().unwrap() ;
        let price = OrderLinePriceModel {
            unit: row.get::<usize>(_orderline::InMemColIdx::PriceUnit.into()).unwrap().parse().unwrap(),
            total: row.get::<usize>(_orderline::InMemColIdx::PriceTotal.into()).unwrap().parse().unwrap()
        };
        let qty_paid_last_update = {
            let p = row.get::<usize>(_orderline::InMemColIdx::QtyPaidLastUpdate.into());
            let p = p.unwrap().as_str();
            if let Ok(v) = DateTime::parse_from_rfc3339(p) {
                Some(v)
            } else { None }
        };
        let qty = OrderLineQuantityModel {
            reserved: row.get::<usize>(_orderline::InMemColIdx::QtyReserved.into()).unwrap().parse().unwrap(),
            paid: row.get::<usize>(_orderline::InMemColIdx::QtyPaid.into()).unwrap().parse().unwrap(),
            paid_last_update: qty_paid_last_update
        };
        if qty.paid_last_update.is_none() {
            assert_eq!(qty.paid, 0);
        }
        let reserved_until = {
            let s = row.get::<usize>(_orderline::InMemColIdx::PolicyReserved.into()).unwrap();
            DateTime::parse_from_rfc3339(s.as_str()).unwrap()
        };
        let warranty_until = {
            let s = row.get::<usize>(_orderline::InMemColIdx::PolicyReserved.into()).unwrap();
            DateTime::parse_from_rfc3339(s.as_str()).unwrap()
        };
        let policy = OrderLineAppliedPolicyModel { reserved_until, warranty_until };
        OrderLineModel { id_: OrderLineIdentity{store_id: seller_id,  product_id,
              product_type: ProductType::from(prod_typ)}, price, policy, qty }
    }
} // end of impl into OrderLineModel

impl From<ContactModel> for AppInMemFetchedSingleRow {
    fn from(value: ContactModel) -> Self {
        let phones_str = value.phones.iter().map(|d| {
            format!("{}-{}", d.nation.to_string(), d.number)
        }).collect::<Vec<String>>();
        let mut row = (0 .. _contact::InMemColIdx::TotNumColumns.into())
            .map(|_num| {String::new()}).collect::<Self>();
        let _ = [
            (_contact::InMemColIdx::Emails,  value.emails.join(_contact::MULTI_VAL_COLUMN_SEPARATOR)),
            (_contact::InMemColIdx::Phones,  phones_str.join(_contact::MULTI_VAL_COLUMN_SEPARATOR)),
            (_contact::InMemColIdx::FirstName, value.first_name),
            (_contact::InMemColIdx::LastName,  value.last_name),
        ].into_iter().map(|(idx, val)| {
            let idx:usize = idx.into();
            row[idx] = val;
        }).collect::<Vec<()>>();
        row
    }
}
impl Into<ContactModel> for AppInMemFetchedSingleRow {
    fn into(self) -> ContactModel {
        let emails = self.get::<usize>(_contact::InMemColIdx::Emails.into())
            .unwrap().split(_contact::MULTI_VAL_COLUMN_SEPARATOR).into_iter()
            .map(|s| s.to_string())  .collect() ;
        let phones = self.get::<usize>(_contact::InMemColIdx::Phones.into())
            .unwrap().split(_contact::MULTI_VAL_COLUMN_SEPARATOR).into_iter()
            .map(|s| {
                let mut s = s.split("-");
                let nation = s.next().unwrap().parse().unwrap();
                let number = s.next().unwrap().to_string();
                PhoneNumberDto { nation, number }
            }).collect();
        let (first_name, last_name) = (
            self.get::<usize>(_contact::InMemColIdx::FirstName.into()).unwrap().to_owned(),
            self.get::<usize>(_contact::InMemColIdx::LastName.into()).unwrap().to_owned()
        );
        ContactModel { first_name, last_name, emails, phones }
    }
}

impl From<PhyAddrModel> for AppInMemFetchedSingleRow {
    fn from(value: PhyAddrModel) -> Self {
        let mut row = (0 .. _phy_addr::InMemColIdx::TotNumColumns.into())
            .map(|_num| {String::new()}).collect::<Self>();
        let _ = [
            (_phy_addr::InMemColIdx::Detail,  value.detail),
            (_phy_addr::InMemColIdx::Distinct, value.distinct),
            (_phy_addr::InMemColIdx::Street,  value.street_name.unwrap_or("".to_string())),
            (_phy_addr::InMemColIdx::Region,  value.region),
            (_phy_addr::InMemColIdx::City,    value.city),
            (_phy_addr::InMemColIdx::Country, value.country.into() ),
        ].into_iter().map(|(idx,val)| {
            let idx:usize = idx.into();
            row[idx] = val;
        }).collect::<()>();
        row
    }
}
impl Into<PhyAddrModel> for AppInMemFetchedSingleRow {
    fn into(self) -> PhyAddrModel {
        let (country, region, city, distinct, street, detail) = (
            self.get::<usize>(_phy_addr::InMemColIdx::Country.into()).unwrap().to_owned().into() ,
            self.get::<usize>(_phy_addr::InMemColIdx::Region.into()).unwrap().to_owned(),
            self.get::<usize>(_phy_addr::InMemColIdx::City.into()).unwrap().to_owned(),
            self.get::<usize>(_phy_addr::InMemColIdx::Distinct.into()).unwrap().to_owned(),
            self.get::<usize>(_phy_addr::InMemColIdx::Street.into()).unwrap().to_owned(),
            self.get::<usize>(_phy_addr::InMemColIdx::Detail.into()).unwrap().to_owned()
        );
        let street_name = if street.is_empty() {None} else {Some(street)};
        PhyAddrModel { country, region, city, distinct, street_name, detail }
    }
}

impl From<ShippingOptionModel> for AppInMemFetchedSingleRow {
    fn from(value: ShippingOptionModel) -> Self {
        let mut row = (0 .. _ship_opt::InMemColIdx::TotNumColumns.into())
            .map(|_num| {String::new()}).collect::<Self>();
        let _ = [
            (_ship_opt::InMemColIdx::SellerID,  value.seller_id.to_string()),
            (_ship_opt::InMemColIdx::Method, value.method.into()),
        ].into_iter().map(|(idx,val)| {
            let idx:usize = idx.into();
            row[idx] = val;
        }).collect::<()>();
        row
    }
}
impl Into<ShippingOptionModel> for AppInMemFetchedSingleRow {
    fn into(self) -> ShippingOptionModel {
        let (seller_id, method) = (
            self.get::<usize>(_ship_opt::InMemColIdx::SellerID.into()).unwrap().parse().unwrap() ,
            self.get::<usize>(_ship_opt::InMemColIdx::Method.into()).unwrap().to_owned()
        );
        ShippingOptionModel { seller_id, method:ShippingMethod::from(method) } 
    }
}


#[async_trait]
impl AbsOrderRepo for OrderInMemRepo {
    async fn new(ds:Arc<AppDataStoreContext>) -> DefaultResult<Box<dyn AbsOrderRepo>, AppError>
        where Self:Sized
    {
        let timenow = LocalTime::now().into();
        match Self::build(ds, timenow).await {
            Ok(obj) => Ok(Box::new(obj)),
            Err(e) => Err(e)
        }
    }
    fn stock(&self) -> Arc<Box<dyn AbsOrderStockRepo>>
    { self._stock.clone() }

    async fn create (&self, usr_id:u32, lineset:OrderLineModelSet,
                     bl:BillingModel, sh:ShippingModel)
        -> DefaultResult<Vec<OrderLinePayDto>, AppError> 
    {
        let oid = lineset.order_id.as_str();
        let mut tabledata:[(String, AppInMemFetchedSingleTable);4] = [
            (_contact::TABLE_LABEL.to_string(), HashMap::new()),
            (_phy_addr::TABLE_LABEL.to_string(), HashMap::new()),
            (_ship_opt::TABLE_LABEL.to_string(), _ship_opt::to_inmem_tbl(oid, sh.option)),
            (_orderline::TABLE_LABEL.to_string(), _orderline::to_inmem_tbl(oid, &lineset.lines)),
        ];
        {
            let items = _contact::to_inmem_tbl(oid, usr_id,
                 _pkey_partial_label::SHIPPING, sh.contact);
            items.into_iter().map(|(k,v)| {tabledata[0].1.insert(k, v);}).count();
            let items = _contact::to_inmem_tbl(oid, usr_id,
                 _pkey_partial_label::BILLING, bl.contact);
            items.into_iter().map(|(k,v)| {tabledata[0].1.insert(k, v);}).count();
        }
        if let Some(addr) = bl.address {
            let items = _phy_addr::to_inmem_tbl(oid, _pkey_partial_label::BILLING, addr);
            items.into_iter().map(|(k,v)| {tabledata[1].1.insert(k, v);}).count();
        }
        if let Some(addr) = sh.address {
            let items = _phy_addr::to_inmem_tbl(oid, _pkey_partial_label::SHIPPING, addr);
            items.into_iter().map(|(k,v)| {tabledata[1].1.insert(k, v);}).count();
        }
        let data = HashMap::from_iter(tabledata.into_iter());
        let _num = self.datastore.save(data).await?;
        let paylines = lineset.lines.into_iter().map(OrderLineModel::into).collect();
        Ok(paylines)
    } // end of fn create

    async fn fetch_all_lines(&self, oid:String) -> DefaultResult<Vec<OrderLineModel>, AppError>
    {
        let op = _pkey_partial_label::InMemDStoreFiltKeyOID {oid:oid.as_str(), label:None};
        let tbl_label = _orderline::TABLE_LABEL.to_string();
        let keys = self.datastore.filter_keys(tbl_label, &op).await?;
        self.fetch_lines_common(keys).await
    }

    async fn fetch_billing(&self, oid:String) -> DefaultResult<(BillingModel, u32), AppError>
    {
        let op = _pkey_partial_label::InMemDStoreFiltKeyOID {
                oid:oid.as_str(),  label: Some(_pkey_partial_label::BILLING) };
        let tbl_labels = [ _contact::TABLE_LABEL , _phy_addr::TABLE_LABEL ];
        let mut info = vec![];
        for table_name in tbl_labels.iter() {
            let keys = self.datastore.filter_keys(table_name.to_string(), &op).await?;
            info.push ((table_name.to_string(), keys));
        };
        let info = HashMap::from_iter(info.into_iter());
        let mut data = self.datastore.fetch(info).await ?;
        let (result1, result2) = (
            data.remove(tbl_labels[0]).unwrap().into_iter().next(),
            data.remove(tbl_labels[1]).unwrap().into_values().next()
        );
        if let Some((pkey,raw_cta)) = result1 {
            let usr_id  = _contact::inmem_parse_usr_id(pkey.as_str());
            let contact = raw_cta.into();
            let address = if let Some(raw_pa) = result2 {
                Some(raw_pa.into())
            } else { None };
            Ok((BillingModel{contact, address}, usr_id))
        } else {
            let ioe = std::io::ErrorKind::NotFound;
            let detail = format!("no-contact-data");
            let e = AppError {code:AppErrorCode::IOerror(ioe), detail:Some(detail)};
            Err(e)
        }
    } // end of fn fetch_billing
    
    async fn fetch_shipping(&self, oid:String) -> DefaultResult<(ShippingModel, u32), AppError>
    {
        let ops = [
            _pkey_partial_label::InMemDStoreFiltKeyOID {oid:oid.as_str(), label: Some(_pkey_partial_label::SHIPPING)},
            _pkey_partial_label::InMemDStoreFiltKeyOID {oid:oid.as_str(), label: None }
        ];
        let data = [ 
            (_contact::TABLE_LABEL, &ops[0]),
            (_phy_addr::TABLE_LABEL, &ops[0]),
            (_ship_opt::TABLE_LABEL, &ops[1])
        ];
        let mut info = vec![];
        for (table_name, op) in data.into_iter() {
            let keys = self.datastore.filter_keys(table_name.to_string(), op).await?;
            info.push ((table_name.to_string(), keys));
        };
        let info = HashMap::from_iter(info.into_iter());
        let mut data = self.datastore.fetch(info).await ?;
        let (result1, result2, result3) = (
            data.remove(_contact::TABLE_LABEL).unwrap().into_iter().next(),
            data.remove(_phy_addr::TABLE_LABEL).unwrap().into_values().next(),
            data.remove(_ship_opt::TABLE_LABEL).unwrap().into_values(),
        );
        if let Some((pkey, raw_cta)) = result1 {
            let usr_id  = _contact::inmem_parse_usr_id(pkey.as_str());
            let contact = raw_cta.into();
            let address = if let Some(raw_pa) = result2 {
                Some(raw_pa.into())
            } else { None }; // shipping option can be empty
            let option = result3.map(AppInMemFetchedSingleRow::into).collect();
            Ok((ShippingModel{contact, address, option}, usr_id))
        } else {
            let ioe = std::io::ErrorKind::NotFound;
            let detail = format!("no-contact-data");
            let e = AppError {code:AppErrorCode::IOerror(ioe), detail:Some(detail)};
            Err(e)
        }
    } // end of fetch_shipping
    
    async fn update_lines_payment(&self, data:OrderPaymentUpdateDto,
                                  usr_cb:AppOrderRepoUpdateLinesUserFunc)
        -> DefaultResult<OrderPaymentUpdateErrorDto, AppError>
    {
        let table_name = _orderline::TABLE_LABEL;
        let (oid, d_lines) = (data.oid, data.lines);
        let num_data_items = d_lines.len();
        let (mut models, g_lock) = {
            let pids = d_lines.iter().map(|d| {
                _orderline::inmem_pkey(oid.as_ref(), d.seller_id, d.product_type.clone(), d.product_id)
            }).collect();
            let info = HashMap::from([(table_name.to_string(), pids)]);
            let (mut rawdata, lock) = self.datastore.fetch_acquire(info).await?;
            let rawdata = rawdata.remove(table_name).unwrap();
            let ms = rawdata.into_values().map(AppInMemFetchedSingleRow::into).collect();
            (ms, lock)
        };
        let errors = usr_cb(&mut models, d_lines);
        if errors.len() < num_data_items {
            let rows = _orderline::to_inmem_tbl(oid.as_str(), &models);
            let info = HashMap::from([(table_name.to_string(), rows)]);
            let _num = self.datastore.save_release(info, g_lock)?;
        } // no need to save if all data items cause errors
        Ok(OrderPaymentUpdateErrorDto {oid, lines:errors})
    } // end of fn update_lines_payment

    async fn fetch_lines_by_rsvtime(&self, time_start: DateTime<FixedOffset>,
                                  time_end: DateTime<FixedOffset>,
                                  usr_cb: AppOrderFetchRangeCallback )
        -> DefaultResult<(), AppError>
    { // fetch lines by range of reserved time
        let table_name = _orderline::TABLE_LABEL;
        let op = _pkey_partial_label::InMemDStoreFilterTimeRangeOp {
            col_idx: _orderline::InMemColIdx::PolicyReserved.into(),
            t0:time_start, t1:time_end,
        };
        let keys_flattened = self.datastore.filter_keys(table_name.to_string(), &op).await?;
        let key_grps = _orderline::pk_group_by_oid(keys_flattened);
        for (oid, keys) in key_grps.into_iter() {
            let ms = self.fetch_lines_common(keys).await?;
            let mset = OrderLineModelSet { order_id:oid, lines: ms };
            usr_cb(self, mset).await?;
        }
        Ok(())
    } // end of fn fetch_lines_by_rsvtime
        
    async fn fetch_lines_by_pid(&self, oid:&str, pids:Vec<OrderLineIdentity>)
        -> DefaultResult<Vec<OrderLineModel>, AppError>
    {
        let keys = pids.into_iter().map(|d| {
            _orderline::inmem_pkey(oid, d.store_id, d.product_type, d.product_id)
        }).collect() ;
        self.fetch_lines_common(keys).await
    }

    async fn owner_id(&self, order_id:&str) -> DefaultResult<u32, AppError>
    {
        let tbl_label = _contact::TABLE_LABEL.to_string();
        let op = _pkey_partial_label::InMemDStoreFiltKeyOID {
                oid: order_id,  label: Some(_pkey_partial_label::BILLING) };
        let keys = self.datastore.filter_keys(tbl_label.clone(), &op).await?;
        let info = HashMap::from([(tbl_label.clone(), keys)]);
        let mut data = self.datastore.fetch(info).await ?;
        let result = data.remove(tbl_label.as_str()).unwrap().into_iter().next();
        if let Some((pkey, _raw_val)) = result {
            let usr_id = _contact::inmem_parse_usr_id(pkey.as_str());
            Ok(usr_id)
        } else {
            let detail = order_id.to_string();
            Err(AppError { code: AppErrorCode::InvalidInput, detail: Some(detail) })
        }
    }

    async fn scheduled_job_last_time(&self) -> DateTime<FixedOffset>
    {
        let guard = self._sched_job_last_launched.lock().await;
        guard.clone()
    }

    async fn scheduled_job_time_update(&self)
    {
        let mut guard = self._sched_job_last_launched.lock().await;
        let t:DateTime<FixedOffset> = LocalTime::now().into();
        *guard = t;
    }
} // end of impl AbsOrderRepo


impl OrderInMemRepo {
    pub async fn build(ds:Arc<AppDataStoreContext>, curr_time:DateTime<FixedOffset>)
        -> DefaultResult<Self, AppError>
    {
        if let Some(m) = &ds.in_mem {
            m.create_table(_contact::TABLE_LABEL).await?;
            m.create_table(_phy_addr::TABLE_LABEL).await?;
            m.create_table(_ship_opt::TABLE_LABEL).await?;
            m.create_table(_orderline::TABLE_LABEL).await?;
            let stock_repo = StockLvlInMemRepo::build(m.clone(), curr_time).await ?;
            let job_time = DateTime::parse_from_rfc3339("2019-03-13T12:59:54+08:00").unwrap();
            let obj = Self {
                _sched_job_last_launched: Mutex::new(job_time),
                _stock:Arc::new(Box::new(stock_repo)),
               datastore:m.clone(),
            };
            Ok(obj)
        } else {
            Err(AppError {code:AppErrorCode::MissingDataStore,
                detail: Some(format!("in-memory"))}  )
        }
    }
    async fn fetch_lines_common(&self, keys:Vec<String>)
        -> DefaultResult<Vec<OrderLineModel>, AppError>
    {
        let tbl_label = _orderline::TABLE_LABEL;
        let info = HashMap::from([(tbl_label.to_string(), keys)]);
        let mut data = self.datastore.fetch(info).await ?;
        let data = data.remove(tbl_label).unwrap();
        let olines = data.into_values().map(AppInMemFetchedSingleRow::into)
            .collect();
        Ok(olines)
    }
} // end of impl OrderInMemRepo

