use std::{
    io,
    path::PathBuf,
    process::{Command, Output},
};
use tokio::fs;

const ROOT_PREFIX: &str = "/tmp/alus/sandbox";

pub struct SandboxInstance {
    root: PathBuf,
    mounts: Vec<Mount>,
}

impl SandboxInstance {
    pub async fn prepare() -> io::Result<Self> {
        let root = generate_root(ROOT_PREFIX.into()).await?;
        let instance = SandboxInstance {
            root,
            mounts: Vec::new(),
        };

        Ok(instance)
    }

    pub fn with_mount(&mut self, path: PathBuf, read_only: bool) {
        self.mounts.push(Mount { path, read_only });
    }

    pub fn run(self, exec: String) -> crate::error::Result<Output> {
        let output = Command::new("sh")
            .arg("-c")
            .arg(exec)
            .env_clear()
            .output()?;

        Ok(output)
    }
}

pub struct Mount {
    pub path: PathBuf,
    pub read_only: bool,
}

async fn generate_root(prefix: PathBuf) -> io::Result<PathBuf> {
    fs::create_dir_all(&prefix).await?;
    let mut i = 0;
    loop {
        let path = prefix.join(i.to_string());
        i += 1;

        if fs::create_dir(&path).await.is_ok() {
            break Ok(path);
        }
    }
}
