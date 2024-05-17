use std::boxed::Box;
use std::result::Result as DefaultResult;
use std::sync::Arc;

use ecommerce_common::api::web::dto::QuotaResourceErrorDto;
use ecommerce_common::logging::{app_log_event, AppLogContext, AppLogLevel};
use ecommerce_common::model::BaseProductIdentity;

use crate::api::web::dto::CartDto;
use crate::constant::hard_limit;
use crate::error::AppError;
use crate::repository::AbsCartRepo;
use crate::{AppAuthQuotaMatCode, AppAuthedClaim};

pub(crate) struct ModifyCartLineUseCase {
    pub repo: Box<dyn AbsCartRepo>,
    pub log_ctx: Arc<AppLogContext>,
    pub authed_usr: AppAuthedClaim,
}
pub(crate) struct DiscardCartUseCase {
    pub repo: Box<dyn AbsCartRepo>,
    pub authed_usr: AppAuthedClaim,
}
pub(crate) struct RetrieveCartUseCase {
    pub repo: Box<dyn AbsCartRepo>,
    pub authed_usr: AppAuthedClaim,
}

pub(crate) enum ModifyCartUsKsResult {
    Success,
    NotFound,
    QuotaExceed(QuotaResourceErrorDto),
    ServerError(AppError),
}
pub(crate) enum DiscardCartUsKsResult {
    Success,
    NotFound,
    ServerError(AppError),
}
pub(crate) enum RetrieveCartUsKsResult {
    Success(CartDto),
    NotFound,
    ServerError(AppError),
}

impl ModifyCartLineUseCase {
    pub(crate) async fn execute(self, seq_num: u8, data: CartDto) -> ModifyCartUsKsResult {
        if seq_num >= hard_limit::MAX_NUM_CARTS_PER_USER {
            return ModifyCartUsKsResult::NotFound;
        }
        match self.validate_update(seq_num, data).await {
            Ok(v) => match v {
                Some(e) => ModifyCartUsKsResult::QuotaExceed(e),
                None => ModifyCartUsKsResult::Success,
            },
            Err(e) => ModifyCartUsKsResult::ServerError(e),
        }
    }

    async fn validate_update(
        &self,
        seq_num: u8,
        data: CartDto,
    ) -> DefaultResult<Option<QuotaResourceErrorDto>, AppError> {
        let owner = self.authed_usr.profile;
        let pids = data
            .lines
            .iter()
            .map(|cl| BaseProductIdentity {
                store_id: cl.seller_id,
                product_type: cl.product_type.clone(),
                product_id: cl.product_id,
            })
            .collect::<Vec<_>>();
        let mut obj = self.repo.fetch_lines_by_pid(owner, seq_num, pids).await?;
        obj.update(data);
        let logctx = &self.log_ctx;
        app_log_event!(
            logctx,
            AppLogLevel::DEBUG,
            "seq_num:{seq_num}, num-adding:{},\
                       num-updating:{}",
            obj.saved_lines.len(),
            obj.new_lines.len()
        );
        let num_saved = self.repo.num_lines_saved(owner, seq_num).await?;
        let total_num_lines = num_saved + obj.new_lines.len();
        let max_limit = self
            .authed_usr
            .quota_limit(AppAuthQuotaMatCode::NumOrderLines);
        if total_num_lines < (max_limit as usize) {
            let _num_updated = self.repo.update(obj).await?;
            Ok(None)
        } else {
            let e = QuotaResourceErrorDto {
                given: total_num_lines,
                max_: max_limit,
            };
            Ok(Some(e))
        }
    }
} // end of impl ModifyCartLineUseCase

impl DiscardCartUseCase {
    pub(crate) async fn execute(self, seq_num: u8) -> DiscardCartUsKsResult {
        if seq_num < hard_limit::MAX_NUM_CARTS_PER_USER {
            let owner = self.authed_usr.profile;
            match self.repo.discard(owner, seq_num).await {
                Ok(_v) => DiscardCartUsKsResult::Success,
                Err(e) => DiscardCartUsKsResult::ServerError(e),
            }
        } else {
            DiscardCartUsKsResult::NotFound
        }
    }
}

impl RetrieveCartUseCase {
    pub(crate) async fn execute(self, seq_num: u8) -> RetrieveCartUsKsResult {
        if seq_num < hard_limit::MAX_NUM_CARTS_PER_USER {
            let owner = self.authed_usr.profile;
            match self.repo.fetch_cart(owner, seq_num).await {
                Ok(m) => RetrieveCartUsKsResult::Success(m.into()),
                Err(e) => RetrieveCartUsKsResult::ServerError(e),
            }
        } else {
            RetrieveCartUsKsResult::NotFound
        }
    }
}
