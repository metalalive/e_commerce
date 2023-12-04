use std::marker::{Sync, Send};
use std::result::Result as DefaultResult;
use std::collections::HashMap;
use std::cell::{RefCell, RefMut};
use std::vec;

use async_trait::async_trait;
use tokio::sync::{Mutex, MutexGuard};

use crate::config::AppInMemoryDbCfg;
use crate::error::{AppError, AppErrorCode};

// simple implementation of in-memory data storage

// application callers are responsible to maintain the structure
// of each row in each table. Each element of a row is stringified 
// regardless of its original types (integer, floating-point number)
type InnerKey = String;
type InnerTableLabel = String;
type InnerRow = Vec<String>;
type InnerTable = HashMap<InnerKey, InnerRow>;
type AllTable = HashMap<InnerTableLabel, InnerTable>;
pub type AppInMemUpdateData = AllTable;
pub type AppInMemDeleteInfo = InnerTable; // list of IDs per table
pub type AppInMemFetchKeys = InnerTable; // list of IDs per table
pub type AppInMemFetchedSingleRow = InnerRow; // list of IDs per table
pub type AppInMemFetchedSingleTable = InnerTable; // list of IDs per table
pub type AppInMemFetchedData = AllTable; // TODO, rename to data set
pub type AppInMemDstoreLock<'a> = MutexGuard<'a, RefCell<AppInMemFetchedData>>;

pub trait AbsDStoreFilterKeyOp : Send + Sync {
    fn filter(&self, k:&InnerKey, v:&InnerRow) -> bool;
}

#[async_trait]
pub trait AbstInMemoryDStore : Send + Sync
{
    fn new(cfg:&AppInMemoryDbCfg) -> Self where Self:Sized;
    async fn create_table (&self, label:&str) -> DefaultResult<(), AppError>;
    async fn delete(&self, _info:AppInMemDeleteInfo) -> DefaultResult<usize, AppError>;
    async fn save(&self, _data:AppInMemUpdateData) -> DefaultResult<usize, AppError>;
    async fn fetch(&self, _info:AppInMemFetchKeys) -> DefaultResult<AppInMemFetchedData, AppError>;
    async fn filter_keys(&self, tbl_label:InnerTableLabel, op:&dyn AbsDStoreFilterKeyOp)
        -> DefaultResult<Vec<InnerKey>, AppError>;
    // read-modify-write semantic, for atomic operation
    async fn fetch_acquire(&self, _info:AppInMemFetchKeys)
        -> DefaultResult<(AppInMemFetchedData, AppInMemDstoreLock), AppError>;
    fn save_release(&self, _data:AppInMemUpdateData, lock:AppInMemDstoreLock )
        -> DefaultResult<usize, AppError>;
}

// make it visible for testing purpose, this type could be limited in super module.
pub struct AppInMemoryDStore {
    max_items_per_table : u32,
    // TODO, replace with read/write lock
    table_map : Mutex<RefCell<AllTable>> 
}

impl AppInMemoryDStore {
    async fn try_get_table (&self) -> MutexGuard<RefCell<AllTable>>
    {
        self.table_map.lock().await
    }
    fn _check_capacity(&self, _map:&AllTable) -> DefaultResult<(), AppError>
    {
        let mut invalid = _map.iter().filter(
            |(_, table)| {self.max_items_per_table as usize <= table.len()}
        );
        if let Some((label, _)) =  invalid.next() {
            let msg = format!("{}, {}, {}", module_path!(), line!(), label);
            Err(AppError{detail:Some(msg.to_string()),
                    code:AppErrorCode::ExceedingMaxLimit })
        } else {
            Ok(())
        }
    }
    fn _check_table_existence (_map:&AllTable, keys:Vec<&InnerTableLabel>) -> DefaultResult<(), AppError>
    {
        let mut invalid = keys.iter().filter(
            |label| {!_map.contains_key(label.as_str())}
        );
        if let Some(d) =  invalid.next() {
            Err(AppError{detail:Some(d.to_string()),
                    code:AppErrorCode::DataTableNotExist })
        } else {
            Ok(())
        }
    }
    fn fetch_common(mut _map:RefMut<AllTable>, _info:AppInMemFetchKeys)
        -> DefaultResult<AppInMemFetchedData, AppError>
    {
        let unchecked_labels = _info.keys().collect::<Vec<&InnerTableLabel>>();
        Self::_check_table_existence(&*_map, unchecked_labels)?;
        let rs_a = _info.iter().map( |(label, ids)| {
            let table = _map.get_mut(label.as_str()).unwrap();
            let rs_t = ids.iter().filter(
                    |id| {table.contains_key(id.as_str())}
                ).map(
                    |id| {
                        let row = table.get(id).unwrap();
                        (id.clone(), row.clone())
                    }
                ).collect::<Vec<(InnerKey, InnerRow)>>();
            let rs_t = HashMap::from_iter(rs_t.into_iter());
            (label.clone(), rs_t)
        }).collect::<Vec<(InnerTableLabel, InnerTable)>>();
        let rs_a = HashMap::from_iter(rs_a.into_iter());
        Ok(rs_a)
    }
    fn save_common(&self, mut _map:RefMut<AllTable>, _data:AppInMemUpdateData)
        -> DefaultResult<usize, AppError>
    {
        let unchecked_labels = _data.keys().collect::<Vec<&InnerTableLabel>>();
        Self::_check_table_existence(&*_map, unchecked_labels)?;
        self._check_capacity(&*_map)?;
        let tot_cnt = _data.iter().map( |(label, d_grp)| {
            let table = _map.get_mut(label.as_str()).unwrap();
            d_grp.iter().map(|(id, row)| {
                table.insert(id.clone(), row.clone());
            }).count()
        }).sum() ;
        self._check_capacity(&*_map)?;
        Ok(tot_cnt)
    }
} // end of impl AppInMemoryDStore


#[async_trait]
impl AbstInMemoryDStore for AppInMemoryDStore {
    fn new(cfg:&AppInMemoryDbCfg) -> Self {
        let t_map = HashMap::new();
        let t_map = Mutex::new(RefCell::new(t_map));
        Self { table_map: t_map, max_items_per_table: cfg.max_items }
    }

    async fn create_table (&self, label:&str) -> DefaultResult<(), AppError>
    {
        let guard = self.try_get_table().await;
        let mut _map = guard.borrow_mut();
        if !_map.contains_key(label) {
            let newtable = HashMap::new();
            _map.insert(label.to_string(), newtable);
        }
        Ok(())
    }

    async fn delete(&self, _info:AppInMemDeleteInfo) -> DefaultResult<usize, AppError>
    {
        let guard = self.try_get_table().await;
        let mut _map = guard.borrow_mut();
        let unchecked_labels = _info.keys().collect::<Vec<&InnerTableLabel>>();
        Self::_check_table_existence(&*_map, unchecked_labels)?;
        let tot_cnt = _info.iter().map( |(label, ids)| {
            let table = _map.get_mut(label.as_str()).unwrap();
            ids.iter().map(|id| {table.remove(id);}).count()
        }).sum() ;
        Ok(tot_cnt)
    }

    async fn fetch(&self, _info:AppInMemFetchKeys) -> DefaultResult<AppInMemFetchedData, AppError>
    {
        let guard = self.try_get_table().await;
        Self::fetch_common(guard.borrow_mut(), _info)
    }
    async fn fetch_acquire(&self, _info:AppInMemFetchKeys)
        -> DefaultResult<(AppInMemFetchedData, AppInMemDstoreLock), AppError>
    {
        let guard = self.try_get_table().await;
        let rs_a = Self::fetch_common(guard.borrow_mut(), _info) ?;
        Ok((rs_a, guard))
    }

    async fn save(&self, _data:AppInMemUpdateData) -> DefaultResult<usize, AppError>
    {
        let guard = self.try_get_table().await;
        self.save_common(guard.borrow_mut(), _data)
    }
    fn save_release(&self, _data:AppInMemUpdateData, lock:AppInMemDstoreLock)
        -> DefaultResult<usize, AppError>
    {
        self.save_common(lock.borrow_mut(), _data)
    }

    async fn filter_keys(&self, tbl_label:InnerTableLabel, op:&dyn AbsDStoreFilterKeyOp)
        -> DefaultResult<Vec<InnerKey>, AppError>
    {
        let guard = self.try_get_table().await;
        let mut _map = guard.borrow_mut();
        let unchecked_labels = vec![&tbl_label];
        Self::_check_table_existence(&*_map, unchecked_labels)?;
        let table = _map.get(tbl_label.as_str()).unwrap();
        let out = table.iter().filter_map(|(k,v)| {
            if op.filter(k,v) {Some(k.clone())} else {None}
        }).collect();
        Ok(out)
    }
} // end of AppInMemoryDStore

