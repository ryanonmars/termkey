//! Operating-system integration. The encrypted vault format is platform independent.
pub(crate) mod fs;
pub mod paths;
#[cfg(windows)]
mod windows_fs;
