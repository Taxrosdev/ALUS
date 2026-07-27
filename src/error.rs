use std::process::ExitStatus;

#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum Error {
    #[error("io error")]
    Io(#[from] std::io::Error),
    #[error("json serialization error")]
    Json(#[from] serde_json::Error),
    #[error("toml serialization error")]
    Toml(#[from] toml::de::Error),
    #[error("treeup error")]
    Treeup(#[from] treeup::Error),
    #[error("network error")]
    Reqwest(#[from] reqwest::Error),
    #[error("hook exited nonzero")]
    HookExit(ExitStatus),
}

pub type Result<T> = std::result::Result<T, Error>;
