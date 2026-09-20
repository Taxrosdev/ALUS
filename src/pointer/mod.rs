use std::{io, sync::Arc};
use treeup::downloader::ObjectDownloader;
use treeup::object::Object;

pub mod branch;
pub use branch::Branch;

use crate::{commit::Commit, repo::Repo};

#[derive(Debug, Clone)]
pub enum Pointer {
    Commit { hash: Vec<u8> },
    Branch { branch: Branch },
}

impl Pointer {
    pub async fn resolve_all(
        repo: &Repo,
        pointer: String,
        downloader: Arc<impl ObjectDownloader>,
    ) -> crate::error::Result<Option<Pointer>> {
        if let Some(pointer) = Self::resolve_local(repo, pointer.clone()).await? {
            return Ok(Some(pointer));
        };

        Self::resolve_remote(repo, downloader, pointer).await
    }

    pub async fn resolve_local(repo: &Repo, pointer: String) -> io::Result<Option<Self>> {
        if let Ok(hash) = hex::decode(&pointer)
            && Commit::exists(&*repo.object_cas, &hash).await?
        {
            return Ok(Some(Pointer::Commit { hash }));
        }

        if let Some(branch) = Branch::get(repo, pointer).await? {
            return Ok(Some(Pointer::Branch { branch }));
        }

        Ok(None)
    }

    pub async fn resolve_remote(
        repo: &Repo,
        downloader: Arc<impl ObjectDownloader>,
        pointer: String,
    ) -> crate::error::Result<Option<Pointer>> {
        if let Ok(hash) = hex::decode(&pointer)
            && Commit::download(&*repo.object_cas, downloader.clone(), &hash)
                .await
                .is_ok()
        {
            return Ok(Some(Pointer::Commit { hash }));
        };

        Branch::pull(repo, pointer, downloader)
            .await
            // TODO: Cleanup
            .map(|branch_option| branch_option.map(|branch| Pointer::Branch { branch }))
    }

    pub async fn get_commit(&self, repo: &Repo) -> crate::Result<Commit> {
        Ok(match self {
            Pointer::Commit { hash } => Commit::get(&*repo.object_cas, hash).await?,
            Pointer::Branch { branch } => branch.get_commit(repo).await?,
        })
    }

    /// Exactly the same as `Self::get_commit`, except will pull the commit if it doesn't already
    /// exist
    pub async fn pull_commit(
        &self,
        repo: &Repo,
        downloader: Arc<impl ObjectDownloader>,
    ) -> crate::error::Result<Commit> {
        Ok(match self {
            Pointer::Commit { hash } => Commit::get(&*repo.object_cas, hash).await?,
            Pointer::Branch { branch } => branch.pull_commit(repo, downloader).await?,
        })
    }
}
