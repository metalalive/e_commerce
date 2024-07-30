mod mariadb;

use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;

use ecommerce_common::api::dto::{CountryCode, PhoneNumberDto};
use ecommerce_common::constant::ProductType;
use ecommerce_common::model::order::{BillingModel, ContactModel, PhyAddrModel};
use ecommerce_common::model::BaseProductIdentity;
use payment::model::{
    BuyerPayInState, ChargeBuyerModel, ChargeLineBuyerModel, ChargeMethodModel, ChargeToken,
    OrderCurrencySnapshot, OrderLineModel, OrderLineModelSet, PayLineAmountModel,
};

#[rustfmt::skip]
fn ut_setup_orderline_set(
    buyer_id: u32,
    order_id_hex: &str,
    num_charges: u32,
    create_time: DateTime<Utc>,
    currency_snapshot: HashMap<u32, OrderCurrencySnapshot>,
    d_lines: Vec<(u32, ProductType, u64, Decimal, Decimal, u32, Decimal, u32, Duration)>,
) -> OrderLineModelSet {
    let lines = d_lines
        .into_iter()
        .map(|d| {
            let (store_id, product_type, product_id,
                 charge_rsv_unit, charge_rsv_total, charge_rsv_qty,
                 charge_paid_total, charge_paid_qty, rsv_time_delta
                ) = d;
            let pid = BaseProductIdentity {store_id, product_type, product_id};
            let reserved_until = create_time + rsv_time_delta;
            let rsv_total = PayLineAmountModel {
                unit: charge_rsv_unit,
                total: charge_rsv_total,
                qty: charge_rsv_qty,
            };
            let paid_total = PayLineAmountModel {
                unit: charge_rsv_unit,
                total: charge_paid_total,
                qty: charge_paid_qty,
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
        buyer_id,
        lines,
        currency_snapshot,
        create_time: create_time.to_utc(),
        num_charges,
    }
} // end of fn ut_setup_orderline_set

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

#[rustfmt::skip]
fn ut_setup_buyer_charge(
    owner: u32,
    create_time: DateTime<Utc>,
    oid: String,
    state: BuyerPayInState,
    method: ChargeMethodModel,
    d_lines: Vec<(u32, ProductType, u64, Decimal, Decimal, u32)>,
    currency_snapshot: HashMap<u32, OrderCurrencySnapshot>,
) -> ChargeBuyerModel {
    let token = ChargeToken::encode(owner, create_time);
    let lines = d_lines
        .into_iter()
        .map(|dl| ChargeLineBuyerModel {
            pid: BaseProductIdentity {
                store_id: dl.0, product_type: dl.1, product_id: dl.2,
            },
            amount: PayLineAmountModel {
                unit: dl.3, total: dl.4, qty: dl.5,
            },
        })
        .collect();
    ChargeBuyerModel {
        owner, create_time, token, oid, lines,
        currency_snapshot, state, method,
    }
}
