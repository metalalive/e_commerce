use std::boxed::Box;
use std::collections::HashMap;
use std::result::Result as DefaultResult;
use std::sync::Arc;

use async_trait::async_trait;

use crate::datastore::{
    AbsDStoreFilterKeyOp, AbstInMemoryDStore, AppInMemFetchKeys, AppInMemFetchedSingleRow,
    AppInMemFetchedSingleTable,
};
use crate::error::AppError;
use crate::model::{BaseProductIdentity, CartLineModel, CartModel};
use crate::repository::AbsCartRepo;

#[allow(non_snake_case)]
mod CartTable {
    use super::{AppInMemFetchedSingleRow, AppInMemFetchedSingleTable, CartModel, HashMap};
    pub(super) const LABEL: &'static str = "cart_metadata";
    pub(super) struct UpdateArg<'a>(pub(super) &'a CartModel);

    fn pkey(usr_id: u32, seq: u8) -> String {
        format!("{usr_id}-{seq}")
    }

    impl Into<AppInMemFetchedSingleRow> for UpdateArg<'_> {
        fn into(self) -> AppInMemFetchedSingleRow {
            let obj = self.0;
            vec![obj.title.clone()]
        }
    }
    impl Into<AppInMemFetchedSingleTable> for UpdateArg<'_> {
        fn into(self) -> AppInMemFetchedSingleTable {
            let key = pkey(self.0.owner, self.0.seq_num);
            let value = self.into();
            HashMap::from([(key, value)])
        }
    }
} // end of inner-mod CartTable

#[allow(non_snake_case)]
mod CartLineTable {
    use super::{AppInMemFetchedSingleTable, BaseProductIdentity, CartModel, HashMap};
    pub(super) const LABEL: &'static str = "cart_line";
    pub(super) struct UpdateArg(pub(super) CartModel);

    fn pkey(usr_id: u32, seq: u8, id_: BaseProductIdentity) -> String {
        let prod_typ_num: u8 = id_.product_type.into();
        format!(
            "{}-{}-{}-{}-{}",
            usr_id, seq, id_.store_id, prod_typ_num, id_.product_id
        )
    }

    impl Into<AppInMemFetchedSingleTable> for UpdateArg {
        fn into(self) -> AppInMemFetchedSingleTable {
            let (usr_id, seq, mut saved_lines, new_lines) = (
                self.0.owner,
                self.0.seq_num,
                self.0.saved_lines,
                self.0.new_lines,
            );
            saved_lines.extend(new_lines.into_iter());
            let iter0 = saved_lines.into_iter().map(|line| {
                let (id_, qty) = (line.id_, line.qty_req);
                let key = pkey(usr_id, seq, id_);
                let row = vec![qty.to_string()];
                (key, row)
            });
            HashMap::from_iter(iter0)
        }
    }
} // end of inner-mod CartLineTable

struct InnerFilterKeyOp {
    usr_id: u32,
    seq_num: u8,
    pids: Option<Vec<BaseProductIdentity>>,
}
impl AbsDStoreFilterKeyOp for InnerFilterKeyOp {
    fn filter(&self, k: &String, _v: &Vec<String>) -> bool {
        let mut tokens = k.split("-");
        let (curr_usr, curr_seq_num) = (
            tokens.next().unwrap().parse::<u32>().unwrap(),
            tokens.next().unwrap().parse::<u8>().unwrap(),
        );
        let mut cond = curr_usr == self.usr_id && curr_seq_num == self.seq_num;
        if let Some(prod_ids) = self.pids.as_ref() {
            let (store_id, product_type, product_id) = (
                tokens.next().unwrap().parse().unwrap(),
                tokens.next().unwrap().parse().unwrap(),
                tokens.next().unwrap().parse().unwrap(),
            );
            let curr_p_id = BaseProductIdentity {
                store_id,
                product_type,
                product_id,
            };
            let extra = prod_ids.contains(&curr_p_id);
            cond = cond && extra
        }
        cond
    } // end of fn filter
} // end of impl InnerFilterKeyOp

impl TryFrom<(String, Vec<String>)> for CartLineModel {
    type Error = AppError;
    fn try_from(value: (String, Vec<String>)) -> DefaultResult<Self, Self::Error> {
        let (key, mut row) = (value.0, value.1);
        let mut tokens = key.split("-");
        // skip first two token, user-id and seq-num
        let _usr_id = tokens.next();
        let _seq_num = tokens.next();
        let (store_id, product_type, product_id) = (
            tokens.next().unwrap().parse().unwrap(),
            tokens.next().unwrap().parse().unwrap(),
            tokens.next().unwrap().parse().unwrap(),
        );
        let qty_req = row.remove(0).parse().unwrap();
        let out = CartLineModel {
            id_: BaseProductIdentity {
                store_id,
                product_type,
                product_id,
            },
            qty_req,
        };
        Ok(out)
    }
}

impl From<(String, Vec<String>, Vec<CartLineModel>)> for CartModel {
    fn from(value: (String, Vec<String>, Vec<CartLineModel>)) -> Self {
        let (key, mut row, saved_lines) = (value.0, value.1, value.2);
        let mut tokens = key.split("-");
        let (owner, seq_num) = (
            tokens.next().unwrap().parse().unwrap(),
            tokens.next().unwrap().parse().unwrap(),
        );
        CartModel {
            owner,
            seq_num,
            title: row.remove(0),
            saved_lines,
            new_lines: Vec::new(),
        }
    }
}

pub struct CartInMemRepo {
    datastore: Arc<Box<dyn AbstInMemoryDStore>>,
}

#[async_trait]
impl AbsCartRepo for CartInMemRepo {
    async fn update(&self, obj: CartModel) -> DefaultResult<usize, AppError> {
        let rows0 = CartTable::UpdateArg(&obj).into();
        let rows1 = CartLineTable::UpdateArg(obj).into();
        let data = HashMap::from([
            (CartTable::LABEL.to_string(), rows0),
            (CartLineTable::LABEL.to_string(), rows1),
        ]);
        let num_saved = self.datastore.save(data).await?;
        Ok(num_saved)
    }
    async fn discard(&self, owner: u32, seq: u8) -> DefaultResult<(), AppError> {
        let info = self.filter_keys(owner, seq, None).await?;
        let _num_affected = self.datastore.delete(info).await?;
        Ok(())
    }
    async fn num_lines_saved(&self, owner: u32, seq: u8) -> DefaultResult<usize, AppError> {
        let info = self.filter_keys(owner, seq, None).await?;
        let mut result = self.datastore.fetch(info).await?;
        let rowset = result.remove(CartLineTable::LABEL).unwrap();
        Ok(rowset.len())
    }

    async fn fetch_cart(&self, owner: u32, seq: u8) -> DefaultResult<CartModel, AppError> {
        let info = self.filter_keys(owner, seq, None).await?;
        self.fetch_common(owner, seq, info).await
    }

    async fn fetch_lines_by_pid(
        &self,
        owner: u32,
        seq: u8,
        pids: Vec<BaseProductIdentity>,
    ) -> DefaultResult<CartModel, AppError> {
        let info = self.filter_keys(owner, seq, Some(pids)).await?;
        self.fetch_common(owner, seq, info).await
    }
} // end of impl AbsCartRepo for CartInMemRepo

impl CartInMemRepo {
    pub async fn new(m: Arc<Box<dyn AbstInMemoryDStore>>) -> DefaultResult<Self, AppError> {
        m.create_table(CartTable::LABEL).await?;
        m.create_table(CartLineTable::LABEL).await?;
        Ok(Self { datastore: m })
    }

    async fn filter_keys(
        &self,
        usr_id: u32,
        seq_num: u8,
        pids: Option<Vec<BaseProductIdentity>>,
    ) -> DefaultResult<AppInMemFetchKeys, AppError> {
        let mut op = InnerFilterKeyOp {
            usr_id,
            seq_num,
            pids,
        };
        let mut key_set = Vec::new();

        let tbl_name = CartLineTable::LABEL.to_string();
        let keys = self.datastore.filter_keys(tbl_name.clone(), &op).await?;
        key_set.push((tbl_name, keys));

        op.pids = None;
        let tbl_name = CartTable::LABEL.to_string();
        let keys = self.datastore.filter_keys(tbl_name.clone(), &op).await?;
        key_set.push((tbl_name, keys));
        Ok(HashMap::from_iter(key_set.into_iter()))
    }

    async fn fetch_common(
        &self,
        owner: u32,
        seq: u8,
        keys: AppInMemFetchKeys,
    ) -> DefaultResult<CartModel, AppError> {
        let mut result = self.datastore.fetch(keys).await?;
        let (rows_toplvl, rows_lines) = (
            result.remove(CartTable::LABEL).unwrap(),
            result.remove(CartLineTable::LABEL).unwrap(),
        );
        let mut errors = Vec::new();
        let m_lines = rows_lines
            .into_iter()
            .filter_map(|(k, v)| match CartLineModel::try_from((k, v)) {
                Ok(m) => Some(m),
                Err(e) => {
                    errors.push(e);
                    None
                }
            })
            .collect::<Vec<_>>();
        if errors.is_empty() {
            let obj = if let Some((k, v)) = rows_toplvl.into_iter().next() {
                CartModel::from((k, v, m_lines))
            } else {
                CartModel {
                    owner,
                    seq_num: seq,
                    title: "Untitled".to_string(),
                    new_lines: Vec::new(),
                    saved_lines: Vec::new(),
                }
            };
            Ok(obj)
        } else {
            Err(errors.remove(0))
        }
    }
} // end of impl CartInMemRepo
