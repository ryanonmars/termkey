use std::fs;
use std::path::{Path, PathBuf};

#[cfg(windows)]
use std::process::Command;

use serde_json::Value;

use crate::cli::BrowserCommands;
use crate::error::{Result, TermKeyError};
use crate::ui::borders::print_success;

const CHROME_EXTENSION_ID: &str = "fpnkkpgaogkddgangnphpgbbfdcpfjah";
const CHROME_NATIVE_HOST_NAME: &str = "com.ryanonmars.termkey";
const EXTENSION_SOURCE_ENV: &str = "TERMKEY_BROWSER_EXTENSION_SOURCE";
const NATIVE_HOST_BINARY_ENV: &str = "TERMKEY_NATIVE_HOST_BINARY";

pub fn run(command: &BrowserCommands) -> Result<()> {
    match command {
        BrowserCommands::Install => install_browser_support("installed"),
        BrowserCommands::Repair => install_browser_support("reinstalled"),
        BrowserCommands::Status => print_status(),
    }
}

fn install_browser_support(action: &str) -> Result<()> {
    let source_dir = locate_extension_source()?;
    let native_host_binary = locate_native_host_binary()?;
    let managed_extension_dir = managed_extension_dir();

    sync_directory(&source_dir, &managed_extension_dir)?;
    let manifest_path = install_native_host_manifest(&native_host_binary)?;

    print_success(&format!("Chrome integration {}.", action));
    println!();
    println!("  Stable Chrome extension ID: {}", CHROME_EXTENSION_ID);
    println!(
        "  Extension folder for Load unpacked: {}",
        managed_extension_dir.display()
    );
    println!("  Native host manifest: {}", manifest_path.display());
    println!();
    println!("  Next step in Chrome:");
    println!("  1. Open chrome://extensions");
    println!("  2. Turn on Developer mode");
    println!("  3. Click Load unpacked");
    println!("  4. Select {}", managed_extension_dir.display());
    println!();
    println!("  Run `termkey browser status` any time to verify the setup paths.");

    Ok(())
}

fn print_status() -> Result<()> {
    let bundled_extension_source = locate_extension_source().ok();
    let native_host_binary = locate_native_host_binary().ok();
    let managed_extension_dir = managed_extension_dir();
    let manifest_path = native_host_manifest_path()?;
    let manifest_status = native_host_manifest_status(&manifest_path)?;

    println!();
    println!("  TermKey Browser Integration");
    println!("  ───────────────────────────");
    println!("  Chrome extension ID: {}", CHROME_EXTENSION_ID);
    println!(
        "  Bundled extension source: {}",
        describe_optional_path(bundled_extension_source.as_deref())
    );
    println!(
        "  Managed extension folder: {}",
        describe_existing_path(&managed_extension_dir)
    );
    println!(
        "  Native host binary: {}",
        describe_optional_path(native_host_binary.as_deref())
    );
    println!(
        "  Chrome native host manifest: {} ({})",
        manifest_path.display(),
        manifest_status
    );
    println!();
    println!("  Chrome still requires one manual step for non-store extensions:");
    println!("  Load unpacked from {}", managed_extension_dir.display());
    println!("  after enabling Developer mode on chrome://extensions.");
    println!();
    println!("  Use `termkey browser install` if any of the paths above are missing or stale.");

    Ok(())
}

fn describe_optional_path(path: Option<&Path>) -> String {
    match path {
        Some(path) => describe_existing_path(path),
        None => "missing".to_string(),
    }
}

fn describe_existing_path(path: &Path) -> String {
    if path.exists() {
        format!("{} (present)", path.display())
    } else {
        format!("{} (missing)", path.display())
    }
}

fn managed_extension_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = current_user_home_dir() {
            return home
                .join("Applications")
                .join("TermKey Browser Extension");
        }
    }

    crate::vault::storage::vault_dir()
        .join("browser")
        .join("chrome-extension")
}

fn locate_extension_source() -> Result<PathBuf> {
    let current_exe = std::env::current_exe().map_err(TermKeyError::Io)?;
    let exe_dir = current_exe.parent().ok_or_else(|| {
        TermKeyError::ConfigError("Could not determine the TermKey executable directory.".into())
    })?;

    for candidate in extension_source_candidates(exe_dir, &current_exe) {
        if is_extension_bundle_dir(&candidate) {
            return Ok(candidate);
        }
    }

    Err(TermKeyError::ConfigError(
        "Chrome extension bundle not found. Build it with `npm run build:extension`, or use an installer that includes browser support.".into(),
    ))
}

fn extension_source_candidates(exe_dir: &Path, current_exe: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Ok(path) = std::env::var(EXTENSION_SOURCE_ENV) {
        candidates.push(PathBuf::from(path));
    }

    candidates.push(exe_dir.join("browser-extension").join("chrome"));
    candidates.push(exe_dir.join("browser-extension"));

    if let Some(resources_dir) = macos_resources_dir(current_exe) {
        candidates.push(resources_dir.join("browser-extension").join("chrome"));
    }

    if let Some(resources_dir) = macos_installed_app_resources_dir() {
        candidates.push(resources_dir.join("browser-extension").join("chrome"));
    }

    if let Some(repo_root) = repo_root_from_exe(exe_dir) {
        candidates.push(repo_root.join("browser-extension").join("chrome"));
        candidates.push(repo_root.join("apps").join("extension"));
    }

    candidates
}

fn is_extension_bundle_dir(path: &Path) -> bool {
    path.join("manifest.json").is_file()
        && path.join("popup.html").is_file()
        && path.join("dist").join("background.js").is_file()
}

fn locate_native_host_binary() -> Result<PathBuf> {
    let current_exe = std::env::current_exe().map_err(TermKeyError::Io)?;
    let exe_dir = current_exe.parent().ok_or_else(|| {
        TermKeyError::ConfigError("Could not determine the TermKey executable directory.".into())
    })?;

    for candidate in native_host_binary_candidates(exe_dir, &current_exe) {
        if candidate.is_file() {
            return Ok(candidate);
        }
    }

    Err(TermKeyError::ConfigError(
        "Native host binary not found. Reinstall TermKey or build `termkey-native-host` first."
            .into(),
    ))
}

fn native_host_binary_candidates(exe_dir: &Path, current_exe: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let binary_name = native_host_binary_name();

    if let Ok(path) = std::env::var(NATIVE_HOST_BINARY_ENV) {
        candidates.push(PathBuf::from(path));
    }

    candidates.push(exe_dir.join(binary_name));

    if let Some(resources_dir) = macos_resources_dir(current_exe) {
        candidates.push(resources_dir.join("bin").join(binary_name));
    }

    if let Some(resources_dir) = macos_installed_app_resources_dir() {
        candidates.push(resources_dir.join("bin").join(binary_name));
    }

    if let Some(repo_root) = repo_root_from_exe(exe_dir) {
        candidates.push(repo_root.join("target").join("debug").join(binary_name));
        candidates.push(repo_root.join("target").join("release").join(binary_name));
    }

    candidates
}

fn native_host_binary_name() -> &'static str {
    if cfg!(windows) {
        "termkey-native-host.exe"
    } else {
        "termkey-native-host"
    }
}

fn repo_root_from_exe(exe_dir: &Path) -> Option<PathBuf> {
    let target_dir = exe_dir.parent()?;
    if target_dir.file_name()?.to_str()? != "target" {
        return None;
    }

    Some(target_dir.parent()?.to_path_buf())
}

fn macos_resources_dir(current_exe: &Path) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let parent = current_exe.parent()?;

        if parent.file_name()?.to_str()? == "MacOS" {
            let contents_dir = parent.parent()?;
            if contents_dir.file_name()?.to_str()? != "Contents" {
                return None;
            }

            return Some(contents_dir.join("Resources"));
        }

        if parent.file_name()?.to_str()? == "bin" {
            let resources_dir = parent.parent()?;
            if resources_dir.file_name()?.to_str()? != "Resources" {
                return None;
            }

            let contents_dir = resources_dir.parent()?;
            if contents_dir.file_name()?.to_str()? != "Contents" {
                return None;
            }

            return Some(resources_dir.to_path_buf());
        }

        return None;
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = current_exe;
        None
    }
}

fn macos_installed_app_resources_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let path = PathBuf::from("/Applications/TermKey.app/Contents/Resources");
        if path.exists() {
            return Some(path);
        }
    }

    None
}

fn sync_directory(source: &Path, destination: &Path) -> Result<()> {
    if destination.exists() {
        fs::remove_dir_all(destination)?;
    }

    fs::create_dir_all(destination)?;
    copy_directory_recursive(source, destination)?;
    Ok(())
}

fn copy_directory_recursive(source: &Path, destination: &Path) -> Result<()> {
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let target_path = destination.join(entry.file_name());

        if file_type.is_dir() {
            fs::create_dir_all(&target_path)?;
            copy_directory_recursive(&entry.path(), &target_path)?;
        } else {
            fs::copy(entry.path(), &target_path)?;
        }
    }

    Ok(())
}

fn install_native_host_manifest(native_host_binary: &Path) -> Result<PathBuf> {
    let manifest_path = native_host_manifest_path()?;

    if let Some(parent) = manifest_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let manifest = serde_json::json!({
        "name": CHROME_NATIVE_HOST_NAME,
        "description": "TermKey native messaging host",
        "path": native_host_binary.display().to_string(),
        "type": "stdio",
        "allowed_origins": [format!("chrome-extension://{}/", CHROME_EXTENSION_ID)],
    });

    let rendered = serde_json::to_string_pretty(&manifest)?;
    fs::write(&manifest_path, rendered)?;
    set_native_host_manifest_permissions(&manifest_path)?;
    register_windows_native_host_manifest(&manifest_path)?;

    Ok(manifest_path)
}

fn native_host_manifest_path() -> Result<PathBuf> {
    let home = current_user_home_dir().ok_or_else(|| {
        TermKeyError::ConfigError("Could not determine the current user home directory.".into())
    })?;

    #[cfg(target_os = "macos")]
    {
        return Ok(home
            .join("Library")
            .join("Application Support")
            .join("Google")
            .join("Chrome")
            .join("NativeMessagingHosts")
            .join(format!("{CHROME_NATIVE_HOST_NAME}.json")));
    }

    #[cfg(target_os = "linux")]
    {
        return Ok(home
            .join(".config")
            .join("google-chrome")
            .join("NativeMessagingHosts")
            .join(format!("{CHROME_NATIVE_HOST_NAME}.json")));
    }

    #[cfg(windows)]
    {
        let local_app_data = std::env::var("LOCALAPPDATA").map_err(|_| {
            TermKeyError::ConfigError(
                "Could not determine LOCALAPPDATA for Chrome native host registration.".into(),
            )
        })?;

        return Ok(PathBuf::from(local_app_data)
            .join("TermKey")
            .join("ChromeNativeMessagingHosts")
            .join(format!("{CHROME_NATIVE_HOST_NAME}.json")));
    }

    #[allow(unreachable_code)]
    Err(TermKeyError::ConfigError(
        "Browser integration is not supported on this platform.".into(),
    ))
}

fn current_user_home_dir() -> Option<PathBuf> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
        .map(PathBuf::from)
}

fn native_host_manifest_status(manifest_path: &Path) -> Result<String> {
    if !manifest_path.exists() {
        return Ok("missing".to_string());
    }

    let contents = fs::read_to_string(manifest_path)?;
    let parsed: Value = serde_json::from_str(&contents)?;
    let expected_origin = format!("chrome-extension://{}/", CHROME_EXTENSION_ID);
    let allowed_origins = parsed
        .get("allowed_origins")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            TermKeyError::ConfigError(
                "Chrome native host manifest is missing allowed_origins.".into(),
            )
        })?;

    let has_expected_origin = allowed_origins
        .iter()
        .filter_map(Value::as_str)
        .any(|origin| origin == expected_origin);

    if !has_expected_origin {
        return Ok("stale extension ID".to_string());
    }

    let configured_path = parsed
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if configured_path.is_empty() {
        return Ok("stale binary path".to_string());
    }

    if !Path::new(configured_path).exists() {
        return Ok("missing binary".to_string());
    }

    Ok("ready".to_string())
}

#[cfg(unix)]
fn set_native_host_manifest_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o644))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_native_host_manifest_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(windows)]
fn register_windows_native_host_manifest(manifest_path: &Path) -> Result<()> {
    let status = Command::new("reg")
        .args([
            "add",
            &format!(
                r"HKCU\Software\Google\Chrome\NativeMessagingHosts\{}",
                CHROME_NATIVE_HOST_NAME
            ),
            "/ve",
            "/t",
            "REG_SZ",
            "/d",
            manifest_path.to_string_lossy().as_ref(),
            "/f",
        ])
        .status()
        .map_err(TermKeyError::Io)?;

    if !status.success() {
        return Err(TermKeyError::ConfigError(
            "Failed to register the Chrome native host in the Windows registry.".into(),
        ));
    }

    Ok(())
}

#[cfg(not(windows))]
fn register_windows_native_host_manifest(_manifest_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn extension_bundle_validation_requires_built_artifacts() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("manifest.json"), "{}").unwrap();
        fs::write(dir.path().join("popup.html"), "<!doctype html>").unwrap();
        fs::create_dir_all(dir.path().join("dist")).unwrap();
        fs::write(
            dir.path().join("dist").join("background.js"),
            "console.log('ok');",
        )
        .unwrap();

        assert!(is_extension_bundle_dir(dir.path()));
    }

    #[test]
    fn sync_directory_replaces_previous_contents() {
        let source = TempDir::new().unwrap();
        let destination = TempDir::new().unwrap();

        fs::write(source.path().join("manifest.json"), "{}").unwrap();
        fs::write(source.path().join("popup.html"), "<!doctype html>").unwrap();
        fs::create_dir_all(source.path().join("dist")).unwrap();
        fs::write(
            source.path().join("dist").join("background.js"),
            "console.log('ok');",
        )
        .unwrap();
        fs::write(destination.path().join("old.txt"), "stale").unwrap();

        let target = destination.path().join("chrome-extension");
        sync_directory(source.path(), &target).unwrap();

        assert!(target.join("manifest.json").exists());
        assert!(target.join("dist").join("background.js").exists());
        assert!(!target.join("old.txt").exists());
    }
}
