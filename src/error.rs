#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum Error {
    #[error("generic io error")]
    Io(#[from] std::io::Error),
    #[error("json serialization error")]
    Json(#[from] serde_json::Error),
    #[error("toml serialization error")]
    Toml(#[from] toml::de::Error),
    #[error("treeup object error")]
    TreeupObject(#[from] treeup::object::error::Error),
    #[error("treeup blob error")]
    TreeupBlob(#[from] treeup::blob::error::Error),
    #[error("network error")]
    Reqwest(#[from] reqwest::Error),
    #[error("hook exited nonzero")]
    HookExit(i32),
    #[error("hook container crashed unexpectedly")]
    HookRunner,
    #[error("hash not hex encoded")]
    Hex(#[from] hex::FromHexError),
}

pub type Result<T> = std::result::Result<T, Error>;
