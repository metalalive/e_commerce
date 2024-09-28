use std::result::Result;

use chrono::{DateTime, Utc};
use mysql_async::Params;

use ecommerce_common::adapter::repository::OidBytes;
use ecommerce_common::error::AppErrorCode;

use super::super::AppRepoErrorDetail;
use super::DATETIME_FMT_P0F;
use crate::model::{Label3party, Payout3partyModel, Payout3partyStripeModel, PayoutInnerModel};

pub(super) struct InsertPayoutMetaArgs(String, Params);
pub(super) struct InsertPayout3partyArgs(String, Params);

#[rustfmt::skip]
impl TryFrom<(PayoutInnerModel, Label3party)> for InsertPayoutMetaArgs {
    type Error = (AppErrorCode, AppRepoErrorDetail);
    
    fn try_from(value: (PayoutInnerModel, Label3party)) -> Result<Self, Self::Error> {
        let (p_inner, label3pt) = value;
        let stmt = "INSERT INTO `payout_meta`(`buyer_usr_id`,`charged_time`,`store_id`,`create_time`,\
                    `storestaff_usr_id`,`order_id`,`amount_base`,`amount_merchant`, `label3party`)\
                    VALUES (?,?,?,?, ?,?,?,?, ?)";
        
        // note the currency snoapshot for specific order should be saved in another module
        // `order-replica`, no need to persist them at here
        let amt_base = p_inner.amount_base();
        let (amt_merc, _rate, _snapshot) = p_inner.amount_merchant();
        let (
            merchant_id, capture_time, buyer_id, charge_ctime,
            storestaff_id, _amount_m, order_id
        ) = p_inner.into_parts();
        let oid_b = OidBytes::try_from(order_id.as_str())
            .map_err(|(code, msg)| (code, AppRepoErrorDetail::OrderIDparse(msg)))?;
        
        let args = vec![
            buyer_id.into(), charge_ctime.format(DATETIME_FMT_P0F).to_string().into(),
            merchant_id.into(), capture_time.format(DATETIME_FMT_P0F).to_string().into(),
            storestaff_id.into(), oid_b.as_column().into(), amt_base.into(), amt_merc.into(),
            label3pt.to_string().into(),
        ];
        let params = Params::Positional(args);
        Ok(Self(stmt.to_string(), params))
    }
} // end of impl InsertPayout3partyArgs

impl InsertPayoutMetaArgs {
    pub(super) fn into_parts(self) -> (String, Params) {
        (self.0, self.1)
    }
}

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

impl InsertPayout3partyArgs {
    pub(super) fn into_parts(self) -> (String, Params) {
        (self.0, self.1)
    }
}
