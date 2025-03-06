use chrono::offset::FixedOffset;
use chrono::DateTime;
use std::cmp::{Eq, PartialEq};
use std::collections::HashMap;
use std::result::Result as DefaultResult;
use std::vec::Vec;

use ecommerce_common::api::dto::CurrencyDto;
use ecommerce_common::error::AppErrorCode;

use crate::api::dto::ProdAttrValueDto;
use crate::api::rpc::dto::{ProdAttrPriceSetDto, ProductPriceEditDto};
use crate::api::web::dto::OrderLineRsvReqDto;
use crate::error::AppError;

type ProdAttrPricingMap = Option<HashMap<String, i32>>;

#[rustfmt::skip]
pub type ProductPriceCreateArgs = (u64, u32, [DateTime<FixedOffset>; 3], ProdAttrPricingMap);

#[derive(Debug, Clone, Eq)]
pub struct ProdAttriPriceModel {
    pricing: ProdAttrPricingMap,
    last_update: DateTime<FixedOffset>,
} // TODO, expose to order-line module for reuse

#[derive(Debug, Eq)]
pub struct ProductPriceModel {
    price: u32, // TODO, rename to base-price
    start_after: DateTime<FixedOffset>,
    end_before: DateTime<FixedOffset>,
    product_id: u64,
    attributes: ProdAttriPriceModel,
    is_create: bool,
} // TODO, extra pricing from product attributes

impl PartialEq for ProdAttriPriceModel {
    fn eq(&self, other: &Self) -> bool {
        (self.last_update == other.last_update) && (self.pricing == other.pricing)
    }
}

impl PartialEq for ProductPriceModel {
    fn eq(&self, other: &Self) -> bool {
        (self.price == other.price)
            && (self.product_id == other.product_id)
            && (self.start_after == other.start_after)
            && (self.end_before == other.end_before)
            && (self.attributes == other.attributes)
    }
}

impl Clone for ProductPriceModel {
    fn clone(&self) -> Self {
        Self {
            price: self.price,
            product_id: self.product_id,
            start_after: self.start_after,
            end_before: self.end_before,
            attributes: self.attributes.clone(),
            is_create: self.is_create,
        }
    }
}
impl From<(DateTime<FixedOffset>, ProdAttrPricingMap)> for ProdAttriPriceModel {
    fn from(d: (DateTime<FixedOffset>, ProdAttrPricingMap)) -> Self {
        Self {
            pricing: d.1,
            last_update: d.0,
        }
    }
}

impl<'a> TryFrom<&'a ProdAttrPriceSetDto> for ProdAttriPriceModel {
    type Error = AppError;

    fn try_from(d: &'a ProdAttrPriceSetDto) -> DefaultResult<Self, Self::Error> {
        let pricing = if d.extra_charge.is_empty() {
            None
        } else {
            let mut map = HashMap::new();
            for item in &d.extra_charge {
                let k = Self::map_key(item.label_id.as_str(), &item.value);
                if map.contains_key(&k) {
                    return Err(AppError {
                        code: AppErrorCode::InvalidInput,
                        detail: Some(format!("prod-price-dup-attrval: {}", k)),
                    });
                }
                map.insert(k, item.price);
            }
            Some(map)
        };
        Ok(Self::from((d.last_update, pricing)))
    }
}

impl ProdAttriPriceModel {
    fn map_key(label_id: &str, value: &ProdAttrValueDto) -> String {
        let val_str = match value {
            ProdAttrValueDto::Int(n) => n.to_string(),
            ProdAttrValueDto::Str(s) => s.clone(),
            ProdAttrValueDto::Bool(b) => b.to_string(),
        };
        format!("{}-{}", label_id, val_str)
    }
    pub(super) fn total_amount(&self) -> DefaultResult<i32, AppError> {
        self.pricing.as_ref().map_or(Ok(0i32), |m| {
            m.values()
                .try_fold(0i32, |acc, &num| acc.checked_add(num))
                .ok_or_else(|| AppError {
                    code: AppErrorCode::DataCorruption,
                    detail: Some("prod-attr-price-sum-overflow".to_string()),
                })
        })
    }
    pub(super) fn applied_attributes(&self) -> Vec<String> {
        match &self.pricing {
            Some(map) => map.keys().cloned().collect(),
            None => Vec::new(),
        }
    }
    pub(crate) fn serialize_map(&self) -> DefaultResult<String, AppError> {
        serde_json::to_string(&self.pricing).map_err(|e| {
            let detail = format!("prod-attr-price-serialize-map : {:?}", e);
            AppError {
                code: AppErrorCode::DataCorruption,
                detail: Some(detail),
            }
        })
    }
    pub(crate) fn deserialize_map(raw: &str) -> DefaultResult<ProdAttrPricingMap, AppError> {
        serde_json::from_str::<ProdAttrPricingMap>(raw).map_err(|e| {
            let detail = format!("prod-attr-price-deserialize-map : {:?}", e);
            AppError {
                code: AppErrorCode::DataCorruption,
                detail: Some(detail),
            }
        })
    }
    pub(crate) fn lastupdate(&self) -> DateTime<FixedOffset> {
        self.last_update
    }
} // end of impl ProdAttriPriceModel

impl<'a> TryFrom<&'a ProductPriceEditDto> for ProductPriceModel {
    type Error = AppError;

    fn try_from(d: &'a ProductPriceEditDto) -> DefaultResult<Self, Self::Error> {
        let attributes = ProdAttriPriceModel::try_from(&d.attributes)?;
        Ok(Self {
            price: d.price,
            product_id: d.product_id,
            start_after: d.start_after,
            end_before: d.end_before,
            attributes,
            is_create: true,
        })
    }
}

impl From<ProductPriceCreateArgs> for ProductPriceModel {
    fn from(d: ProductPriceCreateArgs) -> Self {
        Self {
            product_id: d.0,
            price: d.1,
            start_after: d.2[0],
            end_before: d.2[1],
            attributes: ProdAttriPriceModel::from((d.2[2], d.3)),
            is_create: false,
        }
    }
}

impl ProductPriceModel {
    #[rustfmt::skip]
    pub(crate) fn into_parts(self) -> ProductPriceCreateArgs {
        let Self {product_id, price, start_after, end_before, attributes, is_create: _} = self;
        let ProdAttriPriceModel {
            last_update: attr_lastupdate, pricing: attr_pricing
        } = attributes;
        let ts = [start_after, end_before, attr_lastupdate];
        (product_id, price, ts, attr_pricing)
    }
    pub(crate) fn base_price(&self) -> u32 {
        // TODO, separate method for calculating price with extra attribute combination
        self.price
    }
    pub fn product_id(&self) -> u64 {
        self.product_id
    }
    #[cfg(feature = "mariadb")]
    pub(crate) fn start_after(&self) -> DateTime<FixedOffset> {
        self.start_after
    }
    #[cfg(feature = "mariadb")]
    pub(crate) fn end_before(&self) -> DateTime<FixedOffset> {
        self.end_before
    }
    pub(crate) fn attrs_charge(&self) -> &ProdAttriPriceModel {
        &self.attributes
    }
    #[cfg(feature = "mariadb")]
    pub(crate) fn split_by_update_state(ms: Vec<Self>) -> (Vec<Self>, Vec<Self>) {
        let (mut l_add, mut l_modify) = (vec![], vec![]);
        ms.into_iter()
            .map(|p| {
                if p.is_create {
                    l_add.push(p);
                } else {
                    l_modify.push(p)
                }
            })
            .count(); // TODO, swtich to feature `drain-filter` when it becomes stable
        (l_add, l_modify)
    }

    fn find_product(&self, d: &OrderLineRsvReqDto) -> bool {
        // TODO, validate expiry of the pricing rule
        let id_match = self.product_id() == d.product_id;
        let chosen_attr_match = d.applied_attr.as_ref().map_or(true, |chosen| {
            if chosen.is_empty() {
                true
            } else {
                self.attrs_charge().pricing.as_ref().map_or(false, |vm| {
                    chosen.iter().all(|c| {
                        let k = ProdAttriPriceModel::map_key(c.label_id.as_str(), &c.value);
                        vm.contains_key(&k)
                    })
                })
            }
        });
        id_match && chosen_attr_match
    }

    pub(super) fn extract_attributes(
        &self,
        d: &OrderLineRsvReqDto,
    ) -> DefaultResult<ProdAttriPriceModel, AppError> {
        let newmap = if let Some(chosen) = d.applied_attr.as_ref() {
            if chosen.is_empty() {
                None
            } else if let Some(vm) = self.attrs_charge().pricing.as_ref() {
                let mut nonexist = Vec::new();
                let map_iter = chosen.iter().filter_map(|c| {
                    let k = ProdAttriPriceModel::map_key(c.label_id.as_str(), &c.value);
                    if let Some(extra_amount) = vm.get(&k) {
                        Some((k, *extra_amount))
                    } else {
                        nonexist.push(c);
                        None
                    }
                });
                let map = HashMap::from_iter(map_iter);
                if nonexist.is_empty() {
                    Some(map)
                } else {
                    let detail = format!("price-attr-extract-fail : {:?}", nonexist);
                    return Err(AppError {
                        code: AppErrorCode::InvalidInput,
                        detail: Some(detail),
                    });
                }
            } else {
                let detail = format!("price-attr-extract-fail : {:?}", chosen);
                return Err(AppError {
                    code: AppErrorCode::InvalidInput,
                    detail: Some(detail),
                });
            }
        } else {
            None
        };
        let args = (self.attrs_charge().lastupdate(), newmap);
        Ok(ProdAttriPriceModel::from(args))
    } // end of fn extract_attributes

    fn update(&mut self, d: &ProductPriceEditDto) -> DefaultResult<(), AppError> {
        (self.price, self.end_before) = (d.price, d.end_before);
        self.start_after = d.start_after;
        let new_attrs = ProdAttriPriceModel::try_from(&d.attributes)?;
        self.attributes = new_attrs;
        Ok(())
    }
} // end of impl ProductPriceModel

pub struct ProductPriceModelSet {
    pub store_id: u32,
    pub currency: CurrencyDto,
    pub items: Vec<ProductPriceModel>,
}

impl ProductPriceModelSet {
    pub fn update(
        mut self,
        updating: Vec<ProductPriceEditDto>,
        creating: Vec<ProductPriceEditDto>,
        new_currency: CurrencyDto,
    ) -> DefaultResult<Self, AppError> {
        let mut es = Vec::new();
        let num_updated = updating
            .iter()
            .filter_map(|d| {
                let result = self
                    .items
                    .iter_mut()
                    .find(|obj| obj.product_id == d.product_id && !obj.is_create);
                if let Some(obj) = result {
                    obj.update(d).map_err(|e| es.push(e)).ok()
                } else {
                    None
                }
            })
            .count();
        if num_updated != updating.len() {
            return Err(AppError {
                code: AppErrorCode::InvalidInput,
                detail: Some("updating-data-to-nonexist-obj".to_string()),
            });
        }
        let mut new_items = creating
            .iter()
            .filter_map(|m| ProductPriceModel::try_from(m).map_err(|e| es.push(e)).ok())
            .collect();
        if es.is_empty() {
            self.items.append(&mut new_items);
            self.currency = new_currency;
            Ok(self)
        } else {
            Err(es.remove(0))
        }
    } // end of fn update

    pub(crate) fn find_product(&self, d: &OrderLineRsvReqDto) -> Option<&ProductPriceModel> {
        if self.store_id == d.seller_id {
            self.items.iter().find(|m| m.find_product(d))
        } else {
            None
        }
    }
} // end of impl ProductPriceModelSet
