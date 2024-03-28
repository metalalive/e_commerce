
use crate::api::web::dto::{CartDto, CartLineDto};

use super::BaseProductIdentity;

pub struct CartLineModel {
    pub id_: BaseProductIdentity,
    // TODO, add fields which indicate
    // - product variant (e.g. specific size, color, etc.
    //   it may be structure like JSON but immutable in this service)
    // - extra cost to the product variant above
    pub qty_req: u32,
    // current price and stock level are excluded
}

pub struct CartModel {
    pub owner : u32,
    pub seq_num : u8,
    pub title: String,
    pub saved_lines : Vec<CartLineModel>,
    pub new_lines : Vec<CartLineModel>,
    // TODO, add fields which indicate
    // - extra cost amount for tax, 
    // - sharable flag, can be shared among users
    // - list of user IDs that can edit the cart
}

impl From<CartLineDto> for CartLineModel {
    fn from(value: CartLineDto) -> Self {
        Self {
            id_: BaseProductIdentity { store_id: value.seller_id, product_type: value.product_type,
            product_id: value.product_id }, qty_req: value.quantity
        }
    }
}

impl Into<CartLineDto> for CartLineModel {
    fn into(self) -> CartLineDto {
        CartLineDto {
            product_id:self.id_.product_id, product_type:self.id_.product_type,
            seller_id:self.id_.store_id, quantity:self.qty_req
        }
    }
}

impl Into<CartDto> for CartModel {
    fn into(self) -> CartDto {
        CartDto {
            title: self.title, lines: self.saved_lines.into_iter()
            .map(CartLineModel::into).collect::<Vec<_>>()
        }
    }
}

impl CartModel {
    pub fn update(&mut self, data:CartDto) {
        let (new_title, d_lines) = (data.title, data.lines);
        self.title = new_title;
        self.new_lines = d_lines.into_iter().filter_map(|d| {
            let result = self.get_line_mut(&d);
            if let Some(v) = result {
                v.qty_req = d.quantity;
                None
            } else { Some(d) }
        }).map(CartLineModel::from).collect::<Vec<_>>();
    }

    fn get_line_mut(&mut self, item: &CartLineDto) -> Option<&mut CartLineModel>
    {
        let result = self.saved_lines.iter_mut().find(
            |obj| obj.id_.store_id == item.seller_id
                && obj.id_.product_type == item.product_type
                && obj.id_.product_id == item.product_id );
        result
    }
} // end of impl CartModel
