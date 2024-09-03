use payment::api::web::dto::{StoreOnboardReqDto, StoreOnboardStripeReqDto};

pub(super) fn ut_default_store_onboard_req_stripe() -> StoreOnboardReqDto {
    let s = StoreOnboardStripeReqDto {
        return_url: "https://mariadb.com/kb/en/documentation/".to_string(),
        refresh_url: "https://www.postgresql.org/docs/".to_string(),
    };
    StoreOnboardReqDto::Stripe(s)
}
