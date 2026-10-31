//! Process-level path validation uses an empty synthetic working directory.
use std::process::Command;
use tempfile::TempDir;

#[test]
fn help_works_without_home_but_storage_commands_fail_before_creating_files() {
    let directory = TempDir::new().unwrap();
    let run = |arguments: &[&str], override_path: Option<&str>| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_termkey"));
        command
            .current_dir(directory.path())
            .args(arguments)
            .env_remove("HOME")
            .env_remove("USERPROFILE")
            .env_remove("TERMKEY_VAULT_DIR");
        if let Some(path) = override_path {
            command.env("TERMKEY_VAULT_DIR", path);
        }
        command.output().unwrap()
    };
    let help = run(&["--help"], None);
    assert!(help.status.success());
    assert!(!String::from_utf8_lossy(&help.stdout)
        .lines()
        .any(|line| line.trim_start().starts_with("browser ")));
    for override_path in [None, Some("relative-vault")] {
        let result = run(&["config", "--show"], override_path);
        assert!(!result.status.success());
        let message = format!(
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(message.contains(if override_path.is_some() {
            "absolute"
        } else {
            "home directory"
        }));
        assert!(std::fs::read_dir(directory.path())
            .unwrap()
            .next()
            .is_none());
    }
}
