use serde::Deserialize;
use std::result::Result;

struct ExpectedApCode<'a>(u8, &'a str);

impl<'a> serde::de::Expected for ExpectedApCode<'a> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        let msg = format!("expect ap-code: {}, label:{}", self.0, self.1);
        formatter.write_str(msg.as_str())
    }
}

pub fn jsn_validate_ap_code<'de, D>(
    raw: D,
    quota_ap_code: u8,
    app_label: &str,
) -> Result<u8, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let val = u8::deserialize(raw)?;
    if val == quota_ap_code {
        Ok(val)
    } else {
        let unexp = serde::de::Unexpected::Unsigned(val as u64);
        let exp = ExpectedApCode(quota_ap_code, app_label);
        Err(serde::de::Error::invalid_value(unexp, &exp))
    }
}

struct ExpectedQuotaMatCode {
    max_: u8,
    min_: u8,
}

impl serde::de::Expected for ExpectedQuotaMatCode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        let msg = format!("valid range: {}-{}", self.min_, self.max_);
        formatter.write_str(msg.as_str())
    }
}

pub fn quota_matcode_deserialize_error<'de, D>(given: u8, valid: (u8, u8)) -> D::Error
where
    D: serde::Deserializer<'de>,
{
    let unexp = serde::de::Unexpected::Unsigned(given as u64);
    let exp = ExpectedQuotaMatCode {
        max_: valid.1,
        min_: valid.0,
    };
    serde::de::Error::invalid_value(unexp, &exp)
}
