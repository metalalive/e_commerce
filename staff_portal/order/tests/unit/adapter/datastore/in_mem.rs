use std::collections::{HashMap, HashSet};
use std::collections::hash_map::RandomState;

use order::AppInMemoryDbCfg;
use order::datastore::{
    AbstInMemoryDStore, AppInMemoryDStore, AppInMemUpdateData,
    AppInMemFetchKeys, AppInMemDeleteInfo, AbsDStoreFilterKeyOp
};
use order::error::AppErrorCode;

const UT_NUM_TABLES : usize = 3;
const UT_TABLE_LABEL_A : &'static str = "app-table-12";
const UT_TABLE_LABEL_B : &'static str = "app-table-34";
const UT_TABLE_LABEL_C : &'static str = "app-table-56";
const UT_TABLE_LABELS : [&'static str ; UT_NUM_TABLES] = [
    UT_TABLE_LABEL_A, UT_TABLE_LABEL_B, UT_TABLE_LABEL_C
];

#[tokio::test]
async fn save_ok_1 ()
{
    let cfg = AppInMemoryDbCfg { alias: "Sheipa".to_string(), max_items: 10 };
    let dstore = AppInMemoryDStore::new(&cfg);
    for label in UT_TABLE_LABELS.clone().into_iter() {
        let result = dstore.create_table(label).await;
        assert!(result.is_ok());
    }
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
    let result = dstore.save(new_data).await;
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
    let result = dstore.fetch(fetching_keys).await;
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
} // end of save_ok_1


#[tokio::test]
async fn save_ok_2 ()
{
    let cfg = AppInMemoryDbCfg { alias: "Sheipa".to_string(), max_items: 10 };
    let dstore = AppInMemoryDStore::new(&cfg);
    assert_eq!(dstore.create_table(UT_TABLE_LABEL_A).await.is_ok(), true);
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
    let result = dstore.save(new_data).await;
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
    let result = dstore.save(new_data).await;
    assert_eq!(result.is_ok(), true);
    assert_eq!(result.unwrap(), 2);

    let fetching_keys : AppInMemFetchKeys = {
        let mut out = HashMap::new();
        let t1 = ["yoLo", "G831", "G802"].into_iter().map(String::from).collect();
        out.insert(UT_TABLE_LABEL_A.to_string(), t1);
        out
    };
    let result = dstore.fetch(fetching_keys).await;
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
} // end of save_ok_2


#[tokio::test]
async fn fetch_acquire_save_release_ok ()
{
    let cfg = AppInMemoryDbCfg { alias: "Sheipa".to_string(), max_items: 10 };
    let dstore = AppInMemoryDStore::new(&cfg);
    assert_eq!(dstore.create_table(UT_TABLE_LABEL_A).await.is_ok(), true);
    let new_data : AppInMemUpdateData = {
        let mut out = HashMap::new();
        let t1 = {
            let mut t = HashMap::new();
            let row = ["tee", "0.076", "1827", "r6p0"] .into_iter().map(String::from).collect();
            t.insert("G802".to_string(), row);
            let row = ["sbitz", "0.01101001", "59", "r4p10"] .into_iter().map(String::from).collect();
            t.insert("yoLo".to_string(), row);
            let row = ["kay", "1.5883", "1007", "r6p1"] .into_iter().map(String::from).collect();
            t.insert("Alie".to_string(), row);
            t
        };
        out.insert(UT_TABLE_LABEL_A.to_string(), t1);
        out
    };
    let result = dstore.save(new_data).await;
    assert_eq!(result.is_ok(), true);
    assert_eq!(result.unwrap(), 3);

    let fetching_keys : AppInMemFetchKeys = {
        let mut out = HashMap::new();
        let t1 = ["Aaron", "yoLo", "G831", "G802"].into_iter().map(String::from).collect();
        out.insert(UT_TABLE_LABEL_A.to_string(), t1);
        out
    };
    let result = dstore.fetch_acquire(fetching_keys).await;
    assert_eq!(result.is_ok(), true);
    let (mut actual_fetched, actual_lock) = result.unwrap();
    if let Some(a_table) = actual_fetched.get_mut(UT_TABLE_LABEL_A)
    {
        let actual_item = a_table.get("yoLo").unwrap().iter().map(String::as_str).collect::<Vec<&str>>();
        assert_eq!(actual_item, ["sbitz", "0.01101001", "59", "r4p10"]);
        let actual_item = a_table.get("G802").unwrap().iter().map(String::as_str).collect::<Vec<&str>>();
        assert_eq!(actual_item, ["tee", "0.076", "1827", "r6p0"]);
        let data_edit = a_table.get_mut("yoLo").unwrap();
        data_edit.remove(0);
        data_edit.insert(0, "have-eaten-yet".to_string());
    }
    let result = dstore.save_release(actual_fetched, actual_lock);
    assert_eq!(result.is_ok(), true);
    assert_eq!(result.unwrap(), 2);
    
    let fetching_keys : AppInMemFetchKeys = {
        let mut out = HashMap::new();
        let t1 = vec!["yoLo".to_string()];
        out.insert(UT_TABLE_LABEL_A.to_string(), t1);
        out
    };
    let result = dstore.fetch(fetching_keys).await;
    assert_eq!(result.is_ok(), true);
    let actual_fetched = result.unwrap();
    if let Some(a_table) = actual_fetched.get(UT_TABLE_LABEL_A)
    {
        let actual_item = a_table.get("yoLo").unwrap().iter().map(String::as_str).collect::<Vec<&str>>();
        assert_eq!(actual_item, ["have-eaten-yet", "0.01101001", "59", "r4p10"]);
    }
} // end of fetch_acquire_save_release_ok


#[tokio::test]
async fn delete_ok ()
{
    let chosen_key = "Palau";
    let cfg = AppInMemoryDbCfg { alias: "Sheipa".to_string(), max_items: 10 };
    let dstore = AppInMemoryDStore::new(&cfg);
    assert_eq!(dstore.create_table(UT_TABLE_LABEL_A).await.is_ok(), true);
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
    let result = dstore.save(new_data).await;
    assert_eq!(result.is_ok(), true);
    assert_eq!(result.unwrap(), 3);
    let fetching_keys : AppInMemFetchKeys = {
        let mut out = HashMap::new();
        let t1 = [chosen_key].into_iter().map(String::from).collect();
        out.insert(UT_TABLE_LABEL_A.to_string(), t1);
        out
    };
    {
        let result = dstore.fetch(fetching_keys.clone()).await;
        assert_eq!(result.is_ok(), true);
        let actual_fetched = result.unwrap();
        if let Some(a_table) = actual_fetched.get(UT_TABLE_LABEL_A)
        {
            let actual_item = a_table.get(chosen_key).unwrap().iter().map(String::as_str).collect::<Vec<&str>>();
            assert_eq!(actual_item, ["shaw", "10.14", "122", "r4p6"]);
        }
    }
    let deleting_keys : AppInMemDeleteInfo = fetching_keys.clone();
    let result = dstore.delete(deleting_keys).await;
    assert_eq!(result.is_ok(), true);
    assert_eq!(result.unwrap(), 1usize);
    {
        let result = dstore.fetch(fetching_keys).await;
        assert_eq!(result.is_ok(), true);
        let actual_fetched = result.unwrap();
        if let Some(a_table) = actual_fetched.get(UT_TABLE_LABEL_A)
        {
            assert_eq!(a_table.get(chosen_key).is_none(), true);
        }
    }
} // end of delete_ok


#[tokio::test]
async fn access_nonexist_table ()
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
    let result = dstore.save(new_data).await;
    assert_eq!(result.is_err(), true);
    let actual = result.err().unwrap();
    assert_eq!(actual.code , AppErrorCode::DataTableNotExist);
}


#[tokio::test]
async fn exceed_limit_error ()
{
    let cfg = AppInMemoryDbCfg { alias: "Sheipa".to_string(), max_items:5 };
    let dstore = AppInMemoryDStore::new(&cfg);
    assert_eq!(dstore.create_table(UT_TABLE_LABEL_A).await.is_ok(), true);
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
    let result = dstore.save(new_data).await;
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
    let result = dstore.save(new_data).await;
    assert_eq!(result.is_err(), true);
    let actual = result.err().unwrap();
    assert_eq!(actual.code, AppErrorCode::ExceedingMaxLimit);
    assert_eq!(actual.detail.is_some(), true);
} // end of exceed_limit_error


struct UtestDstoreFiltKeyOp {patt:String}

impl AbsDStoreFilterKeyOp for UtestDstoreFiltKeyOp
{
    fn filter(&self, k:&String, _v:&Vec<String>) -> bool {
        k.contains(self.patt.as_str())
    }
}


#[tokio::test]
async fn filter_key_ok ()
{
    let cfg = AppInMemoryDbCfg { alias: "Alishan".to_string(), max_items:8 };
    let dstore = AppInMemoryDStore::new(&cfg);
    assert_eq!(dstore.create_table(UT_TABLE_LABEL_A).await.is_ok(), true);
    let search_id = "hemu";
    let init_data:[Vec<String>;4] = [
        ["teehe", "0.076", "1827", "r6p0"] .into_iter().map(String::from).collect(),
        ["shaw", "10.14", "122", "r4p6"] .into_iter().map(String::from).collect(),
        ["sbitz", "0.01101001", "59", "r4p10"] .into_iter().map(String::from).collect(),
        ["tito", "0.01101001", "59", "watching"] .into_iter().map(String::from).collect(),
    ];
    let new_data : AppInMemUpdateData = {
        let mut out = HashMap::new();
        let t1 = {
            let data = [
                (format!("{search_id}-bisa"), init_data[0].clone()),
                (format!("elf-schden"), init_data[1].clone()),
                (format!("gopher-neihts"), init_data[2].clone()),
                (format!("ferris-{search_id}"), init_data[3].clone()),
            ];
            HashMap::from_iter(data.into_iter())
        };
        out.insert(UT_TABLE_LABEL_A.to_string(), t1);
        out
    };
    let result = dstore.save(new_data).await;
    assert_eq!(result.is_ok(), true);
    assert_eq!(result.unwrap(), 4);
    let op = UtestDstoreFiltKeyOp{patt:search_id.to_string()};
    let result = dstore.filter_keys(UT_TABLE_LABEL_A.to_string(), &op).await;
    assert_eq!(result.is_ok(), true);
    let actual_keys = result.unwrap();
    let expect_keys = vec![format!("{search_id}-bisa"), format!("ferris-{search_id}")];
    let actual_keys:HashSet<String,RandomState> = HashSet::from_iter(actual_keys.into_iter());
    let expect_keys:HashSet<String,RandomState> = HashSet::from_iter(expect_keys.into_iter());
    //let diff = actual_keys.difference(&expect_keys);
    assert_eq!(actual_keys, expect_keys);
    assert_eq!(actual_keys.contains("gopher-neihts"), false);
} // end of filter_key_ok

