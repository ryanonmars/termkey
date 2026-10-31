use crate::config;
use crate::error::{Result, TermKeyError};
use crate::vault::{session::VaultSession, storage};
use zeroize::Zeroizing;

pub fn run() -> Result<()> {
    let cfg = config::load_config()?;
    let recovery = match cfg
        .recovery
        .as_ref()
        .ok_or(TermKeyError::RecoveryNotConfigured)?
    {
        config::RecoveryConfig::Legacy(_) => return Err(TermKeyError::LegacyRecoveryUnsupported),
        config::RecoveryConfig::V2(recovery) => recovery,
    };

    let bytes = storage::read_encoded_file(&storage::vault_path())?;
    let header =
        crate::vault::format::parse_header(&bytes, crate::vault::model::VaultHeader::MAGIC)?;
    if !recovery.is_valid_for(header.vault_id) {
        return Err(TermKeyError::RecoveryNotConfigured);
    }

    let phrase = Zeroizing::new(
        rpassword::prompt_password("24-word recovery phrase: ").map_err(TermKeyError::Io)?,
    );
    let dek = crate::crypto::recovery::recover_dek(recovery, recovery.vault_id, &phrase)?;
    let new_password = crate::commands::passwd::prompt_new_password()?;
    VaultSession::recover(dek, recovery.vault_id, new_password, storage::vault_path())?;
    crate::ui::borders::print_success("Vault recovered with a new encryption key. The previous recovery phrase is no longer active. Configure a new recovery phrase in Settings.");
    Ok(())
}
