use std::{io, sync::Arc};
use treeup::{downloader::Downloader, object::Object};

pub mod branch;
pub use branch::Branch;

use crate::{commit::Commit, repo::Repo};

#[derive(Debug, Clone)]
pub enum Pointer {
    Commit { hash: String },
    Branch { branch: Branch },
}

impl Pointer {
    pub async fn resolve_all(
        repo: &Repo,
        pointer: String,
        downloader: Arc<dyn Downloader>,
    ) -> crate::error::Result<Option<Pointer>> {
        if let Some(pointer) = Self::resolve_local(repo, pointer.clone()).await? {
            return Ok(Some(pointer));
        };

        Self::resolve_remote(repo, downloader, pointer).await
    }

    pub async fn resolve_local(repo: &Repo, pointer: String) -> io::Result<Option<Self>> {
        if Commit::exists(&repo.treeup, &pointer).await? {
            return Ok(Some(Pointer::Commit { hash: pointer }));
        }

        if let Some(branch) = Branch::get(repo, pointer).await? {
            return Ok(Some(Pointer::Branch { branch }));
        }

        Ok(None)
    }

    pub async fn resolve_remote(
        repo: &Repo,
        downloader: Arc<dyn Downloader>,
        pointer: String,
    ) -> crate::error::Result<Option<Pointer>> {
        if Commit::download(&repo.treeup, downloader.clone(), &pointer)
            .await
            .is_ok()
        {
            return Ok(Some(Pointer::Commit { hash: pointer }));
        };

        Branch::pull(repo, pointer, downloader)
            .await
            // TODO: Cleanup
            .map(|branch_option| branch_option.map(|branch| Pointer::Branch { branch }))
    }

    pub async fn commit_hash(&self, repo: &Repo) -> io::Result<&str> {
        Ok(match self {
            Pointer::Commit { hash } => hash,
            Pointer::Branch { branch } => branch.commit_hash(repo).await?,
        })
    }

    pub async fn get_commit(&self, repo: &Repo) -> io::Result<Commit> {
        Ok(match self {
            Pointer::Commit { hash } => Commit::get(&repo.treeup, hash).await?,
            Pointer::Branch { branch } => branch.get_commit(repo).await?,
        })
    }

    /// Exactly the same as `Self::get_commit`, expect will pull the commit if it doesn't already
    /// exist
    pub async fn pull_commit(
        &self,
        repo: &Repo,
        downloader: Arc<dyn Downloader>,
    ) -> crate::error::Result<Commit> {
        Ok(match self {
            Pointer::Commit { hash } => Commit::get(&repo.treeup, hash).await?,
            Pointer::Branch { branch } => branch.pull_commit(repo, downloader).await?,
        })
    }
}
