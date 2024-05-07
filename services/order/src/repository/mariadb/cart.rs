use std::result::Result as DefaultResult;
use std::sync::Arc;

use async_trait::async_trait;
use sqlx::mysql::{MySqlArguments, MySqlRow};
use sqlx::{Acquire, Arguments, Executor, IntoArguments, MySql, Row, Statement};

use ecommerce_common::constant::ProductType;
use ecommerce_common::error::AppErrorCode;

use crate::datastore::AppMariaDbStore;
use crate::error::AppError;
use crate::model::{BaseProductIdentity, CartLineModel, CartModel};
use crate::repository::AbsCartRepo;

use super::run_query_once;

struct InsertUpdateTopLvlArg<'a>(&'a CartModel);
struct InsertLineArg(u32, u8, Vec<CartLineModel>);
struct UpdateLineArg(u32, u8, Vec<CartLineModel>);
struct DiscardLineArg(u32, u8);
struct DiscardTopLvlArg(u32, u8);

struct FetchTotNumLinesArg(u32, u8);
struct FetchTopLvlArg(u32, u8);
struct FetchLinesArg(u32, u8, Option<Vec<BaseProductIdentity>>);

impl<'a> From<InsertUpdateTopLvlArg<'a>> for (String, MySqlArguments) {
    fn from(value: InsertUpdateTopLvlArg<'a>) -> (String, MySqlArguments) {
        let sql_patt = "INSERT INTO `cart_toplvl_meta`(`usr_id`,`seq`,`title`) VALUES (?,?,?) \
                        ON DUPLICATE KEY UPDATE `title`=?";
        let mut args = MySqlArguments::default();
        args.add(value.0.owner);
        args.add(value.0.seq_num);
        args.add(value.0.title.clone());
        args.add(value.0.title.clone());
        (sql_patt.to_string(), args)
    }
}

impl InsertLineArg {
    fn sql_pattern(num_batch: usize) -> String {
        let col_seq = (0..num_batch)
            .map(|_| "(?,?,?,?,?,?)")
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "INSERT INTO `cart_line_detail`(`usr_id`,`seq`,`store_id`,`product_type`,\
            `product_id`,`quantity`) VALUES {col_seq}"
        )
    }
}
impl<'q> IntoArguments<'q, MySql> for InsertLineArg {
    fn into_arguments(self) -> <MySql as sqlx::database::HasArguments<'q>>::Arguments {
        let mut args = MySqlArguments::default();
        let (usr_id, seq_num, lines) = (self.0, self.1, self.2);
        lines
            .into_iter()
            .map(|line| {
                let (id_, quantity) = (line.id_, line.qty_req);
                let BaseProductIdentity {
                    store_id,
                    product_type,
                    product_id,
                } = id_;
                let prod_typ_num: u8 = product_type.into();
                args.add(usr_id);
                args.add(seq_num);
                args.add(store_id);
                args.add(prod_typ_num.to_string());
                args.add(product_id);
                args.add(quantity);
            })
            .count();
        args
    }
}
impl From<InsertLineArg> for (String, MySqlArguments) {
    fn from(value: InsertLineArg) -> (String, MySqlArguments) {
        (
            InsertLineArg::sql_pattern(value.2.len()),
            value.into_arguments(),
        )
    }
}

impl UpdateLineArg {
    fn sql_pattern(num_batch: usize) -> String {
        let case_op = (0..num_batch)
            .map(|_| "WHEN (`store_id`=? AND `product_type`=? AND `product_id`=?) THEN ? ")
            .collect::<Vec<_>>()
            .join("");
        let where_op = (0..num_batch)
            .map(|_| "(`store_id`=? AND `product_type`=? AND `product_id`=?)")
            .collect::<Vec<_>>()
            .join("OR");
        // `usr_id`,`seq`,`store_id`,`product_type`,`product_id`
        format!(
            "UPDATE `cart_line_detail` SET \
                `quantity` = CASE {case_op} ELSE `quantity` END \
                WHERE `usr_id`=? AND `seq`=?  AND ({where_op})"
        )
    }
}
impl<'a> IntoArguments<'a, MySql> for UpdateLineArg {
    fn into_arguments(self) -> <MySql as sqlx::database::HasArguments<'a>>::Arguments {
        let mut args = MySqlArguments::default();
        let (usr_id, seq, lines) = (self.0, self.1, self.2);
        lines
            .iter()
            .map(|line| {
                let prod_typ_num: u8 = line.id_.product_type.clone().into();
                let (seller, p_id, qty) = (line.id_.store_id, line.id_.product_id, line.qty_req);
                args.add(seller);
                args.add(prod_typ_num.to_string());
                args.add(p_id);
                args.add(qty);
            })
            .count();
        args.add(usr_id);
        args.add(seq);
        lines
            .into_iter()
            .map(|line| {
                let prod_typ_num: u8 = line.id_.product_type.clone().into();
                let (seller, p_id) = (line.id_.store_id, line.id_.product_id);
                args.add(seller);
                args.add(prod_typ_num.to_string());
                args.add(p_id);
            })
            .count();
        args
    }
}
impl From<UpdateLineArg> for (String, MySqlArguments) {
    fn from(value: UpdateLineArg) -> (String, MySqlArguments) {
        (
            UpdateLineArg::sql_pattern(value.2.len()),
            value.into_arguments(),
        )
    }
}

impl From<DiscardTopLvlArg> for (String, MySqlArguments) {
    fn from(value: DiscardTopLvlArg) -> (String, MySqlArguments) {
        let (usr_id, seq_num) = (value.0, value.1);
        let sql_patt = "DELETE FROM `cart_toplvl_meta` WHERE `usr_id`=? AND `seq`=?";
        let mut args = MySqlArguments::default();
        args.add(usr_id);
        args.add(seq_num);
        (sql_patt.to_string(), args)
    }
}

impl From<DiscardLineArg> for (String, MySqlArguments) {
    fn from(value: DiscardLineArg) -> (String, MySqlArguments) {
        let (usr_id, seq_num) = (value.0, value.1);
        let sql_patt = "DELETE FROM `cart_line_detail` WHERE `usr_id`=? AND `seq`=?";
        let mut args = MySqlArguments::default();
        args.add(usr_id);
        args.add(seq_num);
        (sql_patt.to_string(), args)
    }
}

impl From<FetchTotNumLinesArg> for (String, MySqlArguments) {
    fn from(value: FetchTotNumLinesArg) -> (String, MySqlArguments) {
        let (usr_id, seq_num) = (value.0, value.1);
        let sql_patt = "SELECT COUNT(*) FROM `cart_line_detail` WHERE `usr_id`=? AND `seq`=?";
        let mut args = MySqlArguments::default();
        args.add(usr_id);
        args.add(seq_num);
        (sql_patt.to_string(), args)
    }
}

impl From<FetchTopLvlArg> for (String, MySqlArguments) {
    fn from(value: FetchTopLvlArg) -> (String, MySqlArguments) {
        let (usr_id, seq_num) = (value.0, value.1);
        let sql_patt = "SELECT `usr_id`,`seq`,`title` FROM `cart_toplvl_meta` \
                        WHERE `usr_id`=? AND `seq`=?";
        let mut args = MySqlArguments::default();
        args.add(usr_id);
        args.add(seq_num);
        (sql_patt.to_string(), args)
    }
}

impl FetchLinesArg {
    fn sql_pattern(num_batch: usize) -> String {
        let mut sql_patt = "SELECT `store_id`,`product_type`,`product_id`,`quantity`\
                        FROM `cart_line_detail` WHERE `usr_id`=? AND `seq`=?"
            .to_string();
        if num_batch > 0 {
            let where_op = (0..num_batch)
                .map(|_| "(`store_id`=? AND `product_type`=? AND `product_id`=?)")
                .collect::<Vec<_>>()
                .join("OR");
            let extra = format!(" AND ({where_op})");
            sql_patt += extra.as_str();
        }
        sql_patt
    }
}
impl<'a> IntoArguments<'a, MySql> for FetchLinesArg {
    fn into_arguments(self) -> <MySql as sqlx::database::HasArguments<'a>>::Arguments {
        let (usr_id, seq_num, opt_pids) = (self.0, self.1, self.2);
        let mut args = MySqlArguments::default();
        args.add(usr_id);
        args.add(seq_num);
        if let Some(pids) = opt_pids {
            pids.into_iter()
                .map(|id_| {
                    let prod_typ_num: u8 = id_.product_type.into();
                    let (seller, p_id) = (id_.store_id, id_.product_id);
                    args.add(seller);
                    args.add(prod_typ_num.to_string());
                    args.add(p_id);
                })
                .count();
        }
        args
    }
}
impl From<FetchLinesArg> for (String, MySqlArguments) {
    fn from(value: FetchLinesArg) -> (String, MySqlArguments) {
        let num_batch = if let Some(v) = value.2.as_ref() {
            v.len()
        } else {
            0usize
        };
        (
            FetchLinesArg::sql_pattern(num_batch),
            value.into_arguments(),
        )
    }
}

impl TryFrom<MySqlRow> for CartModel {
    type Error = AppError;
    fn try_from(row: MySqlRow) -> DefaultResult<Self, Self::Error> {
        let owner = row.try_get::<u32, usize>(0)?;
        let seq_num = row.try_get::<u8, usize>(1)?;
        let title = row.try_get::<String, usize>(2)?;
        Ok(Self {
            owner,
            seq_num,
            title,
            saved_lines: Vec::new(),
            new_lines: Vec::new(),
        })
    }
}
impl TryFrom<MySqlRow> for CartLineModel {
    type Error = AppError;
    fn try_from(row: MySqlRow) -> DefaultResult<Self, Self::Error> {
        let store_id = row.try_get::<u32, usize>(0)?;
        let product_type = row.try_get::<&str, usize>(1)?.parse::<ProductType>()?;
        let product_id = row.try_get::<u64, usize>(2)?;
        let qty_req = row.try_get::<u32, usize>(3)?;
        Ok(Self {
            id_: BaseProductIdentity {
                store_id,
                product_type,
                product_id,
            },
            qty_req,
        })
    }
}

pub(crate) struct CartMariaDbRepo {
    _db: Arc<AppMariaDbStore>,
}

#[async_trait]
impl AbsCartRepo for CartMariaDbRepo {
    async fn update(&self, obj: CartModel) -> DefaultResult<usize, AppError> {
        let mut conn = self._db.acquire().await?;
        let mut tx = conn.begin().await?;
        let (sql_patt, args) = InsertUpdateTopLvlArg(&obj).into();
        // Note the mysql running the raw sql `INSERT ON DUPLICATE KEY UPDATE` will return
        // different num-affected values (1 if insert, and 2 if update)
        let _rs = run_query_once(&mut tx, sql_patt, args, None).await?;
        let (usr_id, seq_num, saved_lines, new_lines) =
            (obj.owner, obj.seq_num, obj.saved_lines, obj.new_lines);
        let (num_updating, num_inserting) = (saved_lines.len(), new_lines.len());
        if !saved_lines.is_empty() {
            let (sql_patt, args) = UpdateLineArg(usr_id, seq_num, saved_lines).into();
            let _rs = run_query_once(&mut tx, sql_patt, args, Some(num_updating)).await?;
        }
        if !new_lines.is_empty() {
            let (sql_patt, args) = InsertLineArg(usr_id, seq_num, new_lines).into();
            let _rs = run_query_once(&mut tx, sql_patt, args, Some(num_inserting)).await?;
        }
        tx.commit().await?;
        Ok(num_inserting + num_updating)
    } // end of fn update

    async fn discard(&self, owner: u32, seq: u8) -> DefaultResult<(), AppError> {
        let mut conn = self._db.acquire().await?;
        let mut tx = conn.begin().await?;
        let (sql_patt, args) = DiscardLineArg(owner, seq).into();
        let _rs = run_query_once(&mut tx, sql_patt, args, None).await?;
        let (sql_patt, args) = DiscardTopLvlArg(owner, seq).into();
        let _rs = run_query_once(&mut tx, sql_patt, args, Some(1)).await?;
        tx.commit().await?;
        Ok(())
    }

    async fn num_lines_saved(&self, owner: u32, seq: u8) -> DefaultResult<usize, AppError> {
        let (sql_patt, args) = FetchTotNumLinesArg(owner, seq).into();
        let mut conn = self._db.acquire().await?;
        let stmt = conn.prepare(sql_patt.as_str()).await?;
        let query = stmt.query_with(args);
        let exec = &mut *conn;
        let row = exec.fetch_one(query).await?;
        let num_lines_saved = row.try_get::<i64, usize>(0)?;
        Ok(num_lines_saved as usize)
    }

    async fn fetch_cart(&self, owner: u32, seq: u8) -> DefaultResult<CartModel, AppError> {
        let rawsql_toplvl = FetchTopLvlArg(owner, seq).into();
        let rawsql_line = FetchLinesArg(owner, seq, None).into();
        let out = self
            .fetch_common(owner, seq, rawsql_toplvl, rawsql_line)
            .await?;
        Ok(out)
    } // end of fn fetch_cart

    async fn fetch_lines_by_pid(
        &self,
        owner: u32,
        seq: u8,
        pids: Vec<BaseProductIdentity>,
    ) -> DefaultResult<CartModel, AppError> {
        let rawsql_toplvl = FetchTopLvlArg(owner, seq).into();
        let rawsql_line = FetchLinesArg(owner, seq, Some(pids)).into();
        let out = self
            .fetch_common(owner, seq, rawsql_toplvl, rawsql_line)
            .await?;
        Ok(out)
    } // end of fn fetch_lines_by_pid
} // end of impl CartMariaDbRepo

impl CartMariaDbRepo {
    pub async fn new(dbs: Vec<Arc<AppMariaDbStore>>) -> DefaultResult<Self, AppError> {
        if dbs.is_empty() {
            Err(AppError {
                code: AppErrorCode::MissingDataStore,
                detail: Some("mariadb".to_string()),
            })
        } else {
            let _db = dbs.first().unwrap().clone();
            Ok(Self { _db })
        }
    }
    async fn fetch_common(
        &self,
        owner: u32,
        seq: u8,
        rawsql_toplvl: (String, MySqlArguments),
        rawsql_line: (String, MySqlArguments),
    ) -> DefaultResult<CartModel, AppError> {
        let mut conn = self._db.acquire().await?;
        let result = {
            let stmt = conn.prepare(rawsql_toplvl.0.as_str()).await?;
            let query = stmt.query_with(rawsql_toplvl.1);
            let exec = &mut *conn;
            exec.fetch_optional(query).await?
        };
        if let Some(row) = result {
            let mut cart = CartModel::try_from(row)?;
            let stmt = conn.prepare(rawsql_line.0.as_str()).await?;
            let query = stmt.query_with(rawsql_line.1);
            let exec = &mut *conn;
            let rows = exec.fetch_all(query).await?;
            let mut errors = rows
                .into_iter()
                .filter_map(|row| match CartLineModel::try_from(row) {
                    Ok(v) => {
                        cart.saved_lines.push(v);
                        None
                    }
                    Err(e) => Some(e),
                })
                .collect::<Vec<_>>();
            if errors.is_empty() {
                Ok(cart)
            } else {
                Err(errors.remove(0))
            }
        } else {
            Ok(CartModel {
                owner,
                seq_num: seq,
                title: "Untitled".to_string(),
                saved_lines: Vec::new(),
                new_lines: Vec::new(),
            })
        }
    }
} // end of impl CartMariaDbRepo
