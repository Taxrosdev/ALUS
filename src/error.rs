#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum Error {
    #[error("io error")]
    Io(#[from] std::io::Error),
    #[error("serialization error")]
    Serde(#[from] serde_json::Error),
    #[error("treeup error")]
    Treeup(#[from] treeup::Error),
    #[error("network error")]
    Reqwest(#[from] reqwest::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
