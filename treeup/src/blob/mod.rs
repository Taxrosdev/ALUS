//! Blobs are Files stored on disks that are then hard-linked into their final location, this allows
//! for fast and quick IO and tree creation/deploying.

use async_trait::async_trait;
use std::{
    io,
    path::{Path, PathBuf},
};
use tokio::fs;
use utils::atomic_rename;

use crate::utils::permissions::Permissions;
use crate::{
    downloader::{DownloadKind, Downloader},
    object::Deployable,
    repo::Repo,
};

/// A reference to a Blob, containing all information that may be required for deploying.
#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
pub struct BlobRef {
    hash: String,
    pub size: u64,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    mode: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    uid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    gid: Option<u32>,
}

impl BlobRef {
    /// Get the path on-disk of this Blob
    pub async fn local_path(repo: &Repo, hash: &str) -> io::Result<PathBuf> {
        let parent_path = repo.blobs_path.join(&hash[..2]);
        fs::create_dir_all(&parent_path).await?;
        Ok(parent_path.join(&hash[2..]))
    }

    /// Get the path on-disk of this Blob
    ///
    /// Does not try to automatically create the parent directory.
    #[must_use]
    pub fn parentless_local_path(repo: &Repo, hash: &str) -> PathBuf {
        let parent_path = repo.blobs_path.join(&hash[..2]);
        parent_path.join(&hash[2..])
    }

    pub async fn exists(&self, repo: &Repo) -> io::Result<bool> {
        let path = Self::parentless_local_path(repo, &self.hash);

        fs::try_exists(&path).await
    }

    /// Download the referenced Blob onto disk
    pub async fn download(
        &self,
        repo: &Repo,
        downloader: Box<dyn Downloader>,
    ) -> crate::error::Result<()> {
        let path = Self::local_path(repo, &self.hash).await?;
        let tmp_path = Self::local_path(repo, &self.hash)
            .await?
            .with_extension(".tmp");

        let raw = downloader
            .fetch(&self.hash, DownloadKind::Blob)
            .await
            .map_err(crate::error::Error::DownloaderError)?;

        let calc_hash = blake3::hash(&raw).to_hex().to_string();
        if self.hash != calc_hash {
            return Err(crate::Error::HashError(self.hash.clone(), calc_hash));
        }

        fs::write(&tmp_path, raw).await?;
        atomic_rename(&tmp_path, &path)?;
        Ok(())
    }
}

#[async_trait]
impl Deployable for BlobRef {
    async fn create(repo: &Repo, path: &Path) -> io::Result<Self> {
        let mut hasher = blake3::Hasher::new();
        hasher.update_mmap_rayon(path)?;
        let hash = hasher.finalize().to_string();

        let blob_path = Self::local_path(repo, &hash).await?;

        if !fs::try_exists(&blob_path).await? {
            fs::hard_link(path, blob_path).await?;
        }

        let permissions = Permissions::get(path).await?;

        Ok(BlobRef {
            hash: hash.clone(),
            size: fs::metadata(path).await?.len(),

            uid: permissions.uid,
            gid: permissions.gid,
            mode: permissions.mode,
        })
    }

    async fn deploy(&self, repo: &Repo, deploy_path: &Path) -> io::Result<()> {
        let path = Self::local_path(repo, &self.hash).await?;
        fs::hard_link(path, deploy_path).await?;

        Permissions::deploy(deploy_path, self.mode, self.uid, self.gid).await?;

        Ok(())
    }
}
