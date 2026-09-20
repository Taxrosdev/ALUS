pub mod config;

use std::{
    io,
    path::{Path, PathBuf},
    sync::Arc,
};
use treeup::object::cas::PackfileCAS;

use crate::repo::config::ConfigWrapper;

#[derive(Clone)]
pub struct Repo {
    pub object_cas: Arc<PackfileCAS>,
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
            object_cas: Arc::new(PackfileCAS::create(local_path.join("objects"), 4096).await?),
            blobs_path: local_path.join("blobs"),
            config,
            local_path,
        })
    }
}
