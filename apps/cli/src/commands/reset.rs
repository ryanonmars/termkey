use std::fs;
use std::path::Path;

use dialoguer::{Confirm, Input};

use crate::error::{Result, TermKeyError};
use crate::ui::borders::print_success;
use crate::vault::storage;

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
    remove_termkey_data_at(&storage::vault_dir())
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

fn remove_termkey_data_at(data_dir: &Path) -> Result<usize> {
    if !data_dir.exists() {
        return Ok(0);
    }
    if !data_dir.is_dir() {
        return Err(TermKeyError::ConfigError(format!(
            "TermKey data path is not a directory: {}",
            data_dir.display()
        )));
    }

    let mut removed = 0;
    for filename in TERMKEY_DATA_FILES {
        removed += remove_file_if_present(&data_dir.join(filename))? as usize;
    }
    for entry in fs::read_dir(data_dir)? {
        let entry = entry?;
        let filename = entry.file_name();
        let filename = filename.to_string_lossy();
        if filename.starts_with(".vault.ck.tmp-") || filename.starts_with(".config.json.tmp-") {
            removed += remove_file_if_present(&entry.path())? as usize;
        }
    }

    if fs::read_dir(data_dir)?.next().is_none() {
        fs::remove_dir(data_dir)?;
    }
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

        assert_eq!(remove_termkey_data_at(dir.path()).unwrap(), 8);
        assert!(backup.exists());
        assert!(dir.path().exists());
    }

    #[test]
    fn removes_an_empty_termkey_data_directory() {
        let dir = TempDir::new().unwrap();
        let data_dir = dir.path().join(".termkey");
        fs::create_dir(&data_dir).unwrap();
        fs::write(data_dir.join("vault.ck"), "vault data").unwrap();

        assert_eq!(remove_termkey_data_at(&data_dir).unwrap(), 1);
        assert!(!data_dir.exists());
    }
}
