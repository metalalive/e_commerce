use std::result::Result;

use chrono::{DateTime, Local, Utc};

use ecommerce_common::api::rpc::dto::{StoreProfileReplicaDto, StoreStaffRepDto};

use super::external_processor::Merchant3partyStripeModel;

#[derive(Debug)]
pub enum MerchantModelError {
    NotExist,
    InActive,
    ShopNameEmpty,
    StaffCorruptedTime(Vec<(u32, String)>),
}

pub enum Merchant3partyModel {
    Stripe(Merchant3partyStripeModel),
    Unknown,
}

pub struct MerchantProfileModel {
    pub(crate) id: u32, // store-id from storefront service
    pub(crate) name: String,
    pub(crate) supervisor_id: u32,
    pub(crate) staff_ids: Vec<u32>,
    // TODO, refresh owner-id and  staff-ids periodically
    pub(crate) last_update: DateTime<Utc>,
}

impl TryFrom<(u32, &StoreProfileReplicaDto)> for MerchantProfileModel {
    type Error = MerchantModelError;
    fn try_from(value: (u32, &StoreProfileReplicaDto)) -> Result<Self, Self::Error> {
        let (id, store_rep) = value;
        if !store_rep.active {
            return Err(MerchantModelError::InActive);
        } else if store_rep.label.is_empty() {
            return Err(MerchantModelError::ShopNameEmpty);
        }
        let last_update = Local::now().to_utc();
        let staff_ids = if let Some(vs) = store_rep.staff.as_ref() {
            Self::collect_vaild_staff(vs, last_update)?
        } else {
            Vec::new()
        };
        Ok(Self {
            id,
            staff_ids,
            last_update,
            name: store_rep.label.clone(),
            supervisor_id: store_rep.supervisor_id,
        })
    }
}

impl MerchantProfileModel {
    fn collect_vaild_staff(
        vs: &[StoreStaffRepDto],
        t_now: DateTime<Utc>,
    ) -> Result<Vec<u32>, MerchantModelError> {
        let mut errors = Vec::new();
        let out = vs
            .iter()
            .filter_map(|v| {
                let r0 = DateTime::parse_from_rfc3339(v.start_after.as_str())
                    .map_err(|_e| errors.push((v.staff_id, v.start_after.clone())))
                    .ok();
                let r1 = DateTime::parse_from_rfc3339(v.end_before.as_str())
                    .map_err(|_e| errors.push((v.staff_id, v.end_before.clone())))
                    .ok();
                if let (Some(t0), Some(t1)) = (r0, r1) {
                    Some((v.staff_id, t0, t1))
                } else {
                    None
                }
            })
            .filter_map(|(sid, t0, t1)| {
                if (t_now > t0) && (t1 > t_now) {
                    Some(sid)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        if errors.is_empty() {
            Ok(out)
        } else {
            Err(MerchantModelError::StaffCorruptedTime(errors))
        }
    } // end of fn collect_vaild_staff

    pub fn valid_supervisor(&self, usr_id: u32) -> bool {
        self.supervisor_id == usr_id
    }
    pub fn name(&self) -> &str {
        self.name.as_str()
    }
    pub fn valid_staff(&self, usr_id: u32) -> bool {
        let mut found = self.supervisor_id == usr_id;
        if !found {
            found = self.staff_ids.contains(&usr_id);
        }
        found
    }
} // end of impl MerchantProfileModel
