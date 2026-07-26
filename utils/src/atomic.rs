use nix::fcntl::{AT_FDCWD, RenameFlags, renameat2};
use std::{io, path::Path};

/// Atomic renames using `renameat2`
///
/// # Errors
/// From the underlying filesystem syscall.
pub fn atomic_rename(old_path: &Path, new_path: &Path) -> io::Result<()> {
    if renameat2(
        AT_FDCWD,
        old_path,
        AT_FDCWD,
        new_path,
        RenameFlags::RENAME_EXCHANGE,
    )
    .is_err()
    {
        renameat2(
            AT_FDCWD,
            old_path,
            AT_FDCWD,
            new_path,
            RenameFlags::RENAME_NOREPLACE,
        )?;
    }

    Ok(())
}
