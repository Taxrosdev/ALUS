use std::{
    collections::HashMap,
    io,
    path::{Path, PathBuf},
    process::Command,
};
use tokio::fs;
use treeup::{Tree, object::Object};

use crate::{logging, repo::Repo};

pub struct Components {
    pub entries: HashMap<String, ComponentDefinition>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct ComponentDefinition {
    pub name: String,
    pub description: Option<String>,
    pub unique_key: Option<String>,
    #[serde(default)]
    // TODO: implement
    migrate_from: Vec<String>,

    #[serde(default)]
    internal: Vec<Internal>,
    #[serde(default)]
    external: Vec<External>,
    #[serde(default)]
    pub trigger: Trigger,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
/// Actually stored inside of the commit
pub struct CommittedComponent {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub unique_key: Option<String>,
    /// map between a tree and the extraction path
    trees: HashMap<PathBuf, String>,

    pub trigger: Trigger,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Internal {
    path: PathBuf,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct External {
    source_path: PathBuf,
    inserted_path: PathBuf,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, Default)]
#[serde(rename_all = "PascalCase")]
pub enum Trigger {
    Exec {
        command: String,
        output_contains: Option<String>,
    },
    Default,
    #[default]
    NeverDefault,
}

impl Components {
    pub async fn load_definitions(path: &Path) -> crate::Result<Self> {
        let mut entries = HashMap::new();

        let mut read_dir = fs::read_dir(path).await?;
        while let Some(entry) = read_dir.next_entry().await? {
            if let Some((id, ext)) = entry
                .file_name()
                .into_string()
                .map_err(|_| crate::Error::Io(io::ErrorKind::InvalidFilename.into()))?
                .split_once('.')
            {
                if ext != "toml" {
                    logging::die(format!("File extension in {} is not .toml", path.display()))
                }

                let raw = fs::read_to_string(entry.path()).await?;
                let component: ComponentDefinition = toml::from_str(&raw)?;

                // Ensure there is only one of internal/external, and that one is specified.
                if !component.internal.is_empty() && !component.external.is_empty() {
                    logging::die("Both Insert and Extract defined for a Component")
                }
                if component.internal.is_empty() && component.external.is_empty() {
                    logging::die("No Insert and Extract sections defined for a Component")
                }

                entries.insert(id.to_string(), component);
            }
        }

        Ok(Self { entries })
    }
}

impl ComponentDefinition {
    /// Create a CommittedComponent and its tree
    pub async fn commit(
        &self,
        id: String,
        repo: &Repo,
        usr_path: &Path,
    ) -> io::Result<CommittedComponent> {
        let mut trees = HashMap::new();

        // TODO: Find a way to exclude this from the eventually commited usr
        for internal in &self.internal {
            let tree = Tree::create(
                repo.object_cas.clone(),
                &repo.blobs_path,
                &usr_path.join(internal.path.clone()),
            )
            .await?;
            trees.insert(internal.path.to_path_buf(), tree.hash()?);
        }

        for external in &self.external {
            let tree = Tree::create(
                repo.object_cas.clone(),
                &repo.blobs_path,
                &external.source_path,
            )
            .await?;
            trees.insert(external.inserted_path.to_path_buf(), tree.hash()?);
        }

        Ok(CommittedComponent {
            name: self.name.clone(),
            description: self.description.clone(),
            id,
            unique_key: self.unique_key.clone(),
            trees,
            trigger: self.trigger.clone(),
        })
    }
}

impl CommittedComponent {
    pub async fn deploy(&self, repo: &Repo, usr_path: &Path) -> crate::Result<()> {
        for (path, hash) in &self.trees {
            let tree = Tree::get(&*repo.object_cas, &hex::decode(hash)?).await?;
            tree.deploy(&repo.blobs_path, &usr_path.join(path)).await?;
        }

        Ok(())
    }
}

impl Trigger {
    /// Check if this component should automatically be enabled
    pub async fn check_trigger(&self, _staging_usr_path: &Path) -> io::Result<bool> {
        match self {
            Trigger::NeverDefault => Ok(false),
            Trigger::Default => Ok(true),
            Trigger::Exec {
                command,
                output_contains,
            } => {
                let output = Command::new("sh")
                    .arg("-c")
                    .arg(command)
                    .env_clear()
                    .env("PATH", "/usr/bin")
                    .output()?;

                if let Some(search) = output_contains {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    return Ok(stdout.find(search).is_some());
                }

                Ok(output.status.success())
            }
        }
    }
}
