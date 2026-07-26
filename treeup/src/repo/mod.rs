use std::{path::PathBuf, sync::Arc};

#[derive(Clone)]
pub struct Repo {
    pub objects_path: Arc<PathBuf>,
    pub blobs_path: Arc<PathBuf>,
}
