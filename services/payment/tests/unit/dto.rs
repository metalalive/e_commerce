use chrono::{DateTime, Duration, Utc};

use ecommerce_common::api::dto::{CountryCode, CurrencyDto};
use ecommerce_common::api::rpc::dto::{
    ShopLocationRepDto, StoreEmailRepDto, StorePhoneRepDto, StoreProfileReplicaDto,
    StoreStaffRepDto,
};

use payment::api::web::dto::{
    CapturePay3partyRespDto, CapturePayRespDto, StoreOnboardReqDto, StoreOnboardStripeReqDto,
};

pub(super) fn ut_default_store_onboard_req_stripe() -> StoreOnboardReqDto {
    let s = StoreOnboardStripeReqDto {
        return_url: "https://mariadb.com/kb/en/documentation/".to_string(),
        refresh_url: "https://www.postgresql.org/docs/".to_string(),
    };
    StoreOnboardReqDto::Stripe(s)
}

#[rustfmt::skip]
fn ut_default_shop_location_dto() -> ShopLocationRepDto {
    ShopLocationRepDto {
        country: CountryCode::TW, locality: "Taitung".to_string(),
        street: "auphauph Rd".to_string(), detail: "fisher port".to_string(),
        floor: 1
    }
}
#[rustfmt::skip]
fn ut_default_store_emails_dto() -> Vec<StoreEmailRepDto> {
    ["asher@briegalo.org", "garcia@beruian00.nw"]
        .into_iter()
        .map(|a| StoreEmailRepDto { addr: a.to_string() })
        .collect()
}
#[rustfmt::skip]
fn ut_default_store_phones_dto() -> Vec<StorePhoneRepDto> {
    [("91", "820018203"), ("886", "920281151")]
        .into_iter()
        .map(|(code, lnum)| StorePhoneRepDto {
            country_code: code.to_string(),
            line_number: lnum.to_string(),
        })
        .collect()
}

#[rustfmt::skip]
pub(super) fn ut_setup_storeprofile_dto(
    shop_name: &str,
    supervisor_id: u32,
    staff_usr_ids: Vec<u32>,
    start_time: DateTime<Utc>,
) -> StoreProfileReplicaDto {
    let start_after = start_time;
    let end_before = start_after + Duration::minutes(10);
    let staff = staff_usr_ids.into_iter()
        .map(|staff_id| StoreStaffRepDto {
            start_after: start_after.to_rfc3339(),
            end_before: end_before.to_rfc3339(),
            staff_id
        })
        .collect::<Vec<_>>();
    let location = ut_default_shop_location_dto();
    let emails = ut_default_store_emails_dto();
    let phones = ut_default_store_phones_dto();
    StoreProfileReplicaDto {
        label: shop_name.to_string(), active: true, supervisor_id,
        emails: Some(emails), phones: Some(phones),
        location: Some(location),  staff: Some(staff),
    }
}

pub(super) fn ut_setup_capture_pay_resp_dto(store_id: u32) -> CapturePayRespDto {
    CapturePayRespDto {
        store_id,
        amount: "5566.7788".to_string(),
        currency: CurrencyDto::INR,
        processor: CapturePay3partyRespDto::Stripe {
            amount: "601.87".to_string(),
            currency: CurrencyDto::USD,
        },
    }
}
