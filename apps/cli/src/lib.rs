pub mod cli;
pub mod clipboard;
pub mod commands;
pub mod config;
pub mod crypto;
pub mod error;
pub mod links;
pub mod platform;
pub mod repl;
pub mod ui;
pub mod update;
pub mod vault;

use std::path::Path;

/// Validate storage paths before any command can create sensitive files.
pub fn initialize_storage_paths() -> error::Result<()> {
    platform::paths::validate()
}

pub fn apply_configured_vault_dir_override() {
    if platform::paths::has_explicit_override() {
        return;
    }
    if let Ok(cfg) = config::load_config() {
        let default_cfg = config::Config::default();
        if cfg.vault_path != default_cfg.vault_path {
            let configured = Path::new(&cfg.vault_path);
            // Foreign-platform or relative paths copied in config must not
            // redirect storage into a relative current-working-directory path.
            if configured.is_absolute() {
                if let Some(parent) = configured.parent() {
                    if let Ok(paths) = platform::paths::resolve() {
                        if paths.config_dir != paths.data_dir {
                            platform::paths::retain_config_directory(
                                parent.to_path_buf(),
                                paths.config_dir,
                            );
                        }
                    }
                    std::env::set_var("TERMKEY_VAULT_DIR", parent);
                }
            }
        }
    }
}
