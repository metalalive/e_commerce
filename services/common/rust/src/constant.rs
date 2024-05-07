use std::hash::Hash;
use std::str::FromStr;

use crate::error::ProductTypeParseError;

pub mod env_vars {
    pub const SYS_BASEPATH: &str = "SYS_BASE_PATH";
    pub const SERVICE_BASEPATH: &str = "SERVICE_BASE_PATH";
    // relative path starting from app / service home folder
    pub const CFG_FILEPATH: &str = "CONFIG_FILE_PATH";
    pub const EXPECTED_LABELS: [&str; 3] = [
        SYS_BASEPATH,
        SERVICE_BASEPATH,
        CFG_FILEPATH,
    ];
}

// standard library hides the default implementation of the trait `PartialEq`
// somewhere in compiler code, the trait `Hash` seems to prefer the default
// code working with itself, it is needless to implement trait `PartialEq`
// for `ProductType` at here.
#[derive(Debug, Eq, PartialEq, Hash)]
pub enum ProductType {
    Item,
    Package,
    Unknown(u8),
}

impl From<u8> for ProductType {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::Item,
            2 => Self::Package,
            _others => Self::Unknown(value),
        }
    }
}
impl From<ProductType> for u8 {
    fn from(value: ProductType) -> u8 {
        match value {
            ProductType::Unknown(v) => v,
            ProductType::Item => 1,
            ProductType::Package => 2,
        }
    }
}
impl Clone for ProductType {
    fn clone(&self) -> Self {
        match self {
            Self::Item => Self::Item,
            Self::Unknown(v) => Self::Unknown(*v),
            Self::Package => Self::Package,
        }
    }
}
impl FromStr for ProductType {
    type Err = ProductTypeParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.parse::<u8>() {
            Ok(v) => Ok(Self::from(v)),
            Err(e) => Err(ProductTypeParseError(e))
        }
    }
}

pub mod logging {
    use serde::Deserialize;

    #[allow(clippy::upper_case_acronyms)]
    #[derive(Deserialize)]
    pub enum Level {
        TRACE,
        DEBUG,
        INFO,
        WARNING,
        ERROR,
        FATAL,
    }

    #[allow(clippy::upper_case_acronyms)]
    #[derive(Deserialize)]
    #[serde(rename_all = "lowercase")]
    pub enum Destination {
        CONSOLE,
        LOCALFS,
    } // TODO, Fluentd
}
