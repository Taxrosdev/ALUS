use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use super::{DownloadKind, Downloader, ReqwestDownloader};
use async_trait::async_trait;
use bytes::{Bytes, BytesMut};

#[derive(Clone)]
pub struct ProgressDownloader {
    client: reqwest::Client,
    objects_base_url: String,
    blobs_base_url: String,
    downloaded: Arc<AtomicU64>,
}

#[async_trait]
impl Downloader for ProgressDownloader {
    async fn fetch(
        &self,
        hash: &str,
        kind: DownloadKind,
    ) -> Result<Bytes, Box<dyn std::error::Error + Send + Sync>> {
        let base_url = match kind {
            DownloadKind::Object => &self.objects_base_url,
            DownloadKind::Blob => &self.blobs_base_url,
        };

        let res = self
            .client
            .get(format!("{}/{}/{}", base_url, &hash[..2], &hash[2..]))
            .send()
            .await?;

        let mut res = res.error_for_status()?;

        let mut raw = BytesMut::new();
        if let Some(len) = res.content_length() {
            raw.reserve(len.try_into()?);
        }

        while let Some(chunk) = res.chunk().await? {
            raw.extend_from_slice(&chunk);
            self.downloaded
                .fetch_add(chunk.len() as u64, Ordering::Relaxed);
        }

        Ok(raw.into())
    }
}

impl ProgressDownloader {
    #[must_use]
    pub fn new(downloaded: Arc<AtomicU64>, objects_base_url: &str, blobs_base_url: &str) -> Self {
        let objects_base_url = objects_base_url.trim_end_matches('/');
        let blobs_base_url = blobs_base_url.trim_end_matches('/');

        Self {
            client: reqwest::Client::new(),
            objects_base_url: objects_base_url.to_string(),
            blobs_base_url: blobs_base_url.to_string(),
            downloaded,
        }
    }

    #[must_use]
    pub fn from_reqwest_downloader(
        reqwest_downloader: ReqwestDownloader,
        downloaded: Arc<AtomicU64>,
    ) -> Self {
        Self {
            client: reqwest_downloader.client,
            objects_base_url: reqwest_downloader.objects_base_url,
            blobs_base_url: reqwest_downloader.blobs_base_url,
            downloaded,
        }
    }
}
