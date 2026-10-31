use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use dialoguer::Confirm;

use crate::commands::update_cmd::{detect_install_method, InstallMethod};
use crate::error::{Result, TermKeyError};
use crate::ui::borders::print_success;

const MACOS_APP: &str = "/Applications/TermKey.app";
const MACOS_UNINSTALLER: &str = "/Applications/Uninstall TermKey.app";

#[derive(Debug, PartialEq, Eq)]
enum UninstallPlan {
    Homebrew,
    MacOsInstaller,
    Executable(PathBuf),
    WindowsPortable(PathBuf),
}

pub fn run(skip_confirmation: bool) -> Result<()> {
    let current_exe = std::env::current_exe()?;
    let plan = plan_uninstall(
        &current_exe,
        detect_install_method(&current_exe),
        cfg!(target_os = "macos"),
        Path::new(MACOS_APP).is_dir(),
        Path::new(MACOS_UNINSTALLER).is_dir(),
    )?;

    let plan = portable_plan(plan, cfg!(windows));
    if let UninstallPlan::WindowsPortable(path) = &plan {
        println!("  Portable Windows installation: close TermKey, then run this PowerShell command to remove the executable:");
        if let Some(command) = powershell_remove_command(std::slice::from_ref(path)) {
            println!("    {command}");
        } else {
            println!("    Remove termkey.exe manually after closing TermKey.");
        }
        println!("  Your vault, settings, and backups are preserved.");
        print_retained_vault_instructions(&crate::vault::storage::vault_dir());
        return Ok(());
    }

    println!();
    println!("  This removes the TermKey application. Your vault, settings, and encrypted backups are preserved.");
    print_retained_vault_instructions(&crate::vault::storage::vault_dir());
    if matches!(plan, UninstallPlan::Executable(_)) {
        println!("  Only this executable will be removed; source files and other installations are preserved.");
    }
    if !skip_confirmation {
        let confirmed = Confirm::with_theme(&crate::ui::prompt::SAFE_THEME)
            .with_prompt("Uninstall TermKey")
            .default(false)
            .interact()
            .map_err(|error| TermKeyError::Io(std::io::Error::other(error)))?;
        if !confirmed {
            return Err(TermKeyError::Cancelled);
        }
    }

    if matches!(plan, UninstallPlan::MacOsInstaller) {
        println!("  Opening the macOS uninstaller. Confirm removal in its dialog; macOS may request administrator authorization.");
    }
    execute_plan(&plan, run_command)?;
    if matches!(plan, UninstallPlan::MacOsInstaller) && current_exe.exists() {
        return Err(TermKeyError::ConfigError(
            "The macOS uninstaller closed without removing this installation. It may have been cancelled; no vault data was removed.".into(),
        ));
    }
    print_success(
        "TermKey uninstalled. Your vault, settings, and encrypted backups were preserved.",
    );
    print_retained_vault_instructions(&crate::vault::storage::vault_dir());
    Ok(())
}

fn portable_plan(plan: UninstallPlan, windows: bool) -> UninstallPlan {
    match plan {
        UninstallPlan::Executable(path) if windows => UninstallPlan::WindowsPortable(path),
        other => other,
    }
}

fn powershell_remove_command(paths: &[PathBuf]) -> Option<String> {
    let values: Option<Vec<String>> = paths
        .iter()
        .map(|path| {
            let value = path.to_str()?;
            if value.chars().any(char::is_control) {
                return None;
            }
            Some(format!("'{}'", value.replace('\'', "''")))
        })
        .collect();
    Some(format!(
        "Remove-Item -LiteralPath @({}) -Force -ErrorAction Stop",
        values?.join(", ")
    ))
}

fn cleanup_command(
    data_directory: &Path,
    config_directory: &Path,
    windows: bool,
) -> Option<String> {
    let absolute = |directory: &Path| -> Option<PathBuf> {
        Some(if directory.is_absolute() {
            directory.to_owned()
        } else {
            std::env::current_dir().ok()?.join(directory)
        })
    };
    let data_directory = absolute(data_directory)?;
    let config_directory = absolute(config_directory)?;
    let mut paths = vec![
        data_directory.join("vault.ck"),
        data_directory.join("vault.tmp"),
        data_directory.join("vault.lock"),
        config_directory.join("config.json"),
        config_directory.join("config.tmp"),
        config_directory.join("config.lock"),
    ];
    for directory in [&data_directory, &config_directory] {
        if let Ok(entries) = fs::read_dir(directory) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name = name.to_str()?;
                if name.starts_with(".vault.ck.tmp-") || name.starts_with(".config.json.tmp-") {
                    paths.push(entry.path());
                }
            }
        }
    }
    paths.sort();
    paths.dedup();
    if windows {
        return powershell_remove_command(&paths);
    }
    let quoted: Option<Vec<String>> = paths
        .iter()
        .map(|path| {
            let value = path.to_str()?;
            if value.chars().any(char::is_control) {
                return None;
            }
            Some(format!("'{}'", value.replace('\'', "'\"'\"'")))
        })
        .collect();
    Some(format!("rm -f -- {}", quoted?.join(" ")))
}

#[cfg(test)]
fn retained_vault_cleanup_command(directory: &Path) -> Option<String> {
    cleanup_command(directory, directory, cfg!(windows))
}

fn print_retained_vault_instructions(directory: &Path) {
    println!(
        "  Vault retained at: {}",
        crate::links::sanitize_terminal_text(&directory.display().to_string())
    );
    println!("  To permanently delete its vault data and settings after uninstalling:");
    let config_path = crate::config::storage::config_path();
    let config_directory = config_path.parent().unwrap_or(directory);
    println!(
        "  Configuration retained at: {}",
        crate::links::sanitize_terminal_text(&config_path.display().to_string())
    );
    println!("  Close all TermKey instances before running the cleanup command.");
    if let Some(command) = cleanup_command(directory, config_directory, cfg!(windows)) {
        println!("    {command}");
    } else {
        println!("    Remove the vault and settings files manually from this folder.");
    }
    println!("  Exported backups are preserved; remove any unwanted backups separately.");
    println!("  You can also run `termkey reset` before uninstalling.");
}

fn plan_uninstall(
    current_exe: &Path,
    install_method: InstallMethod,
    is_macos: bool,
    installed_app_exists: bool,
    uninstaller_exists: bool,
) -> Result<UninstallPlan> {
    if matches!(install_method, InstallMethod::Homebrew) {
        return Ok(UninstallPlan::Homebrew);
    }

    if is_macos && !matches!(install_method, InstallMethod::SourceBuild) {
        let bundled_executable = Path::new(MACOS_APP).join("Contents/Resources/bin/termkey");
        let is_installer_location =
            current_exe == Path::new("/usr/local/bin/termkey") || current_exe == bundled_executable;
        if is_installer_location && uninstaller_exists {
            return Ok(UninstallPlan::MacOsInstaller);
        }
        if is_installer_location && installed_app_exists {
            return Err(TermKeyError::ConfigError(
                "The TermKey app is installed, but /Applications/Uninstall TermKey.app is missing. Reinstall the macOS package to restore its uninstaller, then run `termkey uninstall` again. Your vault has not been changed.".into(),
            ));
        }
    }

    Ok(UninstallPlan::Executable(current_exe.to_path_buf()))
}

fn execute_plan(
    plan: &UninstallPlan,
    mut command_runner: impl FnMut(&str, &[OsString]) -> Result<()>,
) -> Result<()> {
    match plan {
        UninstallPlan::Homebrew => {
            command_runner("brew", &["uninstall".into(), "termkey".into()])
        }
        UninstallPlan::MacOsInstaller => {
            command_runner("/usr/bin/open", &[
                "-W".into(), MACOS_UNINSTALLER.into(), "--args".into(),
                "--vault-dir".into(), crate::vault::storage::vault_dir().into_os_string(),
            ])
        }
        UninstallPlan::WindowsPortable(_) => Err(TermKeyError::ConfigError(
            "Close TermKey before removing the portable Windows executable.".into(),
        )),
        UninstallPlan::Executable(path) => fs::remove_file(path).map_err(|error| {
            TermKeyError::ConfigError(format!(
                "Could not remove executable '{}': {error}. Remove it using the permissions required by your installation. Your vault and settings were not changed.",
                crate::links::sanitize_terminal_text(&path.display().to_string())
            ))
        }),
    }
}

fn run_command(program: &str, args: &[OsString]) -> Result<()> {
    let status = Command::new(program).args(args).status().map_err(|error| {
        TermKeyError::ConfigError(format!(
            "Could not run {program}: {error}. Your vault and settings were not changed."
        ))
    })?;
    if !status.success() {
        return Err(TermKeyError::ConfigError(format!(
            "{program} did not complete the uninstall successfully ({status}). Your vault and settings were not changed."
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn portable_windows_uninstall_requires_close_and_literal_path_removal() {
        let path = PathBuf::from("C:/Users/Synthetic/O'Brien $(ignored)/termkey.exe");
        let plan = portable_plan(UninstallPlan::Executable(path.clone()), true);
        assert!(matches!(plan, UninstallPlan::WindowsPortable(_)));
        let command = powershell_remove_command(&[path]).unwrap();
        assert!(command.starts_with("Remove-Item -LiteralPath @("));
        assert!(command.contains("O''Brien $(ignored)"));
        assert!(
            execute_plan(&plan, |_, _| panic!("must not spawn a self-delete command")).is_err()
        );
    }

    #[test]
    fn cleanup_handles_separate_linux_data_and_configuration() {
        let dir = TempDir::new().unwrap();
        let data = dir.path().join("data");
        let config = dir.path().join("config");
        let command = cleanup_command(&data, &config, false).unwrap();
        assert!(command.contains(data.join("vault.ck").to_str().unwrap()));
        assert!(command.contains(config.join("config.json").to_str().unwrap()));
        assert!(!command.contains(data.join("config.json").to_str().unwrap()));
    }

    #[cfg(unix)]
    #[test]
    fn follow_up_command_works_without_termkey_and_preserves_backups() {
        let dir = TempDir::new().unwrap();
        let data_dir = dir.path().join("vault '$(touch injected)' folder");
        fs::create_dir(&data_dir).unwrap();
        for name in crate::commands::reset::TERMKEY_DATA_FILES {
            fs::write(data_dir.join(name), "data").unwrap();
        }
        let stale = data_dir.join(".vault.ck.tmp-test");
        fs::write(&stale, "encrypted temporary data").unwrap();
        let backup = data_dir.join("backup.termkey");
        fs::write(&backup, "backup").unwrap();
        let command = retained_vault_cleanup_command(&data_dir).unwrap();
        let status = Command::new("/bin/sh")
            .args(["-c", &command])
            .current_dir(dir.path())
            .status()
            .unwrap();
        assert!(status.success());
        for name in crate::commands::reset::TERMKEY_DATA_FILES {
            assert!(!data_dir.join(name).exists());
        }
        assert!(!stale.exists());
        assert_eq!(fs::read_to_string(backup).unwrap(), "backup");
        assert!(!dir.path().join("injected").exists());
    }

    #[test]
    fn cleanup_command_does_not_render_terminal_control_characters() {
        assert!(
            retained_vault_cleanup_command(Path::new("/tmp/vault\x1b]52;c;payload\x07")).is_none()
        );
    }

    #[test]
    fn homebrew_uninstall_uses_package_manager_without_data_removal_flags() {
        let plan = plan_uninstall(
            Path::new("/opt/homebrew/Cellar/termkey/1.0.2/bin/termkey"),
            InstallMethod::Homebrew,
            true,
            true,
            true,
        )
        .unwrap();
        let mut invocations = Vec::new();
        execute_plan(&plan, |program, args| {
            invocations.push((program.to_owned(), args.to_vec()));
            Ok(())
        })
        .unwrap();
        assert_eq!(
            invocations,
            vec![(
                "brew".to_string(),
                vec!["uninstall".into(), "termkey".into()]
            )]
        );
    }

    #[test]
    fn macos_installer_uses_companion_for_cli_and_app_bundle() {
        for binary in [
            "/usr/local/bin/termkey",
            "/Applications/TermKey.app/Contents/Resources/bin/termkey",
        ] {
            let plan = plan_uninstall(
                Path::new(binary),
                InstallMethod::DirectDownload,
                true,
                true,
                true,
            )
            .unwrap();
            let mut invocations = Vec::new();
            execute_plan(&plan, |program, args| {
                invocations.push((program.to_owned(), args.to_vec()));
                Ok(())
            })
            .unwrap();
            assert_eq!(
                invocations,
                vec![(
                    "/usr/bin/open".to_string(),
                    vec![
                        "-W".into(),
                        MACOS_UNINSTALLER.into(),
                        "--args".into(),
                        "--vault-dir".into(),
                        crate::vault::storage::vault_dir().into_os_string()
                    ]
                )]
            );
        }
    }

    #[test]
    fn installer_without_companion_refuses_partial_app_removal() {
        assert!(plan_uninstall(
            Path::new("/usr/local/bin/termkey"),
            InstallMethod::DirectDownload,
            true,
            true,
            false,
        )
        .is_err());
    }

    #[test]
    fn source_and_manual_uninstall_remove_only_exact_executable() {
        for method in [InstallMethod::SourceBuild, InstallMethod::DirectDownload] {
            let dir = TempDir::new().unwrap();
            let binary = dir.path().join("termkey");
            fs::write(&binary, b"fake executable").unwrap();
            let preserved = [
                "vault.ck",
                "config.json",
                "backup.termkey",
                "Cargo.toml",
                "other-termkey",
            ];
            for name in preserved {
                fs::write(dir.path().join(name), name).unwrap();
            }
            let plan = plan_uninstall(&binary, method, true, true, true).unwrap();
            assert_eq!(plan, UninstallPlan::Executable(binary.clone()));
            execute_plan(&plan, |_, _| {
                panic!("manual uninstall must not spawn commands")
            })
            .unwrap();
            assert!(!binary.exists());
            for name in preserved {
                assert_eq!(fs::read_to_string(dir.path().join(name)).unwrap(), name);
            }
        }
    }

    #[test]
    fn package_manager_failure_is_returned_without_manual_fallback() {
        let result = execute_plan(&UninstallPlan::Homebrew, |_, _| {
            Err(TermKeyError::ConfigError("simulated failure".into()))
        });
        assert!(matches!(result, Err(TermKeyError::ConfigError(_))));
    }
}
