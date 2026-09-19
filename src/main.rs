use alus::{
    Result,
    commit::Commit,
    logging,
    pointer::{Branch, Pointer},
    pull::Puller,
    repo::Repo,
};
use clap::{Parser, Subcommand};
use std::{path::PathBuf, sync::Arc};
use tokio::fs;
use treeup::{downloader::ReqwestDownloader, object::Object};
use treeup_core::downloader::ObjectDownloader;
use utils::EndsWithSlash;

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
    /// Permanently switches to a branch ONLY. Day-to-Day system administration should use this.
    Switch { pointer: String },
    /// Checks out *one time only*. Useful for inspecting a commit/branch in development.
    Checkout { pointer: String },
    /// Refreshes to the latest branch/commit and switches to it.
    /// TLDR: Updates your computer!
    Update,
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
    CurrentBranch { branch: Option<String> },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let repo_path = args.repo.unwrap_or_else(|| PathBuf::from("/.alus"));
    let usr_path = args.usr.unwrap_or_else(|| PathBuf::from("/usr"));
    let boot_path = args.boot.unwrap_or_else(|| PathBuf::from("/boot"));
    let mut repo = Repo::new(repo_path).await?;

    match args.command {
        Commands::Update => {
            let remote = get_remote(&repo);
            let downloader = Arc::new(ReqwestDownloader::new(
                &(remote.to_string() + "objects"),
                &(remote.to_string() + "blobs"),
                remote.clone().into(),
            ));

            // Get the latest commit from current
            let branch = get_current_branch(&repo);
            let _ = Branch::pull(&repo, branch.clone(), downloader.clone()).await;
            let commit = resolve_pointer_remote(&repo, branch, downloader.clone()).await?;
            let commit_hash = commit.hash()?;

            // Pull
            let tree_puller = Puller::new(Arc::new(repo.clone()), downloader, None);
            tree_puller.download_commit(commit.clone(), false).await?;

            // Switch/Checkout
            fs::create_dir_all(&boot_path).await?;
            let initramfs = &boot_path.join(format!("initramfs-{commit_hash}"));
            let vmlinuz = &boot_path.join(format!("vmlinuz-{commit_hash}"));

            commit
                .deploy(&repo, usr_path, initramfs, vmlinuz, &PathBuf::from("/"))
                .await?;
        }
        Commands::Switch {
            pointer: pointer_str,
        } => {
            let commit = resolve_pointer(&repo, pointer_str.clone()).await?;
            let commit_hash = commit.hash()?;

            fs::create_dir_all(&boot_path).await?;
            let initramfs = &boot_path.join(format!("initramfs-{commit_hash}"));
            let vmlinuz = &boot_path.join(format!("vmlinuz-{commit_hash}"));

            commit
                .deploy(&repo, usr_path, initramfs, vmlinuz, &PathBuf::from("/"))
                .await?;

            repo.config.set_current_branch(pointer_str)?;
        }
        Commands::Checkout { pointer } => {
            let commit = resolve_pointer(&repo, pointer.clone()).await?;
            let commit_hash = commit.hash()?;

            fs::create_dir_all(&boot_path).await?;
            let initramfs = &boot_path.join(format!("initramfs-{commit_hash}"));
            let vmlinuz = &boot_path.join(format!("vmlinuz-{commit_hash}"));

            commit
                .deploy(&repo, usr_path, initramfs, vmlinuz, &PathBuf::from("/"))
                .await?;
        }
        Commands::Pull {
            pointer,
            clone_from,
        } => {
            let remote = get_remote(&repo);
            let clone_from = match clone_from {
                Some(path) => Some(Arc::new(Repo::new(path).await?)),
                None => None,
            };

            let downloader = Arc::new(ReqwestDownloader::new(
                &(remote.to_string() + "objects"),
                &(remote.to_string() + "blobs"),
                remote.into(),
            ));

            let commit = resolve_pointer_remote(&repo, pointer, downloader.clone()).await?;

            let tree_puller = Puller::new(Arc::new(repo), downloader, clone_from);

            tree_puller.download_commit(commit, false).await?;
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
            ConfigCommand::CurrentBranch { branch } => match branch {
                Some(limit) => repo.config.set_current_branch(limit)?,
                None => logging::log(repo.config.current_branch().unwrap_or("".to_string())),
            },
        },
    }
    Ok(())
}

async fn resolve_pointer(repo: &Repo, pointer: String) -> Result<Commit> {
    Ok(match repo.config.remote() {
        Some(remote) => {
            let downloader = Arc::new(ReqwestDownloader::new(
                &(remote.to_string() + "objects"),
                &(remote.to_string() + "blobs"),
                remote.clone().into(),
            ));
            resolve_pointer_remote(repo, pointer, downloader).await?
        }
        None => resolve_pointer_local(repo, pointer).await?,
    })
}

async fn resolve_pointer_local(repo: &Repo, pointer: String) -> Result<Commit> {
    match Pointer::resolve_local(repo, pointer).await? {
        Some(pointer) => Ok(pointer.get_commit(repo).await?),
        None => logging::die("Could not find pointer."),
    }
}

async fn resolve_pointer_remote(
    repo: &Repo,
    pointer: String,
    downloader: Arc<impl ObjectDownloader>,
) -> Result<Commit> {
    match Pointer::resolve_all(repo, pointer, downloader.clone()).await? {
        Some(pointer) => Ok(pointer.pull_commit(repo, downloader).await?),
        None => logging::die("Could not find pointer."),
    }
}

/// Will die if remote is not set
fn get_remote(repo: &Repo) -> EndsWithSlash {
    match repo.config.remote() {
        Some(remote) => remote.clone(),
        None => logging::die("No remote configured for Repo."),
    }
}

/// Will die if current is not set
fn get_current_branch(repo: &Repo) -> String {
    match repo.config.current_branch() {
        Some(branch) => branch,
        None => logging::die("Repo has no current.\nNever switched previously?"),
    }
}
