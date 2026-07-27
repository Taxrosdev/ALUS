use async_trait::async_trait;
use bytes::Bytes;
use futures_core::Stream;
use std::pin::Pin;

mod reqwest;
pub use reqwest::*;
mod progress;
pub use progress::*;

#[async_trait]
/// Ulitity to Fetch from a remote `Repo`
pub trait Downloader: Send + Sync {
    async fn fetch(
        &self,
        hash: &str,
        kind: DownloadKind,
    ) -> Result<
        Pin<Box<dyn Stream<Item = Result<Bytes, Box<dyn std::error::Error + Send + Sync>>> + Send>>,
        Box<dyn std::error::Error + Send + Sync>,
    >;

    async fn get_remote(&self) -> String;
}

#[derive(Copy, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum DownloadKind {
    Object,
    Blob,
}
