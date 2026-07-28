use alus::{commit::Commit, logging, repo::Repo};
use std::{fs, io, path::PathBuf};
use treeup::object::Object;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sysroot = PathBuf::from("/sysroot");
    let usr_path = sysroot.join("usr");

    let commit_hash = get_commit_hash()?;
    logging::log(format!("Commit {}", commit_hash));
    let repo = Repo::new(sysroot.join(".alus")).await?;
    let commit = Commit::get(&repo.treeup, &commit_hash).await?;
    commit.deploy_usr(&repo, usr_path, &sysroot).await?;

    Ok(())
}

fn get_commit_hash() -> io::Result<String> {
    let raw = fs::read_to_string("/proc/cmdline")?;
    let parts = raw.split_whitespace().map(|part| part.split_once('='));

    for part in parts {
        if let Some((name, val)) = part
            && name.to_lowercase() == "commit"
        {
            return Ok(val.to_string());
        }
    }

    panic!("'commit' not in kernel cmdline");
}
