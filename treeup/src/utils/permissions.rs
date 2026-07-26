use std::os::unix::fs::{MetadataExt, PermissionsExt, lchown};
use std::{io, path::Path};
use tokio::fs;

pub struct Permissions {
    pub mode: Option<u32>,
    pub uid: Option<u32>,
    pub gid: Option<u32>,
}

impl Permissions {
    pub async fn get(path: impl AsRef<Path>) -> io::Result<Permissions> {
        let metadata = fs::metadata(path).await?;

        Ok(Permissions {
            mode: Some(metadata.mode()),
            uid: Some(metadata.uid()),
            gid: Some(metadata.gid()),
        })
    }

    pub async fn deploy(
        path: impl AsRef<Path>,
        mode: Option<u32>,
        uid: Option<u32>,
        gid: Option<u32>,
    ) -> io::Result<()> {
        if let Some(mode) = mode {
            let permissions = std::fs::Permissions::from_mode(mode);
            fs::set_permissions(&path, permissions).await?;
        }

        lchown(path, uid, gid)?;

        Ok(())
    }
}
