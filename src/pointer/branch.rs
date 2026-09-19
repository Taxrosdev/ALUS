use reqwest::{Client, StatusCode};
use std::{io, sync::Arc};
use tokio::fs;
use treeup::object::Object;
use treeup_core::downloader::{Downloader, ObjectDownloader};

use crate::{Result, commit::Commit, repo::Repo};

#[derive(Debug, Clone)]
pub struct Branch {
    pub _name: String,
    pub target: String,
}

impl Branch {
    pub async fn get(repo: &Repo, branch_name: String) -> io::Result<Option<Self>> {
        let path = repo.local_path.join("branch").join(&branch_name);
        if !path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(path).await?.trim().to_string();

        Ok(Some(Branch {
            _name: branch_name,
            target: content,
        }))
    }

    pub async fn pull(
        repo: &Repo,
        branch_name: String,
        downloader: Arc<impl Downloader>,
    ) -> crate::error::Result<Option<Self>> {
        let remote = downloader.remote();
        let url = format!("{}branch/{}", remote, branch_name);
        let client = Client::new();
        let response = client.get(&url).send().await?;

        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }

        let response = response.error_for_status()?;

        let target = response.text().await?.trim().to_string();

        Self::set(repo, branch_name.clone(), target.clone()).await?;

        Ok(Some(Branch {
            _name: branch_name,
            target,
        }))
    }

    pub async fn set(repo: &Repo, branch_name: String, target: String) -> io::Result<()> {
        let parent_path = repo.local_path.join("branch");
        fs::create_dir_all(&parent_path).await?;
        let path = parent_path.join(&branch_name);

        fs::write(path, target).await?;

        Ok(())
    }

    pub async fn get_commit(&self, repo: &Repo) -> Result<Commit> {
        // TODO: Handle dangling commits better
        Ok(Commit::get(&*repo.object_cas, &hex::decode(&self.target)?).await?)
    }

    /// Exactly the same as `Self::get_commit`, except will pull the commit if it doesn't already
    /// exist
    pub async fn pull_commit(
        &self,
        repo: &Repo,
        downloader: Arc<impl ObjectDownloader>,
    ) -> Result<Commit> {
        let hash = hex::decode(&self.target)?;

        // Download if doesn't exist.
        if !Commit::exists(&*repo.object_cas, &hash).await? {
            Commit::download(&*repo.object_cas, downloader, &hash).await?;
        }

        Ok(Commit::get(&*repo.object_cas, &hash).await?)
    }
}
