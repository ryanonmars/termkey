use std::collections::BTreeSet;
use std::fs::{self, File};
use std::path::Path;

use dialoguer::{Confirm, Input};
use fs4::FileExt;

use crate::error::{Result, TermKeyError};
use crate::ui::borders::print_success;
use crate::vault::storage;

#[cfg(test)]
pub(crate) const TERMKEY_DATA_FILES: &[&str] = &[
    "vault.ck",
    "vault.tmp",
    "vault.lock",
    "config.json",
    "config.tmp",
    "config.lock",
];

pub fn run(skip_confirmation: bool) -> Result<()> {
    if !skip_confirmation {
        confirm_reset()?;
    }

    let removed_data_files = reset_current_vault()?;

    print_success("TermKey vault data and settings removed.");
    println!("  Removed {removed_data_files} TermKey data file(s).");
    println!("  Encrypted backups were not touched.");
    println!("  TermKey remains installed. Run `termkey` to set up a new vault.");
    Ok(())
}

pub(crate) fn reset_current_vault() -> Result<usize> {
    reset_paths(
        &storage::vault_dir(),
        &crate::config::storage::config_path(),
    )
}

fn confirm_reset() -> Result<()> {
    println!();
    println!("  This permanently removes your TermKey vault and settings.");
    println!("  Encrypted backup files are preserved.");
    println!();
    let confirmed = Confirm::with_theme(&crate::ui::prompt::SAFE_THEME)
        .with_prompt("Continue with vault reset")
        .default(false)
        .interact()
        .map_err(|error| TermKeyError::Io(std::io::Error::other(error)))?;
    if !confirmed {
        return Err(TermKeyError::Cancelled);
    }

    let confirmation: String = Input::with_theme(&crate::ui::prompt::SAFE_THEME)
        .with_prompt("Type DELETE to permanently remove TermKey data")
        .interact_text()
        .map_err(|error| TermKeyError::Io(std::io::Error::other(error)))?;
    if confirmation.trim() != "DELETE" {
        return Err(TermKeyError::Cancelled);
    }
    Ok(())
}

#[cfg(test)]
fn remove_termkey_data_at(data_dir: &Path) -> Result<usize> {
    reset_paths(data_dir, &data_dir.join("config.json"))
}

fn reset_lock(path: &Path) -> Result<Option<File>> {
    let parent = path.parent().ok_or(TermKeyError::InvalidVaultFormat)?;
    if !parent.exists() {
        return Ok(None);
    }
    crate::platform::fs::ensure_private_directory(parent)?;
    let file = crate::platform::fs::open_private_lock(path)?;
    FileExt::try_lock(&file).map_err(|error| match error {
        fs4::TryLockError::WouldBlock => TermKeyError::VaultConflict,
        fs4::TryLockError::Error(error) => TermKeyError::Io(error),
    })?;
    Ok(Some(file))
}

fn reset_paths(data_dir: &Path, config_path: &Path) -> Result<usize> {
    // Use the same persistent lock inodes as normal writers. Never unlink a
    // lock file: another process may already have opened it and be waiting.
    // Fixed order matches vault operations followed by configuration updates.
    let _vault_lock = reset_lock(&data_dir.join("vault.lock"))?;
    let _config_lock = reset_lock(&config_path.with_extension("lock"))?;
    let mut paths = BTreeSet::from([
        data_dir.join("vault.ck"),
        data_dir.join("vault.tmp"),
        config_path.to_path_buf(),
        config_path.with_extension("tmp"),
    ]);
    let directories = BTreeSet::from([
        data_dir.to_path_buf(),
        config_path
            .parent()
            .ok_or(TermKeyError::InvalidVaultFormat)?
            .to_path_buf(),
    ]);
    for directory in directories {
        if !directory.exists() {
            continue;
        }
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let filename = entry.file_name();
            let filename = filename.to_string_lossy();
            if filename.starts_with(".vault.ck.tmp-") || filename.starts_with(".config.json.tmp-") {
                paths.insert(entry.path());
            }
        }
    }
    let mut removed = 0;
    for path in paths {
        removed += remove_file_if_present(&path)? as usize;
    }
    // Empty lock anchors contain no vault data and remain for safe future use.
    Ok(removed)
}

fn remove_file_if_present(path: &Path) -> Result<bool> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    if metadata.file_type().is_dir() {
        return Err(TermKeyError::ConfigError(format!(
            "Refusing to remove directory where a TermKey data file is expected: {}",
            path.display()
        )));
    }
    fs::remove_file(path)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::remove_termkey_data_at;
    use tempfile::TempDir;

    #[test]
    fn reset_refuses_active_writer_without_deleting_any_data() {
        use fs4::FileExt;
        let dir = TempDir::new().unwrap();
        let vault = dir.path().join("vault.ck");
        fs::write(&vault, b"synthetic ciphertext").unwrap();
        let lock = crate::platform::fs::open_private_lock(&dir.path().join("vault.lock")).unwrap();
        FileExt::lock(&lock).unwrap();
        assert!(matches!(
            remove_termkey_data_at(dir.path()),
            Err(crate::error::TermKeyError::VaultConflict)
        ));
        assert_eq!(fs::read(&vault).unwrap(), b"synthetic ciphertext");
        FileExt::unlock(&lock).unwrap();
        assert_eq!(remove_termkey_data_at(dir.path()).unwrap(), 1);
    }

    #[test]
    fn reset_clears_separate_xdg_config_and_preserves_other_files() {
        let root = TempDir::new().unwrap();
        let data = root.path().join("data");
        let config = root.path().join("config");
        fs::create_dir(&data).unwrap();
        fs::create_dir(&config).unwrap();
        fs::write(data.join("vault.ck"), b"synthetic").unwrap();
        fs::write(config.join("config.json"), b"{}").unwrap();
        fs::write(config.join("unrelated"), b"keep").unwrap();
        assert_eq!(
            super::reset_paths(&data, &config.join("config.json")).unwrap(),
            2
        );
        assert!(config.join("unrelated").exists());
        assert!(data.join("vault.lock").exists());
        assert!(config.join("config.lock").exists());
    }

    #[test]
    fn removes_termkey_data_but_preserves_backup_files() {
        let dir = TempDir::new().unwrap();
        for filename in [
            "vault.ck",
            "vault.tmp",
            "vault.lock",
            "config.json",
            "config.tmp",
            "config.lock",
            ".vault.ck.tmp-deadbeef",
            ".config.json.tmp-deadbeef",
        ] {
            fs::write(dir.path().join(filename), "termkey data").unwrap();
        }
        let backup = dir.path().join("backup.ck");
        fs::write(&backup, "backup data").unwrap();

        assert_eq!(remove_termkey_data_at(dir.path()).unwrap(), 6);
        assert!(backup.exists());
        assert!(dir.path().exists());
    }

    #[test]
    fn reset_preserves_lock_anchors_for_future_writers() {
        let dir = TempDir::new().unwrap();
        let data_dir = dir.path().join(".termkey");
        fs::create_dir(&data_dir).unwrap();
        fs::write(data_dir.join("vault.ck"), "vault data").unwrap();

        assert_eq!(remove_termkey_data_at(&data_dir).unwrap(), 1);
        assert!(!data_dir.join("vault.ck").exists());
        assert!(data_dir.join("vault.lock").exists());
        assert!(data_dir.join("config.lock").exists());
    }
}
