use std::io::{self, ErrorKind, Read, Write};

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;
use termkey::{apply_configured_vault_dir_override, config, crypto, vault};
use termkey::vault::model::{Entry, SecretType};

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum NativeRequest {
    Ping,
    Status,
    GetAutofillEntry { id: String },
    FindSiteMatches { url: String },
    ListEntries,
    Unlock { password: String },
}

#[derive(Default)]
struct HostState {
    unlocked_password: Option<Zeroizing<String>>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusResponse {
    app: &'static str,
    version: &'static str,
    vault_path: String,
    vault_exists: bool,
    first_run_complete: bool,
    recovery_configured: bool,
    locked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedSite {
    origin: String,
    hostname: String,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct EntrySummary {
    id: String,
    name: String,
    secret_type: String,
    network: String,
    has_secondary_password: bool,
    public_address: Option<String>,
    username: Option<String>,
    url: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct SiteMatchSummary {
    id: String,
    name: String,
    username: Option<String>,
    url: Option<String>,
    match_type: &'static str,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct SiteMatchesResponse {
    site_url: String,
    site_origin: String,
    site_hostname: String,
    matches: Vec<SiteMatchSummary>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct AutofillEntryResponse {
    id: String,
    name: String,
    username: Option<String>,
    password: String,
    url: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum NativeResponse {
    Pong { app: &'static str, version: &'static str },
    Status(StatusResponse),
    AutofillEntry { entry: AutofillEntryResponse },
    SiteMatches(SiteMatchesResponse),
    ListEntries { entries: Vec<EntrySummary> },
    Unlock { unlocked: bool },
    Error { message: String },
}

fn load_status_for_state(state: &HostState) -> StatusResponse {
    let config = config::load_config().unwrap_or_default();
    let vault_exists = vault::storage::vault_exists();

    StatusResponse {
        app: "termkey",
        version: env!("CARGO_PKG_VERSION"),
        vault_path: vault::storage::vault_path().display().to_string(),
        vault_exists,
        first_run_complete: config.first_run_complete,
        recovery_configured: config.recovery.is_some(),
        locked: vault_exists && state.unlocked_password.is_none(),
    }
}

fn read_message(reader: &mut impl Read) -> io::Result<Option<Vec<u8>>> {
    let mut len_buf = [0_u8; 4];
    let mut read_len = 0;

    while read_len < len_buf.len() {
        let bytes_read = reader.read(&mut len_buf[read_len..])?;
        if bytes_read == 0 {
            if read_len == 0 {
                return Ok(None);
            }

            return Err(io::Error::new(
                ErrorKind::UnexpectedEof,
                "native host payload length prefix was truncated",
            ));
        }

        read_len += bytes_read;
    }

    let payload_len = u32::from_le_bytes(len_buf) as usize;
    let mut payload = vec![0_u8; payload_len];
    reader.read_exact(&mut payload)?;
    Ok(Some(payload))
}

fn write_message(writer: &mut impl Write, response: &NativeResponse) -> io::Result<()> {
    let payload = serde_json::to_vec(response).map_err(io::Error::other)?;
    writer.write_all(&(payload.len() as u32).to_le_bytes())?;
    writer.write_all(&payload)?;
    writer.flush()
}

fn unlock_vault(state: &mut HostState, password: String) -> NativeResponse {
    if !vault::storage::vault_exists() {
        return NativeResponse::Error {
            message: "Vault not found. Run `termkey init` first.".to_string(),
        };
    }

    match vault::storage::read_vault(password.as_bytes(), &vault::storage::vault_path()) {
        Ok(_) => {
            state.unlocked_password = Some(Zeroizing::new(password));
            NativeResponse::Unlock { unlocked: true }
        }
        Err(err) => NativeResponse::Error {
            message: err.to_string(),
        },
    }
}

fn require_unlocked_password(state: &HostState) -> Result<&str, NativeResponse> {
    state
        .unlocked_password
        .as_deref()
        .ok_or_else(|| NativeResponse::Error {
            message: "Vault is locked. Unlock it first.".to_string(),
        })
        .map(|value| value.as_str())
}

fn summarize_entry(index: usize, entry: &Entry) -> EntrySummary {
    EntrySummary {
        id: (index + 1).to_string(),
        name: entry.name.clone(),
        secret_type: match &entry.secret_type {
            SecretType::PrivateKey => "Private Key".to_string(),
            SecretType::SeedPhrase => "Seed Phrase".to_string(),
            SecretType::Password => "Password".to_string(),
            SecretType::Other(label) => {
                if label.trim().is_empty() {
                    "Other".to_string()
                } else {
                    label.trim().to_string()
                }
            }
        },
        network: entry.network.clone(),
        has_secondary_password: entry.has_secondary_password,
        public_address: entry.public_address.clone(),
        username: entry.username.clone(),
        url: entry.url.clone(),
    }
}

fn parse_site(input: &str) -> Option<ParsedSite> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    let normalized = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };

    let scheme_end = normalized.find("://")?;
    let scheme = &normalized[..scheme_end];
    let remainder = &normalized[scheme_end + 3..];
    let authority_end = remainder
        .find(|ch| matches!(ch, '/' | '?' | '#'))
        .unwrap_or(remainder.len());
    let authority = &remainder[..authority_end];
    if authority.is_empty() {
        return None;
    }

    let authority_without_userinfo = authority.rsplit('@').next()?;
    let host_port = authority_without_userinfo.trim();
    if host_port.is_empty() {
        return None;
    }

    let hostname = if host_port.starts_with('[') {
        let closing = host_port.find(']')?;
        host_port[..=closing].to_ascii_lowercase()
    } else {
        host_port
            .split(':')
            .next()
            .map(str::to_ascii_lowercase)?
    };

    Some(ParsedSite {
        origin: format!("{scheme}://{}", authority_without_userinfo.to_ascii_lowercase()),
        hostname,
    })
}

fn classify_site_match(current: &ParsedSite, candidate: &ParsedSite) -> Option<&'static str> {
    if current.origin == candidate.origin {
        return Some("exact_origin");
    }

    if current.hostname == candidate.hostname {
        return Some("exact_host");
    }

    if current
        .hostname
        .ends_with(&format!(".{}", candidate.hostname))
    {
        return Some("subdomain");
    }

    None
}

fn match_rank(match_type: &str) -> u8 {
    match match_type {
        "exact_origin" => 3,
        "exact_host" => 2,
        "subdomain" => 1,
        _ => 0,
    }
}

fn find_site_matches(state: &HostState, site_url: String) -> NativeResponse {
    let password = match require_unlocked_password(state) {
        Ok(password) => password,
        Err(response) => return response,
    };

    let current_site = match parse_site(&site_url) {
        Some(site) => site,
        None => {
            return NativeResponse::Error {
                message: "Current tab URL is not a supported website.".to_string(),
            }
        }
    };

    let vault = match vault::storage::read_vault(password.as_bytes(), &vault::storage::vault_path()) {
        Ok(vault) => vault,
        Err(err) => {
            return NativeResponse::Error {
                message: err.to_string(),
            }
        }
    };

    let mut matches: Vec<(u8, SiteMatchSummary)> = vault
        .entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.secret_type == SecretType::Password)
        .filter_map(|(index, entry)| {
            let stored_url = entry.url.as_deref()?;
            let stored_site = parse_site(stored_url)?;
            let match_type = classify_site_match(&current_site, &stored_site)?;

            Some((
                match_rank(match_type),
                SiteMatchSummary {
                    id: (index + 1).to_string(),
                    name: entry.name.clone(),
                    username: entry.username.clone(),
                    url: entry.url.clone(),
                    match_type,
                },
            ))
        })
        .collect();

    matches.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| left.1.name.to_lowercase().cmp(&right.1.name.to_lowercase()))
    });

    NativeResponse::SiteMatches(SiteMatchesResponse {
        site_url,
        site_origin: current_site.origin,
        site_hostname: current_site.hostname,
        matches: matches.into_iter().map(|(_, summary)| summary).collect(),
    })
}

fn list_entries(state: &HostState) -> NativeResponse {
    let password = match require_unlocked_password(state) {
        Ok(password) => password,
        Err(response) => return response,
    };

    let vault = match vault::storage::read_vault(password.as_bytes(), &vault::storage::vault_path()) {
        Ok(vault) => vault,
        Err(err) => {
            return NativeResponse::Error {
                message: err.to_string(),
            }
        }
    };

    let entries = vault
        .entries
        .iter()
        .enumerate()
        .map(|(index, entry)| summarize_entry(index, entry))
        .collect();

    NativeResponse::ListEntries { entries }
}

fn get_autofill_entry(state: &HostState, id: String) -> NativeResponse {
    let password = match require_unlocked_password(state) {
        Ok(password) => password,
        Err(response) => return response,
    };

    let vault = match vault::storage::read_vault(password.as_bytes(), &vault::storage::vault_path()) {
        Ok(vault) => vault,
        Err(err) => {
            return NativeResponse::Error {
                message: err.to_string(),
            }
        }
    };

    let entry = match vault.find_entry_by_id(&id) {
        Some(entry) => entry,
        None => {
            return NativeResponse::Error {
                message: format!("Entry '{id}' not found."),
            }
        }
    };

    if entry.secret_type != SecretType::Password {
        return NativeResponse::Error {
            message: "Selected entry is not a password entry.".to_string(),
        };
    }

    if entry.has_secondary_password {
        return NativeResponse::Error {
            message: "Entries protected by a secondary password are not supported for autofill yet."
                .to_string(),
        };
    }

    NativeResponse::AutofillEntry {
        entry: AutofillEntryResponse {
            id,
            name: entry.name.clone(),
            username: entry.username.clone(),
            password: entry.secret.clone(),
            url: entry.url.clone(),
        },
    }
}

fn handle_request(state: &mut HostState, payload: &[u8]) -> NativeResponse {
    let request = match serde_json::from_slice::<NativeRequest>(payload) {
        Ok(request) => request,
        Err(err) => {
            return NativeResponse::Error {
                message: format!("invalid request: {err}"),
            }
        }
    };

    match request {
        NativeRequest::Ping => NativeResponse::Pong {
            app: "termkey",
            version: env!("CARGO_PKG_VERSION"),
        },
        NativeRequest::Status => NativeResponse::Status(load_status_for_state(state)),
        NativeRequest::GetAutofillEntry { id } => get_autofill_entry(state, id),
        NativeRequest::FindSiteMatches { url } => find_site_matches(state, url),
        NativeRequest::ListEntries => list_entries(state),
        NativeRequest::Unlock { password } => unlock_vault(state, password),
    }
}

fn main() -> io::Result<()> {
    crypto::secure::harden_process();
    apply_configured_vault_dir_override();

    let mut stdin = io::stdin().lock();
    let mut stdout = io::stdout().lock();
    let mut state = HostState::default();

    while let Some(payload) = read_message(&mut stdin)? {
        let response = handle_request(&mut state, &payload);
        write_message(&mut stdout, &response)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        classify_site_match, handle_request, load_status_for_state, parse_site, read_message,
        write_message, HostState, NativeResponse,
    };
    use chrono::Utc;
    use std::io::Cursor;
    use std::sync::{Mutex, OnceLock};
    use tempfile::TempDir;
    use termkey::vault::model::{Entry, SecretType, VaultData};
    use termkey::vault::storage::write_vault;
    use zeroize::Zeroizing;

    fn env_lock() -> &'static Mutex<()> {
        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        ENV_LOCK.get_or_init(|| Mutex::new(()))
    }

    fn test_vault_with_entry() -> VaultData {
        VaultData {
            entries: vec![Entry {
                name: "Email".to_string(),
                secret: "super-secret".to_string(),
                secret_type: SecretType::Password,
                network: "Password".to_string(),
                public_address: None,
                username: Some("ryan".to_string()),
                url: Some("https://example.com".to_string()),
                notes: String::new(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                has_secondary_password: false,
                entry_key_wrapped: None,
                entry_key_nonce: None,
                entry_key_salt: None,
                encrypted_secret: None,
                encrypted_secret_nonce: None,
            }],
            version: 1,
        }
    }

    #[test]
    fn ping_returns_pong() {
        let response = handle_request(&mut HostState::default(), br#"{"type":"ping"}"#);

        assert_eq!(
            response,
            NativeResponse::Pong {
                app: "termkey",
                version: env!("CARGO_PKG_VERSION"),
            }
        );
    }

    #[test]
    fn invalid_json_returns_error() {
        let response = handle_request(&mut HostState::default(), b"not-json");

        assert!(matches!(response, NativeResponse::Error { .. }));
    }

    #[test]
    fn status_returns_status_payload() {
        let response = handle_request(&mut HostState::default(), br#"{"type":"status"}"#);

        assert!(matches!(response, NativeResponse::Status(_)));
    }

    #[test]
    fn status_is_unlocked_when_state_has_password() {
        let state = HostState {
            unlocked_password: Some(Zeroizing::new("secret".to_string())),
        };

        let status = load_status_for_state(&state);

        if status.vault_exists {
            assert!(!status.locked);
        }
    }

    #[test]
    fn unlock_succeeds_with_valid_password() {
        let _guard = env_lock().lock().unwrap();
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("vault.ck");
        write_vault(&VaultData::new(), b"correct horse battery staple", &path).unwrap();

        let previous_vault_dir = std::env::var_os("TERMKEY_VAULT_DIR");
        std::env::set_var("TERMKEY_VAULT_DIR", dir.path());
        let response = handle_request(
            &mut HostState::default(),
            br#"{"type":"unlock","password":"correct horse battery staple"}"#,
        );
        match previous_vault_dir {
            Some(value) => std::env::set_var("TERMKEY_VAULT_DIR", value),
            None => std::env::remove_var("TERMKEY_VAULT_DIR"),
        }

        assert!(matches!(response, NativeResponse::Unlock { unlocked: true }));
    }

    #[test]
    fn list_entries_requires_unlock() {
        let response = handle_request(&mut HostState::default(), br#"{"type":"list_entries"}"#);

        assert!(matches!(response, NativeResponse::Error { .. }));
    }

    #[test]
    fn list_entries_returns_metadata_when_unlocked() {
        let _guard = env_lock().lock().unwrap();
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("vault.ck");
        write_vault(&test_vault_with_entry(), b"correct horse battery staple", &path).unwrap();

        let previous_vault_dir = std::env::var_os("TERMKEY_VAULT_DIR");
        std::env::set_var("TERMKEY_VAULT_DIR", dir.path());

        let mut state = HostState {
            unlocked_password: Some(Zeroizing::new("correct horse battery staple".to_string())),
        };
        let response = handle_request(&mut state, br#"{"type":"list_entries"}"#);

        match previous_vault_dir {
            Some(value) => std::env::set_var("TERMKEY_VAULT_DIR", value),
            None => std::env::remove_var("TERMKEY_VAULT_DIR"),
        }

        match response {
            NativeResponse::ListEntries { entries } => {
                assert_eq!(entries.len(), 1);
                assert_eq!(entries[0].name, "Email");
                assert_eq!(entries[0].username.as_deref(), Some("ryan"));
            }
            other => panic!("unexpected response: {:?}", other),
        }
    }

    #[test]
    fn parse_site_supports_full_urls_and_host_only_values() {
        let full = parse_site("https://accounts.example.com/login?next=1").unwrap();
        let host_only = parse_site("accounts.example.com").unwrap();

        assert_eq!(full.origin, "https://accounts.example.com");
        assert_eq!(full.hostname, "accounts.example.com");
        assert_eq!(host_only.hostname, "accounts.example.com");
    }

    #[test]
    fn classify_site_match_prefers_origin_then_host_then_subdomain() {
        let current = parse_site("https://app.example.com/login").unwrap();
        let exact_origin = parse_site("https://app.example.com").unwrap();
        let exact_host = parse_site("http://app.example.com").unwrap();
        let subdomain = parse_site("example.com").unwrap();

        assert_eq!(classify_site_match(&current, &exact_origin), Some("exact_origin"));
        assert_eq!(classify_site_match(&current, &exact_host), Some("exact_host"));
        assert_eq!(classify_site_match(&current, &subdomain), Some("subdomain"));
    }

    #[test]
    fn message_roundtrip_uses_native_framing() {
        let mut out = Vec::new();
        write_message(
            &mut out,
            &NativeResponse::Pong {
                app: "termkey",
                version: env!("CARGO_PKG_VERSION"),
            },
        )
        .unwrap();

        let payload = read_message(&mut Cursor::new(out)).unwrap().unwrap();
        let decoded: serde_json::Value = serde_json::from_slice(&payload).unwrap();

        assert_eq!(decoded["type"], "pong");
        assert_eq!(decoded["app"], "termkey");
    }
}
