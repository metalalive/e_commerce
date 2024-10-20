use std::collections::hash_map::RandomState;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use chrono::{DateTime, Local, Utc};
use rust_decimal::Decimal;

use ecommerce_common::api::dto::{
    CurrencyDto, CurrencySnapshotDto, OrderCurrencySnapshotDto, OrderLinePayDto,
    OrderSellerCurrencyDto,
};
use ecommerce_common::model::BaseProductIdentity;

use super::{PayLineAmountError, PayLineAmountModel};

#[derive(Debug)]
pub enum OrderModelError {
    EmptyLine,
    ZeroQuantity(BaseProductIdentity),
    RsvExpired(BaseProductIdentity),
    RsvError(BaseProductIdentity, String),
    InvalidAmount(BaseProductIdentity, PayLineAmountError),
    MissingActorsCurrency(Vec<u32>),
    MissingExRate(CurrencyDto),
    CorruptedExRate(CurrencyDto, String),
}

pub struct OrderLineModel {
    pub pid: BaseProductIdentity, // product ID
    pub rsv_total: PayLineAmountModel,
    pub paid_total: PayLineAmountModel,
    pub reserved_until: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderCurrencySnapshot {
    pub label: CurrencyDto,
    pub rate: Decimal,
}

pub struct OrderLineModelSet {
    pub id: String,
    pub buyer_id: u32, // buyer's profile ID in user-management service
    pub lines: Vec<OrderLineModel>,
    pub create_time: DateTime<Utc>,
    pub num_charges: u32, // TODO, discard needless field
    // - the map indicates currencies and locked exchange rate applied
    //   in buyer or sellers business.
    // - note current base currency in this project defaults to USD
    pub currency_snapshot: HashMap<u32, OrderCurrencySnapshot>,
}

#[rustfmt::skip]
impl TryFrom<(OrderLinePayDto, CurrencyDto)> for OrderLineModel {
    type Error = OrderModelError;
    fn try_from(value: (OrderLinePayDto, CurrencyDto)) -> Result<Self, Self::Error>
    {
        let (oline, currency_label) = value;
        let OrderLinePayDto {
            seller_id, product_id, product_type,
            reserved_until, quantity, amount: amount_dto,
        } = oline;
        let pid = BaseProductIdentity {store_id: seller_id, product_type, product_id};
        let rsv_parse_result = DateTime::parse_from_rfc3339(reserved_until.as_str());
        let now = Local::now().fixed_offset();

        if quantity == 0 {
            Err(Self::Error::ZeroQuantity(pid))
        } else if let Err(e) = rsv_parse_result.as_ref() {
            Err(Self::Error::RsvError(pid, e.to_string()))
        } else if &now >= rsv_parse_result.as_ref().unwrap() {
            Err(Self::Error::RsvExpired(pid))
        } else {
            let reserved_until = rsv_parse_result.unwrap().to_utc();
            let rsv_total = PayLineAmountModel::try_from((quantity, amount_dto, currency_label))
                .map_err(|e| Self::Error::InvalidAmount(pid.clone(), e)) ?;
            let paid_total = PayLineAmountModel::default();
            Ok(Self {pid, paid_total, rsv_total, reserved_until})
        }
    } // end of fn try-from
} // end of impl TryFrom for OrderLineModel

impl TryFrom<(CurrencyDto, &Vec<CurrencySnapshotDto>)> for OrderCurrencySnapshot {
    type Error = OrderModelError;
    fn try_from(value: (CurrencyDto, &Vec<CurrencySnapshotDto>)) -> Result<Self, Self::Error> {
        let (label, search_src) = value;
        let raw_rate = search_src
            .iter()
            .find(|s| s.name == label)
            .map(|s| s.rate.to_string())
            .ok_or(OrderModelError::MissingExRate(label.clone()))?;
        let rate = Decimal::from_str(raw_rate.as_str())
            .map_err(|_e| OrderModelError::CorruptedExRate(label.clone(), raw_rate.to_string()))?;
        Ok(Self { label, rate })
    }
} // end of impl OrderCurrencySnapshot

impl OrderLineModelSet {
    fn verify_seller_currency_integrity(
        required: &[OrderLinePayDto],
        provided: &[OrderSellerCurrencyDto],
    ) -> Result<(), OrderModelError> {
        let iter0 = required.iter().map(|v| v.seller_id);
        let iter1 = provided.iter().map(|v| v.seller_id);
        let required_ids: HashSet<u32, RandomState> = HashSet::from_iter(iter0);
        let provided_ids = HashSet::from_iter(iter1);
        let uncovered = &required_ids - &provided_ids;
        if uncovered.is_empty() {
            Ok(())
        } else {
            let ids = uncovered.into_iter().collect::<Vec<_>>();
            Err(OrderModelError::MissingActorsCurrency(ids))
        }
    }
    fn try_build_currency_snapshot(
        buyer_id: u32,
        data: OrderCurrencySnapshotDto,
    ) -> Result<HashMap<u32, OrderCurrencySnapshot>, Vec<OrderModelError>> {
        let OrderCurrencySnapshotDto {
            snapshot,
            sellers: sellers_label,
            buyer: buyer_label,
        } = data;
        let mut errors = Vec::new();
        let map_iter = sellers_label.into_iter().filter_map(|d| {
            let OrderSellerCurrencyDto {
                currency: s_label,
                seller_id,
            } = d;
            OrderCurrencySnapshot::try_from((s_label, &snapshot))
                .map(|m| (seller_id, m))
                .map_err(|e| errors.push(e))
                .ok()
        });
        let mut map = HashMap::from_iter(map_iter);
        if errors.is_empty() {
            let m =
                OrderCurrencySnapshot::try_from((buyer_label, &snapshot)).map_err(|e| vec![e])?;
            map.insert(buyer_id, m);
            Ok(map)
        } else {
            Err(errors)
        }
    } // end of fn try_build_currency_snapshot
} // end of impl OrderLineModelSet

impl TryFrom<(String, u32, Vec<OrderLinePayDto>, OrderCurrencySnapshotDto)> for OrderLineModelSet {
    type Error = Vec<OrderModelError>;

    fn try_from(
        value: (String, u32, Vec<OrderLinePayDto>, OrderCurrencySnapshotDto),
    ) -> Result<Self, Self::Error> {
        let (oid, buyer_id, lines_dto, currency_d) = value;
        let mut errors = vec![];
        if lines_dto.is_empty() {
            errors.push(OrderModelError::EmptyLine);
        }
        let _ = Self::verify_seller_currency_integrity(&lines_dto, &currency_d.sellers)
            .map_err(|e| errors.push(e));
        let result =
            Self::try_build_currency_snapshot(buyer_id, currency_d).map_err(|e| errors.extend(e));
        if !errors.is_empty() {
            return Err(errors);
        }
        let currency_snapshot = result.unwrap();
        let buyer_curr_label = currency_snapshot
            .get(&buyer_id)
            .map(|v| v.label.clone())
            .unwrap();
        let lines = lines_dto
            .into_iter()
            .filter_map(|d| {
                let args = (d, buyer_curr_label.clone());
                OrderLineModel::try_from(args)
                    .map_err(|e| errors.push(e))
                    .ok()
            })
            .collect::<Vec<_>>();
        if errors.is_empty() {
            Ok(Self {
                id: oid,
                buyer_id,
                lines,
                currency_snapshot,
                create_time: Local::now().to_utc(),
                num_charges: 0,
            })
        } else {
            Err(errors)
        }
    } // end of fn try-from
} // end of impl try-from for OrderLineModelSet
