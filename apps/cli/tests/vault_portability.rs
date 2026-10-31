//! Synthetic fixtures generated with the production library, never cfg(test) KDFs.
//! CI exchanges these encrypted bundles between all five target executables.
use std::fs;
use std::path::Path;
use std::process::Command;

use chrono::{TimeZone, Utc};
use tempfile::TempDir;
use termkey::config::model::RecoveryConfigV2;
use termkey::crypto::{cipher, entry_key, kdf, recovery};
use termkey::vault::format::{decode_v3, decode_v3_with_dek, encode_v3};
use termkey::vault::model::{BackupHeader, Entry, SecretType, VaultData, VaultHeader};
use termkey::vault::session::VaultSession;
use termkey::vault::storage;
use zeroize::Zeroizing;

const MASTER: &str = "synthetic portability password 🔑";
const SECONDARY: &str = "synthetic secondary password";
const SECRET: &str = "synthetic-only secret, never a real credential";
const NEW_MASTER: &str = "synthetic rotated password";

fn password(value: &str) -> Zeroizing<String> {
    Zeroizing::new(value.to_owned())
}

fn sample_vault() -> VaultData {
    let now = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let mut plain = Entry {
        name: "Synthetic café 日本語".into(),
        secret: SECRET.into(),
        secret_type: SecretType::Password,
        network: String::new(),
        public_address: None,
        username: Some("synthetic-user".into()),
        url: Some("https://example.invalid/login".into()),
        notes: "Synthetic fixture only".into(),
        created_at: now,
        updated_at: now,
        has_secondary_password: false,
        entry_key_wrapped: None,
        entry_key_nonce: None,
        entry_key_salt: None,
        encrypted_secret: None,
        encrypted_secret_nonce: None,
    };
    let mut protected = plain.clone();
    protected.name = "Synthetic secondary-protected".into();
    let key = entry_key::generate_entry_key();
    let (secret, secret_nonce) = entry_key::encrypt_secret(&key, SECRET).unwrap();
    let (wrapped, nonce, salt) = entry_key::wrap_entry_key(&key, SECONDARY).unwrap();
    protected.secret = "[encrypted]".into();
    protected.has_secondary_password = true;
    protected.entry_key_wrapped = Some(wrapped);
    protected.entry_key_nonce = Some(nonce);
    protected.entry_key_salt = Some(salt);
    protected.encrypted_secret = Some(secret);
    protected.encrypted_secret_nonce = Some(secret_nonce);
    plain.notes = "Synthetic fixture; no production credentials".into();
    VaultData {
        entries: vec![plain, protected],
        version: 1,
        revision: 0,
    }
}

fn verify_contents(vault: &VaultData) {
    assert!(vault.validate().is_ok());
    assert!(vault.entries.len() == 2);
    assert!(vault.entries[0].name == "Synthetic café 日本語");
    assert!(vault.entries[0]
        .reveal_secret(None)
        .is_ok_and(|value| value.as_str() == SECRET));
    let entry = &vault.entries[1];
    assert!(entry.reveal_secret(None).is_err());
    assert!(entry
        .reveal_secret(Some("incorrect synthetic password"))
        .is_err());
    assert!(entry
        .reveal_secret(Some(SECONDARY))
        .is_ok_and(|value| value.as_str() == SECRET));
}

fn legacy_bytes(vault: &VaultData, version: u32, magic: &[u8; 4]) -> Vec<u8> {
    let salt = kdf::generate_salt();
    let nonce = cipher::generate_nonce();
    let key = kdf::derive_key(
        MASTER.as_bytes(),
        &salt,
        kdf::DEFAULT_M_COST,
        kdf::DEFAULT_T_COST,
        kdf::DEFAULT_P_COST,
    )
    .unwrap();
    let plaintext = Zeroizing::new(serde_json::to_vec(vault).unwrap());
    let ciphertext = cipher::encrypt(&key, &nonce, &plaintext).unwrap();
    let mut bytes = magic.to_vec();
    bytes.extend(version.to_le_bytes());
    if version == VaultHeader::FORMAT_VERSION_V2 {
        let metadata = serde_json::to_vec(&vault.metadata()).unwrap();
        bytes.extend((metadata.len() as u32).to_le_bytes());
        bytes.extend(metadata);
    }
    bytes.extend(salt);
    bytes.extend(kdf::DEFAULT_M_COST.to_le_bytes());
    bytes.extend(kdf::DEFAULT_T_COST.to_le_bytes());
    bytes.extend(kdf::DEFAULT_P_COST.to_le_bytes());
    bytes.extend(nonce);
    bytes.extend((ciphertext.len() as u32).to_le_bytes());
    bytes.extend(ciphertext);
    bytes
}

fn write_bundle(directory: &Path) {
    fs::create_dir_all(directory).unwrap();
    let vault = sample_vault();
    for (name, version) in [("v1.ck", 1), ("v2.ck", 2)] {
        fs::write(
            directory.join(name),
            legacy_bytes(&vault, version, VaultHeader::MAGIC),
        )
        .unwrap();
    }
    let encoded = encode_v3(&vault, MASTER.as_bytes(), VaultHeader::MAGIC, None, None).unwrap();
    let opened = decode_v3(MASTER.as_bytes(), &encoded, VaultHeader::MAGIC).unwrap();
    fs::write(directory.join("v3.ck"), encoded.as_slice()).unwrap();
    storage::write_backup(&vault, MASTER.as_bytes(), &directory.join("backup.termkey")).unwrap();
    fs::write(
        directory.join("legacy-backup.ck"),
        legacy_bytes(&vault, 1, BackupHeader::MAGIC),
    )
    .unwrap();
    let phrase = bip39::Mnemonic::from_entropy(&[7u8; 32]).unwrap();
    let config =
        recovery::create_recovery_config(opened.vault_id, &opened.dek, &phrase.to_string())
            .unwrap();
    fs::write(
        directory.join("recovery.json"),
        serde_json::to_vec(&config).unwrap(),
    )
    .unwrap();
}

fn read_bundle(directory: &Path) {
    // Receiving an artifact is a file transfer, not permission inheritance from
    // the producer's UID. Read private current-user copies on the destination.
    let received = TempDir::new().unwrap();
    for name in [
        "v1.ck",
        "v2.ck",
        "v3.ck",
        "backup.termkey",
        "legacy-backup.ck",
        "recovery.json",
    ] {
        fs::copy(directory.join(name), received.path().join(name)).unwrap();
    }
    let directory = received.path();
    for name in ["v1.ck", "v2.ck", "v3.ck"] {
        let source = directory.join(name);
        verify_contents(&storage::read_vault(MASTER.as_bytes(), &source).unwrap());
        assert!(storage::read_vault(b"incorrect password", &source).is_err());
        // Exercise migration/save/reopen on a private copy, never mutate received fixtures.
        let working = TempDir::new().unwrap();
        let path = working.path().join("vault.ck");
        fs::copy(&source, &path).unwrap();
        let mut session = VaultSession::open(password(MASTER), path.clone())
            .unwrap()
            .session;
        verify_contents(&session.vault);
        session.save().unwrap();
        verify_contents(
            &VaultSession::open(password(MASTER), path)
                .unwrap()
                .session
                .vault,
        );
        let mut corrupt = fs::read(&source).unwrap();
        let last = corrupt.last_mut().unwrap();
        *last ^= 1;
        let broken = working.path().join("corrupt.ck");
        fs::write(&broken, corrupt).unwrap();
        assert!(storage::read_vault(MASTER.as_bytes(), &broken).is_err());
    }
    for name in ["backup.termkey", "legacy-backup.ck"] {
        let vault = storage::read_backup(MASTER.as_bytes(), &directory.join(name)).unwrap();
        verify_contents(&vault);
        assert!(storage::read_backup(b"incorrect password", &directory.join(name)).is_err());
        let destination = TempDir::new().unwrap();
        let session = VaultSession::create(
            vault,
            password(NEW_MASTER),
            destination.path().join("vault.ck"),
        )
        .unwrap();
        verify_contents(&session.vault);
    }
    let bytes = fs::read(directory.join("v3.ck")).unwrap();
    let opened = decode_v3(MASTER.as_bytes(), &bytes, VaultHeader::MAGIC).unwrap();
    let config: RecoveryConfigV2 =
        serde_json::from_slice(&fs::read(directory.join("recovery.json")).unwrap()).unwrap();
    let phrase = bip39::Mnemonic::from_entropy(&[7u8; 32]).unwrap();
    let key = recovery::recover_dek(&config, opened.vault_id, &phrase.to_string()).unwrap();
    verify_contents(
        &decode_v3_with_dek(&key, &bytes, VaultHeader::MAGIC)
            .unwrap()
            .0,
    );
    let incorrect = bip39::Mnemonic::from_entropy(&[8u8; 32]).unwrap();
    assert!(recovery::recover_dek(&config, opened.vault_id, &incorrect.to_string()).is_err());
    let working = TempDir::new().unwrap();
    let path = working.path().join("vault.ck");
    fs::write(&path, &bytes).unwrap();
    let mut recovered =
        VaultSession::recover(key, opened.vault_id, password(NEW_MASTER), path.clone()).unwrap();
    verify_contents(&recovered.vault);
    recovered.save().unwrap();
    verify_contents(
        &VaultSession::open(password(NEW_MASTER), path.clone())
            .unwrap()
            .session
            .vault,
    );
    assert!(decode_v3_with_dek(&opened.dek, &fs::read(path).unwrap(), VaultHeader::MAGIC).is_err());
}

#[test]
fn production_vault_round_trip_and_fixture_exchange() {
    let temp = TempDir::new().unwrap();
    write_bundle(temp.path());
    read_bundle(temp.path());
    if let Some(output) = std::env::var_os("TERMKEY_PORTABILITY_OUT") {
        write_bundle(Path::new(&output));
    }
    if let Some(input) = std::env::var_os("TERMKEY_PORTABILITY_INPUT") {
        let root = Path::new(&input);
        let mut bundles = Vec::new();
        fn find(directory: &Path, bundles: &mut Vec<std::path::PathBuf>) {
            if directory.join("v3.ck").is_file() {
                bundles.push(directory.to_owned());
                return;
            }
            for entry in fs::read_dir(directory).unwrap() {
                let entry = entry.unwrap();
                if entry.file_type().unwrap().is_dir() {
                    find(&entry.path(), bundles);
                }
            }
        }
        find(root, &mut bundles);
        assert!(!bundles.is_empty(), "no synthetic fixture bundles found");
        if let Ok(expected) = std::env::var("TERMKEY_PORTABILITY_EXPECT_BUNDLES") {
            let expected: usize = expected.parse().expect("invalid expected bundle count");
            assert!(
                bundles.len() == expected,
                "cross-platform fixture set is incomplete"
            );
        }
        for bundle in bundles {
            read_bundle(&bundle);
        }
    }
}

#[test]
fn stale_writer_cannot_overwrite_newer_vault() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("vault.ck");
    let mut first = VaultSession::create(sample_vault(), password(MASTER), path.clone()).unwrap();
    let mut stale = VaultSession::open(password(MASTER), path.clone())
        .unwrap()
        .session;
    first.vault.entries[0].notes = "new synthetic metadata".into();
    first.save().unwrap();
    let expected = fs::read(&path).unwrap();
    stale.vault.entries[0].notes = "stale metadata".into();
    assert!(stale.save().is_err());
    assert!(fs::read(path).unwrap() == expected);
}

#[test]
fn lock_child_worker() {
    if let Some(path) = std::env::var_os("TERMKEY_TEST_LOCK_PATH") {
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .unwrap();
        assert!(
            fs4::FileExt::try_lock(&file).is_err(),
            "another process acquired an already-held vault lock"
        );
    }
}

#[test]
fn process_lock_exclusion() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("vault.lock");
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    fs4::FileExt::lock(&file).unwrap();
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "lock_child_worker"])
        .env("TERMKEY_TEST_LOCK_PATH", &path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "child process lock-exclusion test failed"
    );
}

#[cfg(windows)]
#[test]
fn windows_open_handle_and_readonly_target_preserve_existing_vault() {
    use std::os::windows::fs::OpenOptionsExt;
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("vault.ck");
    let mut session = VaultSession::create(sample_vault(), password(MASTER), path.clone()).unwrap();
    let expected = fs::read(&path).unwrap();
    let handle = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&path)
        .unwrap();
    session.vault.entries[0].notes = "must not persist".into();
    assert!(session.save().is_err());
    drop(handle);
    assert!(fs::read(&path).unwrap() == expected);
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&path, permissions).unwrap();
    assert!(session.save().is_err());
    assert!(fs::read(&path).unwrap() == expected);
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_readonly(false);
    fs::set_permissions(&path, permissions).unwrap();
    session.save().unwrap();
    verify_contents(
        &VaultSession::open(password(MASTER), path)
            .unwrap()
            .session
            .vault,
    );
}

#[test]
fn production_address_derivation_vectors() {
    use termkey::crypto::derive::derive_address;
    let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    #[cfg(feature = "derive-eth")]
    assert!(
        derive_address(
            "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
            &SecretType::PrivateKey,
            "Ethereum"
        )
        .unwrap()
        .as_deref()
            == Some("0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266")
    );
    #[cfg(feature = "derive-btc")]
    assert!(
        derive_address(mnemonic, &SecretType::SeedPhrase, "Bitcoin")
            .unwrap()
            .as_deref()
            == Some("bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu")
    );
    #[cfg(feature = "derive-sol")]
    assert!(
        derive_address(mnemonic, &SecretType::SeedPhrase, "Solana")
            .unwrap()
            .as_deref()
            == Some("HAgk14JpMQLgt6rVgv7cBQFJWFto5Dqxi472uT3DKpqk")
    );
    let _ = (derive_address, mnemonic);
}

#[cfg(target_os = "linux")]
#[test]
fn headless_clipboard_failure_never_falls_back_to_secret_output() {
    // Never touch a user's graphical clipboard; CI containers have no display.
    if std::env::var_os("DISPLAY").is_some()
        || std::env::var_os("WAYLAND_DISPLAY").is_some()
        || std::env::var_os("XDG_RUNTIME_DIR").is_some()
    {
        return;
    }
    let result = termkey::clipboard::copy_and_clear(password(SECRET), std::time::Duration::ZERO);
    assert!(
        result.is_err(),
        "headless environment unexpectedly provided a graphical clipboard"
    );
    if let Err(error) = result {
        assert!(!error.to_string().contains(SECRET));
    }
}
