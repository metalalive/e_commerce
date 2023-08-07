use std::collections::HashMap;

use order::AppInMemoryDbCfg;
use order::datastore::{AppInMemoryDStore, AppInMemUpdateData, AppInMemFetchKeys, AppInMemDeleteInfo};
use order::error::AppErrorCode;

const UT_NUM_TABLES : usize = 3;
const UT_TABLE_LABEL_A : &'static str = "app-table-12";
const UT_TABLE_LABEL_B : &'static str = "app-table-34";
const UT_TABLE_LABEL_C : &'static str = "app-table-56";
const UT_TABLE_LABELS : [&'static str ; UT_NUM_TABLES] = [
    UT_TABLE_LABEL_A, UT_TABLE_LABEL_B, UT_TABLE_LABEL_C
];

#[test]
fn datastore_in_mem_save_ok_1 ()
{
    let cfg = AppInMemoryDbCfg { alias: "Sheipa".to_string(), max_items: 10 };
    let dstore = AppInMemoryDStore::new(&cfg);
    let all_created = UT_TABLE_LABELS.clone().into_iter().all(
        |label| {dstore.create_table(label).is_ok()}
    );
    assert_eq!(all_created, true);
    let new_data : AppInMemUpdateData = {
        let mut out = HashMap::new();
        let t1 = {
            let mut t = HashMap::new();
            let row = ["tee", "0.076", "1827", "r6p0"] .into_iter().map(String::from).collect();
            t.insert("G802".to_string(), row);
            let row = ["hie", "1.3689", "20", "r6p1"].into_iter().map(String::from).collect();
            t.insert("GIj0e".to_string(), row);
            t
        };
        let t2 = {
            let mut t = HashMap::new();
            let row = ["mie", "0.076", "llama"].into_iter().map(String::from).collect();
            t.insert("1800".to_string(), row);
            let row = ["man", "1.368", "alpaca"].into_iter().map(String::from).collect();
            t.insert("1680".to_string(), row);
            t
        };
        out.insert(UT_TABLE_LABEL_A.to_string(), t1);
        out.insert(UT_TABLE_LABEL_C.to_string(), t2);
        out
    };
    let result = dstore.save(new_data);
    assert_eq!(result.is_ok(), true);
    assert_eq!(result.unwrap(), 4);

    let fetching_keys : AppInMemFetchKeys = {
        let mut out = HashMap::new();
        let t1 = ["initDee", "GIj0e", "U8ry1g"].into_iter().map(String::from).collect();
        let t2 = ["93orwjtr", "9eujr"].into_iter().map(String::from).collect();
        let t3 = ["18o0", "1680", "1800"].into_iter().map(String::from).collect();
        out.insert(UT_TABLE_LABEL_A.to_string(), t1);
        out.insert(UT_TABLE_LABEL_B.to_string(), t2);
        out.insert(UT_TABLE_LABEL_C.to_string(), t3);
        out
    };
    let result = dstore.fetch(fetching_keys);
    assert_eq!(result.is_ok(), true);
    let actual_fetched = result.unwrap();
    {
        let a_table = actual_fetched.get(UT_TABLE_LABEL_A).unwrap();
        let actual_item = a_table.get("GIj0e").unwrap().iter()
            .map(String::as_str).collect::<Vec<&str>>();
        assert_eq!(actual_item, ["hie", "1.3689", "20", "r6p1"]); 
        assert_eq!(a_table.get("U8ry1g").is_none(), true);
        assert_eq!(a_table.get("initDee").is_none(), true);
    } {
        let a_table = actual_fetched.get(UT_TABLE_LABEL_B).unwrap();
        assert_eq!(a_table.get("9eujr").is_none(), true);
        assert_eq!(a_table.get("93orwjtr").is_none(), true);
    } {
        let a_table = actual_fetched.get(UT_TABLE_LABEL_C).unwrap();
        let actual_item = a_table.get("1680").unwrap().iter().map(String::as_str).collect::<Vec<&str>>();
        assert_eq!(actual_item, ["man", "1.368", "alpaca"]);
        let actual_item = a_table.get("1800").unwrap().iter().map(String::as_str).collect::<Vec<&str>>();
        assert_eq!(actual_item, ["mie", "0.076", "llama"]); 
        assert_eq!(a_table.get("18o0").is_none(), true);
    }
} // end of datastore_in_mem_save_ok_1


#[test]
fn datastore_in_mem_save_ok_2 ()
{
    let cfg = AppInMemoryDbCfg { alias: "Sheipa".to_string(), max_items: 10 };
    let dstore = AppInMemoryDStore::new(&cfg);
    assert_eq!(dstore.create_table(UT_TABLE_LABEL_A).is_ok(), true);
    let new_data : AppInMemUpdateData = {
        let mut out = HashMap::new();
        let t1 = {
            let mut t = HashMap::new();
            let row = ["tee", "0.076", "1827", "r6p0"] .into_iter().map(String::from).collect();
            t.insert("G802".to_string(), row);
            let row = ["sbitz", "0.01101001", "59", "r4p10"] .into_iter().map(String::from).collect();
            t.insert("yoLo".to_string(), row);
            t
        };
        out.insert(UT_TABLE_LABEL_A.to_string(), t1);
        out
    };
    let result = dstore.save(new_data);
    assert_eq!(result.is_ok(), true);
    assert_eq!(result.unwrap(), 2);
    let new_data : AppInMemUpdateData = {
        let mut out = HashMap::new();
        let t1 = {
            let mut t = HashMap::new();
            let row = ["shreding", "0.1011", "52", "r5p6"] .into_iter().map(String::from).collect();
            t.insert("G802".to_string(), row); // modify existing row
            let row = ["Hit", "1.816", "107", "r6p4"] .into_iter().map(String::from).collect();
            t.insert("G831".to_string(), row);
            t
        };
        out.insert(UT_TABLE_LABEL_A.to_string(), t1);
        out
    };
    let result = dstore.save(new_data);
    assert_eq!(result.is_ok(), true);
    assert_eq!(result.unwrap(), 2);

    let fetching_keys : AppInMemFetchKeys = {
        let mut out = HashMap::new();
        let t1 = ["yoLo", "G831", "G802"].into_iter().map(String::from).collect();
        out.insert(UT_TABLE_LABEL_A.to_string(), t1);
        out
    };
    let result = dstore.fetch(fetching_keys);
    assert_eq!(result.is_ok(), true);
    let actual_fetched = result.unwrap();
    if let Some(a_table) = actual_fetched.get(UT_TABLE_LABEL_A)
    {
        let actual_item = a_table.get("yoLo").unwrap().iter().map(String::as_str).collect::<Vec<&str>>();
        assert_eq!(actual_item, ["sbitz", "0.01101001", "59", "r4p10"]);
        let actual_item = a_table.get("G802").unwrap().iter().map(String::as_str).collect::<Vec<&str>>();
        assert_eq!(actual_item, ["shreding", "0.1011", "52", "r5p6"]);
        let actual_item = a_table.get("G831").unwrap().iter().map(String::as_str).collect::<Vec<&str>>();
        assert_eq!(actual_item, ["Hit", "1.816", "107", "r6p4"]);
    }
} // end of datastore_in_mem_save_ok_2


#[test]
fn datastore_in_mem_delete_ok ()
{
    let chosen_key = "Palau";
    let cfg = AppInMemoryDbCfg { alias: "Sheipa".to_string(), max_items: 10 };
    let dstore = AppInMemoryDStore::new(&cfg);
    assert_eq!(dstore.create_table(UT_TABLE_LABEL_A).is_ok(), true);
    let new_data : AppInMemUpdateData = {
        let mut out = HashMap::new();
        let t1 = {
            let mut t = HashMap::new();
            let row = ["tee", "0.076", "1827", "r6p0"] .into_iter().map(String::from).collect();
            t.insert("Fiji".to_string(), row);
            let row = ["sbitz", "0.01101001", "59", "r4p10"] .into_iter().map(String::from).collect();
            t.insert("Indonesia".to_string(), row);
            let row = ["shaw", "10.14", "122", "r4p6"] .into_iter().map(String::from).collect();
            t.insert(chosen_key.to_string(), row);
            t
        };
        out.insert(UT_TABLE_LABEL_A.to_string(), t1);
        out
    };
    let result = dstore.save(new_data);
    assert_eq!(result.is_ok(), true);
    assert_eq!(result.unwrap(), 3);
    let fetching_keys : AppInMemFetchKeys = {
        let mut out = HashMap::new();
        let t1 = [chosen_key].into_iter().map(String::from).collect();
        out.insert(UT_TABLE_LABEL_A.to_string(), t1);
        out
    };
    {
        let result = dstore.fetch(fetching_keys.clone());
        assert_eq!(result.is_ok(), true);
        let actual_fetched = result.unwrap();
        if let Some(a_table) = actual_fetched.get(UT_TABLE_LABEL_A)
        {
            let actual_item = a_table.get(chosen_key).unwrap().iter().map(String::as_str).collect::<Vec<&str>>();
            assert_eq!(actual_item, ["shaw", "10.14", "122", "r4p6"]);
        }
    }
    let deleting_keys : AppInMemDeleteInfo = fetching_keys.clone();
    let result = dstore.delete(deleting_keys);
    assert_eq!(result.is_ok(), true);
    assert_eq!(result.unwrap(), 1usize);
    {
        let result = dstore.fetch(fetching_keys);
        assert_eq!(result.is_ok(), true);
        let actual_fetched = result.unwrap();
        if let Some(a_table) = actual_fetched.get(UT_TABLE_LABEL_A)
        {
            assert_eq!(a_table.get(chosen_key).is_none(), true);
        }
    }
} // end of datastore_in_mem_delete_ok


#[test]
fn datastore_in_mem_access_nonexist_table ()
{
    let cfg = AppInMemoryDbCfg { alias: "Sheipa".to_string(), max_items: 10 };
    let dstore = AppInMemoryDStore::new(&cfg);
    let new_data : AppInMemUpdateData = {
        let mut out = HashMap::new();
        let t1 = {
            let mut t = HashMap::new();
            let row = ["tee", "0.076", "1827", "r6p0"] .into_iter().map(String::from).collect();
            t.insert("G802".to_string(), row);
            t
        };
        out.insert(UT_TABLE_LABEL_A.to_string(), t1);
        out
    };
    let result = dstore.save(new_data);
    assert_eq!(result.is_err(), true);
    let actual = result.err().unwrap();
    assert_eq!(actual.code , AppErrorCode::DataTableNotExist);
}


#[test]
fn datastore_in_mem_exceed_limit_error ()
{
    let cfg = AppInMemoryDbCfg { alias: "Sheipa".to_string(), max_items:5 };
    let dstore = AppInMemoryDStore::new(&cfg);
    assert_eq!(dstore.create_table(UT_TABLE_LABEL_A).is_ok(), true);
    let new_data : AppInMemUpdateData = {
        let mut out = HashMap::new();
        let t1 = {
            let mut t = HashMap::new();
            let row = ["tee", "0.076", "1827", "r6p0"] .into_iter().map(String::from).collect();
            t.insert("Taiwan".to_string(), row);
            let row = ["sbitz", "0.01101001", "59", "r4p10"] .into_iter().map(String::from).collect();
            t.insert("Phillipine".to_string(), row);
            let row = ["shaw", "10.14", "122", "r4p6"] .into_iter().map(String::from).collect();
            t.insert("Malaysia".to_string(), row);
            t
        };
        out.insert(UT_TABLE_LABEL_A.to_string(), t1);
        out
    };
    let result = dstore.save(new_data);
    assert_eq!(result.is_ok(), true);
    assert_eq!(result.unwrap(), 3);
    let new_data : AppInMemUpdateData = {
        let mut out = HashMap::new();
        let t1 = {
            let mut t = HashMap::new();
            let row = ["tee", "0.076", "1827", "r6p0"] .into_iter().map(String::from).collect();
            t.insert("sand-island".to_string(), row);
            let row = ["sbitz", "0.01101001", "59", "r4p10"] .into_iter().map(String::from).collect();
            t.insert("Ubek".to_string(), row);
            let row = ["shaw", "10.14", "122", "r4p6"] .into_iter().map(String::from).collect();
            t.insert("Gili".to_string(), row);
            t
        };
        out.insert(UT_TABLE_LABEL_A.to_string(), t1);
        out
    };
    let result = dstore.save(new_data);
    assert_eq!(result.is_err(), true);
    let actual = result.err().unwrap();
    assert_eq!(actual.code, AppErrorCode::ExceedingMaxLimit);
    assert_eq!(actual.detail.is_some(), true);
} // end of datastore_in_mem_exceed_limit_error

