use async_recursion::async_recursion;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::{sync::Semaphore, time::sleep};
use treeup::{
    Tree,
    blob::BlobRef,
    downloader::{ProgressDownloader, ReqwestDownloader},
    object::Object,
};
use treeup_core::object_cas::ObjectCAS;

use crate::{commit::Commit, logging::Progress, repo::Repo};

#[derive(Clone)]
pub struct TreePuller {
    repo: Arc<Repo>,
    resolve_limit: Arc<Semaphore>,
    blob_limit: Arc<Semaphore>,
    reqwest_downloader: Arc<ReqwestDownloader>,

    progress: Progress,
    clone_from: Option<Arc<Repo>>,
}

impl TreePuller {
    pub fn new(
        repo: Arc<Repo>,
        reqwest_downloader: Arc<ReqwestDownloader>,
        progress: Progress,
        clone_from: Option<Arc<Repo>>,
    ) -> Self {
        Self {
            repo: repo.clone(),
            reqwest_downloader,
            resolve_limit: Arc::new(Semaphore::new(repo.config.resolve_limit() as usize)),
            blob_limit: Arc::new(Semaphore::new(repo.config.download_limit() as usize)),

            progress,
            clone_from,
        }
    }

    pub async fn download_commit(
        self,
        commit: Commit,
        metadata_only: bool,
    ) -> crate::error::Result<()> {
        let mut tasks = Vec::new();

        if !metadata_only {
            if !commit.initramfs.exists(&self.repo.blobs_path).await? {
                tasks.push(tokio::spawn(Self::download_blob(
                    self.clone(),
                    commit.initramfs.clone(),
                )))
            }
            if !commit.vmlinuz.exists(&self.repo.blobs_path).await? {
                tasks.push(tokio::spawn(Self::download_blob(
                    self.clone(),
                    commit.vmlinuz.clone(),
                )))
            }
        }

        let usr_hash = hex::decode(commit.usr_tree)?;
        tasks.push(tokio::spawn(async move {
            Self::download_tree(self.clone(), &usr_hash, metadata_only).await
        }));

        for task in tasks {
            task.await.expect("tokio join error")?;
        }

        Ok(())
    }

    #[async_recursion]
    pub async fn download_tree(
        self,
        object_hash: &[u8],
        metadata_only: bool,
    ) -> crate::error::Result<()> {
        let tree = if Tree::exists(&*self.repo.object_cas, object_hash).await? {
            Tree::get(&*self.repo.object_cas, object_hash).await?
        } else {
            // Download THIS tree
            let _limit = self.resolve_limit.acquire().await.unwrap();

            let clone_from = self.clone_from.clone();
            clone_or_download_tree(
                &*self.repo.object_cas,
                clone_from.map(|repo| repo.object_cas.clone()),
                object_hash,
                self.reqwest_downloader.clone(),
            )
            .await?;

            Tree::get(&*self.repo.object_cas, object_hash).await?
        };
        self.progress.resolve.inc(1);

        let mut tasks = Vec::new();

        if !metadata_only {
            // Download dependent file
            for file in tree.files {
                if !file.blob.exists(&self.repo.blobs_path).await? {
                    tasks.push(tokio::spawn(Self::download_blob(self.clone(), file.blob)))
                };
            }
        }

        self.progress.resolve.inc_length(tree.subtrees.len() as u64);

        // Download dependent subtrees
        for subtree in tree.subtrees {
            let tree_hash = hex::decode(subtree.hash)?;
            let tree = self.clone();
            tasks.push(tokio::spawn(async move {
                Self::download_tree(tree, &tree_hash, metadata_only).await
            }));
        }

        for task in tasks {
            task.await.expect("tokio join error")?;
        }

        Ok(())
    }

    async fn download_blob(self, blob: BlobRef) -> crate::error::Result<()> {
        self.progress.download.inc_length(blob.size);

        let _permit = self.blob_limit.acquire().await.unwrap();

        let downloaded = Arc::new(AtomicU64::new(0));
        let done = Arc::new(AtomicBool::new(false));

        // Try clone, if not continue
        if let Some(old_cas) = self.clone_from {
            let success = blob
                .try_clone(&old_cas.blobs_path, &self.repo.blobs_path)
                .await?;

            if success {
                self.progress.download.inc(blob.size);
                return Ok(());
            }
        }

        let poll_handle = {
            let pb = self.progress.download.clone();
            let downloaded = downloaded.clone();
            let done = done.clone();
            tokio::spawn(async move {
                let dur = Duration::from_millis(100);
                let mut prev = 0u64;
                while !done.load(Ordering::Relaxed) {
                    let now = downloaded.load(Ordering::Relaxed);
                    let delta = now.saturating_sub(prev);
                    pb.inc(delta);
                    prev = now;
                    sleep(dur).await;
                }
                let now = downloaded.load(Ordering::Relaxed);
                let delta = now.saturating_sub(prev);
                pb.inc(delta);
            })
        };

        // Actually download
        let downloader = ProgressDownloader::from_downloader(self.reqwest_downloader, downloaded);
        blob.download(&self.repo.blobs_path, Arc::new(downloader))
            .await?;

        done.store(true, Ordering::Relaxed);
        let _ = poll_handle.await;

        Ok(())
    }
}

/// Tries to clone, or downloads if cannot clone.
async fn clone_or_download_tree(
    cas: &impl ObjectCAS,
    old_cas: Option<Arc<impl ObjectCAS>>,
    object_hash: &[u8],
    downloader: Arc<ReqwestDownloader>,
) -> crate::error::Result<()> {
    // Try and clone the existing tree
    if let Some(old_cas) = &old_cas {
        let clone_success = Tree::try_clone(&**old_cas, cas, object_hash).await?;

        if clone_success {
            return Ok(());
        }
    };

    Tree::download(cas, downloader, object_hash).await?;

    Ok(())
}
