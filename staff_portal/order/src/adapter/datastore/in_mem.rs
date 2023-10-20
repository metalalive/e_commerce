use std::marker::{Sync, Send};
use std::result::Result as DefaultResult;
use std::collections::HashMap;
use std::cell::{RefCell, RefMut};
use std::sync::{Mutex, MutexGuard};
use std::vec;

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
pub type AppInMemFetchedData = AllTable;

pub trait AbsDStoreFilterKeyOp {
    fn filter(&self, k:&InnerKey) -> bool;
}

pub trait AbstInMemoryDStore : Send + Sync
{
    fn new(cfg:&AppInMemoryDbCfg) -> Self where Self:Sized;
    fn create_table (&self, label:&str) -> DefaultResult<(), AppError>;
    fn save(&self, _data:AppInMemUpdateData) -> DefaultResult<usize, AppError>;
    fn delete(&self, _info:AppInMemDeleteInfo) -> DefaultResult<usize, AppError>;
    fn fetch(&self, _info:AppInMemFetchKeys) -> DefaultResult<AppInMemFetchedData, AppError>;
    fn filter_keys(&self, tbl_label:InnerTableLabel, op:&dyn AbsDStoreFilterKeyOp)
        -> DefaultResult<Vec<InnerKey>, AppError>;
    // read-modify-write semantic, for atomic operation
    fn fetch_acquire(&self, _info:AppInMemFetchKeys)
        -> DefaultResult<(AppInMemFetchedData, MutexGuard<RefCell<AppInMemFetchedData>>), AppError>;
    fn save_release(&self, _data:AppInMemUpdateData, lock:MutexGuard<RefCell<AppInMemFetchedData>>)
        -> DefaultResult<usize, AppError>;
}

// make it visible for testing purpose, this type could be limited in super module.
pub struct AppInMemoryDStore {
    max_items_per_table : u32,
    // TODO, use read/write lock with async operation support
    table_map : Mutex<RefCell<AllTable>> 
}

impl AppInMemoryDStore {
    fn try_get_table (&self) -> DefaultResult<MutexGuard<RefCell<AllTable>> , AppError>
    {
        match self.table_map.lock() {
            Ok(guard) => Ok(guard),
            Err(e) => Err(AppError{detail:Some(e.to_string()),
                    code:AppErrorCode::AcquireLockFailure })
        }
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


impl AbstInMemoryDStore for AppInMemoryDStore {
    fn new(cfg:&AppInMemoryDbCfg) -> Self {
        let t_map = HashMap::new();
        let t_map = Mutex::new(RefCell::new(t_map));
        Self { table_map: t_map, max_items_per_table: cfg.max_items }
    }

    fn create_table (&self, label:&str) -> DefaultResult<(), AppError>
    {
        let guard = self.try_get_table()?;
        let mut _map = guard.borrow_mut();
        if !_map.contains_key(label) {
            let newtable = HashMap::new();
            _map.insert(label.to_string(), newtable);
        }
        Ok(())
    }

    fn delete(&self, _info:AppInMemDeleteInfo) -> DefaultResult<usize, AppError>
    {
        let guard = self.try_get_table()?;
        let mut _map = guard.borrow_mut();
        let unchecked_labels = _info.keys().collect::<Vec<&InnerTableLabel>>();
        Self::_check_table_existence(&*_map, unchecked_labels)?;
        let tot_cnt = _info.iter().map( |(label, ids)| {
            let table = _map.get_mut(label.as_str()).unwrap();
            ids.iter().map(|id| {table.remove(id);}).count()
        }).sum() ;
        Ok(tot_cnt)
    }

    fn fetch(&self, _info:AppInMemFetchKeys) -> DefaultResult<AppInMemFetchedData, AppError>
    {
        let guard = self.try_get_table()?;
        Self::fetch_common(guard.borrow_mut(), _info)
    }
    fn fetch_acquire(&self, _info:AppInMemFetchKeys)
        -> DefaultResult<(AppInMemFetchedData, MutexGuard<RefCell<AppInMemFetchedData>>), AppError>
    {
        let guard = self.try_get_table()?;
        let rs_a = Self::fetch_common(guard.borrow_mut(), _info) ?;
        Ok((rs_a, guard))
    }

    fn save(&self, _data:AppInMemUpdateData) -> DefaultResult<usize, AppError>
    {
        let guard = self.try_get_table()?;
        self.save_common(guard.borrow_mut(), _data)
    }
    fn save_release(&self, _data:AppInMemUpdateData, lock:MutexGuard<RefCell<AppInMemFetchedData>>)
        -> DefaultResult<usize, AppError>
    {
        self.save_common(lock.borrow_mut(), _data)
    }

    fn filter_keys(&self, tbl_label:InnerTableLabel, op:&dyn AbsDStoreFilterKeyOp)
        -> DefaultResult<Vec<InnerKey>, AppError>
    {
        let guard = self.try_get_table()?;
        let mut _map = guard.borrow_mut();
        let unchecked_labels = vec![&tbl_label];
        Self::_check_table_existence(&*_map, unchecked_labels)?;
        let table = _map.get(tbl_label.as_str()).unwrap();
        let out = table.keys().filter_map(|k| {
            if op.filter(k) {Some(k.clone())} else {None}
        }).collect();
        Ok(out)
    }
} // end of AppInMemoryDStore

