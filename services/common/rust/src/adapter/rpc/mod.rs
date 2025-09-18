pub mod py_celery;

use std::collections::HashMap;
use std::fs::File;
use std::result::Result;

use serde::Serialize;
use serde_json::{Value as JsnVal, Map as JsnMap};

use crate::config::{AppBasepathCfg, AppRpcMockCfg};

type MockDataRoutes = HashMap<String, HashMap<String, Vec<JsnVal>>>;

pub struct MockDataSource {
    routes: MockDataRoutes,
}

impl MockDataSource {
    pub fn try_build(basepath: &AppBasepathCfg, cfg: &AppRpcMockCfg) -> Result<Self, String> {
        let fullpath = basepath.service.clone() + "/" + cfg.test_data.as_str();
        let file = File::open(fullpath).map_err(|e| e.to_string())?;
        let routes =
            serde_json::from_reader::<File, MockDataRoutes>(file).map_err(|e| e.to_string())?;
        Ok(Self { routes })
    }

    pub fn extract(&mut self, route_key: &str, usr_id: u32) -> Result<Vec<u8>, String> {
        let tdata = self
            .routes
            .get_mut(route_key)
            .ok_or("invalid-route".to_string())?
            .get_mut(usr_id.to_string().as_str())
            .ok_or("invalid-usr-id".to_string())?;
        if tdata.is_empty() {
            Err("empty-test-data".to_string())
        } else {
            let v = tdata.remove(0);
            Ok(v.to_string().into_bytes())
        }
    }
}

pub fn base_response<T:Serialize>(num_fields:usize, status:&str, result: Option<T>) -> Result<JsnVal, String> {
    let s = JsnVal::String(status.to_string());
    let r = if let Some(v) = result {
        serde_json::to_value(v).map_err(|e| e.to_string())? 
    } else {
        JsnVal::Null
    };
    let mut objmap = JsnMap::with_capacity(num_fields);
    objmap.insert("status".to_string(), s);
    objmap.insert("result".to_string(), r);
    Ok(JsnVal::Object(objmap))
}
