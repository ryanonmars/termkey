use std::fs::{self, File};
use std::io;
use std::path::Path;

/// Create or protect the explicitly selected leaf directory only. Existing
/// ancestors are never recursively chmod'ed or re-ACL'ed.
pub(crate) fn ensure_private_directory(path: &Path) -> io::Result<()> {
    reject_symlink(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        #[cfg(not(target_os = "macos"))]
        {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = fs::DirBuilder::new();
            builder.recursive(true).mode(0o700);
            builder.create(path)?;
        }
        #[cfg(target_os = "macos")]
        super::macos_fs::create_directories(path)?;
        let directory = fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY)
            .open(path)?;
        require_current_owner(&directory)?;
        set_private_mode(&directory, 0o700)?;
        #[cfg(target_os = "macos")]
        super::macos_fs::protect(&directory)?;
    }
    #[cfg(windows)]
    super::windows_fs::ensure_private_directory(path)?;
    Ok(())
}

/// Open a lock file without following links; a new file is private at creation.
pub(crate) fn open_private_lock(path: &Path) -> io::Result<File> {
    reject_symlink(path)?;
    #[cfg(target_os = "macos")]
    {
        super::macos_fs::open_private(path, false)
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)?;
        if !file.metadata()?.is_file() {
            return Err(io::Error::other("TermKey lock path is not a regular file"));
        }
        require_current_owner(&file)?;
        set_private_mode(&file, 0o600)?;
        Ok(file)
    }
    #[cfg(windows)]
    {
        super::windows_fs::open_private_file(path, false)
    }
}

pub(crate) fn create_private_file(path: &Path) -> io::Result<File> {
    #[cfg(target_os = "macos")]
    {
        super::macos_fs::open_private(path, true)
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)?;
        protect_file(&file)?;
        Ok(file)
    }
    #[cfg(windows)]
    {
        super::windows_fs::open_private_file(path, true)
    }
}

/// Reading a copied vault/backup also establishes its destination protection
/// before any encrypted or recovery data is loaded into process memory.
pub(crate) fn open_private_read(path: &Path) -> io::Result<File> {
    reject_symlink(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let file = fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)?;
        if !file.metadata()?.is_file() {
            return Err(io::Error::other(
                "Protected TermKey path is not a regular file",
            ));
        }
        protect_file(&file)?;
        Ok(file)
    }
    #[cfg(windows)]
    {
        super::windows_fs::open_private_existing_file(path)
    }
}

pub(crate) fn protect_file(file: &File) -> io::Result<()> {
    #[cfg(unix)]
    {
        require_current_owner(file)?;
        set_private_mode(file, 0o600)?;
        #[cfg(target_os = "macos")]
        super::macos_fs::protect(file)?;
        Ok(())
    }
    #[cfg(windows)]
    {
        super::windows_fs::protect_file(file)
    }
}

fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        super::windows_fs::is_reparse_point(metadata)
    }
    #[cfg(unix)]
    {
        metadata.file_type().is_symlink()
    }
}

fn reject_symlink(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if is_link(&metadata) => Err(io::Error::other(
            "Refusing a symbolic link for a protected TermKey path",
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(unix)]
fn set_private_mode(file: &File, mode: u32) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    file.set_permissions(fs::Permissions::from_mode(mode))?;
    if file.metadata()?.permissions().mode() & 0o777 != mode {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Filesystem did not enforce private TermKey permissions",
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn require_current_owner(file: &File) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt;
    if file.metadata()?.uid() != unsafe { libc::geteuid() } {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Protected TermKey path is not owned by the current user",
        ));
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;
    #[cfg(unix)]
    #[test]
    fn private_permissions_exist_before_sensitive_data_is_written() {
        use std::os::unix::fs::PermissionsExt;
        let root = TempDir::new().unwrap();
        let leaf = root.path().join("new/data");
        ensure_private_directory(&leaf).unwrap();
        assert_eq!(
            fs::metadata(&leaf).unwrap().permissions().mode() & 0o777,
            0o700
        );
        let mut file = create_private_file(&leaf.join("secret.tmp")).unwrap();
        assert_eq!(file.metadata().unwrap().permissions().mode() & 0o777, 0o600);
        file.write_all(b"synthetic").unwrap();
    }
    #[test]
    fn copied_file_is_restricted_before_reading() {
        use std::os::unix::fs::PermissionsExt;
        let root = TempDir::new().unwrap();
        let path = root.path().join("copied-vault.ck");
        fs::write(&path, b"synthetic ciphertext").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o666)).unwrap();
        let file = open_private_read(&path).unwrap();
        assert_eq!(file.metadata().unwrap().permissions().mode() & 0o777, 0o600);
    }

    #[cfg(unix)]
    #[test]
    fn leaf_protection_never_changes_existing_parent_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let root = TempDir::new().unwrap();
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o755)).unwrap();
        ensure_private_directory(&root.path().join("vault")).unwrap();
        assert_eq!(
            fs::metadata(root.path()).unwrap().permissions().mode() & 0o777,
            0o755
        );
    }
    #[cfg(unix)]
    #[test]
    fn locks_reject_symlinks_without_touching_the_target() {
        use std::os::unix::fs::symlink;
        let root = TempDir::new().unwrap();
        let target = root.path().join("unrelated");
        fs::write(&target, b"untouched").unwrap();
        let link = root.path().join("vault.lock");
        symlink(&target, &link).unwrap();
        assert!(open_private_lock(&link).is_err());
        assert_eq!(fs::read(target).unwrap(), b"untouched");
    }
}
