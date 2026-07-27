use async_trait::async_trait;
use bytes::Bytes;
use futures_core::Stream;
use std::{
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use tokio_stream::StreamExt;

use crate::downloader::{DownloadKind, Downloader, ReqwestDownloader};

#[derive(Clone)]
pub struct ProgressDownloader {
    reqwest: Arc<ReqwestDownloader>,
    downloaded: Arc<AtomicU64>,
}

impl ProgressDownloader {
    #[must_use]
    pub fn from_reqwest_downloader(
        reqwest: Arc<ReqwestDownloader>,
        downloaded: Arc<AtomicU64>,
    ) -> Self {
        Self {
            reqwest,
            downloaded,
        }
    }
}

#[async_trait]
impl Downloader for ProgressDownloader {
    async fn fetch(
        &self,
        hash: &str,
        kind: DownloadKind,
    ) -> Result<
        Pin<Box<dyn Stream<Item = Result<Bytes, Box<dyn std::error::Error + Send + Sync>>> + Send>>,
        Box<dyn std::error::Error + Send + Sync>,
    > {
        let stream = self.reqwest.fetch(hash, kind).await?;
        let downloaded = self.downloaded.clone();

        Ok(Box::pin(stream.map(move |r| {
            r.inspect(|chunk| {
                downloaded.fetch_add(chunk.len() as u64, Ordering::Relaxed);
            })
        })))
    }

    async fn get_remote(&self) -> String {
        self.reqwest.get_remote().await
    }
}
