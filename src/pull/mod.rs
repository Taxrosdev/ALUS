use async_recursion::async_recursion;
use futures_util::{StreamExt, TryStreamExt, stream};
use indicatif::{ProgressBar, ProgressStyle};
use std::{
    collections::HashSet,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::{sync::Semaphore, time::sleep};
use treeup::{
    Tree,
    blob::BlobRef,
    downloader::{PackfileDownloader, ProgressDownloader, ReqwestDownloader},
    object::Object,
    object_cas::ObjectCAS,
};

use crate::{commit::Commit, logging::Progress, repo::Repo};

#[derive(Clone)]
pub struct Puller {
    repo: Arc<Repo>,
    clone_from: Option<Arc<Repo>>,
    blob_downloader: Arc<ReqwestDownloader>,
    object_downloader: Arc<PackfileDownloader>,
}

impl Puller {
    pub fn new(
        repo: Arc<Repo>,
        reqwest_downloader: Arc<ReqwestDownloader>,
        packfile_downloader: Arc<PackfileDownloader>,
        clone_from: Option<Arc<Repo>>,
    ) -> Self {
        Puller {
            repo,
            clone_from,
            blob_downloader: reqwest_downloader,
            object_downloader: packfile_downloader,
        }
    }

    pub async fn download_commit(
        self,
        commit: Commit,
        metadata_only: bool,
    ) -> crate::error::Result<()> {
        let mut blobs = Vec::new();

        // Progress Bars
        let progress = Progress::new();

        if !metadata_only {
            if !commit.initramfs.exists(&self.repo.blobs_path).await? {
                blobs.push(commit.initramfs.clone())
            }
            if !commit.vmlinuz.exists(&self.repo.blobs_path).await? {
                blobs.push(commit.vmlinuz.clone())
            }
        }

        let usr_hash = hex::decode(commit.usr_tree)?;
        blobs.extend(
            Self::download_tree_metadata_recursively(
                self.clone(),
                usr_hash,
                Arc::new(Semaphore::new(self.repo.config.resolve_limit() as usize)),
            )
            .await?,
        );
        progress.finish_resolving();

        // Download blobs
        if !metadata_only {
            let blobs_path = self.repo.blobs_path.clone();
            let missing_blobs = stream::iter(blobs)
                .map(async |blob| (blob.clone(), blob.exists(&blobs_path).await))
                .buffer_unordered(32) // Not really tuneable
                .filter_map(async |(blob, exists)| match exists {
                    Ok(true) => None,
                    Ok(false) => Some(Ok(blob)),
                    Err(e) => Some(Err(e)),
                })
                .try_collect::<Vec<BlobRef>>()
                .await?;

            // Download ProgressBar
            let total_size: u64 = missing_blobs.iter().map(|blob| blob.size).sum();
            let download_progress = ProgressBar::new(total_size)
                    .with_message("Downloading...")
                    .with_style(
                        ProgressStyle::with_template(
                            "{spinner:.cyan} {msg} {bytes}/{total_bytes} {bar:30.cyan/blue} [{bytes_per_sec}] {eta}",
                        )
                        .expect("Progress bar error")
                    );
            download_progress.set_position(0);
            progress.multi_progress.add(download_progress.clone());

            let downloaded = Arc::new(AtomicU64::new(0));
            let download_poll = {
                let downloaded = downloaded.clone();
                let download_progress = download_progress.clone();
                tokio::spawn(async move {
                    let duration = Duration::from_millis(10);
                    let downloaded = downloaded;
                    loop {
                        sleep(duration).await;
                        let downloaded_amt = downloaded.load(std::sync::atomic::Ordering::Relaxed);
                        download_progress.set_position(downloaded_amt);
                    }
                })
            };

            stream::iter(missing_blobs)
                .map(|blob| Self::download_blob(self.clone(), blob, downloaded.clone()))
                .buffer_unordered(self.repo.config.download_limit() as usize)
                .try_collect::<Vec<_>>()
                .await?;

            download_poll.abort();
            let _ = download_poll.await;
            download_progress.finish_with_message("Downloaded");
        }

        Ok(())
    }

    async fn download_tree(&self, object_hash: &[u8]) -> crate::error::Result<Tree> {
        if Tree::exists(&*self.repo.object_cas, object_hash).await? {
            Ok(Tree::get(&*self.repo.object_cas, object_hash).await?)
        } else {
            // Download THIS tree
            let clone_from = self.clone_from.clone();
            clone_or_download_tree(
                &*self.repo.object_cas,
                clone_from.map(|repo| repo.object_cas.clone()),
                object_hash,
                self.object_downloader.clone(),
            )
            .await?;

            Ok(Tree::get(&*self.repo.object_cas, object_hash).await?)
        }
    }

    #[async_recursion]
    pub async fn download_tree_metadata_recursively(
        self,
        object_hash: Vec<u8>,
        semaphore: Arc<Semaphore>,
    ) -> crate::error::Result<HashSet<BlobRef>> {
        let permit = semaphore.acquire().await.unwrap();
        let tree = self.download_tree(&object_hash).await?;
        drop(permit);

        let mut blobs = tree
            .files
            .iter()
            .map(|f| f.blob.clone())
            .collect::<HashSet<_>>();

        // Download dependent subtrees
        let mut tasks = Vec::new();
        for subtree in tree.subtrees {
            let tree_hash = hex::decode(subtree.hash)?;
            let semaphore = semaphore.clone();
            let puller = self.clone();
            tasks.push(tokio::spawn(Self::download_tree_metadata_recursively(
                puller.clone(),
                tree_hash,
                semaphore.clone(),
            )));
        }

        for task in tasks {
            let new_blobs = task.await.expect("tokio join error")?;
            blobs.extend(new_blobs)
        }

        Ok(blobs)
    }

    async fn download_blob(
        self,
        blob: BlobRef,
        downloaded: Arc<AtomicU64>,
    ) -> crate::error::Result<()> {
        // Try clone, if not continue
        if let Some(old_cas) = self.clone_from.clone()
            && blob
                .try_clone(&old_cas.blobs_path, &self.repo.blobs_path)
                .await
                .is_ok()
        {
            downloaded.fetch_add(blob.size, Ordering::Relaxed);
        } else {
            // Actually download
            let downloader =
                ProgressDownloader::from_downloader(self.blob_downloader.clone(), downloaded);
            blob.download(&self.repo.blobs_path, Arc::new(downloader))
                .await?;
        }

        Ok(())
    }
}

/// Tries to clone, or downloads if cannot clone.
async fn clone_or_download_tree(
    cas: &impl ObjectCAS,
    old_cas: Option<Arc<impl ObjectCAS>>,
    object_hash: &[u8],
    downloader: Arc<PackfileDownloader>,
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
