#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("io error")]
    IoError(#[from] std::io::Error),
    #[error("downloader error")]
    /// Guaranteed to be `reqwest::Error` with `ReqwestDownloader` (default).
    DownloaderError(#[from] Box<dyn std::error::Error + Send + Sync>),
    #[error("hash error")]
    /// Expected, Received
    HashError(String, String),
    #[error("serialization error")]
    SerdeError(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
