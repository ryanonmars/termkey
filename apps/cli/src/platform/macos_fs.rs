//! Darwin extended ACLs can grant access independently of mode 0600/0700.
//! Use an explicit, non-inheriting empty ACL at creation and verify it on
//! existing handles. Only the selected file or leaf directory is changed.
use std::ffi::{c_int, c_void, CString};
use std::fs::File;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::ptr;

const ACL_TYPE_EXTENDED: c_int = 0x100;
const ACL_FLAG_NO_INHERIT: c_int = 1 << 17;
const FILESEC_MODE: c_int = 4;
const FILESEC_ACL: c_int = 5;

// Signatures/constants from the Darwin SDK's sys/acl.h and sys/fcntl.h.
unsafe extern "C" {
    fn acl_init(count: c_int) -> *mut c_void;
    fn acl_free(object: *mut c_void) -> c_int;
    fn acl_get_flagset_np(object: *mut c_void, flags: *mut *mut c_void) -> c_int;
    fn acl_add_flag_np(flags: *mut c_void, flag: c_int) -> c_int;
    fn acl_get_fd_np(fd: c_int, kind: c_int) -> *mut c_void;
    fn acl_set_fd_np(fd: c_int, acl: *mut c_void, kind: c_int) -> c_int;
    fn acl_valid(acl: *mut c_void) -> c_int;
    fn acl_get_entry(acl: *mut c_void, index: c_int, entry: *mut *mut c_void) -> c_int;
    fn filesec_init() -> *mut c_void;
    fn filesec_free(security: *mut c_void);
    fn filesec_set_property(security: *mut c_void, property: c_int, value: *const c_void) -> c_int;
    fn openx_np(path: *const libc::c_char, flags: c_int, security: *mut c_void) -> c_int;
    fn mkdirx_np(path: *const libc::c_char, security: *mut c_void) -> c_int;
}

struct Acl(*mut c_void);
impl Drop for Acl {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { acl_free(self.0) };
        }
    }
}
impl Acl {
    fn empty() -> io::Result<Self> {
        let acl = Self(unsafe { acl_init(0) });
        if acl.0.is_null() {
            return Err(io::Error::last_os_error());
        }
        let mut flags = ptr::null_mut();
        if unsafe { acl_get_flagset_np(acl.0, &mut flags) } != 0
            || unsafe { acl_add_flag_np(flags, ACL_FLAG_NO_INHERIT) } != 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(acl)
    }
}
struct Security(*mut c_void);
impl Drop for Security {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { filesec_free(self.0) };
        }
    }
}
impl Security {
    fn private(mode: libc::mode_t) -> io::Result<Self> {
        let security = Self(unsafe { filesec_init() });
        if security.0.is_null() {
            return Err(io::Error::last_os_error());
        }
        let acl = Acl::empty()?;
        if unsafe { filesec_set_property(security.0, FILESEC_MODE, ptr::from_ref(&mode).cast()) }
            != 0
            || unsafe {
                filesec_set_property(security.0, FILESEC_ACL, ptr::from_ref(&acl.0).cast())
            } != 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(security)
    }
}

pub(super) fn protect(file: &File) -> io::Result<()> {
    let acl = Acl::empty()?;
    if unsafe { acl_set_fd_np(file.as_raw_fd(), acl.0, ACL_TYPE_EXTENDED) } != 0 {
        return Err(io::Error::last_os_error());
    }
    verify(file)
}

fn verify(file: &File) -> io::Result<()> {
    let actual = Acl(unsafe { acl_get_fd_np(file.as_raw_fd(), ACL_TYPE_EXTENDED) });
    if actual.0.is_null() {
        // Darwin's acl_get_fd_np delegates to filesec_get_property, which
        // reports ENOENT when a valid handle has no extended ACL property.
        let error = io::Error::last_os_error();
        return if error.raw_os_error() == Some(libc::ENOENT) {
            Ok(())
        } else {
            Err(error)
        };
    }
    if unsafe { acl_valid(actual.0) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let mut entry = ptr::null_mut();
    // A valid ACL has no first entry when Darwin reports EINVAL or ENOENT.
    if unsafe { acl_get_entry(actual.0, 0, &mut entry) } == -1 {
        let error = io::Error::last_os_error();
        if matches!(error.raw_os_error(), Some(libc::EINVAL | libc::ENOENT)) {
            return Ok(());
        }
        return Err(error);
    }
    Err(io::Error::new(
        io::ErrorKind::PermissionDenied,
        "Protected TermKey path still has extended ACL entries",
    ))
}

fn path_string(path: &Path) -> io::Result<CString> {
    CString::new(path.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Path contains a NUL byte"))
}

pub(super) fn open_private(path: &Path, exclusive: bool) -> io::Result<File> {
    let security = Security::private(0o600)?;
    let path = path_string(path)?;
    let flags = libc::O_RDWR
        | libc::O_CREAT
        | libc::O_NOFOLLOW
        | libc::O_CLOEXEC
        | if exclusive { libc::O_EXCL } else { 0 };
    let fd = unsafe { openx_np(path.as_ptr(), flags, security.0) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let file = unsafe { File::from_raw_fd(fd) };
    if !file.metadata()?.is_file() {
        return Err(io::Error::other(
            "Protected TermKey path is not a regular file",
        ));
    }
    if exclusive {
        // Check creation protection before any repair, including under a
        // shared parent with inheritable allow entries.
        verify(&file)?;
    }
    super::fs::protect_file(&file)?;
    Ok(file)
}

pub(super) fn create_directories(path: &Path) -> io::Result<()> {
    let security = Security::private(0o700)?;
    let cpath = path_string(path)?;
    if unsafe { mkdirx_np(cpath.as_ptr(), security.0) } == 0 {
        return Ok(());
    }
    let error = io::Error::last_os_error();
    match error.kind() {
        io::ErrorKind::AlreadyExists if path.is_dir() => Ok(()),
        io::ErrorKind::NotFound => {
            let parent = path.parent().ok_or(error)?;
            create_directories(parent)?;
            if unsafe { mkdirx_np(cpath.as_ptr(), security.0) } == 0 {
                Ok(())
            } else {
                Err(io::Error::last_os_error())
            }
        }
        _ => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;
    use tempfile::TempDir;

    #[test]
    fn extended_and_inherited_acls_do_not_bypass_private_modes() {
        let root = TempDir::new().unwrap();
        let status = Command::new("/bin/chmod")
            .args([
                "+a",
                "everyone allow read,write,execute,file_inherit,directory_inherit",
            ])
            .arg(root.path())
            .status()
            .unwrap();
        assert!(status.success());

        // An inherited Darwin allow ACL remains effective even after chmod 600.
        let copied = root.path().join("copied-vault.ck");
        std::fs::write(&copied, b"synthetic ciphertext").unwrap();
        std::fs::set_permissions(&copied, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert!(verify(&File::open(&copied).unwrap()).is_err());
        let file = super::super::fs::open_private_read(&copied).unwrap();
        verify(&file).unwrap();
        assert_eq!(std::fs::read(&copied).unwrap(), b"synthetic ciphertext");

        let created =
            super::super::fs::create_private_file(&root.path().join("export.tmp")).unwrap();
        verify(&created).unwrap();
        let lock = super::super::fs::open_private_lock(&root.path().join("vault.lock")).unwrap();
        verify(&lock).unwrap();
        let leaf = root.path().join("new/leaf");
        super::super::fs::ensure_private_directory(&leaf).unwrap();
        verify(&File::open(leaf).unwrap()).unwrap();

        let existing_leaf = root.path().join("existing");
        std::fs::create_dir(&existing_leaf).unwrap();
        assert!(verify(&File::open(&existing_leaf).unwrap()).is_err());
        super::super::fs::ensure_private_directory(&existing_leaf).unwrap();
        verify(&File::open(existing_leaf).unwrap()).unwrap();
        let existing_lock = root.path().join("existing.lock");
        std::fs::write(&existing_lock, []).unwrap();
        assert!(verify(&File::open(&existing_lock).unwrap()).is_err());
        verify(&super::super::fs::open_private_lock(&existing_lock).unwrap()).unwrap();

        // The caller's shared parent was not recursively re-ACL'ed.
        assert!(verify(&File::open(root.path()).unwrap()).is_err());
    }
}
