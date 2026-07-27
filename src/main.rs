mod commit;
mod error;
mod logging;
mod pointer;
mod pull;
mod repo;

use clap::{Parser, Subcommand};
use std::{io, path::PathBuf, sync::Arc};
use tokio::fs;
use treeup::{
    downloader::{Downloader, ReqwestDownloader},
    object::Object,
};

use crate::{commit::Commit, logging::Progress, pointer::Pointer, pull::TreePuller, repo::Repo};

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    command: Commands,

    #[arg(long)]
    /// Defaults to /boot
    boot: Option<PathBuf>,
    /// Defaults to /usr
    #[arg(long)]
    usr: Option<PathBuf>,
    /// Defaults to /.alus
    #[arg(long)]
    repo: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    Checkout {
        pointer: String,
    },
    Pull {
        pointer: String,
        /// Clone from existing Blobs/Trees locally, if exists.
        /// Useful for installers.
        #[arg(long)]
        clone_from: Option<PathBuf>,
    },
    Commit {
        initramfs: PathBuf,
        vmlinuz: PathBuf,
        parent_commit: Option<String>,
    },
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
}

#[derive(Subcommand)]
enum ConfigCommand {
    Remote { url: Option<String> },
    DownloadLimit { limit: Option<u64> },
    ResolveLimit { limit: Option<u64> },
}

#[tokio::main]
async fn main() -> crate::error::Result<()> {
    let args = Args::parse();
    let repo_path = args.repo.unwrap_or_else(|| PathBuf::from("/.alus"));
    let usr_path = args.usr.unwrap_or_else(|| PathBuf::from("/usr"));
    let boot_path = args.boot.unwrap_or_else(|| PathBuf::from("/boot"));
    let mut repo = Repo::new(repo_path).await?;

    match args.command {
        Commands::Checkout { pointer } => {
            let commit = resolve_pointer(&repo, pointer.clone()).await?;
            let pointer = Pointer::resolve_local(&repo, pointer)
                .await?
                .expect("could get commit then immediately deleted?");
            let commit_hash = pointer.commit_hash(&repo).await?;

            fs::create_dir_all(&boot_path).await?;
            let initramfs = &boot_path.join(format!("initramfs-{commit_hash}"));
            let vmlinuz = &boot_path.join(format!("vmlinuz-{commit_hash}"));

            commit.deploy(&repo, usr_path, initramfs, vmlinuz).await?;
        }
        Commands::Pull {
            pointer,
            clone_from,
        } => {
            if let Some(remote) = repo.config.remote() {
                let clone_from = match clone_from {
                    Some(path) => Some(Arc::new(Repo::new(path).await?)),
                    None => None,
                };

                let downloader = Arc::new(ReqwestDownloader::new(
                    &(remote.to_string() + "objects"),
                    &(remote.to_string() + "blobs"),
                    remote.clone(),
                ));

                let commit = resolve_pointer_remote(&repo, pointer, downloader.clone()).await?;

                let tree_puller =
                    TreePuller::new(Arc::new(repo), downloader, Progress::new(), clone_from);

                tree_puller.download_commit(commit, false).await?;
            } else {
                logging::die("No remote configured for Repo.")
            }
        }
        Commands::Commit {
            initramfs,
            vmlinuz,
            parent_commit,
        } => {
            let commit =
                Commit::create(&repo, &usr_path, &initramfs, &vmlinuz, parent_commit).await?;

            logging::log("Created commit ".to_string() + &commit.hash()?);
        }
        Commands::Config { command } => match command {
            ConfigCommand::Remote { url } => match url {
                Some(url) => repo.config.set_remote(url)?,
                None => logging::log(
                    repo.config
                        .remote()
                        .map(|remote| remote.to_string())
                        .unwrap_or("".to_string()),
                ),
            },
            ConfigCommand::ResolveLimit { limit } => match limit {
                Some(limit) => repo.config.set_resolve_limit(Some(limit))?,
                None => logging::log(repo.config.resolve_limit().to_string()),
            },
            ConfigCommand::DownloadLimit { limit } => match limit {
                Some(limit) => repo.config.set_download_limit(Some(limit))?,
                None => logging::log(repo.config.download_limit().to_string()),
            },
        },
    }
    Ok(())
}

async fn resolve_pointer(repo: &Repo, pointer: String) -> crate::error::Result<Commit> {
    Ok(match repo.config.remote() {
        Some(remote) => {
            let downloader = Arc::new(ReqwestDownloader::new(
                &(remote.to_string() + "objects"),
                &(remote.to_string() + "blobs"),
                remote.clone(),
            ));
            resolve_pointer_remote(repo, pointer, downloader).await?
        }
        None => resolve_pointer_local(repo, pointer).await?,
    })
}
async fn resolve_pointer_local(repo: &Repo, pointer: String) -> io::Result<Commit> {
    match Pointer::resolve_local(repo, pointer).await? {
        Some(pointer) => Ok(pointer.get_commit(repo).await?),
        None => panic!("Could not find pointer."),
    }
}

async fn resolve_pointer_remote(
    repo: &Repo,
    pointer: String,
    downloader: Arc<dyn Downloader>,
) -> crate::error::Result<Commit> {
    match Pointer::resolve_all(repo, pointer, downloader.clone()).await? {
        Some(pointer) => Ok(pointer.pull_commit(repo, downloader).await?),
        None => panic!("Could not find pointer."),
    }
}
