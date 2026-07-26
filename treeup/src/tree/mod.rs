use async_trait::async_trait;
use std::{io, path::Path};
use tokio::fs;
use utils::StringLike;

use crate::utils::permissions::Permissions;
use crate::{
    object::{Dependencies, Deployable, Object},
    repo::Repo,
};
mod file;
pub use file::File;
mod symlink;
pub use symlink::Symlink;

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
pub struct Tree {
    pub subtrees: Vec<SubtreeRef>,
    pub files: Vec<File>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub symlinks: Vec<Symlink>,

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

#[async_trait]
impl Object for Tree {
    fn get_dependencies(&self) -> Dependencies<'_> {
        Dependencies {
            objects: self
                .subtrees
                .iter()
                .map(|tree| tree.hash.as_str())
                .collect(),
            blobs: self.files.iter().map(|file| &file.blob).collect(),
        }
    }
}

#[async_trait]
impl Deployable for Tree {
    async fn create(repo: &Repo, path: &Path) -> io::Result<Self> {
        let permissions = Permissions::get(path).await?;

        let mut subtrees = Vec::new();
        let mut files = Vec::new();
        let mut symlinks = Vec::new();

        let mut read_dir = fs::read_dir(path).await?;
        while let Some(entry) = read_dir.next_entry().await? {
            let filetype = entry.file_type().await?;
            let filepath = entry.path();

            if filetype.is_dir() {
                let subtree = Tree::create(repo, &filepath).await?;
                let raw = serde_json::to_string(&subtree)?;
                let hash = blake3::hash(raw.as_bytes()).to_string();

                subtrees.push(SubtreeRef {
                    hash,
                    name: filepath
                        .file_name()
                        .ok_or(io::ErrorKind::InvalidFilename)?
                        .to_os_string()
                        .into(),
                });
            } else if filetype.is_symlink() {
                let symlink = Symlink::create(repo, &filepath).await?;
                symlinks.push(symlink);
            } else if filetype.is_file() {
                let file = File::create(repo, &filepath).await?;
                files.push(file);
            }
        }

        let tree = Tree {
            subtrees,
            files,
            symlinks,
            uid: permissions.uid,
            gid: permissions.gid,
            mode: permissions.mode,
        };

        let raw = serde_json::to_string(&tree)?;
        let hash = blake3::hash(raw.as_bytes()).to_string();
        let object_path = Self::local_path_with_parent(repo, &hash).await?;
        fs::write(object_path, raw).await?;

        Ok(tree)
    }

    async fn deploy(&self, repo: &Repo, deploy_path: &Path) -> io::Result<()> {
        fs::create_dir_all(deploy_path).await?;
        Permissions::deploy(deploy_path.to_path_buf(), self.mode, self.uid, self.gid).await?;

        let mut tasks = Vec::new();

        // Subtrees
        for subtree in &self.subtrees {
            let path = deploy_path.join(subtree.name.to_path_buf());
            let repo = repo.clone();
            let hash = subtree.hash.clone();
            tasks.push(tokio::spawn(async move {
                let tree = Tree::get(&repo, &hash).await?;
                tree.deploy(&repo, &path).await
            }));
        }

        // Files
        for file in &self.files {
            let repo = repo.clone();
            let file = file.clone();
            let deploy_path = deploy_path.to_path_buf();
            tasks.push(tokio::spawn(async move {
                file.deploy(&repo, &deploy_path).await
            }));
        }

        // Symlinks
        for symlink in &self.symlinks {
            let repo = repo.clone();
            let symlink = symlink.clone();
            let deploy_path = deploy_path.to_path_buf();
            tasks.push(tokio::spawn(async move {
                symlink.deploy(&repo, &deploy_path).await
            }));
        }

        for task in tasks {
            task.await.expect("tokio join error on deploy")?;
        }

        Ok(())
    }
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
pub struct SubtreeRef {
    pub hash: String,
    pub name: StringLike,
}
