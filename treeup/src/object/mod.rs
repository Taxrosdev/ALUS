use async_trait::async_trait;
use std::{
    io,
    path::{Path, PathBuf},
};
use tokio::fs;
use utils::atomic_rename;

use crate::{
    BlobRef, Repo,
    downloader::{DownloadKind, Downloader},
};

#[async_trait]
pub trait Deployable: Sized {
    async fn create(repo: &Repo, path: &Path) -> io::Result<Self>;
    async fn deploy(&self, repo: &Repo, deploy_path: &Path) -> io::Result<()>;
}

#[async_trait]
pub trait Object: Sized + serde::de::DeserializeOwned + serde::Serialize {
    #[must_use]
    async fn local_path_with_parent(repo: &Repo, hash: &str) -> io::Result<PathBuf> {
        let parent_path = repo.objects_path.join(&hash[..2]);
        fs::create_dir_all(&parent_path).await?;
        Ok(parent_path.join(&hash[2..]))
    }

    #[must_use]
    fn local_path(repo: &Repo, hash: &str) -> PathBuf {
        let parent_path = repo.objects_path.join(&hash[..2]);
        parent_path.join(&hash[2..])
    }

    async fn get(repo: &Repo, hash: &str) -> io::Result<Self> {
        let path = Self::local_path(repo, hash);
        let raw = fs::read_to_string(path).await?;
        Ok(serde_json::from_str(&raw)?)
    }

    fn hash(&self) -> serde_json::Result<String> {
        let raw = serde_json::to_string(self)?;
        Ok(blake3::hash(raw.as_bytes()).to_string())
    }

    async fn exists(repo: &Repo, hash: &str) -> io::Result<bool> {
        let path = Self::local_path(repo, hash);

        Ok(fs::try_exists(&path).await?)
    }

    async fn download(
        repo: &Repo,
        downloader: Box<dyn Downloader>,
        hash: &str,
    ) -> crate::error::Result<Self> {
        let path = Self::local_path_with_parent(repo, hash).await?;
        let tmp_path = path.with_extension(".tmp");

        let raw = downloader
            .fetch(hash, DownloadKind::Object)
            .await
            .map_err(crate::error::Error::DownloaderError)?;

        let calc_hash = blake3::hash(&raw).to_hex().to_string();
        if hash != calc_hash {
            return Err(crate::Error::HashError(hash.to_string(), calc_hash));
        }

        fs::write(&tmp_path, &raw).await?;
        atomic_rename(&tmp_path, &path)?;
        let object = serde_json::from_str(
            std::str::from_utf8(&raw).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?,
        )?;
        Ok(object)
    }

    /// Get bordering dependencies
    fn get_dependencies(&self) -> Dependencies<'_>;
}

pub struct Dependencies<'a> {
    pub objects: Vec<&'a str>,
    pub blobs: Vec<&'a BlobRef>,
}
