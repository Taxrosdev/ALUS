use std::borrow::Borrow;

pub struct EndsWithSlash(String);

impl From<String> for EndsWithSlash {
    fn from(mut value: String) -> Self {
        if !value.ends_with('/') {
            value.push('/');
        }

        EndsWithSlash(value)
    }
}

impl From<EndsWithSlash> for String {
    fn from(value: EndsWithSlash) -> Self {
        value.0
    }
}

impl AsRef<String> for EndsWithSlash {
    fn as_ref(&self) -> &String {
        &self.0
    }
}

impl AsRef<str> for EndsWithSlash {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Borrow<String> for EndsWithSlash {
    fn borrow(&self) -> &String {
        &self.0
    }
}
impl Borrow<str> for EndsWithSlash {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl std::ops::Deref for EndsWithSlash {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl serde::Serialize for EndsWithSlash {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> serde::Deserialize<'de> for EndsWithSlash {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        String::deserialize(deserializer).map(EndsWithSlash::from)
    }
}
