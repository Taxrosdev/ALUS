pub mod config;

use std::{
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::repo::config::ConfigWrapper;

#[derive(Clone)]
pub struct Repo {
    pub treeup: treeup::Repo,
    pub local_path: PathBuf,
    pub config: ConfigWrapper,
}

impl Repo {
    fn config_path(local_path: &Path) -> PathBuf {
        local_path.join("config")
    }

    pub async fn new(local_path: PathBuf) -> io::Result<Self> {
        let config_path = Self::config_path(&local_path);
        let config = ConfigWrapper::new(config_path).await?;

        let treeup = treeup::Repo {
            objects_path: Arc::new(local_path.join("objects")),
            blobs_path: Arc::new(local_path.join("blobs")),
        };

        Ok(Repo {
            treeup,
            config,
            local_path,
        })
    }
}
