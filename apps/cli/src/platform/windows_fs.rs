//! Private NTFS objects use a protected DACL containing exactly the current
//! user's SID. Unsupported filesystems and ACL failures are errors, never a
//! successful no-op. Administrators can still take ownership using OS privileges.
use std::ffi::{c_void, OsStr};
use std::fs::{self, File};
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::path::Path;
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{CloseHandle, LocalFree, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityInfo,
    SetSecurityInfo, SDDL_REVISION_1, SE_FILE_OBJECT,
};
use windows_sys::Win32::Security::{
    EqualSid, GetAce, GetSecurityDescriptorControl, GetSecurityDescriptorDacl,
    GetSecurityDescriptorOwner, GetTokenInformation, TokenUser, ACCESS_ALLOWED_ACE, ACL,
    DACL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION,
    PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, SE_DACL_PROTECTED, TOKEN_QUERY, TOKEN_USER,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateDirectoryW, CreateFileW, CREATE_NEW, FILE_ALL_ACCESS, FILE_ATTRIBUTE_NORMAL,
    FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_GENERIC_READ,
    FILE_GENERIC_WRITE, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    OPEN_ALWAYS, OPEN_EXISTING, READ_CONTROL, WRITE_DAC, WRITE_OWNER,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

fn wide(value: &OsStr) -> io::Result<Vec<u16>> {
    let mut value: Vec<u16> = value.encode_wide().collect();
    if value.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Path contains a null character",
        ));
    }
    value.push(0);
    Ok(value)
}

struct LocalAllocation(*mut c_void);
impl Drop for LocalAllocation {
    fn drop(&mut self) {
        unsafe {
            LocalFree(self.0);
        }
    }
}
struct Token(HANDLE);
impl Drop for Token {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

struct PrivateDescriptor(LocalAllocation);
impl PrivateDescriptor {
    fn new(directory: bool) -> io::Result<Self> {
        unsafe {
            let mut token = null_mut();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
                return Err(io::Error::last_os_error());
            }
            let token = Token(token);
            let mut required = 0;
            GetTokenInformation(token.0, TokenUser, null_mut(), 0, &mut required);
            if required == 0 {
                return Err(io::Error::last_os_error());
            }
            // TOKEN_USER contains pointers and must be suitably aligned.
            let mut buffer =
                vec![0usize; (required as usize).div_ceil(std::mem::size_of::<usize>())];
            if GetTokenInformation(
                token.0,
                TokenUser,
                buffer.as_mut_ptr().cast(),
                required,
                &mut required,
            ) == 0
            {
                return Err(io::Error::last_os_error());
            }
            let user = &*buffer.as_ptr().cast::<TOKEN_USER>();
            let mut sid_text = null_mut();
            if ConvertSidToStringSidW(user.User.Sid, &mut sid_text) == 0 {
                return Err(io::Error::last_os_error());
            }
            let sid_allocation = LocalAllocation(sid_text.cast());
            let mut len = 0;
            while *sid_text.add(len) != 0 {
                len += 1;
            }
            let sid = String::from_utf16(std::slice::from_raw_parts(sid_text, len))
                .map_err(|_| io::Error::other("Invalid current-user SID"))?;
            drop(sid_allocation);
            let inheritance = if directory { "OICI" } else { "" };
            let sddl = wide(OsStr::new(&format!(
                "O:{sid}D:P(A;{inheritance};FA;;;{sid})"
            )))?;
            let mut descriptor = null_mut();
            if ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                null_mut(),
            ) == 0
            {
                return Err(io::Error::last_os_error());
            }
            Ok(Self(LocalAllocation(descriptor)))
        }
    }
    fn attributes(&self) -> SECURITY_ATTRIBUTES {
        SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: self.0 .0,
            bInheritHandle: 0,
        }
    }
    fn descriptor(&self) -> PSECURITY_DESCRIPTOR {
        self.0 .0
    }
    unsafe fn owner(&self) -> io::Result<*mut c_void> {
        let mut owner = null_mut();
        let mut defaulted = 0;
        if GetSecurityDescriptorOwner(self.descriptor(), &mut owner, &mut defaulted) == 0
            || owner.is_null()
        {
            return Err(io::Error::last_os_error());
        }
        Ok(owner)
    }
}

fn secure_handle(file: &File, descriptor: &PrivateDescriptor) -> io::Result<()> {
    unsafe {
        let mut dacl = null_mut();
        let mut present = 0;
        let mut defaulted = 0;
        if GetSecurityDescriptorDacl(
            descriptor.descriptor(),
            &mut present,
            &mut dacl,
            &mut defaulted,
        ) == 0
            || present == 0
            || dacl.is_null()
        {
            return Err(io::Error::other("Private DACL is missing"));
        }
        let owner = descriptor.owner()?;
        let status = SetSecurityInfo(
            file.as_raw_handle().cast(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION
                | DACL_SECURITY_INFORMATION
                | PROTECTED_DACL_SECURITY_INFORMATION,
            owner,
            null_mut(),
            dacl,
            null(),
        );
        if status != 0 {
            return Err(io::Error::from_raw_os_error(status as i32));
        }
        verify_handle(file, owner)
    }
}

unsafe fn verify_handle(file: &File, expected_owner: *mut c_void) -> io::Result<()> {
    let mut owner = null_mut();
    let mut dacl: *mut ACL = null_mut();
    let mut descriptor = null_mut();
    let status = GetSecurityInfo(
        file.as_raw_handle().cast(),
        SE_FILE_OBJECT,
        OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
        &mut owner,
        null_mut(),
        &mut dacl,
        null_mut(),
        &mut descriptor,
    );
    if status != 0 {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    let _allocation = LocalAllocation(descriptor);
    let denied = || {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Filesystem did not enforce the current-user-only TermKey ACL",
        )
    };
    if owner.is_null()
        || EqualSid(owner, expected_owner) == 0
        || dacl.is_null()
        || (*dacl).AceCount != 1
    {
        return Err(denied());
    }
    let mut control = 0;
    let mut revision = 0;
    if GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) == 0 {
        return Err(io::Error::last_os_error());
    }
    if control & SE_DACL_PROTECTED == 0 {
        return Err(denied());
    }
    let mut ace = null_mut();
    if GetAce(dacl, 0, &mut ace) == 0 || ace.is_null() {
        return Err(io::Error::last_os_error());
    }
    let ace = &*ace.cast::<ACCESS_ALLOWED_ACE>();
    // ACCESS_ALLOWED_ACE_TYPE is zero. Reject deny/object/inherited extra grants.
    if ace.Header.AceType != 0
        || ace.Mask != FILE_ALL_ACCESS
        || EqualSid(
            (&ace.SidStart as *const u32).cast_mut().cast(),
            expected_owner,
        ) == 0
    {
        return Err(denied());
    }
    Ok(())
}

pub(super) fn open_private_file(path: &Path, exclusive: bool) -> io::Result<File> {
    let descriptor = PrivateDescriptor::new(false)?;
    let attributes = descriptor.attributes();
    let path_wide = wide(path.as_os_str())?;
    let handle = unsafe {
        CreateFileW(
            path_wide.as_ptr(),
            FILE_GENERIC_READ | FILE_GENERIC_WRITE | READ_CONTROL | WRITE_DAC | WRITE_OWNER,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            &attributes,
            if exclusive { CREATE_NEW } else { OPEN_ALWAYS },
            FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
            null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    let file = unsafe { File::from_raw_handle(handle.cast()) };
    if !file.metadata()?.is_file() || file.metadata()?.file_type().is_symlink() {
        return Err(io::Error::other(
            "Protected TermKey path is not a regular file",
        ));
    }
    secure_handle(&file, &descriptor)?;
    Ok(file)
}

pub(super) fn open_private_existing_file(path: &Path) -> io::Result<File> {
    let descriptor = PrivateDescriptor::new(false)?;
    let path_wide = wide(path.as_os_str())?;
    let handle = unsafe {
        CreateFileW(
            path_wide.as_ptr(),
            FILE_GENERIC_READ | READ_CONTROL | WRITE_DAC | WRITE_OWNER,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            null(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
            null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    let file = unsafe { File::from_raw_handle(handle.cast()) };
    if !file.metadata()?.is_file() || file.metadata()?.file_type().is_symlink() {
        return Err(io::Error::other(
            "Protected TermKey path is not a regular file",
        ));
    }
    secure_handle(&file, &descriptor)?;
    Ok(file)
}

pub(super) fn protect_file(file: &File) -> io::Result<()> {
    secure_handle(file, &PrivateDescriptor::new(false)?)
}

pub(super) fn ensure_private_directory(path: &Path) -> io::Result<()> {
    let descriptor = PrivateDescriptor::new(true)?;
    match fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.is_dir() || metadata.file_type().is_symlink() => {
            return Err(io::Error::other(
                "Protected TermKey directory is not an ordinary directory",
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let parent = path
                .parent()
                .ok_or_else(|| io::Error::other("Directory has no parent"))?;
            if !parent.exists() {
                ensure_private_directory(parent)?;
            }
            let attributes = descriptor.attributes();
            let path_wide = wide(path.as_os_str())?;
            if unsafe { CreateDirectoryW(path_wide.as_ptr(), &attributes) } == 0 {
                let error = io::Error::last_os_error();
                if error.kind() != io::ErrorKind::AlreadyExists {
                    return Err(error);
                }
            }
        }
        Err(error) => return Err(error),
    }
    let path_wide = wide(path.as_os_str())?;
    let handle = unsafe {
        CreateFileW(
            path_wide.as_ptr(),
            READ_CONTROL | WRITE_DAC | WRITE_OWNER | FILE_READ_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    let directory = unsafe { File::from_raw_handle(handle.cast()) };
    if !directory.metadata()?.is_dir() || directory.metadata()?.file_type().is_symlink() {
        return Err(io::Error::other(
            "Protected TermKey directory changed while opening",
        ));
    }
    secure_handle(&directory, &descriptor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;
    #[test]
    fn native_windows_file_and_directory_acl_are_private() {
        let root = TempDir::new().unwrap();
        let directory = root.path().join("synthetic private vault");
        ensure_private_directory(&directory).unwrap();
        let mut file = open_private_file(&directory.join("vault.tmp"), true).unwrap();
        let descriptor = PrivateDescriptor::new(false).unwrap();
        unsafe {
            verify_handle(&file, descriptor.owner().unwrap()).unwrap();
        }
        file.write_all(b"synthetic ciphertext").unwrap();
        assert!(open_private_file(&directory.join("vault.tmp"), true).is_err());
    }
    #[test]
    fn existing_access_denied_file_fails_explicitly() {
        let root = TempDir::new().unwrap();
        let path = root.path().join("denied.ck");
        let file = open_private_file(&path, true).unwrap();
        unsafe {
            let text = wide(OsStr::new("D:P")).unwrap();
            let mut descriptor = null_mut();
            assert_ne!(
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    text.as_ptr(),
                    SDDL_REVISION_1,
                    &mut descriptor,
                    null_mut()
                ),
                0
            );
            let allocation = LocalAllocation(descriptor);
            let mut present = 0;
            let mut defaulted = 0;
            let mut dacl = null_mut();
            assert_ne!(
                GetSecurityDescriptorDacl(allocation.0, &mut present, &mut dacl, &mut defaulted),
                0
            );
            assert_eq!(
                SetSecurityInfo(
                    file.as_raw_handle().cast(),
                    SE_FILE_OBJECT,
                    DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                    null_mut(),
                    null_mut(),
                    dacl,
                    null()
                ),
                0
            );
        }
        let denied = open_private_existing_file(&path).unwrap_err();
        assert_eq!(denied.kind(), io::ErrorKind::PermissionDenied);
        // The existing handle still has WRITE_DAC so cleanup is deterministic.
        protect_file(&file).unwrap();
    }

    #[test]
    fn non_directory_and_access_failures_are_returned() {
        let root = TempDir::new().unwrap();
        let file = root.path().join("ordinary-file");
        fs::write(&file, b"synthetic").unwrap();
        assert!(ensure_private_directory(&file).is_err());
        assert!(open_private_file(&root.path().join("missing/child"), true).is_err());
    }
}
