use async_trait::async_trait;
use std::{path::Path, path::PathBuf};
use tokio::fs;
use treeup::{
    Tree,
    blob::BlobRef,
    object::{Dependencies, Deployable, Object},
};
use utils::atomic_rename;

use crate::{
    components::{CommittedComponent, Components},
    hooks::Hook,
    logging,
    repo::Repo,
};

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct Commit {
    pub usr_tree: String,

    pub initramfs: BlobRef,
    pub vmlinuz: BlobRef,

    parent_commit: Option<String>,

    components: Vec<CommittedComponent>,
}

#[async_trait]
impl Object for Commit {
    fn get_dependencies(&self) -> Dependencies<'_> {
        Dependencies {
            objects: vec![self.usr_tree.as_str()],
            blobs: vec![&self.initramfs, &self.vmlinuz],
        }
    }
}

impl Commit {
    pub async fn deploy(
        &self,
        repo: &Repo,
        usr_path: PathBuf,
        initramfs_path: &Path,
        vmlinuz_path: &Path,
        sysroot: &Path,
    ) -> crate::error::Result<()> {
        self.deploy_usr(repo, usr_path, sysroot).await?;
        self.deploy_kernel(repo, initramfs_path, vmlinuz_path)
            .await?;

        Ok(())
    }

    pub async fn deploy_kernel(
        &self,
        repo: &Repo,
        initramfs_path: &Path,
        vmlinuz_path: &Path,
    ) -> crate::error::Result<()> {
        logging::debug(format!("Deploying commit {}", self.hash()?));

        // Deploy initramfs/vmlinuz
        logging::log("Deploying new initramfs/vmlinuz...");
        if !fs::try_exists(initramfs_path).await? {
            self.initramfs.deploy(&repo.treeup, initramfs_path).await?
        };
        if !fs::try_exists(vmlinuz_path).await? {
            self.vmlinuz.deploy(&repo.treeup, vmlinuz_path).await?
        };

        Ok(())
    }

    pub async fn deploy_usr(
        &self,
        repo: &Repo,
        usr_path: PathBuf,
        sysroot: &Path,
    ) -> crate::error::Result<()> {
        logging::debug(format!("Deploying commit {}", self.hash()?));

        // Prepare staging usr
        let usr_staging_path = repo.local_path.join("staging");
        if fs::try_exists(&usr_staging_path).await? {
            logging::log("Removing previous staging...");
            logging::warn("A update has likely previously failed. No action required.");

            fs::remove_dir_all(&usr_staging_path).await?;
        }

        // Deploy to staging
        logging::log("Preparing staging usr...");
        logging::debug("Getting usr tree");
        let usr_tree = Tree::get(&repo.treeup, &self.usr_tree).await?;
        logging::debug("Deploying usr tree");
        usr_tree.deploy(&repo.treeup, &usr_staging_path).await?;

        // Components
        logging::debug("Deploying components");
        for component in &self.components {
            // TODO: Manual enable/disable
            if component.trigger.check_trigger(&usr_staging_path).await? {
                component.deploy(repo, &usr_staging_path).await?;
            }
        }

        // Run hooks
        logging::debug("Loading hooks...");
        let hooks = Hook::load_hooks(&usr_staging_path).await?;
        for hook in hooks {
            logging::log(&hook.description);
            hook.run(usr_staging_path.clone(), sysroot).await?;
        }
        logging::debug("Finished hooks");

        // Switch staging <-> usr
        logging::log("Swapping staging and usr");
        atomic_rename(usr_staging_path.clone(), usr_path).await?;
        if fs::try_exists(&usr_staging_path).await? {
            logging::debug("Removing old usr...");
            fs::remove_dir_all(usr_staging_path).await?;
        }

        Ok(())
    }

    pub async fn create(
        repo: &Repo,
        usr_path: &Path,
        initramfs_path: &Path,
        vmlinuz_path: &Path,
        parent_commit: Option<String>,
    ) -> crate::Result<Self> {
        // initramfs/vmlinuz
        logging::debug("Creating blobs for initramfs and vmlinuz");
        let initramfs = BlobRef::create(&repo.treeup, initramfs_path).await?;
        let vmlinuz = BlobRef::create(&repo.treeup, vmlinuz_path).await?;

        logging::debug("Creating components");
        // Components
        let mut components = Vec::new();
        for (id, definition) in
            Components::load_definitions(&usr_path.join("share/alus/components"))
                .await?
                .entries
        {
            let component = definition.commit(id, repo, usr_path).await?;
            components.push(component);
        }

        logging::debug("Creating usr tree");
        let usr_tree = Tree::create(&repo.treeup, usr_path).await?;
        let usr_hash = usr_tree.hash()?;

        let commit = Commit {
            usr_tree: usr_hash,
            initramfs,
            vmlinuz,
            parent_commit,
            components,
        };

        let raw = serde_json::to_string(&commit)?;
        let hash = blake3::hash(raw.as_bytes()).to_string();
        let path = Self::local_path_with_parent(&repo.treeup, &hash).await?;
        fs::write(path, raw).await?;

        Ok(commit)
    }
}
