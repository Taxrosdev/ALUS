use std::path::PathBuf;

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
    #[error("environment variable is not utf8")]
    InvalidEnvironmentVariable(#[from] std::string::FromUtf8Error),
    #[error("File extension for file {0} is not toml")]
    TomlExtensionError(PathBuf),
    #[error("Both Insert and Extract are defined for {0}")]
    InsertExtractError(PathBuf),
    #[error("Neither Insert or Extract are defined for {0}")]
    NoInsertExtractError(PathBuf),
    #[error("Pointer {0} could not be found")]
    PointerNotFound(String),
    #[error("No Remote Configured.")]
    NoRemote,
    #[error("No current branch configured, have you not switch'ed previously?")]
    NoCurrent,
}

pub type Result<T> = std::result::Result<T, Error>;
