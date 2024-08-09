use std::result::Result;

use chrono::{DateTime, Utc};
use mysql_async::Params;

use ecommerce_common::adapter::repository::OidBytes;
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::model::BaseProductIdentity;

use super::super::{AppRepoError, AppRepoErrorDetail, AppRepoErrorFnLabel};
use super::raw_column_to_datetime;
use crate::model::{
    BuyerPayInState, Charge3partyModel, Charge3partyStripeModel, ChargeBuyerMetaModel,
    ChargeBuyerModel, ChargeLineBuyerModel, PayLineAmountModel,
};

const DATETIME_FMT_P0F: &str = "%Y-%m-%d %H:%M:%S";
const DATETIME_FMT_P3F: &str = "%Y-%m-%d %H:%M:%S%.3f";

struct InsertChargeTopLvlArgs(String, Params);
struct InsertChargeStatusArgs {
    curr_state: String,
    t_accepted: Option<String>,
    t_completed: Option<String>,
    t_order_app_synced: Option<String>,
}
struct InsertChargeLinesArgs(String, Vec<Params>);

pub(super) struct InsertChargeArgs(pub(super) Vec<(String, Vec<Params>)>);
pub(super) struct FetchChargeMetaArgs(String, Params);

#[rustfmt::skip]
pub(super) type ChargeMetaRowType = (
    Vec<u8>,
    String, // `state`
    Option<mysql_async::Value>, // `processor_accepted_time`
    Option<mysql_async::Value>, // `processor_completed_time`
    Option<mysql_async::Value>, // `orderapp_synced_time`
    String, // `pay_method`
    String, // `detail_3rdparty`, serialised json
);

impl TryFrom<BuyerPayInState> for InsertChargeStatusArgs {
    type Error = AppRepoError;
    fn try_from(value: BuyerPayInState) -> Result<Self, Self::Error> {
        let (curr_state, times) = match value {
            BuyerPayInState::Initialized => Err(AppErrorCode::InvalidInput),
            BuyerPayInState::ProcessorAccepted(t) => Ok(("ProcessorAccepted", 0, Some(t))),
            BuyerPayInState::ProcessorCompleted(t) => Ok(("ProcessorCompleted", 1, Some(t))),
            BuyerPayInState::OrderAppSynced(t) => Ok(("OrderAppSynced", 2, Some(t))),
        }
        .map(|(label, idx, option_t)| {
            const REPEAT_INIT_VALUE: Option<String> = None;
            let mut times = [REPEAT_INIT_VALUE; 3];
            if let Some(t) = option_t {
                times[idx] = Some(t.format(DATETIME_FMT_P3F).to_string());
            }
            (label.to_string(), times)
        })
        .map_err(|code| AppRepoError {
            fn_label: AppRepoErrorFnLabel::CreateCharge,
            code,
            detail: AppRepoErrorDetail::ChargeStatus(value),
        })?;
        Ok(Self {
            curr_state,
            t_accepted: times[0].to_owned(),
            t_completed: times[1].to_owned(),
            t_order_app_synced: times[2].to_owned(),
        })
    } // end of fn try_from
} // end of impl InsertChargeStatusArgs

impl TryFrom<ChargeBuyerModel> for InsertChargeTopLvlArgs {
    type Error = AppRepoError;
    fn try_from(value: ChargeBuyerModel) -> Result<Self, Self::Error> {
        // at this point the currency snapshot and charge lines should be handled
        // elsewhere, no need to insert again
        let ChargeBuyerMetaModel {
            owner,
            create_time,
            oid,
            state,
            method,
        } = value.meta;
        let oid_b = OidBytes::try_from(oid.as_str()).map_err(|(code, msg)| AppRepoError {
            fn_label: AppRepoErrorFnLabel::CreateCharge,
            detail: AppRepoErrorDetail::OrderIDparse(msg),
            code,
        })?;
        let InsertChargeStatusArgs {
            curr_state,
            t_accepted,
            t_completed,
            t_order_app_synced,
        } = InsertChargeStatusArgs::try_from(state)?;
        #[rustfmt::skip]
        let (pay_mthd, detail_3pty) = match method {
            Charge3partyModel::Stripe(m) => {
                let label = "Stripe".to_string();
                serde_json::to_string(&m)
                    .map(|detail| (label.clone(), detail))
                    .map_err(|e| AppRepoError {
                        code: AppErrorCode::DataCorruption,
                        fn_label: AppRepoErrorFnLabel::CreateOrder,
                        detail: AppRepoErrorDetail::PayDetail(label, e.to_string()),
                    })
            },
            Charge3partyModel::Unknown =>
                Err(AppRepoError {
                    code: AppErrorCode::InvalidInput,
                    fn_label: AppRepoErrorFnLabel::CreateOrder,
                    detail: AppRepoErrorDetail::PayMethodUnsupport("unknown".to_string()),
                }),
        }?;
        let arg = vec![
            owner.into(),
            create_time.format(DATETIME_FMT_P0F).to_string().into(),
            oid_b.0.into(),
            curr_state.into(),
            t_accepted.into(),
            t_completed.into(),
            t_order_app_synced.into(),
            pay_mthd.into(),
            detail_3pty.into(),
        ];
        let params = Params::Positional(arg);
        let stmt = "INSERT INTO `charge_buyer_toplvl`(`usr_id`,`create_time`,`order_id`,\
                    `state`,`processor_accepted_time`,`processor_completed_time`,\
                    `orderapp_synced_time`,`pay_method`,`detail_3rdparty`) VALUES \
                    (?,?,?,?,?,?,?,?,?)";
        Ok(Self(stmt.to_string(), params))
    }
} // end of impl InsertChargeTopLvlArgs

impl From<(u32, String, Vec<ChargeLineBuyerModel>)> for InsertChargeLinesArgs {
    fn from(value: (u32, String, Vec<ChargeLineBuyerModel>)) -> Self {
        let (buyer_id, ctime, lines) = value;
        let params = lines
            .into_iter()
            .map(|line| {
                let ChargeLineBuyerModel { pid, amount } = line;
                let BaseProductIdentity {
                    store_id,
                    product_type,
                    product_id,
                } = pid;
                let PayLineAmountModel { unit, total, qty } = amount;
                let prod_type_num: u8 = product_type.into();
                let arg = vec![
                    buyer_id.into(),
                    ctime.as_str().into(),
                    store_id.into(),
                    prod_type_num.to_string().into(),
                    product_id.into(),
                    unit.into(),
                    total.into(),
                    qty.into(),
                ];
                Params::Positional(arg)
            })
            .collect();
        let stmt = "INSERT INTO `charge_line`(`buyer_id`,`create_time`,`store_id`,\
                    `product_type`,`product_id`,`amt_unit`,`amt_total`,`qty`) \
                    VALUES (?,?,?,?,?,?,?,?)";
        Self(stmt.to_string(), params)
    } // end of fn from
} // end of impl InsertChargeLinesArgs

impl TryFrom<ChargeBuyerModel> for InsertChargeArgs {
    type Error = AppRepoError;
    fn try_from(mut value: ChargeBuyerModel) -> Result<Self, Self::Error> {
        // TODO, modify order-line replica if input charge state is already
        // in `completed` state
        let (buyer_id, ctime) = (
            value.meta.owner,
            value.meta.create_time.format(DATETIME_FMT_P0F).to_string(),
        );
        let c_lines = value.lines.split_off(0);
        assert!(value.lines.is_empty());
        let toplvl_arg = InsertChargeTopLvlArgs::try_from(value)?;
        let lines_arg = InsertChargeLinesArgs::from((buyer_id, ctime, c_lines));
        let inner = vec![
            (toplvl_arg.0, vec![toplvl_arg.1]),
            (lines_arg.0, lines_arg.1),
        ];
        Ok(Self(inner))
    }
} // end of impl InsertChargeArgs

impl From<(u32, DateTime<Utc>)> for FetchChargeMetaArgs {
    fn from(value: (u32, DateTime<Utc>)) -> Self {
        let stmt = "SELECT `order_id`,`state`,`processor_accepted_time`,`processor_completed_time`,\
                    `orderapp_synced_time`,`pay_method`,`detail_3rdparty` FROM `charge_buyer_toplvl`\
                    WHERE `usr_id`=? AND `create_time`=?";
        let args = vec![
            value.0.into(),
            value.1.format(DATETIME_FMT_P0F).to_string().into(),
        ];
        Self(stmt.to_string(), Params::Positional(args))
    }
} // end of impl FetchChargeMetaArgs
impl FetchChargeMetaArgs {
    pub(super) fn into_parts(self) -> (String, Params) {
        (self.0, self.1)
    }
} // end of impl FetchChargeMetaArgs

impl TryFrom<(String, [Option<mysql_async::Value>; 3])> for BuyerPayInState {
    type Error = (AppErrorCode, AppRepoErrorDetail);
    fn try_from(value: (String, [Option<mysql_async::Value>; 3])) -> Result<Self, Self::Error> {
        let (label, time_records) = value;
        let mut time_records = time_records.to_vec();
        assert_eq!(time_records.len(), 3);
        let result = match label.as_str() {
            "ProcessorAccepted" => {
                if let Some(t) = time_records.remove(0) {
                    let t = raw_column_to_datetime(t, 3)?;
                    Ok(Self::ProcessorAccepted(t))
                } else {
                    Err("3pty-accepted-missing-time")
                }
            }
            "ProcessorCompleted" => {
                if let Some(t) = time_records.remove(1) {
                    let t = raw_column_to_datetime(t, 3)?;
                    Ok(Self::ProcessorCompleted(t))
                } else {
                    Err("3pty-completed-missing-time")
                }
            }
            "OrderAppSynced" => {
                if let Some(t) = time_records.remove(2) {
                    let t = raw_column_to_datetime(t, 3)?;
                    Ok(Self::OrderAppSynced(t))
                } else {
                    Err("orderapp-synced-missing-time")
                }
            }
            _others => Err("invalid-buy-in-state"),
        };
        result.map_err(|msg| {
            (
                AppErrorCode::DataCorruption,
                AppRepoErrorDetail::DataRowParse(format!("{msg}: {label}")),
            )
        })
    }
} // end of impl BuyerPayInState

impl TryFrom<(String, String)> for Charge3partyModel {
    type Error = (AppErrorCode, AppRepoErrorDetail);
    fn try_from(value: (String, String)) -> Result<Self, Self::Error> {
        let (label, detail) = value;
        let result = match label.as_str() {
            "Stripe" => serde_json::from_str::<Charge3partyStripeModel>(detail.as_str())
                .map(Charge3partyModel::Stripe)
                .map_err(|e| e.to_string()),
            _others => Err(format!("unknown-3pty-method: {}", label)),
        };
        result.map_err(|msg| {
            (
                AppErrorCode::DataCorruption,
                AppRepoErrorDetail::DataRowParse(msg),
            )
        })
    }
}

#[rustfmt::skip]
impl TryFrom<(u32, DateTime<Utc>, ChargeMetaRowType)> for ChargeBuyerMetaModel {
    type Error = (AppErrorCode, AppRepoErrorDetail);
    fn try_from(value: (u32, DateTime<Utc>, ChargeMetaRowType)) -> Result<Self, Self::Error> {
        let (
            owner, create_time,
            (
                oid_raw,
                buyin_state,
                accepted_time_3pty,
                completed_time_3pty,
                orderapp_synced_time,
                mthd_3pty_label,
                detail_3pty_serial,
            ),
        ) = value;
        let oid = OidBytes::to_app_oid(oid_raw)
            .map_err(|(code, msg)| (code, AppRepoErrorDetail::DataRowParse(msg)))?;
        let state = BuyerPayInState::try_from((
            buyin_state,
            [
                accepted_time_3pty,
                completed_time_3pty,
                orderapp_synced_time,
            ],
        ))?;
        let method = Charge3partyModel::try_from((mthd_3pty_label, detail_3pty_serial))?;
        Ok(Self { owner, create_time, oid, state, method, })
    } // end of fn try-from
} // end of impl ChargeBuyerMetaModel
