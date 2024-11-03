use super::ChargeBuyerModel;
use crate::api::web::dto::ReportChargeRespDto;

impl From<Vec<ChargeBuyerModel>> for ReportChargeRespDto {
    fn from(_value: Vec<ChargeBuyerModel>) -> Self {
        ReportChargeRespDto
    }
}
