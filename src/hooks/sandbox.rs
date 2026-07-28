use nix::{
    libc::SIGCHLD,
    mount::{MntFlags, MsFlags, mount, umount2},
    sched::{CloneFlags, clone},
    sys::{
        prctl::set_pdeathsig,
        signal::Signal,
        stat::{Mode, umask},
        wait::{WaitStatus, waitpid},
    },
    unistd::pivot_root,
};
use std::{
    io,
    os::unix::fs::symlink,
    path::{Path, PathBuf},
    process::Command,
    ptr::addr_of_mut,
};
use tokio::fs;

use crate::logging;

const ROOT_PREFIX: &str = "/tmp/alus/sandbox";

pub struct SandboxInstance {
    root: PathBuf,
    mounts: Vec<Mount>,
    usr: PathBuf,
    usr_read_only: bool,
    host_sysroot: PathBuf,
}

impl SandboxInstance {
    pub async fn prepare(usr: PathBuf, host_sysroot: &Path) -> io::Result<Self> {
        let root = generate_root(ROOT_PREFIX.into()).await?;
        let usr = usr.strip_prefix("/").unwrap_or(&usr).to_path_buf();
        let host_sysroot = host_sysroot.strip_prefix("/").unwrap_or(host_sysroot);

        let instance = SandboxInstance {
            root,
            mounts: Vec::new(),
            usr,
            usr_read_only: true,
            host_sysroot: host_sysroot.to_path_buf(),
        };

        Ok(instance)
    }

    pub fn with_mount(&mut self, path: PathBuf, read_only: bool) {
        self.mounts.push(Mount { path, read_only });
    }

    pub fn with_usr_ro(&mut self, read_only: bool) {
        self.usr_read_only = read_only;
    }

    pub fn run_unsandboxed(&self, exec: &str) -> crate::error::Result<()> {
        let output = Command::new("/usr/bin/sh")
            .arg("-c")
            .arg(exec)
            .env_clear()
            .env("PATH", "/usr/bin")
            .output()?;

        logging::hook(&output);

        if output.status.success() {
            Ok(())
        } else {
            match output.status.code() {
                Some(code) => Err(crate::error::Error::HookExit(code)),
                None => Err(crate::error::Error::HookExit(9)),
            }
        }
    }

    pub fn run_sandboxed(self, exec: &str) -> crate::error::Result<()> {
        // 4 MB
        static mut STACK: [u8; 4 * 1024 * 1024] = [0u8; 4 * 1024 * 1024];
        let flags = CloneFlags::CLONE_NEWNS
            | CloneFlags::CLONE_NEWIPC
            | CloneFlags::CLONE_NEWNET
            | CloneFlags::CLONE_NEWPID
            | CloneFlags::CLONE_NEWUTS;
        let entrypoint = Box::new(|| {
            self.setup_child();
            self.run_unsandboxed(exec).expect("command failed");
            0
        });

        let pid =
            unsafe { clone(entrypoint, &mut *addr_of_mut!(STACK), flags, Some(SIGCHLD)) }.unwrap();
        let status = waitpid(pid, None).unwrap();

        match status {
            WaitStatus::Exited(_, 0) => Ok(()),
            WaitStatus::Exited(_, code) => Err(crate::error::Error::HookExit(code)),
            _ => Err(crate::error::Error::HookRunner),
        }
    }

    fn setup_child(&self) {
        // Kill on parents death
        set_pdeathsig(Signal::SIGKILL).expect("Could not set pdeathsig");

        // Setup Loopback
        Self::setup_lo().expect("Loopback error");

        // Pivot Root
        self.pivot().expect("Root setup error");
    }

    fn setup_lo() -> io::Result<()> {
        Command::new("/usr/sbin/ip")
            .args(["link", "set", "lo", "up"])
            .output()?;
        Ok(())
    }

    fn pivot(&self) -> io::Result<()> {
        const NO_FILESYSTEM: std::option::Option<&Path> = None;
        let old_root = self.root.join("old/");
        std::fs::create_dir_all(&old_root)?;

        // Switch Root
        mount(
            NO_FILESYSTEM,
            "/",
            NO_FILESYSTEM,
            MsFlags::MS_REC | MsFlags::MS_PRIVATE,
            NO_FILESYSTEM,
        )?;
        mount(
            Some(&self.root),
            &self.root,
            NO_FILESYSTEM,
            MsFlags::MS_BIND,
            NO_FILESYSTEM,
        )?;

        pivot_root(&self.root, &old_root).expect("could not pivot root");
        std::env::set_current_dir("/")?;
        let old_root = PathBuf::from("/old");

        // Add important mounts
        create_base_symlinks(PathBuf::from("/"))?;
        std::fs::create_dir("proc")?;
        mount(
            Some("proc"),
            "proc",
            Some("proc"),
            MsFlags::empty(),
            NO_FILESYSTEM,
        )?;
        std::fs::create_dir("tmp")?;
        mount(
            Some("tmpfs"),
            "tmp",
            Some("tmpfs"),
            MsFlags::empty(),
            NO_FILESYSTEM,
        )?;
        std::fs::create_dir("sys")?;
        mount(
            Some(&old_root.join("sys")),
            "sys",
            NO_FILESYSTEM,
            MsFlags::MS_BIND | MsFlags::MS_REC | MsFlags::MS_SLAVE,
            NO_FILESYSTEM,
        )?;
        std::fs::create_dir("dev")?;
        mount(
            Some(&old_root.join("dev")),
            "dev",
            NO_FILESYSTEM,
            MsFlags::MS_BIND | MsFlags::MS_REC | MsFlags::MS_SLAVE,
            NO_FILESYSTEM,
        )?;

        // Mount /usr
        std::fs::create_dir_all("/usr")?;
        mount(
            Some(&old_root.join(&self.usr)),
            "/usr",
            NO_FILESYSTEM,
            MsFlags::MS_BIND | MsFlags::MS_REC | MsFlags::MS_SLAVE,
            NO_FILESYSTEM,
        )?;

        if self.usr_read_only {
            mount(
                NO_FILESYSTEM,
                "/usr",
                NO_FILESYSTEM,
                MsFlags::MS_BIND
                    | MsFlags::MS_REC
                    | MsFlags::MS_SLAVE
                    | MsFlags::MS_RDONLY
                    | MsFlags::MS_REMOUNT,
                NO_FILESYSTEM,
            )?;
        }

        // Bind user-specified mounts
        for mount_spec in &self.mounts {
            std::fs::create_dir_all(&mount_spec.path)?;
            mount(
                Some(&old_root.join(&self.host_sysroot).join(&mount_spec.path)),
                &mount_spec.path,
                NO_FILESYSTEM,
                MsFlags::MS_BIND
                    | MsFlags::MS_REC
                    | MsFlags::MS_SLAVE
                    | if mount_spec.read_only {
                        MsFlags::MS_RDONLY
                    } else {
                        MsFlags::empty()
                    },
                NO_FILESYSTEM,
            )?;

            // Must be seperate mount call due to kernel ignoring outside of remount
            if mount_spec.read_only {
                mount(
                    NO_FILESYSTEM,
                    &mount_spec.path,
                    NO_FILESYSTEM,
                    MsFlags::MS_BIND
                        | MsFlags::MS_REC
                        | MsFlags::MS_SLAVE
                        | MsFlags::MS_RDONLY
                        | MsFlags::MS_REMOUNT,
                    NO_FILESYSTEM,
                )?;
            }
        }

        // Get rid of old root
        umount2(&old_root, MntFlags::MNT_DETACH).expect("could not unmount old_root");
        std::fs::remove_dir(&old_root).expect("could not remove mountpoint of old_root");

        umask(Mode::S_IWGRP | Mode::S_IWOTH);

        Ok(())
    }
}

fn create_base_symlinks(root: PathBuf) -> io::Result<()> {
    for dir in ["bin", "sbin", "lib", "lib64"] {
        symlink(PathBuf::from("/usr/").join(dir), root.join(dir))?;
    }

    Ok(())
}

#[derive(Debug, Clone)]
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
