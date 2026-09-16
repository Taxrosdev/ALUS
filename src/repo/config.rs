use std::{io, path::PathBuf};
use tokio::fs;
use utils::EndsWithSlash;

use crate::logging;

#[derive(Clone)]
pub struct ConfigWrapper {
    config: Config,
    path: PathBuf,
}

#[derive(Clone, serde::Deserialize, serde::Serialize, Default)]
pub struct Config {
    pub remote: Option<EndsWithSlash>,
    pub current_branch: Option<String>,
    pub download_limit: Option<u64>,
    pub resolve_limit: Option<u64>,
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
        self.config.download_limit.unwrap_or(512)
    }

    pub fn resolve_limit(&self) -> u64 {
        self.config.resolve_limit.unwrap_or(512)
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

    pub async fn new(path: PathBuf) -> io::Result<Self> {
        let config = if path.exists() {
            let raw = fs::read_to_string(&path).await?;
            serde_json::from_str(&raw)?
        } else {
            Config::default()
        };

        Ok(Self { config, path })
    }

    pub fn set_current_branch(&mut self, branch: String) -> io::Result<()> {
        logging::debug("Setting current branch...");
        self.config.current_branch = Some(branch);
        self.write()?;
        Ok(())
    }

    pub fn current_branch(&self) -> Option<String> {
        self.config
            .current_branch
            .as_ref()
            .map(|branch| branch.trim().to_string())
    }
}
