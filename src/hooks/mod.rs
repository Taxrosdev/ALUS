mod sandbox;

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use tokio::fs;

use crate::hooks::sandbox::SandboxInstance;

const HOOKS_PATH: &str = "share/alus/hooks";

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Hook {
    pub description: String,
    #[serde(default)]
    unsandboxed: bool,

    /// Paths to watch for changes. On change, rerun this Hook.
    /// TODO: Actually implement this. As of current, this is always reexecuted.
    #[serde(default)]
    pub watch: Vec<PathBuf>,

    #[serde(default)]
    permissions: HashMap<PathBuf, Permission>,

    /// Will be ran with sh -c
    exec: String,
}

impl Hook {
    pub async fn run(&self, usr: PathBuf, host_sysroot: &Path) -> crate::error::Result<()> {
        let mut sandbox = SandboxInstance::prepare(usr, host_sysroot).await?;

        for (path, permission) in self.permissions.iter() {
            let read_only = match permission {
                Permission::ReadOnly => true,
                Permission::ReadWrite => false,
            };
            if path == &PathBuf::from("usr") {
                sandbox.with_usr_ro(read_only);
            } else {
                sandbox.with_mount(path.to_owned(), read_only);
            }
        }

        if self.unsandboxed {
            sandbox.run_unsandboxed(&self.exec)?;
        } else {
            sandbox.run_sandboxed(&self.exec)?;
        }

        Ok(())
    }

    pub async fn load_hooks(prefix: &Path) -> crate::error::Result<Vec<Hook>> {
        let mut hooks = Vec::new();
        let mut read_dir = fs::read_dir(prefix.join(HOOKS_PATH)).await?;
        while let Some(entry) = read_dir.next_entry().await? {
            if entry.file_name().to_string_lossy().ends_with(".toml") {
                let raw = fs::read_to_string(entry.path()).await?;
                hooks.push(toml::from_str(&raw)?);
            }
        }

        Ok(hooks)
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub enum Permission {
    #[serde(rename = "ro")]
    ReadOnly,
    #[serde(rename = "rw")]
    ReadWrite,
}
