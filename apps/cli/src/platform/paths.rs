use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::error::{Result, TermKeyError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoragePaths {
    pub data_dir: PathBuf,
    pub config_dir: PathBuf,
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
enum Platform {
    MacOs,
    Linux,
    Windows,
}

fn error(message: &str) -> TermKeyError {
    TermKeyError::ConfigError(message.to_string())
}

fn absolute(value: OsString, name: &str) -> Result<PathBuf> {
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(error(&format!(
            "{name} must be an absolute directory path."
        )));
    }
    Ok(path)
}

fn nonempty(value: Option<OsString>) -> Option<OsString> {
    value.filter(|value| !value.is_empty())
}

fn resolve_with(
    platform: Platform,
    env: impl Fn(&str) -> Option<OsString>,
    legacy_exists: impl Fn(&Path) -> bool,
    honor_override: bool,
) -> Result<StoragePaths> {
    if honor_override {
        if let Some(value) = nonempty(env("TERMKEY_VAULT_DIR")) {
            let directory = absolute(value, "TERMKEY_VAULT_DIR")?;
            return Ok(StoragePaths {
                data_dir: directory.clone(),
                config_dir: directory,
            });
        }
    }
    let home = match platform {
        Platform::Windows => nonempty(env("USERPROFILE")).or_else(|| nonempty(env("HOME"))),
        _ => nonempty(env("HOME")),
    }.ok_or_else(|| error("Cannot determine your home directory. Set an absolute TERMKEY_VAULT_DIR before starting TermKey."))?;
    let home = absolute(home, "Home directory")?;
    let legacy = home.join(".termkey");
    if matches!(platform, Platform::Linux)
        && !legacy_exists(&legacy.join("vault.ck"))
        && !legacy_exists(&legacy.join("config.json"))
    {
        // The XDG specification says to ignore relative environment values.
        let xdg = |name: &str, fallback: PathBuf| {
            nonempty(env(name))
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .unwrap_or(fallback)
                .join("termkey")
        };
        return Ok(StoragePaths {
            data_dir: xdg("XDG_DATA_HOME", home.join(".local/share")),
            config_dir: xdg("XDG_CONFIG_HOME", home.join(".config")),
        });
    }
    Ok(StoragePaths {
        data_dir: legacy.clone(),
        config_dir: legacy,
    })
}

fn platform() -> Platform {
    #[cfg(target_os = "windows")]
    {
        Platform::Windows
    }
    #[cfg(target_os = "linux")]
    {
        Platform::Linux
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        Platform::MacOs
    }
}

// A configured data path must not relocate a separate XDG config directory.
// Only startup records this association; explicit environment changes to a
// different data directory still receive their normal co-located config.
static CONFIG_DIRECTORY: OnceLock<(PathBuf, PathBuf)> = OnceLock::new();

pub(crate) fn retain_config_directory(data_dir: PathBuf, config_dir: PathBuf) {
    let _ = CONFIG_DIRECTORY.set((data_dir, config_dir));
}

pub fn resolve() -> Result<StoragePaths> {
    let mut paths = resolve_with(
        platform(),
        |name| std::env::var_os(name),
        Path::exists,
        true,
    )?;
    if let Some((data_dir, config_dir)) = CONFIG_DIRECTORY.get() {
        if &paths.data_dir == data_dir {
            paths.config_dir = config_dir.clone();
        }
    }
    Ok(paths)
}

pub fn default_paths() -> Result<StoragePaths> {
    resolve_with(
        platform(),
        |name| std::env::var_os(name),
        Path::exists,
        false,
    )
}

pub fn validate() -> Result<()> {
    resolve().map(|_| ())
}

pub(crate) fn has_explicit_override() -> bool {
    nonempty(std::env::var_os("TERMKEY_VAULT_DIR")).is_some()
}

pub(crate) fn data_dir() -> PathBuf {
    resolve()
        .expect("TermKey storage paths must be validated before accessing the vault")
        .data_dir
}

pub(crate) fn config_dir() -> PathBuf {
    resolve()
        .expect("TermKey storage paths must be validated before accessing configuration")
        .config_dir
}

pub(crate) fn default_vault_path() -> String {
    data_dir().join("vault.ck").to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn environment<'a>(values: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
        move |key| {
            values
                .iter()
                .find(|(name, _)| *name == key)
                .map(|(_, value)| OsString::from(value))
        }
    }
    #[test]
    fn missing_home_never_falls_back_to_working_directory() {
        for platform in [Platform::MacOs, Platform::Linux, Platform::Windows] {
            assert!(resolve_with(platform, |_| None, |_| false, true).is_err());
        }
    }
    #[test]
    fn explicit_override_does_not_require_home() {
        let directory = std::env::temp_dir().join("termkey-test-override");
        let paths = resolve_with(
            Platform::MacOs,
            |key| (key == "TERMKEY_VAULT_DIR").then(|| directory.clone().into_os_string()),
            |_| false,
            true,
        )
        .unwrap();
        assert_eq!(paths.data_dir, directory);
        assert_eq!(paths.config_dir, paths.data_dir);
    }
    #[cfg(unix)]
    #[test]
    fn linux_xdg_and_legacy_layouts_are_distinct_without_migration() {
        let env = environment(&[
            ("HOME", "/home/synthetic"),
            ("XDG_DATA_HOME", "/data"),
            ("XDG_CONFIG_HOME", "/config"),
        ]);
        let new = resolve_with(Platform::Linux, &env, |_| false, true).unwrap();
        assert_eq!(new.data_dir, Path::new("/data/termkey"));
        assert_eq!(new.config_dir, Path::new("/config/termkey"));
        let legacy = resolve_with(
            Platform::Linux,
            &env,
            |path| path.ends_with(".termkey/vault.ck"),
            true,
        )
        .unwrap();
        assert_eq!(legacy.data_dir, Path::new("/home/synthetic/.termkey"));
        assert_eq!(legacy.config_dir, legacy.data_dir);
    }
    #[cfg(unix)]
    #[test]
    fn relative_xdg_is_ignored_and_macos_layout_is_unchanged() {
        let env = environment(&[("HOME", "/home/synthetic"), ("XDG_DATA_HOME", "relative")]);
        let linux = resolve_with(Platform::Linux, &env, |_| false, true).unwrap();
        assert_eq!(
            linux.data_dir,
            Path::new("/home/synthetic/.local/share/termkey")
        );
        let mac = resolve_with(Platform::MacOs, &env, |_| false, true).unwrap();
        assert_eq!(mac.data_dir, Path::new("/home/synthetic/.termkey"));
    }
    #[cfg(windows)]
    #[test]
    fn windows_profile_wins_over_shell_home() {
        let env = environment(&[
            ("HOME", r"C:\shell-home"),
            ("USERPROFILE", r"C:\Users\synthetic"),
        ]);
        let paths = resolve_with(Platform::Windows, &env, |_| false, true).unwrap();
        assert_eq!(paths.data_dir, Path::new(r"C:\Users\synthetic\.termkey"));
    }
}
