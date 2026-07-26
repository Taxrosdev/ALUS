use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};

/// `StringLike` is a container for `OsString` and regular `String`, automatically converting to
/// `String` when possible.
///
/// Useful for cases involving serialization, which tend to be verbose with `OsString`.
#[derive(Clone, Debug, PartialEq, Eq, Ord, PartialOrd, serde::Deserialize, serde::Serialize)]
pub enum StringLike {
    Str(String),
    OsStr(OsString),
}

impl From<String> for StringLike {
    #[inline]
    fn from(value: String) -> Self {
        Self::Str(value)
    }
}

impl AsRef<OsStr> for StringLike {
    #[inline]
    fn as_ref(&self) -> &OsStr {
        match self {
            StringLike::Str(str) => str.as_ref(),
            StringLike::OsStr(os_str) => os_str,
        }
    }
}

impl AsRef<Path> for StringLike {
    #[inline]
    fn as_ref(&self) -> &Path {
        match self {
            StringLike::Str(str) => str.as_ref(),
            StringLike::OsStr(os_str) => os_str.as_ref(),
        }
    }
}

impl TryFrom<StringLike> for String {
    type Error = OsString;

    #[inline]
    fn try_from(value: StringLike) -> Result<Self, Self::Error> {
        match value {
            StringLike::OsStr(os_str) => os_str.into_string(),
            StringLike::Str(str) => Ok(str),
        }
    }
}

impl From<OsString> for StringLike {
    #[inline]
    fn from(value: OsString) -> Self {
        match value.into_string() {
            Ok(str) => Self::Str(str),
            Err(os_str) => Self::OsStr(os_str),
        }
    }
}

impl StringLike {
    /// Converts a `StringLike` to an owned `PathBuf`.
    #[must_use]
    #[inline]
    pub fn to_path_buf(&self) -> PathBuf {
        match self {
            StringLike::Str(str) => PathBuf::from(str),
            StringLike::OsStr(os_str) => PathBuf::from(os_str),
        }
    }

    /// Converts a `StringLike` to an owned `OsString`.
    #[must_use]
    #[inline]
    pub fn to_os_string(&self) -> OsString {
        match self {
            StringLike::Str(str) => OsString::from(str),
            StringLike::OsStr(os_str) => os_str.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn osstr_to_str() {
        let test_str = OsString::from("test");
        let test = StringLike::from(test_str);

        assert_eq!(test, StringLike::Str("test".to_string()))
    }

    #[test]
    fn to_path_buf() {
        let test = StringLike::Str("test".to_string()).to_path_buf();
        assert_eq!(test, PathBuf::from("test"))
    }

    #[test]
    fn to_os_string() {
        let test = StringLike::Str("test".to_string()).to_os_string();
        assert_eq!(test, OsString::from("test"))
    }
}
