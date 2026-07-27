use std::{
    io,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::fs;
use utils::EndsWithSlash;

pub struct Repo {
    pub treeup: treeup::Repo,
    pub local_path: PathBuf,
    pub config: ConfigWrapper,
}

pub struct ConfigWrapper {
    config: Config,
    path: PathBuf,
}

#[derive(serde::Deserialize, serde::Serialize, Default)]
pub struct Config {
    pub remote: Option<EndsWithSlash>,
    pub current_branch: Option<String>,
    pub download_limit: Option<u64>,
    pub resolve_limit: Option<u64>,
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

impl ConfigWrapper {
    pub fn set_remote(&mut self, remote_url: String) -> io::Result<()> {
        self.config.remote = Some(remote_url.into());

        self.write()?;
        Ok(())
    }

    pub fn set_download_limit(&mut self, limit: Option<u64>) -> io::Result<()> {
        self.config.download_limit = limit;

        self.write()?;
        Ok(())
    }

    pub fn set_resolve_limit(&mut self, limit: Option<u64>) -> io::Result<()> {
        self.config.resolve_limit = limit;

        self.write()?;
        Ok(())
    }

    pub fn download_limit(&self) -> u64 {
        self.config.download_limit.unwrap_or(5)
    }

    pub fn resolve_limit(&self) -> u64 {
        self.config.resolve_limit.unwrap_or(5)
    }

    pub fn remote(&self) -> Option<&EndsWithSlash> {
        self.config.remote.as_ref()
    }

    fn write(&self) -> io::Result<()> {
        let raw = serde_json::to_string_pretty(&self.config)?;
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.path, raw)?;

        Ok(())
    }

    async fn new(path: PathBuf) -> io::Result<Self> {
        let config = if path.exists() {
            let raw = fs::read_to_string(&path).await?;
            serde_json::from_str(&raw)?
        } else {
            Config::default()
        };

        Ok(Self { config, path })
    }
}
