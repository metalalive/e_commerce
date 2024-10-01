use std::marker::Send;
use std::result::Result;

use chrono::{DateTime, Utc};
use mysql_async::prelude::{FromRow, Query, WithParams};
use mysql_async::{Conn, Params};
use rust_decimal::Decimal;

use ecommerce_common::adapter::repository::OidBytes;
use ecommerce_common::error::AppErrorCode;

use super::super::AppRepoErrorDetail;
use super::DATETIME_FMT_P0F;
use crate::model::{Label3party, Payout3partyModel, Payout3partyStripeModel, PayoutInnerModel};

pub(super) struct InsertPayoutMetaArgs(String, Params);
pub(super) struct InsertPayout3partyArgs(String, Params);
pub(super) struct FetchPayoutMetaArgs(String, Params);
pub(super) struct FetchPayout3partyArgs(String, Params, Label3party);

pub(super) type PayoutMetaRowType = (
    mysql_async::Value, // `create_time`
    u32,                // `storestaff-usr-id`
    Vec<u8>,            // `order-id`
    Decimal,            // `amount-buyer`
    String,             // `3party-label`
);

type Payout3ptyStripeRowType = (
    String,  // `tx-grp`
    String,  // `acct-id`
    String,  // `transfer-id`
    Decimal, // `amount-base`
);

#[rustfmt::skip]
impl TryFrom<(PayoutInnerModel, Label3party)> for InsertPayoutMetaArgs {
    type Error = (AppErrorCode, AppRepoErrorDetail);
    
    fn try_from(value: (PayoutInnerModel, Label3party)) -> Result<Self, Self::Error> {
        let (p_inner, label3pt) = value;
        let stmt = "INSERT INTO `payout_meta`(`buyer_usr_id`,`charged_time`,`store_id`,`create_time`,\
                    `storestaff_usr_id`,`order_id`,`amount_buyer`,`label3party`)\
                    VALUES (?,?,?,?, ?,?,?,?)";
        
        // note the currency snoapshot for specific order should be saved in another module
        // `order-replica`, no need to persist them at here
        let amt_buyer = p_inner.amount_buyer();
        let (
            merchant_id, capture_time, buyer_id, charge_ctime,
            storestaff_id, _amount_m, order_id
        ) = p_inner.into_parts();
        let oid_b = OidBytes::try_from(order_id.as_str())
            .map_err(|(code, msg)| (code, AppRepoErrorDetail::OrderIDparse(msg)))?;
        
        let args = vec![
            buyer_id.into(), charge_ctime.format(DATETIME_FMT_P0F).to_string().into(),
            merchant_id.into(), capture_time.format(DATETIME_FMT_P0F).to_string().into(),
            storestaff_id.into(), oid_b.as_column().into(), amt_buyer.into(),
            label3pt.to_string().into(),
        ];
        let params = Params::Positional(args);
        Ok(Self(stmt.to_string(), params))
    }
} // end of impl InsertPayout3partyArgs

type Payout3partyCvtFromArg = (u32, DateTime<Utc>, u32, Payout3partyModel);

impl TryFrom<Payout3partyCvtFromArg> for InsertPayout3partyArgs {
    type Error = (AppErrorCode, AppRepoErrorDetail);

    fn try_from(value: Payout3partyCvtFromArg) -> Result<Self, Self::Error> {
        let (buyer_usr_id, charged_ctime, merchant_id, p3pty) = value;
        match p3pty {
            Payout3partyModel::Stripe(s) => {
                Self::try_from_stripe(buyer_usr_id, charged_ctime, merchant_id, s).map_err(|msg| {
                    (
                        AppErrorCode::InvalidInput,
                        AppRepoErrorDetail::PayDetail(Label3party::Stripe.to_string(), msg),
                    )
                })
            }
        }
    }
}
impl InsertPayout3partyArgs {
    #[rustfmt::skip]
    fn try_from_stripe(
        buyer_usr_id: u32,
        charged_ctime: DateTime<Utc>,
        merchant_id: u32,
        value: Payout3partyStripeModel
    ) -> Result<Self, String> {
        let amt_bs = value.amount().ok_or("missing-amount".to_string())?;
        let transfer_id = value.transfer_id().ok_or("missing-transfer-id".to_string())?;
        let tx_grp = value.transfer_group();
        let acct_id = value.connect_account();
        let stmt = "INSERT INTO `payout_3party_stripe`(`buyer_usr_id`,`charged_time`,`store_id`,\
                    `tx_grp`,`acct_id`,`transfer_id`,`amount`) VALUES (?,?,?,?,?,?,?)";
        let args = vec![
            buyer_usr_id.into(), charged_ctime.format(DATETIME_FMT_P0F).to_string().into(),
            merchant_id.into(), tx_grp.into(), acct_id.into(), transfer_id.into(),
            amt_bs.into(),
        ];
        let params = Params::Positional(args);
        Ok(Self(stmt.to_string(), params))
    }
} // end of impl InsertPayout3partyArgs

macro_rules! inner_into_parts {
    ($sqlargs: ty) => {
        impl $sqlargs {
            pub(super) fn into_parts(self) -> (String, Params) {
                (self.0, self.1)
            }
        }
    };
}

inner_into_parts!(InsertPayoutMetaArgs);
inner_into_parts!(InsertPayout3partyArgs);
inner_into_parts!(FetchPayoutMetaArgs);

impl From<(u32, DateTime<Utc>, u32)> for FetchPayoutMetaArgs {
    fn from(value: (u32, DateTime<Utc>, u32)) -> Self {
        let (buyer_id, charged_time, store_id) = value;
        let stmt = "SELECT `create_time`,`storestaff_usr_id`,`order_id`,`amount_buyer`,\
                   `label3party` FROM `payout_meta` WHERE `buyer_usr_id`=? AND `charged_time`=? \
                   AND `store_id`=?";
        let arg = vec![
            buyer_id.into(),
            charged_time.format(DATETIME_FMT_P0F).to_string().into(),
            store_id.into(),
        ];
        let params = Params::Positional(arg);
        Self(stmt.to_string(), params)
    }
}

impl From<(u32, DateTime<Utc>, u32, Label3party)> for FetchPayout3partyArgs {
    fn from(value: (u32, DateTime<Utc>, u32, Label3party)) -> Self {
        let (buyer_id, charged_time, store_id, label3pt) = value;
        let stmt = match &label3pt {
            Label3party::Stripe => {
                "SELECT `tx_grp`,`acct_id`,`transfer_id`,`amount` FROM `payout_3party_stripe`\
                WHERE `buyer_usr_id`=? AND `charged_time`=? AND `store_id`=?"
            }
        };
        let arg = vec![
            buyer_id.into(),
            charged_time.format(DATETIME_FMT_P0F).to_string().into(),
            store_id.into(),
        ];
        let params = Params::Positional(arg);
        Self(stmt.to_string(), params, label3pt)
    }
}

impl FetchPayout3partyArgs {
    pub(super) async fn fetch(
        self,
        conn: &mut Conn,
    ) -> Result<Payout3partyModel, (AppErrorCode, AppRepoErrorDetail)> {
        let Self(stmt, params, label) = self;
        match label {
            Label3party::Stripe => {
                let row = Self::lowlvl_fetch::<Payout3ptyStripeRowType>(stmt, params, conn).await?;
                let arg = (row.0, row.1, Some(row.2), Some(row.3));
                let s = Payout3partyStripeModel::from(arg);
                Ok(Payout3partyModel::Stripe(s))
            }
        }
    }
    async fn lowlvl_fetch<T: FromRow + Send + 'static>(
        stmt: String,
        params: Params,
        conn: &mut Conn,
    ) -> Result<T, (AppErrorCode, AppRepoErrorDetail)> {
        stmt.with(params)
            .first::<T, &mut Conn>(conn)
            .await
            .map_err(|e| {
                let code = AppErrorCode::RemoteDbServerFailure;
                let detail = AppRepoErrorDetail::DatabaseQuery(e.to_string());
                (code, detail)
            })?
            .ok_or((
                AppErrorCode::DataCorruption,
                AppRepoErrorDetail::DatabaseQuery("missing-3party".to_string()),
            ))
    }
} // end of impl FetchPayout3partyArgs
