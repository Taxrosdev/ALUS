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
            if !commit.initramfs.exists(&self.repo.treeup).await? {
                tasks.push(tokio::spawn(Self::download_blob(
                    self.clone(),
                    commit.initramfs.clone(),
                )))
            }
            if !commit.vmlinuz.exists(&self.repo.treeup).await? {
                tasks.push(tokio::spawn(Self::download_blob(
                    self.clone(),
                    commit.vmlinuz.clone(),
                )))
            }
        }

        tasks.push(tokio::spawn(Self::download_tree(
            self.clone(),
            commit.usr_tree,
            metadata_only,
        )));

        for task in tasks {
            task.await.expect("tokio join error")?;
        }

        Ok(())
    }

    #[async_recursion]
    pub async fn download_tree(
        self,
        object_hash: String,
        metadata_only: bool,
    ) -> crate::error::Result<()> {
        let tree = if Tree::exists(&self.repo.treeup, &object_hash).await? {
            Tree::get(&self.repo.treeup, &object_hash).await?
        } else {
            // Download THIS tree
            let _limit = self.resolve_limit.acquire().await.unwrap();

            clone_or_download_tree(
                &self.repo.treeup,
                self.clone_from.clone(),
                &object_hash,
                self.reqwest_downloader.clone(),
            )
            .await?;

            Tree::get(&self.repo.treeup, &object_hash).await?
        };
        self.progress.resolve.inc(1);

        let mut tasks = Vec::new();

        if !metadata_only {
            // Download dependent file
            for file in tree.files {
                if !file.blob.exists(&self.repo.treeup).await? {
                    tasks.push(tokio::spawn(Self::download_blob(self.clone(), file.blob)))
                };
            }
        }

        self.progress.resolve.inc_length(tree.subtrees.len() as u64);

        // Download dependent subtrees
        for subtree in tree.subtrees {
            tasks.push(tokio::spawn(Self::download_tree(
                self.clone(),
                subtree.hash,
                metadata_only,
            )));
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
        if let Some(old_repo) = self.clone_from {
            let success = blob.try_clone(&old_repo.treeup, &self.repo.treeup).await?;

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
        blob.download(&self.repo.treeup, Arc::new(downloader))
            .await?;

        done.store(true, Ordering::Relaxed);
        let _ = poll_handle.await;

        Ok(())
    }
}

/// Tries to clone, or downloads if cannot clone.
async fn clone_or_download_tree(
    repo: &treeup::Repo,
    old_repo: Option<Arc<Repo>>,
    object_hash: &str,
    downloader: Arc<ReqwestDownloader>,
) -> crate::error::Result<()> {
    // Try and clone the existing tree
    if let Some(old_repo) = &old_repo {
        let clone_success = Tree::try_clone(&old_repo.treeup, repo, object_hash).await?;

        if clone_success {
            return Ok(());
        }
    };

    Tree::download(repo, downloader, object_hash).await?;

    Ok(())
}
