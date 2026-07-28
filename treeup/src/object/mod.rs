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

    /// Tries to clone an `Object` from `old_repo` to `new_repo`.
    /// Not to be confused with `clone`.
    ///
    /// Returns whether it was found locally and used.
    async fn try_clone(old_repo: &Repo, new_repo: &Repo, hash: &str) -> io::Result<bool> {
        if !Self::exists(old_repo, hash).await? {
            return Ok(false);
        }

        let old_path = Self::local_path(old_repo, hash);
        let new_path = Self::local_path_with_parent(new_repo, hash).await?;

        if fs::hard_link(&old_path, &new_path).await.is_err() {
            // Fallback to copying. Installers are commonly on removable media, and not on the same
            // partition.
            fs::copy(old_path, new_path).await?;
        }

        Ok(true)
    }

    async fn download(
        repo: &Repo,
        downloader: Arc<dyn Downloader>,
        hash: &str,
    ) -> crate::error::Result<()> {
        let path = Self::local_path_with_parent(repo, hash).await?;
        let tmp_path = path.with_extension("tmp");
        let mut tmp_file = File::create(&tmp_path).await?;

        let stream = downloader
            .fetch(hash, DownloadKind::Object)
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
        if hash != calc_hash {
            fs::remove_file(tmp_path).await?;
            return Err(crate::Error::HashError(hash.to_string(), calc_hash));
        }

        atomic_rename(tmp_path, path).await?;
        Ok(())
    }

    /// Get bordering dependencies
    fn get_dependencies(&self) -> Dependencies<'_>;
}

pub struct Dependencies<'a> {
    pub objects: Vec<&'a str>,
    pub blobs: Vec<&'a BlobRef>,
}
