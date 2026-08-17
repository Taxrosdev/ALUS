pub mod config;

use std::{
    io,
    path::{Path, PathBuf},
    sync::Arc,
};
use treeup::object::cas::BasicFS;

use crate::repo::config::ConfigWrapper;

#[derive(Clone)]
pub struct Repo {
    pub object_cas: Arc<BasicFS>,
    pub blobs_path: PathBuf,
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

        Ok(Repo {
            object_cas: Arc::new(BasicFS::create(local_path.join("objects")).await?),
            blobs_path: local_path.join("objects"),
            config,
            local_path,
        })
    }
}
