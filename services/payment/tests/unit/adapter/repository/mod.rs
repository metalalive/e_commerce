mod mariadb;

use chrono::{DateTime, Duration, FixedOffset, Utc};

use ecommerce_common::api::dto::{CountryCode, PhoneNumberDto};
use ecommerce_common::constant::ProductType;
use ecommerce_common::model::order::{BillingModel, ContactModel, PhyAddrModel};
use ecommerce_common::model::BaseProductIdentity;
use payment::api::web::dto::PaymentMethodReqDto;
use payment::model::{
    BuyerPayInState, ChargeBuyerModel, ChargeLineBuyerModel, ChargeToken, OrderLineModel,
    OrderLineModelSet, PayLineAmountModel,
};

fn ut_setup_orderline_set(
    owner: u32,
    order_id_hex: &str,
    num_charges: u32,
    create_time: DateTime<FixedOffset>,
    d_lines: Vec<(u32, ProductType, u64, [u32; 5], Duration)>,
) -> OrderLineModelSet {
    let lines = d_lines
        .into_iter()
        .map(|d| {
            let (store_id, product_type, product_id, charge_stats, rsv_time_delta) = d;
            let pid = BaseProductIdentity {
                store_id,
                product_type,
                product_id,
            };
            let reserved_until = create_time + rsv_time_delta;
            let rsv_total = PayLineAmountModel {
                unit: charge_stats[0],
                total: charge_stats[1],
                qty: charge_stats[2],
            };
            let paid_total = PayLineAmountModel {
                unit: charge_stats[0],
                total: charge_stats[3],
                qty: charge_stats[4],
            };
            OrderLineModel {
                pid,
                rsv_total,
                paid_total,
                reserved_until,
            }
        })
        .collect();
    OrderLineModelSet {
        id: order_id_hex.to_string(),
        owner,
        lines,
        create_time: create_time.to_utc(),
        num_charges,
    }
}

fn ut_setup_order_bill() -> BillingModel {
    let address = PhyAddrModel {
        country: CountryCode::TW,
        region: "Hualien".to_string(),
        city: "Taidong".to_string(),
        distinct: "old town z1".to_string(),
        street_name: Some("hype tee hee".to_string()),
        detail: "Wolphennwatz".to_string(),
    };
    let contact = ContactModel {
        first_name: "lighting".to_string(),
        last_name: "gasoline".to_string(),
        emails: vec!["cricket@locust.io".to_string()],
        phones: vec![PhoneNumberDto {
            nation: 911,
            number: "0032811018".to_string(),
        }],
    };
    BillingModel {
        contact,
        address: Some(address),
    }
}

fn ut_setup_buyer_charge(
    owner: u32,
    create_time: DateTime<Utc>,
    oid: String,
    state: BuyerPayInState,
    method: PaymentMethodReqDto,
    d_lines: Vec<(u32, ProductType, u64, u32, u32, u32)>,
) -> ChargeBuyerModel {
    let token = ChargeToken::encode(owner, create_time);
    let lines = d_lines
        .into_iter()
        .map(|dl| ChargeLineBuyerModel {
            pid: BaseProductIdentity {
                store_id: dl.0,
                product_type: dl.1,
                product_id: dl.2,
            },
            amount: PayLineAmountModel {
                unit: dl.3,
                total: dl.4,
                qty: dl.5,
            },
        })
        .collect();
    ChargeBuyerModel {
        owner,
        create_time,
        token,
        oid,
        lines,
        state,
        method,
    }
}
