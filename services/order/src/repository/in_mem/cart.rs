use std::boxed::Box;
use std::sync::Arc;
use std::result::Result as DefaultResult;

use async_trait::async_trait;

use crate::datastore::AbstInMemoryDStore;
use crate::error::AppError;
use crate::model::{CartModel, BaseProductIdentity};
use crate::repository::AbsCartRepo;

pub struct CartInMemRepo {
    dstore: Arc<Box<dyn AbstInMemoryDStore>>,
}

#[async_trait]
impl AbsCartRepo for CartInMemRepo
{    
    async fn update(&self, obj: CartModel) -> DefaultResult<usize, AppError>
    {
        Ok(0)
    }
    async fn discard(&self, owner: u32, seq:u8) -> DefaultResult<(), AppError>
    {
        Ok(())
    } 
    async fn num_lines_saved(&self, owner: u32, seq:u8) -> DefaultResult<usize, AppError>
    { Ok(0) }
    
    async fn fetch_cart(&self, owner: u32, seq:u8) -> DefaultResult<CartModel, AppError>
    {
        let empty = CartModel { owner, seq_num: seq, title: "gift".to_string(),
            saved_lines: vec![], new_lines:vec![] };
        Ok(empty)
    }
    
    async fn fetch_lines_by_pid(&self, owner: u32, seq:u8, pids:Vec<BaseProductIdentity>)
        -> DefaultResult<CartModel, AppError>
    {
        let empty = CartModel { owner, seq_num: seq, title: "gift".to_string(),
            saved_lines: vec![], new_lines:vec![] };
        Ok(empty)
    }
} // end of impl AbsCartRepo for CartInMemRepo

impl CartInMemRepo
{
    pub fn new(ds: Arc<Box<dyn AbstInMemoryDStore>>) -> Self
    { Self { dstore: ds } }
} // end of impl CartInMemRepo
