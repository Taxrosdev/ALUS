use async_trait::async_trait;
use std::{io, path::Path};
use utils::StringLike;

use crate::{BlobRef, Repo, object::Deployable};

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
pub struct File {
    pub name: StringLike,
    pub blob: BlobRef,
}

#[async_trait]
impl Deployable for File {
    async fn create(repo: &Repo, path: &Path) -> io::Result<Self> {
        Ok(File {
            name: path
                .file_name()
                .ok_or(io::ErrorKind::InvalidFilename)?
                .to_os_string()
                .into(),
            blob: BlobRef::create(repo, path).await?,
        })
    }

    async fn deploy(&self, repo: &Repo, deploy_parent_path: &Path) -> io::Result<()> {
        let deploy_path = deploy_parent_path.join(self.name.to_os_string());

        self.blob.deploy(repo, &deploy_path).await?;

        Ok(())
    }
}
