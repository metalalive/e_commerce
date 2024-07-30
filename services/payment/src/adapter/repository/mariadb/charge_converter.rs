use std::result::Result;

use mysql_async::Params;

use ecommerce_common::adapter::repository::OidBytes;
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::model::BaseProductIdentity;

use super::super::{AppRepoError, AppRepoErrorDetail, AppRepoErrorFnLabel};
use crate::model::{
    BuyerPayInState, ChargeBuyerModel, ChargeLineBuyerModel, ChargeMethodModel, PayLineAmountModel,
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

impl TryFrom<BuyerPayInState> for InsertChargeStatusArgs {
    type Error = AppRepoError;
    fn try_from(value: BuyerPayInState) -> Result<Self, Self::Error> {
        let (curr_state, times) = match value {
            BuyerPayInState::Initialized => Err(AppErrorCode::InvalidInput),
            BuyerPayInState::OrderAppExpired => Ok(("OrderAppExpired", 0usize, None)),
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
        let ChargeBuyerModel {
            owner,
            create_time,
            token: _,
            oid,
            lines: _,
            currency_snapshot: _, // at this point the currency snapshot is already saved with
            // order replica, no need to insert again
            state,
            method,
        } = value;
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
            ChargeMethodModel::Stripe(m) => {
                let label = "Stripe".to_string();
                serde_json::to_string(&m)
                    .map(|detail| (label.clone(), detail))
                    .map_err(|e| AppRepoError {
                        code: AppErrorCode::DataCorruption,
                        fn_label: AppRepoErrorFnLabel::CreateOrder,
                        detail: AppRepoErrorDetail::PayDetail(label, e.to_string()),
                    })
            },
            ChargeMethodModel::Unknown =>
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
            value.owner,
            value.create_time.format(DATETIME_FMT_P0F).to_string(),
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
