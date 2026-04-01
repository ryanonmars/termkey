use std::io::{self, ErrorKind, Read, Write};
use std::net::IpAddr;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use termkey::vault::model::EntryMeta;
use zeroize::Zeroizing;
use termkey::{apply_configured_vault_dir_override, config, crypto, vault};
use termkey::vault::model::{Entry, SecretType};

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum NativeRequest {
    Ping,
    Status,
    GeneratePassword,
    GetAutofillEntry {
        id: String,
        #[serde(default)]
        password: Option<String>,
        #[serde(default)]
        #[serde(alias = "secondaryPassword")]
        secondary_password: Option<String>,
    },
    FindSiteMatches { url: String },
    SavePasswordEntry {
        name: String,
        #[serde(default)]
        username: Option<String>,
        password: String,
        #[serde(default)]
        url: Option<String>,
        #[serde(default)]
        #[serde(alias = "masterPassword")]
        master_password: Option<String>,
        #[serde(default)]
        #[serde(alias = "secondaryPassword")]
        secondary_password: Option<String>,
    },
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
    registrable_domain: Option<String>,
    has_explicit_port: bool,
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
    has_secondary_password: bool,
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
    GeneratedPassword { password: String },
    AutofillEntry { entry: AutofillEntryResponse },
    SaveEntry { entry_name: String },
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

fn summarize_site_match(index: usize, entry: &EntryMeta, match_type: &'static str) -> SiteMatchSummary {
    SiteMatchSummary {
        id: (index + 1).to_string(),
        name: entry.name.clone(),
        username: entry.username.clone(),
        url: entry.url.clone(),
        match_type,
        has_secondary_password: entry.has_secondary_password,
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
    let has_explicit_port = if host_port.starts_with('[') {
        host_port[hostname.len()..].starts_with(':')
    } else {
        host_port.contains(':')
    };

    Some(ParsedSite {
        origin: format!("{scheme}://{}", authority_without_userinfo.to_ascii_lowercase()),
        registrable_domain: registrable_domain(&hostname),
        hostname,
        has_explicit_port,
    })
}

fn registrable_domain(hostname: &str) -> Option<String> {
    if hostname.starts_with('[') || hostname.parse::<IpAddr>().is_ok() {
        return None;
    }

    let labels: Vec<&str> = hostname.split('.').filter(|label| !label.is_empty()).collect();
    if labels.len() < 2 {
        return None;
    }

    const MULTI_LABEL_SUFFIXES: &[&str] = &[
        "co.uk",
        "org.uk",
        "ac.uk",
        "gov.uk",
        "co.jp",
        "com.au",
        "net.au",
        "org.au",
        "co.nz",
        "com.br",
        "com.mx",
        "co.in",
        "com.sg",
        "com.tr",
        "com.cn",
        "com.hk",
        "com.tw",
    ];

    let suffix = format!("{}.{}", labels[labels.len() - 2], labels[labels.len() - 1]);
    if MULTI_LABEL_SUFFIXES.contains(&suffix.as_str()) && labels.len() >= 3 {
        return Some(
            labels[labels.len() - 3..]
                .join(".")
                .to_ascii_lowercase(),
        );
    }

    Some(labels[labels.len() - 2..].join(".").to_ascii_lowercase())
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SiteRule {
    ExactOrigin(String),
    ExactHost(String),
    RegistrableDomain(String),
}

fn parse_site_rule(input: &str) -> Option<SiteRule> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(value) = trimmed.strip_prefix("origin:") {
        return parse_site(value).map(|site| SiteRule::ExactOrigin(site.origin));
    }

    if let Some(value) = trimmed.strip_prefix("host:") {
        return parse_site(value).map(|site| SiteRule::ExactHost(site.hostname));
    }

    if let Some(value) = trimmed.strip_prefix("domain:") {
        let normalized = parse_site(value)
            .and_then(|site| site.registrable_domain.or(Some(site.hostname)))
            .or_else(|| registrable_domain(&value.to_ascii_lowercase()))?;
        return Some(SiteRule::RegistrableDomain(normalized));
    }

    parse_site(trimmed).map(|site| {
        if trimmed.contains("://") {
            SiteRule::ExactOrigin(site.origin)
        } else {
            SiteRule::ExactHost(site.hostname)
        }
    })
}

fn derive_default_site_rules(url: Option<&str>) -> Vec<SiteRule> {
    let Some(site) = url.and_then(parse_site) else {
        return Vec::new();
    };

    let mut rules = vec![SiteRule::ExactOrigin(site.origin.clone())];

    if !site.has_explicit_port {
        rules.push(SiteRule::ExactHost(site.hostname.clone()));

        if let Some(domain) = site.registrable_domain {
            if domain != site.hostname {
                rules.push(SiteRule::RegistrableDomain(domain));
            }
        }
    }

    rules
}

fn effective_site_rules(entry: &EntryMeta) -> Vec<SiteRule> {
    let rules = if entry.site_rules.is_empty() {
        derive_default_site_rules(entry.url.as_deref())
    } else {
        entry.site_rules
            .iter()
            .filter_map(|rule| parse_site_rule(rule))
            .collect()
    };

    let mut deduped = Vec::new();
    for rule in rules {
        if !deduped.contains(&rule) {
            deduped.push(rule);
        }
    }

    deduped
}

fn classify_site_rule_match(current: &ParsedSite, rule: &SiteRule) -> Option<&'static str> {
    match rule {
        SiteRule::ExactOrigin(origin) => {
            if current.origin == *origin {
                Some("exact_origin")
            } else {
                None
            }
        }
        SiteRule::ExactHost(hostname) => {
            if current.hostname == *hostname {
                Some("exact_host")
            } else if current.hostname.ends_with(&format!(".{hostname}")) {
                Some("subdomain")
            } else {
                None
            }
        }
        SiteRule::RegistrableDomain(domain) => {
            if current.registrable_domain.as_deref() == Some(domain.as_str()) {
                Some("registrable_domain")
            } else {
                None
            }
        }
    }
}

fn match_rank(match_type: &str) -> u8 {
    match match_type {
        "exact_origin" => 4,
        "exact_host" => 3,
        "subdomain" => 2,
        "registrable_domain" => 1,
        _ => 0,
    }
}

fn find_site_matches(_state: &HostState, site_url: String) -> NativeResponse {
    let current_site = match parse_site(&site_url) {
        Some(site) => site,
        None => {
            return NativeResponse::Error {
                message: "Current tab URL is not a supported website.".to_string(),
            }
        }
    };

    let metadata = match vault::storage::read_vault_metadata() {
        Ok(metadata) => metadata,
        Err(err) => {
            return NativeResponse::Error {
                message: err.to_string(),
            }
        }
    };

    let mut matches: Vec<(u8, SiteMatchSummary)> = metadata
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.secret_type == SecretType::Password)
        .filter_map(|(index, entry)| {
            let match_type = effective_site_rules(entry)
                .iter()
                .filter_map(|rule| classify_site_rule_match(&current_site, rule))
                .max_by_key(|match_type| match_rank(match_type))?;

            Some((
                match_rank(match_type),
                summarize_site_match(index, entry, match_type),
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

fn resolve_vault_password(state: &HostState, password: Option<String>) -> Result<String, NativeResponse> {
    let password = match password {
        Some(password) => password,
        None => match require_unlocked_password(state) {
            Ok(password) => password.to_string(),
            Err(response) => return Err(response),
        },
    };

    if password.is_empty() {
        return Err(NativeResponse::Error {
            message: "Enter your master password.".to_string(),
        });
    }

    Ok(password)
}

fn read_vault_with_password(password: &str) -> Result<vault::model::VaultData, NativeResponse> {
    match vault::storage::read_vault(password.as_bytes(), &vault::storage::vault_path()) {
        Ok(vault) => Ok(vault),
        Err(err) => Err(NativeResponse::Error {
            message: err.to_string(),
        }),
    }
}

fn read_vault_for_autofill(state: &HostState, password: Option<String>) -> Result<vault::model::VaultData, NativeResponse> {
    let password = resolve_vault_password(state, password)?;

    read_vault_with_password(&password)
}

fn decrypt_secondary_password_entry(entry: &Entry, secondary_password: &str) -> Result<Zeroizing<String>, NativeResponse> {
    let wrapped = entry
        .entry_key_wrapped
        .as_ref()
        .ok_or_else(|| NativeResponse::Error {
            message: "This entry requires a secondary password to view.".to_string(),
        })?;
    let nonce = entry
        .entry_key_nonce
        .as_ref()
        .ok_or_else(|| NativeResponse::Error {
            message: "This entry requires a secondary password to view.".to_string(),
        })?;
    let salt = entry
        .entry_key_salt
        .as_ref()
        .ok_or_else(|| NativeResponse::Error {
            message: "This entry requires a secondary password to view.".to_string(),
        })?;
    let ciphertext = entry
        .encrypted_secret
        .as_ref()
        .ok_or_else(|| NativeResponse::Error {
            message: "This entry requires a secondary password to view.".to_string(),
        })?;
    let ciphertext_nonce = entry
        .encrypted_secret_nonce
        .as_ref()
        .ok_or_else(|| NativeResponse::Error {
            message: "This entry requires a secondary password to view.".to_string(),
        })?;

    let entry_key = crypto::entry_key::unwrap_entry_key(
        wrapped,
        nonce,
        salt,
        secondary_password,
    )
    .map_err(|err| NativeResponse::Error {
        message: err.to_string(),
    })?;

    crypto::entry_key::decrypt_secret(&entry_key, ciphertext, ciphertext_nonce).map_err(|err| {
        NativeResponse::Error {
            message: err.to_string(),
        }
    })
}

fn get_autofill_entry(
    state: &HostState,
    id: String,
    password: Option<String>,
    secondary_password: Option<String>,
) -> NativeResponse {
    let vault = match read_vault_for_autofill(state, password) {
        Ok(vault) => vault,
        Err(response) => return response,
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
        let secondary_password = match secondary_password {
            Some(password) => password,
            None => {
                return NativeResponse::Error {
                    message: "This entry requires a secondary password to view.".to_string(),
                }
            }
        };

        let secret = match decrypt_secondary_password_entry(entry, &secondary_password) {
            Ok(secret) => secret,
            Err(response) => return response,
        };

        return NativeResponse::AutofillEntry {
            entry: AutofillEntryResponse {
                id,
                name: entry.name.clone(),
                username: entry.username.clone(),
                password: secret.to_string(),
                url: entry.url.clone(),
            },
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

fn normalize_optional_field(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn build_password_entry(
    name: String,
    username: Option<String>,
    password: String,
    url: Option<String>,
    secondary_password: Option<String>,
) -> Result<Entry, NativeResponse> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(NativeResponse::Error {
            message: "Enter a name for this login.".to_string(),
        });
    }

    if password.is_empty() {
        return Err(NativeResponse::Error {
            message: "Password cannot be empty.".to_string(),
        });
    }

    let now = Utc::now();
    let username = normalize_optional_field(username);
    let url = normalize_optional_field(url);
    let secondary_password = normalize_optional_field(secondary_password);

    let (
        has_secondary_password,
        secret,
        entry_key_wrapped,
        entry_key_nonce,
        entry_key_salt,
        encrypted_secret,
        encrypted_secret_nonce,
    ) = if let Some(secondary_password) = secondary_password {
        let entry_key = crypto::entry_key::generate_entry_key();
        let (encrypted_secret, encrypted_secret_nonce) =
            crypto::entry_key::encrypt_secret(&entry_key, &password).map_err(|err| {
                NativeResponse::Error {
                    message: err.to_string(),
                }
            })?;
        let (entry_key_wrapped, entry_key_nonce, entry_key_salt) =
            crypto::entry_key::wrap_entry_key(&entry_key, &secondary_password).map_err(
                |err| NativeResponse::Error {
                    message: err.to_string(),
                },
            )?;

        (
            true,
            "[encrypted]".to_string(),
            Some(entry_key_wrapped),
            Some(entry_key_nonce),
            Some(entry_key_salt),
            Some(encrypted_secret),
            Some(encrypted_secret_nonce),
        )
    } else {
        (false, password, None, None, None, None, None)
    };

    Ok(Entry {
        name,
        secret,
        secret_type: SecretType::Password,
        network: "Password".to_string(),
        public_address: None,
        username,
        url,
        site_rules: Vec::new(),
        notes: String::new(),
        created_at: now,
        updated_at: now,
        has_secondary_password,
        entry_key_wrapped,
        entry_key_nonce,
        entry_key_salt,
        encrypted_secret,
        encrypted_secret_nonce,
    })
}

fn save_password_entry(
    state: &HostState,
    name: String,
    username: Option<String>,
    password: String,
    url: Option<String>,
    master_password: Option<String>,
    secondary_password: Option<String>,
) -> NativeResponse {
    let vault_password = match resolve_vault_password(state, master_password) {
        Ok(password) => password,
        Err(response) => return response,
    };

    let mut vault = match read_vault_with_password(&vault_password) {
        Ok(vault) => vault,
        Err(response) => return response,
    };

    let entry = match build_password_entry(name, username, password, url, secondary_password) {
        Ok(entry) => entry,
        Err(response) => return response,
    };

    if vault.has_entry(&entry.name) {
        return NativeResponse::Error {
            message: format!("Entry '{}' already exists.", entry.name),
        };
    }

    let entry_name = entry.name.clone();
    vault.entries.push(entry);

    match vault::storage::write_vault(&vault, vault_password.as_bytes(), &vault::storage::vault_path())
    {
        Ok(()) => NativeResponse::SaveEntry { entry_name },
        Err(err) => NativeResponse::Error {
            message: err.to_string(),
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
        NativeRequest::GeneratePassword => NativeResponse::GeneratedPassword {
            password: crypto::passwords::generate_password(),
        },
        NativeRequest::GetAutofillEntry {
            id,
            password,
            secondary_password,
        } => get_autofill_entry(state, id, password, secondary_password),
        NativeRequest::FindSiteMatches { url } => find_site_matches(state, url),
        NativeRequest::SavePasswordEntry {
            name,
            username,
            password,
            url,
            master_password,
            secondary_password,
        } => save_password_entry(
            state,
            name,
            username,
            password,
            url,
            master_password,
            secondary_password,
        ),
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
        classify_site_rule_match, handle_request, load_status_for_state, parse_site,
        read_message, write_message, HostState, NativeResponse, SiteRule,
    };
    use chrono::Utc;
    use std::io::Cursor;
    use std::sync::{Mutex, OnceLock};
    use tempfile::TempDir;
    use termkey::crypto::entry_key;
    use termkey::vault::model::{Entry, SecretType, VaultData};
    use termkey::vault::storage::{read_vault, write_vault};
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
                site_rules: Vec::new(),
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

    fn test_vault_with_secondary_entry() -> VaultData {
        let entry_key = entry_key::generate_entry_key();
        let (encrypted_secret, encrypted_secret_nonce) =
            entry_key::encrypt_secret(&entry_key, "super-secret").unwrap();
        let (wrapped_key, key_nonce, key_salt) =
            entry_key::wrap_entry_key(&entry_key, "view-pass").unwrap();

        VaultData {
            entries: vec![Entry {
                name: "Protected Email".to_string(),
                secret: "[encrypted]".to_string(),
                secret_type: SecretType::Password,
                network: "Password".to_string(),
                public_address: None,
                username: Some("ryan".to_string()),
                url: Some("https://secure.example.com".to_string()),
                site_rules: Vec::new(),
                notes: String::new(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                has_secondary_password: true,
                entry_key_wrapped: Some(wrapped_key),
                entry_key_nonce: Some(key_nonce),
                entry_key_salt: Some(key_salt),
                encrypted_secret: Some(encrypted_secret),
                encrypted_secret_nonce: Some(encrypted_secret_nonce),
            }],
            version: 1,
        }
    }

    fn test_vault_with_domain_rule_entry() -> VaultData {
        VaultData {
            entries: vec![Entry {
                name: "Google Account".to_string(),
                secret: "super-secret".to_string(),
                secret_type: SecretType::Password,
                network: "Password".to_string(),
                public_address: None,
                username: Some("ryan".to_string()),
                url: Some("https://accounts.google.com".to_string()),
                site_rules: Vec::new(),
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

    fn test_vault_with_port_specific_entry() -> VaultData {
        VaultData {
            entries: vec![Entry {
                name: "Dashboard".to_string(),
                secret: "super-secret".to_string(),
                secret_type: SecretType::Password,
                network: "Password".to_string(),
                public_address: None,
                username: Some("admin".to_string()),
                url: Some("https://home.ryanonmars.space:3000".to_string()),
                site_rules: Vec::new(),
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

    fn test_vault_with_explicit_site_rule_entry() -> VaultData {
        VaultData {
            entries: vec![Entry {
                name: "Admin Login".to_string(),
                secret: "super-secret".to_string(),
                secret_type: SecretType::Password,
                network: "Password".to_string(),
                public_address: None,
                username: Some("ryan".to_string()),
                url: None,
                site_rules: vec![
                    "host:auth.example.com".to_string(),
                    "domain:example.com".to_string(),
                ],
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
    fn generate_password_returns_generated_password() {
        let response = handle_request(&mut HostState::default(), br#"{"type":"generate_password"}"#);

        match response {
            NativeResponse::GeneratedPassword { password } => {
                assert_eq!(password.len(), 24);
            }
            other => panic!("unexpected response: {:?}", other),
        }
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
    fn site_matches_are_available_without_unlocking() {
        let _guard = env_lock().lock().unwrap();
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("vault.ck");
        write_vault(&test_vault_with_entry(), b"correct horse battery staple", &path).unwrap();

        let previous_vault_dir = std::env::var_os("TERMKEY_VAULT_DIR");
        std::env::set_var("TERMKEY_VAULT_DIR", dir.path());

        let response = handle_request(
            &mut HostState::default(),
            br#"{"type":"find_site_matches","url":"https://example.com/login"}"#,
        );

        match previous_vault_dir {
            Some(value) => std::env::set_var("TERMKEY_VAULT_DIR", value),
            None => std::env::remove_var("TERMKEY_VAULT_DIR"),
        }

        match response {
            NativeResponse::SiteMatches(matches) => {
                assert_eq!(matches.site_hostname, "example.com");
                assert_eq!(matches.matches.len(), 1);
                assert_eq!(matches.matches[0].name, "Email");
            }
            other => panic!("unexpected response: {:?}", other),
        }
    }

    #[test]
    fn site_matches_support_registrable_domain_matching() {
        let _guard = env_lock().lock().unwrap();
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("vault.ck");
        write_vault(&test_vault_with_domain_rule_entry(), b"correct horse battery staple", &path)
            .unwrap();

        let previous_vault_dir = std::env::var_os("TERMKEY_VAULT_DIR");
        std::env::set_var("TERMKEY_VAULT_DIR", dir.path());

        let response = handle_request(
            &mut HostState::default(),
            br#"{"type":"find_site_matches","url":"https://mail.google.com"}"#,
        );

        match previous_vault_dir {
            Some(value) => std::env::set_var("TERMKEY_VAULT_DIR", value),
            None => std::env::remove_var("TERMKEY_VAULT_DIR"),
        }

        match response {
            NativeResponse::SiteMatches(matches) => {
                assert_eq!(matches.matches.len(), 1);
                assert_eq!(matches.matches[0].match_type, "registrable_domain");
            }
            other => panic!("unexpected response: {:?}", other),
        }
    }

    #[test]
    fn site_matches_support_explicit_site_rules_without_url() {
        let _guard = env_lock().lock().unwrap();
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("vault.ck");
        write_vault(
            &test_vault_with_explicit_site_rule_entry(),
            b"correct horse battery staple",
            &path,
        )
        .unwrap();

        let previous_vault_dir = std::env::var_os("TERMKEY_VAULT_DIR");
        std::env::set_var("TERMKEY_VAULT_DIR", dir.path());

        let response = handle_request(
            &mut HostState::default(),
            br#"{"type":"find_site_matches","url":"https://dashboard.example.com"}"#,
        );

        match previous_vault_dir {
            Some(value) => std::env::set_var("TERMKEY_VAULT_DIR", value),
            None => std::env::remove_var("TERMKEY_VAULT_DIR"),
        }

        match response {
            NativeResponse::SiteMatches(matches) => {
                assert_eq!(matches.matches.len(), 1);
                assert_eq!(matches.matches[0].name, "Admin Login");
            }
            other => panic!("unexpected response: {:?}", other),
        }
    }

    #[test]
    fn site_matches_do_not_cross_non_default_ports_by_default() {
        let _guard = env_lock().lock().unwrap();
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("vault.ck");
        write_vault(
            &test_vault_with_port_specific_entry(),
            b"correct horse battery staple",
            &path,
        )
        .unwrap();

        let previous_vault_dir = std::env::var_os("TERMKEY_VAULT_DIR");
        std::env::set_var("TERMKEY_VAULT_DIR", dir.path());

        let response = handle_request(
            &mut HostState::default(),
            br#"{"type":"find_site_matches","url":"https://home.ryanonmars.space"}"#,
        );

        match previous_vault_dir {
            Some(value) => std::env::set_var("TERMKEY_VAULT_DIR", value),
            None => std::env::remove_var("TERMKEY_VAULT_DIR"),
        }

        match response {
            NativeResponse::SiteMatches(matches) => {
                assert!(matches.matches.is_empty());
            }
            other => panic!("unexpected response: {:?}", other),
        }
    }

    #[test]
    fn save_password_entry_accepts_one_off_master_password() {
        let _guard = env_lock().lock().unwrap();
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("vault.ck");
        write_vault(&VaultData::new(), b"correct horse battery staple", &path).unwrap();

        let previous_vault_dir = std::env::var_os("TERMKEY_VAULT_DIR");
        std::env::set_var("TERMKEY_VAULT_DIR", dir.path());

        let response = handle_request(
            &mut HostState::default(),
            br#"{"type":"save_password_entry","name":"Example Login","username":"ryan@example.com","password":"super-secret","url":"https://example.com/login","masterPassword":"correct horse battery staple"}"#,
        );

        let saved_vault = read_vault(b"correct horse battery staple", &path).unwrap();

        match previous_vault_dir {
            Some(value) => std::env::set_var("TERMKEY_VAULT_DIR", value),
            None => std::env::remove_var("TERMKEY_VAULT_DIR"),
        }

        match response {
            NativeResponse::SaveEntry { entry_name } => {
                assert_eq!(entry_name, "Example Login");
            }
            other => panic!("unexpected response: {:?}", other),
        }

        assert_eq!(saved_vault.entries.len(), 1);
        assert_eq!(saved_vault.entries[0].name, "Example Login");
        assert_eq!(
            saved_vault.entries[0].username.as_deref(),
            Some("ryan@example.com")
        );
        assert_eq!(saved_vault.entries[0].url.as_deref(), Some("https://example.com/login"));
        assert_eq!(saved_vault.entries[0].secret, "super-secret");
        assert!(!saved_vault.entries[0].has_secondary_password);
    }

    #[test]
    fn save_password_entry_supports_secondary_password() {
        let _guard = env_lock().lock().unwrap();
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("vault.ck");
        write_vault(&VaultData::new(), b"correct horse battery staple", &path).unwrap();

        let previous_vault_dir = std::env::var_os("TERMKEY_VAULT_DIR");
        std::env::set_var("TERMKEY_VAULT_DIR", dir.path());

        let save_response = handle_request(
            &mut HostState::default(),
            br#"{"type":"save_password_entry","name":"Protected Login","username":"ryan@example.com","password":"super-secret","url":"https://secure.example.com","masterPassword":"correct horse battery staple","secondaryPassword":"view-pass"}"#,
        );
        let autofill_response = handle_request(
            &mut HostState::default(),
            br#"{"type":"get_autofill_entry","id":"1","password":"correct horse battery staple","secondaryPassword":"view-pass"}"#,
        );

        match previous_vault_dir {
            Some(value) => std::env::set_var("TERMKEY_VAULT_DIR", value),
            None => std::env::remove_var("TERMKEY_VAULT_DIR"),
        }

        assert!(matches!(save_response, NativeResponse::SaveEntry { .. }));

        match autofill_response {
            NativeResponse::AutofillEntry { entry } => {
                assert_eq!(entry.name, "Protected Login");
                assert_eq!(entry.password, "super-secret");
            }
            other => panic!("unexpected response: {:?}", other),
        }
    }

    #[test]
    fn autofill_entry_accepts_password_without_unlocking_state() {
        let _guard = env_lock().lock().unwrap();
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("vault.ck");
        write_vault(&test_vault_with_entry(), b"correct horse battery staple", &path).unwrap();

        let previous_vault_dir = std::env::var_os("TERMKEY_VAULT_DIR");
        std::env::set_var("TERMKEY_VAULT_DIR", dir.path());

        let response = handle_request(
            &mut HostState::default(),
            br#"{"type":"get_autofill_entry","id":"1","password":"correct horse battery staple"}"#,
        );

        match previous_vault_dir {
            Some(value) => std::env::set_var("TERMKEY_VAULT_DIR", value),
            None => std::env::remove_var("TERMKEY_VAULT_DIR"),
        }

        match response {
            NativeResponse::AutofillEntry { entry } => {
                assert_eq!(entry.name, "Email");
                assert_eq!(entry.username.as_deref(), Some("ryan"));
                assert_eq!(entry.password, "super-secret");
            }
            other => panic!("unexpected response: {:?}", other),
        }
    }

    #[test]
    fn autofill_secondary_password_entry_requires_secondary_password() {
        let _guard = env_lock().lock().unwrap();
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("vault.ck");
        write_vault(&test_vault_with_secondary_entry(), b"correct horse battery staple", &path)
            .unwrap();

        let previous_vault_dir = std::env::var_os("TERMKEY_VAULT_DIR");
        std::env::set_var("TERMKEY_VAULT_DIR", dir.path());

        let response = handle_request(
            &mut HostState::default(),
            br#"{"type":"get_autofill_entry","id":"1","password":"correct horse battery staple"}"#,
        );

        match previous_vault_dir {
            Some(value) => std::env::set_var("TERMKEY_VAULT_DIR", value),
            None => std::env::remove_var("TERMKEY_VAULT_DIR"),
        }

        match response {
            NativeResponse::Error { message } => {
                assert_eq!(message, "This entry requires a secondary password to view.");
            }
            other => panic!("unexpected response: {:?}", other),
        }
    }

    #[test]
    fn autofill_secondary_password_entry_accepts_secondary_password() {
        let _guard = env_lock().lock().unwrap();
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("vault.ck");
        write_vault(&test_vault_with_secondary_entry(), b"correct horse battery staple", &path)
            .unwrap();

        let previous_vault_dir = std::env::var_os("TERMKEY_VAULT_DIR");
        std::env::set_var("TERMKEY_VAULT_DIR", dir.path());

        let response = handle_request(
            &mut HostState::default(),
            br#"{"type":"get_autofill_entry","id":"1","password":"correct horse battery staple","secondary_password":"view-pass"}"#,
        );

        match previous_vault_dir {
            Some(value) => std::env::set_var("TERMKEY_VAULT_DIR", value),
            None => std::env::remove_var("TERMKEY_VAULT_DIR"),
        }

        match response {
            NativeResponse::AutofillEntry { entry } => {
                assert_eq!(entry.name, "Protected Email");
                assert_eq!(entry.password, "super-secret");
            }
            other => panic!("unexpected response: {:?}", other),
        }
    }

    #[test]
    fn autofill_secondary_password_entry_accepts_camel_case_secondary_password() {
        let _guard = env_lock().lock().unwrap();
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("vault.ck");
        write_vault(&test_vault_with_secondary_entry(), b"correct horse battery staple", &path)
            .unwrap();

        let previous_vault_dir = std::env::var_os("TERMKEY_VAULT_DIR");
        std::env::set_var("TERMKEY_VAULT_DIR", dir.path());

        let response = handle_request(
            &mut HostState::default(),
            br#"{"type":"get_autofill_entry","id":"1","password":"correct horse battery staple","secondaryPassword":"view-pass"}"#,
        );

        match previous_vault_dir {
            Some(value) => std::env::set_var("TERMKEY_VAULT_DIR", value),
            None => std::env::remove_var("TERMKEY_VAULT_DIR"),
        }

        match response {
            NativeResponse::AutofillEntry { entry } => {
                assert_eq!(entry.name, "Protected Email");
                assert_eq!(entry.password, "super-secret");
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
        let exact_origin = SiteRule::ExactOrigin("https://app.example.com".to_string());
        let exact_host = SiteRule::ExactHost("app.example.com".to_string());
        let subdomain = SiteRule::ExactHost("example.com".to_string());
        let registrable_domain = SiteRule::RegistrableDomain("example.com".to_string());

        assert_eq!(classify_site_rule_match(&current, &exact_origin), Some("exact_origin"));
        assert_eq!(classify_site_rule_match(&current, &exact_host), Some("exact_host"));
        assert_eq!(classify_site_rule_match(&current, &subdomain), Some("subdomain"));
        assert_eq!(
            classify_site_rule_match(&current, &registrable_domain),
            Some("registrable_domain")
        );
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
