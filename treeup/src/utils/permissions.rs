use std::os::unix::fs::{MetadataExt, PermissionsExt, lchown};
use std::path::PathBuf;
use std::{io, path::Path};
use tokio::fs;

pub struct Permissions {
    pub mode: Option<u32>,
    pub uid: Option<u32>,
    pub gid: Option<u32>,
}

impl Permissions {
    pub async fn get(path: impl AsRef<Path>) -> io::Result<Permissions> {
        let metadata = fs::symlink_metadata(path).await?;

        Ok(Permissions {
            mode: Some(metadata.mode()),
            uid: Some(metadata.uid()),
            gid: Some(metadata.gid()),
        })
    }

    pub async fn deploy(
        path: PathBuf,
        mode: Option<u32>,
        uid: Option<u32>,
        gid: Option<u32>,
    ) -> io::Result<()> {
        // ALUS runs as root in 99% of scenarios.
        if uid.unwrap_or(0) != 0 || gid.unwrap_or(0) != 0 {
            chown(path.clone(), uid, gid).await?;
        }

        if let Some(mode) = mode {
            let permissions = std::fs::Permissions::from_mode(mode);
            fs::set_permissions(path, permissions).await?;
        }

        Ok(())
    }
}

// Sadly, `path` must be owned due to sending between threads.
async fn chown(path: PathBuf, uid: Option<u32>, gid: Option<u32>) -> io::Result<()> {
    tokio::task::spawn_blocking(move || lchown(path, uid, gid))
        .await
        .expect("tokio join error while chown")
}
