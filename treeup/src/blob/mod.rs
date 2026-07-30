//! Blobs are Files stored on disks that are then hard-linked into their final location, this allows
//! for fast and quick IO and tree creation/deploying.

use async_trait::async_trait;
use std::{
    io::{self, Write},
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt,
};
use tokio_stream::StreamExt;
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
    pub async fn local_path_with_parent(&self, repo: &Repo) -> io::Result<PathBuf> {
        let parent_path = repo.blobs_path.join(&self.hash[..2]);
        fs::create_dir_all(&parent_path).await?;
        Ok(parent_path.join(&self.hash[2..]))
    }

    /// Get the path on-disk of this Blob
    ///
    /// Does not try to automatically create the parent directory.
    #[must_use]
    pub fn local_path(&self, repo: &Repo) -> PathBuf {
        let parent_path = repo.blobs_path.join(&self.hash[..2]);
        parent_path.join(&self.hash[2..])
    }

    pub async fn exists(&self, repo: &Repo) -> io::Result<bool> {
        let path = self.local_path(repo);

        fs::try_exists(&path).await
    }

    /// Download the referenced Blob onto disk
    pub async fn download(
        &self,
        repo: &Repo,
        downloader: Arc<dyn Downloader>,
    ) -> crate::error::Result<()> {
        let path = self.local_path_with_parent(repo).await?;
        let tmp_path = path.with_extension("tmp");
        let mut tmp_file = File::create(&tmp_path).await?;

        let stream = downloader
            .fetch(&self.hash, DownloadKind::Blob)
            .await
            .map_err(crate::error::Error::DownloaderError)?;
        let mut stream = Box::pin(stream);

        let mut hasher = blake3::Hasher::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(crate::error::Error::DownloaderError)?;
            hasher.write_all(&chunk)?;
            tmp_file.write_all(&chunk).await?;
        }

        drop(tmp_file);

        let calc_hash = hasher.finalize().to_hex().to_string();
        if self.hash != calc_hash {
            fs::remove_file(tmp_path).await?;
            return Err(crate::Error::HashError(self.hash.clone(), calc_hash));
        }

        atomic_rename(tmp_path, path).await?;
        Ok(())
    }

    /// Tries to clone a Blob from `old_repo` to `new_repo`.
    /// Not to be confused with `clone`.
    ///
    /// Returns whether it was found locally and used.
    pub async fn try_clone(&self, old_repo: &Repo, new_repo: &Repo) -> io::Result<bool> {
        if !self.exists(old_repo).await? {
            return Ok(false);
        }

        let old_path = self.local_path(old_repo);
        let new_path = self.local_path_with_parent(new_repo).await?;

        if fs::hard_link(&old_path, &new_path).await.is_err() {
            // Fallback to copying. Installers are commonly on removable media, and not on the same
            // partition.
            fs::copy(old_path, new_path).await?;
        }

        Ok(true)
    }
}

#[async_trait]
impl Deployable for BlobRef {
    async fn create(repo: &Repo, path: &Path) -> io::Result<Self> {
        let mut hasher = blake3::Hasher::new();
        hasher.update_mmap_rayon(path)?;
        let hash = hasher.finalize().to_string();

        let permissions = Permissions::get(path).await?;

        let blob = BlobRef {
            hash: hash.clone(),
            size: fs::metadata(path).await?.len(),

            uid: permissions.uid,
            gid: permissions.gid,
            mode: permissions.mode,
        };
        let blob_path = blob.local_path_with_parent(repo).await?;

        if !fs::try_exists(&blob_path).await? {
            fs::hard_link(path, blob_path).await?;
        }

        Ok(blob)
    }

    async fn deploy(&self, repo: &Repo, deploy_path: &Path) -> io::Result<()> {
        let path = self.local_path(repo);
        fs::hard_link(path, deploy_path).await?;

        Permissions::deploy(deploy_path.to_path_buf(), self.mode, self.uid, self.gid).await?;

        Ok(())
    }
}
