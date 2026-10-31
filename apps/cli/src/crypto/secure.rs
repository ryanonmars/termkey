//! Process hardening is best effort at the OS boundary, but failures are returned
//! explicitly. These controls cannot stop privileged debuggers, administrator
//! dump policy, paging, or external diagnostic tools from retaining memory.

#[cfg(unix)]
pub fn harden_process() -> std::io::Result<()> {
    let status = unsafe {
        libc::setrlimit(
            libc::RLIMIT_CORE,
            &libc::rlimit {
                rlim_cur: 0,
                rlim_max: 0,
            },
        )
    };
    if status != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

/// Ask Windows Error Reporting to exclude heap data and disable crash/hang
/// snapshots. This is a per-process mitigation, not a promise that Windows or a
/// privileged debugger can never generate a dump. No registry/admin change is made.
#[cfg(windows)]
pub fn harden_process() -> std::io::Result<()> {
    use windows_sys::Win32::System::ErrorReporting::{
        WerSetFlags, WER_FAULT_REPORTING_DISABLE_SNAPSHOT_CRASH,
        WER_FAULT_REPORTING_DISABLE_SNAPSHOT_HANG, WER_FAULT_REPORTING_FLAG_NOHEAP,
        WER_FAULT_REPORTING_FLAG_NO_HEAP_ON_QUEUE, WER_FAULT_REPORTING_NO_UI,
    };
    let result = unsafe {
        WerSetFlags(
            WER_FAULT_REPORTING_FLAG_NOHEAP
                | WER_FAULT_REPORTING_FLAG_NO_HEAP_ON_QUEUE
                | WER_FAULT_REPORTING_NO_UI
                | WER_FAULT_REPORTING_DISABLE_SNAPSHOT_CRASH
                | WER_FAULT_REPORTING_DISABLE_SNAPSHOT_HANG,
        )
    };
    if result < 0 {
        return Err(std::io::Error::other(format!(
            "Windows error-reporting hardening failed (HRESULT {result:#x})"
        )));
    }
    Ok(())
}

/// Lock a valid memory region. The caller must keep it allocated until unlocked.
/// These optional helpers are not currently used by vault allocations.
#[allow(dead_code)]
pub fn mlock(ptr: *const u8, len: usize) -> bool {
    #[cfg(unix)]
    {
        unsafe { libc::mlock(ptr.cast(), len) == 0 }
    }
    #[cfg(windows)]
    {
        unsafe { windows_sys::Win32::System::Memory::VirtualLock(ptr.cast(), len) != 0 }
    }
}

#[allow(dead_code)]
pub fn munlock(ptr: *const u8, len: usize) -> bool {
    #[cfg(unix)]
    {
        unsafe { libc::munlock(ptr.cast(), len) == 0 }
    }
    #[cfg(windows)]
    {
        unsafe { windows_sys::Win32::System::Memory::VirtualUnlock(ptr.cast(), len) != 0 }
    }
}
