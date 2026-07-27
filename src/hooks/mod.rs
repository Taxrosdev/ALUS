mod sandbox;

use std::{
    collections::HashMap,
    io,
    path::{Path, PathBuf},
};
use tokio::fs;

use crate::{error::Error, hooks::sandbox::SandboxInstance, logging};

const HOOKS_PATH: &str = "share/alus/hooks";

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Hook {
    pub description: String,
    #[serde(default)]
    unsandboxed: bool,

    /// Paths to watch for changes. On change, rerun this Hook.
    /// TODO: Actually implement this. As of current, this is always reexecuted.
    pub watch: Vec<PathBuf>,

    #[serde(default)]
    permissions: HashMap<PathBuf, Permission>,

    /// Will be ran with sh -c
    exec: String,
}

impl Hook {
    pub async fn run(&self, usr: PathBuf) -> crate::error::Result<()> {
        let mut sandbox = SandboxInstance::prepare().await?;

        for (path, permission) in self.permissions.iter() {
            let path = if path == "usr" {
                usr.clone()
            } else {
                path.to_path_buf()
            };

            let read_only = match permission {
                Permission::ReadOnly => false,
                Permission::ReadWrite => true,
            };
            sandbox.with_mount(path, read_only);
        }

        let output = sandbox.run(self.exec.clone()).unwrap();

        logging::hook(&output);

        if !output.status.success() {
            return Err(Error::HookExit(output.status));
        }

        Ok(())
    }

    pub async fn load_hooks(prefix: &Path) -> crate::error::Result<Vec<Hook>> {
        let mut hooks = Vec::new();
        let mut read_dir = fs::read_dir(prefix.join(HOOKS_PATH)).await?;
        while let Some(entry) = read_dir.next_entry().await? {
            let raw = fs::read_to_string(entry.path()).await?;
            hooks.push(toml::from_str(&raw)?);
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
