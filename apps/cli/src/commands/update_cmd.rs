use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{Result, TermKeyError};
use crate::links;
use crate::ui::borders::print_success;
use crate::update::{self, UpdateStatus, CURRENT_VERSION, RELEASES_PAGE_URL};

pub fn run() -> Result<()> {
    let current_exe = std::env::current_exe().map_err(TermKeyError::Io)?;
    let install_method = detect_install_method(&current_exe);
    let update_status = update::get_update_status();

    println!();
    println!("  TermKey Update");
    println!("  ──────────────");
    println!("  Current version: v{}", CURRENT_VERSION);
    println!("  Latest release: {}", latest_release_label(&update_status));
    println!("  Install method: {}", install_method.label());
    println!();

    match install_method {
        InstallMethod::Homebrew => run_homebrew_update(&update_status),
        InstallMethod::DirectDownload => {
            print_manual_update_instructions(&update_status, install_method);
            Ok(())
        }
        InstallMethod::SourceBuild => {
            print_manual_update_instructions(&update_status, install_method);
            Ok(())
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InstallMethod {
    Homebrew,
    DirectDownload,
    SourceBuild,
}

impl InstallMethod {
    fn label(self) -> &'static str {
        match self {
            InstallMethod::Homebrew => "Homebrew",
            InstallMethod::DirectDownload => "Installer/manual download",
            InstallMethod::SourceBuild => "Local source build",
        }
    }
}

fn run_homebrew_update(update_status: &UpdateStatus) -> Result<()> {
    match update_status {
        UpdateStatus::UpToDate => {
            print_success(&format!(
                "Already on the latest release: v{}",
                CURRENT_VERSION
            ));
            println!("  No Homebrew update is needed right now.");
            return Ok(());
        }
        UpdateStatus::Available(info) => {
            println!(
                "  Update available: v{} -> v{}",
                CURRENT_VERSION, info.latest_version
            );
        }
        UpdateStatus::Unknown => {
            println!("  Could not verify the latest GitHub release.");
            println!("  Attempting a Homebrew refresh anyway.");
        }
    }

    run_brew_step(["update"], "`brew update` failed.")?;
    println!();
    run_brew_step(["upgrade", "termkey"], "`brew upgrade termkey` failed.")?;
    println!();
    print_success("Homebrew update completed.");
    println!("  Restart TermKey to use the updated binary.");

    Ok(())
}

fn run_brew_step<const N: usize>(args: [&str; N], failure_message: &str) -> Result<()> {
    println!("  Running: brew {}", args.join(" "));

    let status = Command::new("brew")
        .args(args)
        .status()
        .map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => TermKeyError::ConfigError(
                "Homebrew install detected, but `brew` is not available in PATH.".into(),
            ),
            _ => TermKeyError::Io(err),
        })?;

    if status.success() {
        Ok(())
    } else {
        Err(TermKeyError::ConfigError(failure_message.into()))
    }
}

fn print_manual_update_instructions(update_status: &UpdateStatus, install_method: InstallMethod) {
    match update_status {
        UpdateStatus::UpToDate => {
            print_success(&format!(
                "Already on the latest release: v{}",
                CURRENT_VERSION
            ));
            return;
        }
        UpdateStatus::Available(info) => {
            println!(
                "  Update available: v{} -> v{}",
                CURRENT_VERSION, info.latest_version
            );
        }
        UpdateStatus::Unknown => {
            println!("  Could not verify whether a newer release is available.");
        }
    }

    println!(
        "  {} does not support in-place updates from the CLI.",
        install_method.label()
    );
    println!("  Download the latest release here:");

    let release_url = release_url_for_status(update_status);
    println!(
        "  {}",
        links::format_terminal_hyperlink(&release_url, &release_url)
    );

    if matches!(install_method, InstallMethod::SourceBuild) {
        println!("  This binary appears to come from a local build, so you can also rebuild from source.");
    }
}

fn latest_release_label(update_status: &UpdateStatus) -> String {
    match update_status {
        UpdateStatus::Available(info) => format!("v{} (update available)", info.latest_version),
        UpdateStatus::UpToDate => format!("v{} (up to date)", CURRENT_VERSION),
        UpdateStatus::Unknown => "unknown".to_string(),
    }
}

fn release_url_for_status(update_status: &UpdateStatus) -> String {
    match update_status {
        UpdateStatus::Available(info) => update::release_page_url_for_version(&info.latest_version),
        _ => RELEASES_PAGE_URL.to_string(),
    }
}

fn detect_install_method(current_exe: &Path) -> InstallMethod {
    let candidate_paths = executable_candidate_paths(current_exe);

    if candidate_paths
        .iter()
        .any(|path| is_homebrew_install_path(path))
    {
        return InstallMethod::Homebrew;
    }

    if candidate_paths
        .iter()
        .any(|path| is_source_build_path(path))
    {
        return InstallMethod::SourceBuild;
    }

    InstallMethod::DirectDownload
}

fn executable_candidate_paths(current_exe: &Path) -> Vec<PathBuf> {
    let mut paths = vec![current_exe.to_path_buf()];

    if let Ok(canonical) = fs::canonicalize(current_exe) {
        if canonical != current_exe {
            paths.push(canonical);
        }
    }

    paths
}

fn is_homebrew_install_path(path: &Path) -> bool {
    let parts = path_components(path);

    parts
        .windows(2)
        .any(|window| window[0] == "Cellar" && window[1] == "termkey")
}

fn is_source_build_path(path: &Path) -> bool {
    let parts = path_components(path);

    parts
        .windows(2)
        .any(|window| window[0] == "target" && matches!(window[1].as_str(), "debug" | "release"))
}

fn path_components(path: &Path) -> Vec<String> {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_homebrew_cellar_paths() {
        let path = Path::new("/opt/homebrew/Cellar/termkey/0.2.22/bin/termkey");
        assert_eq!(detect_install_method(path), InstallMethod::Homebrew);
    }

    #[test]
    fn detects_source_build_paths() {
        let path = Path::new("/Users/test/termkey/target/release/termkey");
        assert_eq!(detect_install_method(path), InstallMethod::SourceBuild);
    }

    #[test]
    fn treats_installer_paths_as_direct_downloads() {
        let path = Path::new("/usr/local/bin/termkey");
        assert_eq!(detect_install_method(path), InstallMethod::DirectDownload);
    }

    #[test]
    fn prefers_specific_release_url_when_update_is_available() {
        let status = UpdateStatus::Available(update::UpdateInfo {
            latest_version: "0.2.23".to_string(),
        });

        assert_eq!(
            release_url_for_status(&status),
            "https://github.com/ryanonmars/termkey/releases/tag/v0.2.23"
        );
    }
}
