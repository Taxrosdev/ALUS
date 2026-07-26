mod commit;
mod error;
mod logging;
mod pull;
mod repo;

use clap::{Parser, Subcommand};
use std::{path::PathBuf, sync::Arc};
use tokio::fs;
use treeup::{downloader::ReqwestDownloader, object::Object};

use crate::{commit::Commit, logging::Progress, pull::TreePuller, repo::Repo};

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
            fs::create_dir_all(&boot_path).await?;
            let initramfs = &boot_path.join(format!("initramfs-{pointer}"));
            let vmlinuz = &boot_path.join(format!("vmlinuz-{pointer}"));

            let commit = Commit::get(&repo.treeup, &pointer).await?;
            commit.deploy(&repo, usr_path, initramfs, vmlinuz).await?;
        }
        Commands::Pull { pointer } => {
            if let Some(remote) = repo.config.remote() {
                // Immediately drop, as we need to pass remote into `tree_puller`
                let remote = remote.to_string();

                let reqwest_downloader = Box::new(ReqwestDownloader::new(
                    &(remote.clone() + "objects"),
                    &(remote.clone() + "blobs"),
                ));

                let commit =
                    Commit::download(&repo.treeup, reqwest_downloader.clone(), &pointer).await?;

                let tree_puller =
                    TreePuller::new(Arc::new(repo), reqwest_downloader, Progress::new());

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
                None => logging::log(repo.config.remote().unwrap_or("")),
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
