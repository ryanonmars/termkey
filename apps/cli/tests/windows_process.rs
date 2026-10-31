//! Check the actual Windows process mitigation state, not a mocked API result.
#![cfg(windows)]

#[test]
fn native_windows_error_reporting_excludes_heap_and_snapshots() {
    use windows_sys::Win32::System::ErrorReporting::{
        WerGetFlags, WER_FAULT_REPORTING_DISABLE_SNAPSHOT_CRASH,
        WER_FAULT_REPORTING_DISABLE_SNAPSHOT_HANG, WER_FAULT_REPORTING_FLAG_NOHEAP,
        WER_FAULT_REPORTING_FLAG_NO_HEAP_ON_QUEUE, WER_FAULT_REPORTING_NO_UI,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    termkey::crypto::secure::harden_process().unwrap();
    let mut flags = 0;
    let result = unsafe { WerGetFlags(GetCurrentProcess(), &mut flags) };
    assert!(result >= 0, "could not read Windows error-reporting flags");
    let expected = WER_FAULT_REPORTING_FLAG_NOHEAP
        | WER_FAULT_REPORTING_FLAG_NO_HEAP_ON_QUEUE
        | WER_FAULT_REPORTING_NO_UI
        | WER_FAULT_REPORTING_DISABLE_SNAPSHOT_CRASH
        | WER_FAULT_REPORTING_DISABLE_SNAPSHOT_HANG;
    assert!(
        flags & expected == expected,
        "Windows error reporting did not retain the required process mitigations"
    );
}
