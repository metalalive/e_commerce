mod capture_charge;
mod create_charge;
mod onboard;
mod refresh_charge_status;

pub use capture_charge::{ChargeCaptureUcError, ChargeCaptureUseCase};
pub use create_charge::{ChargeCreateUcError, ChargeCreateUseCase};
pub use onboard::{OnboardStoreUcError, OnboardStoreUseCase, RefreshOnboardStatusUseCase};
pub use refresh_charge_status::{ChargeRefreshUcError, ChargeStatusRefreshUseCase};

use chrono::{DateTime, Utc};

use ecommerce_common::error::AppErrorCode;
use ecommerce_common::util::hex_to_octet;

use crate::model::ChargeToken;

fn try_parse_charge_id(id_serial: &str) -> Result<(u32, DateTime<Utc>), (AppErrorCode, String)> {
    let id_octets = hex_to_octet(id_serial)?;
    let token = ChargeToken::try_from(id_octets)?;
    let (owner_id, ctime) = token.try_into()?;
    Ok((owner_id, ctime))
}
