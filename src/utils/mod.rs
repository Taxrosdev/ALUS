mod stringlike;
use std::{env::var_os, ffi::OsStr, string::FromUtf8Error};

pub use stringlike::*;

mod atomic;
pub use atomic::*;

mod endswithslash;
pub use endswithslash::*;

#[inline]
/// Tries to parse a environment variable
pub fn parse_var(key: impl AsRef<OsStr>) -> Result<Option<String>, FromUtf8Error> {
    match var_os(&key) {
        Some(val) => String::from_utf8(val.as_encoded_bytes().to_vec()).map(Some),
        None => Ok(None),
    }
}
