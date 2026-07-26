use async_recursion::async_recursion;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use tokio::{sync::Semaphore, time::sleep};
use treeup::{
    BlobRef, Tree,
    downloader::{ProgressDownloader, ReqwestDownloader},
    object::Object,
};

use crate::{Progress, commit::Commit, repo::Repo};

#[derive(Clone)]
pub struct TreePuller {
    repo: Arc<Repo>,
    resolve_limit: Arc<Semaphore>,
    blob_limit: Arc<Semaphore>,
    reqwest_downloader: Box<ReqwestDownloader>,

    progress: Progress,
}

impl TreePuller {
    pub fn new(
        repo: Arc<Repo>,
        reqwest_downloader: Box<ReqwestDownloader>,
        progress: Progress,
    ) -> Self {
        Self {
            repo: repo.clone(),
            reqwest_downloader,
            resolve_limit: Arc::new(Semaphore::new(repo.config.resolve_limit() as usize)),
            blob_limit: Arc::new(Semaphore::new(repo.config.download_limit() as usize)),

            progress,
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
            Tree::download(
                &self.repo.treeup,
                self.reqwest_downloader.clone(),
                &object_hash,
            )
            .await?
        };
        self.progress.resolve.inc(1);

        let mut tasks = Vec::new();

        if !metadata_only {
            // Download dependent file
            for file in tree.files {
                tasks.push(tokio::spawn(Self::download_blob(self.clone(), file.blob)))
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
        if blob.exists(&self.repo.treeup).await? {
            return Ok(());
        };

        self.progress.download.inc_length(blob.size);

        let _permit = self.blob_limit.acquire().await.unwrap();

        let downloaded = Arc::new(AtomicU64::new(0));
        let done = Arc::new(AtomicBool::new(false));

        let poll_handle = {
            let pb = self.progress.download.clone();
            let downloaded = downloaded.clone();
            let done = done.clone();
            tokio::spawn(async move {
                let dur = std::time::Duration::from_millis(100);
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
        let downloader =
            ProgressDownloader::from_reqwest_downloader(*self.reqwest_downloader, downloaded);
        blob.download(&self.repo.treeup, Box::new(downloader))
            .await?;

        done.store(true, Ordering::Relaxed);
        let _ = poll_handle.await;

        Ok(())
    }
}
