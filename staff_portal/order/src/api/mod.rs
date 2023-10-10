pub mod web;
pub mod rpc;

use serde::Deserialize;
use serde::de::{Error as DeserializeError, Expected, Unexpected};

use crate::constant::ProductType;

struct ExpectProdTyp {
    numbers: Vec<u8>
}
impl Expected for ExpectProdTyp
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        let s:Vec<String> = self.numbers.iter().map(|n| n.to_string()).collect();
        let s = s.join(",");
        let msg = format!("accepted type number : {s}");
        formatter.write_str(msg.as_str())
    }
}

fn jsn_validate_product_type<'de, D>(raw:D) ->  Result<ProductType, D::Error>
    where D: serde::Deserializer<'de>
{
    match u8::deserialize(raw) {
        Ok(d) => {
            let typ = ProductType::from(d);
            if let ProductType::Unknown(uv) = typ {
                let unexp = Unexpected::Unsigned(uv as u64);
                let exp = ExpectProdTyp{ numbers: vec![
                    ProductType::Item.into(),
                    ProductType::Package.into()
                ]};
                let e = DeserializeError::invalid_value(unexp, &exp) ;
                Err(e)
            } else { Ok(typ) }
        },
        Err(e) => Err(e)
    }
}
fn jsn_serialize_product_type<S>(orig:&ProductType, ser:S)
    -> Result<S::Ok, S::Error> where S: serde::Serializer
{
    let v = orig.clone().into();
    ser.serialize_u8(v)
}
