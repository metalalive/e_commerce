mod keystore;

pub use keystore::{AbstractAuthKeystore, AppAuthKeystore};

use std::io::Error as IoError;

#[derive(Debug)]
pub enum AppAuthError {
    KeyStoreUri(String),
    KeyStoreServer(u16),
    HttpInvalidSetup(String),
    HttpParse(String),
    HttpTimeout(String),
    HttpAbort(String),
    HttpDataCorruption(String),
    HttpOther(hyper::Error),
    NetworkIO(IoError),
    AppParse(String),
    NotSupport,
}
