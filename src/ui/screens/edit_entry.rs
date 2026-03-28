use chrono::Utc;
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use zeroize::Zeroize;

use crate::crypto::entry_key;
use crate::vault::model::{Entry, SecretType};

pub struct EditEntryScreen {
    pub original_name: String,
    entry: Entry,
    current_field: usize,
    secret: String,
    secret_confirm: String,
    current_secondary_password: String,
    custom_secret_type: String,
    network: String,
    custom_network: String,
    use_custom_network: bool,
    show_type_select: bool,
    type_selected: usize,
    show_network_select: bool,
    network_selected: usize,
    scroll_offset: usize,
}

impl Drop for EditEntryScreen {
    fn drop(&mut self) {
        self.secret.zeroize();
        self.secret_confirm.zeroize();
        self.current_secondary_password.zeroize();
    }
}

impl EditEntryScreen {
    pub fn new(entry: Entry) -> Self {
        let original_name = entry.name.clone();
        let type_selected = match &entry.secret_type {
            SecretType::PrivateKey => 0,
            SecretType::SeedPhrase => 1,
            SecretType::Password => 2,
            SecretType::Other(_) => 3,
        };
        let custom_secret_type = match &entry.secret_type {
            SecretType::Other(label) => label.clone(),
            _ => String::new(),
        };
        let (network, custom_network, use_custom_network, network_selected) =
            Self::network_state_for_entry(&entry);

        Self {
            original_name,
            entry,
            current_field: 0,
            secret: String::new(),
            secret_confirm: String::new(),
            current_secondary_password: String::new(),
            custom_secret_type,
            network,
            custom_network,
            use_custom_network,
            show_type_select: false,
            type_selected,
            show_network_select: false,
            network_selected,
            scroll_offset: 0,
        }
    }

    pub fn handle_key(&mut self, key: KeyCode, modifiers: KeyModifiers) -> EditEntryAction {
        if key == KeyCode::Esc {
            return EditEntryAction::Cancel;
        }

        if modifiers.contains(KeyModifiers::CONTROL) && key == KeyCode::Char('s') {
            return self.try_save();
        }

        if self.show_type_select {
            return self.handle_type_select(key);
        }

        if self.show_network_select {
            return self.handle_network_select(key);
        }

        match key {
            KeyCode::Tab => {
                self.current_field = (self.current_field + 1) % self.field_count();
                EditEntryAction::Continue
            }
            KeyCode::BackTab => {
                if self.current_field == 0 {
                    self.current_field = self.field_count() - 1;
                } else {
                    self.current_field -= 1;
                }
                EditEntryAction::Continue
            }
            KeyCode::Up => {
                if self.current_field > 0 {
                    self.current_field -= 1;
                }
                EditEntryAction::Continue
            }
            KeyCode::Down => {
                self.current_field = (self.current_field + 1) % self.field_count();
                EditEntryAction::Continue
            }
            KeyCode::Enter => {
                if self.current_field == 1 {
                    self.show_type_select = true;
                } else if self.is_crypto_type() && self.current_field == self.network_field() {
                    self.show_network_select = true;
                } else if self.current_field == self.field_count() - 1 {
                    return self.try_save();
                } else {
                    self.current_field = (self.current_field + 1) % self.field_count();
                }
                EditEntryAction::Continue
            }
            KeyCode::Char(c) if !modifiers.contains(KeyModifiers::CONTROL) => {
                self.insert_char(c);
                EditEntryAction::Continue
            }
            KeyCode::Backspace => {
                self.delete_char();
                EditEntryAction::Continue
            }
            _ => EditEntryAction::Continue,
        }
    }

    fn network_state_for_entry(entry: &Entry) -> (String, String, bool, usize) {
        if !entry.secret_type.is_crypto_type() {
            return ("Ethereum".to_string(), String::new(), false, 0);
        }

        match entry.network.to_lowercase().as_str() {
            "ethereum" | "eth" => ("Ethereum".to_string(), String::new(), false, 0),
            "bitcoin" | "btc" => ("Bitcoin".to_string(), String::new(), false, 1),
            "solana" | "sol" => ("Solana".to_string(), String::new(), false, 2),
            _ if entry.network.trim().is_empty() => {
                ("Ethereum".to_string(), String::new(), false, 0)
            }
            _ => ("Ethereum".to_string(), entry.network.clone(), true, 3),
        }
    }

    fn handle_type_select(&mut self, key: KeyCode) -> EditEntryAction {
        match key {
            KeyCode::Up => {
                if self.type_selected > 0 {
                    self.type_selected -= 1;
                }
            }
            KeyCode::Down => {
                if self.type_selected < 3 {
                    self.type_selected += 1;
                }
            }
            KeyCode::Enter => {
                self.entry.secret_type = match self.type_selected {
                    0 => SecretType::PrivateKey,
                    1 => SecretType::SeedPhrase,
                    2 => SecretType::Password,
                    _ => SecretType::Other(self.custom_secret_type.trim().to_string()),
                };
                if !self.entry.secret_type.is_other_type() {
                    self.custom_secret_type.clear();
                }
                if self.is_crypto_type() && self.effective_network().is_empty() {
                    self.network = "Ethereum".to_string();
                    self.use_custom_network = false;
                }
                self.show_type_select = false;
                self.current_field = (self.current_field + 1) % self.field_count();
            }
            KeyCode::Esc => {
                self.show_type_select = false;
            }
            _ => {}
        }

        EditEntryAction::Continue
    }

    fn handle_network_select(&mut self, key: KeyCode) -> EditEntryAction {
        match key {
            KeyCode::Up => {
                if self.network_selected > 0 {
                    self.network_selected -= 1;
                }
            }
            KeyCode::Down => {
                if self.network_selected < 3 {
                    self.network_selected += 1;
                }
            }
            KeyCode::Enter => {
                match self.network_selected {
                    0 => {
                        self.network = "Ethereum".to_string();
                        self.use_custom_network = false;
                        self.custom_network.clear();
                    }
                    1 => {
                        self.network = "Bitcoin".to_string();
                        self.use_custom_network = false;
                        self.custom_network.clear();
                    }
                    2 => {
                        self.network = "Solana".to_string();
                        self.use_custom_network = false;
                        self.custom_network.clear();
                    }
                    _ => {
                        self.use_custom_network = true;
                    }
                }
                self.show_network_select = false;
                self.current_field = (self.current_field + 1) % self.field_count();
            }
            KeyCode::Esc => {
                self.show_network_select = false;
            }
            _ => {}
        }

        EditEntryAction::Continue
    }

    fn insert_char(&mut self, c: char) {
        match self.current_field {
            0 => self.entry.name.push(c),
            f if self.has_custom_type_field() && f == self.custom_type_field() => {
                self.custom_secret_type.push(c);
            }
            f if f == self.secret_field() => self.secret.push(c),
            f if f == self.confirm_field() => self.secret_confirm.push(c),
            f if self.has_custom_network_field() && f == self.custom_network_field() => {
                self.custom_network.push(c);
            }
            f if self.is_crypto_type() && f == self.public_address_field() => {
                if let Some(ref mut addr) = self.entry.public_address {
                    addr.push(c);
                } else {
                    self.entry.public_address = Some(c.to_string());
                }
            }
            f if self.is_password_type() && f == self.username_field() => {
                if let Some(ref mut username) = self.entry.username {
                    username.push(c);
                } else {
                    self.entry.username = Some(c.to_string());
                }
            }
            f if self.is_password_type() && f == self.url_field() => {
                if let Some(ref mut url) = self.entry.url {
                    url.push(c);
                } else {
                    self.entry.url = Some(c.to_string());
                }
            }
            f if f == self.notes_field() => self.entry.notes.push(c),
            f if self.entry.has_secondary_password && f == self.secondary_password_field() => {
                self.current_secondary_password.push(c);
            }
            _ => {}
        }
    }

    fn delete_char(&mut self) {
        match self.current_field {
            0 => {
                self.entry.name.pop();
            }
            f if self.has_custom_type_field() && f == self.custom_type_field() => {
                self.custom_secret_type.pop();
            }
            f if f == self.secret_field() => {
                self.secret.pop();
            }
            f if f == self.confirm_field() => {
                self.secret_confirm.pop();
            }
            f if self.has_custom_network_field() && f == self.custom_network_field() => {
                self.custom_network.pop();
            }
            f if self.is_crypto_type() && f == self.public_address_field() => {
                if let Some(ref mut addr) = self.entry.public_address {
                    addr.pop();
                }
            }
            f if self.is_password_type() && f == self.username_field() => {
                if let Some(ref mut username) = self.entry.username {
                    username.pop();
                }
            }
            f if self.is_password_type() && f == self.url_field() => {
                if let Some(ref mut url) = self.entry.url {
                    url.pop();
                }
            }
            f if f == self.notes_field() => {
                self.entry.notes.pop();
            }
            f if self.entry.has_secondary_password && f == self.secondary_password_field() => {
                self.current_secondary_password.pop();
            }
            _ => {}
        }
    }

    fn field_count(&self) -> usize {
        let mut base = 5; // name, type, secret, confirm, notes
        if self.has_custom_type_field() {
            base += 1;
        }
        if self.is_crypto_type() {
            base += 2; // network, public address
        }
        if self.has_custom_network_field() {
            base += 1;
        }
        if self.is_password_type() {
            base += 2; // username, url
        }
        if self.entry.has_secondary_password {
            base += 1;
        }
        base
    }

    fn is_crypto_type(&self) -> bool {
        self.entry.secret_type.is_crypto_type()
    }

    fn is_password_type(&self) -> bool {
        self.entry.secret_type.is_password_type()
    }

    fn has_custom_type_field(&self) -> bool {
        self.entry.secret_type.is_other_type()
    }

    fn has_custom_network_field(&self) -> bool {
        self.is_crypto_type() && self.use_custom_network
    }

    fn custom_type_field(&self) -> usize {
        2
    }

    fn secret_field(&self) -> usize {
        2 + usize::from(self.has_custom_type_field())
    }

    fn confirm_field(&self) -> usize {
        self.secret_field() + 1
    }

    fn network_field(&self) -> usize {
        self.confirm_field() + 1
    }

    fn custom_network_field(&self) -> usize {
        self.network_field() + 1
    }

    fn public_address_field(&self) -> usize {
        self.network_field() + 1 + usize::from(self.has_custom_network_field())
    }

    fn username_field(&self) -> usize {
        self.confirm_field() + 1
    }

    fn url_field(&self) -> usize {
        self.username_field() + 1
    }

    fn notes_field(&self) -> usize {
        let mut idx = self.confirm_field() + 1;
        if self.is_crypto_type() {
            idx += 2;
            if self.has_custom_network_field() {
                idx += 1;
            }
        } else if self.is_password_type() {
            idx += 2;
        }
        idx
    }

    fn secondary_password_field(&self) -> usize {
        self.notes_field() + 1
    }

    fn effective_secret_type(&self) -> Option<SecretType> {
        match &self.entry.secret_type {
            SecretType::Other(_) => {
                let label = self.custom_secret_type.trim();
                if label.is_empty() {
                    None
                } else {
                    Some(SecretType::Other(label.to_string()))
                }
            }
            other => Some(other.clone()),
        }
    }

    fn effective_network(&self) -> String {
        if !self.is_crypto_type() {
            return String::new();
        }

        if self.use_custom_network {
            self.custom_network.trim().to_string()
        } else {
            self.network.clone()
        }
    }

    fn try_save(&self) -> EditEntryAction {
        let name = self.entry.name.trim().to_string();
        if name.is_empty() {
            return EditEntryAction::Continue;
        }

        let Some(secret_type) = self.effective_secret_type() else {
            return EditEntryAction::Continue;
        };

        let secret_changed = !self.secret.is_empty() || !self.secret_confirm.is_empty();
        if secret_changed && (self.secret.is_empty() || self.secret != self.secret_confirm) {
            return EditEntryAction::Continue;
        }

        let mut updated = self.entry.clone();
        updated.name = name;
        updated.secret_type = secret_type;

        if updated.secret_type.is_crypto_type() {
            let network = self.effective_network();
            if network.is_empty() {
                return EditEntryAction::Continue;
            }
            updated.network = network;
            updated.public_address = updated
                .public_address
                .as_ref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            updated.username = None;
            updated.url = None;
        } else if updated.secret_type.is_password_type() {
            updated.network.clear();
            updated.public_address = None;
            updated.username = updated
                .username
                .as_ref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            updated.url = updated
                .url
                .as_ref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
        } else {
            updated.network.clear();
            updated.public_address = None;
            updated.username = None;
            updated.url = None;
        }

        if secret_changed {
            if updated.has_secondary_password {
                let wrapped = updated.entry_key_wrapped.as_ref();
                let nonce = updated.entry_key_nonce.as_ref();
                let salt = updated.entry_key_salt.as_ref();

                let (Some(wrapped), Some(nonce), Some(salt)) = (wrapped, nonce, salt) else {
                    return EditEntryAction::Continue;
                };

                if self.current_secondary_password.is_empty() {
                    return EditEntryAction::Continue;
                }

                let entry_key = match entry_key::unwrap_entry_key(
                    wrapped,
                    nonce,
                    salt,
                    &self.current_secondary_password,
                ) {
                    Ok(key) => key,
                    Err(_) => return EditEntryAction::Continue,
                };
                let (encrypted_secret, encrypted_secret_nonce) =
                    match entry_key::encrypt_secret(&entry_key, &self.secret) {
                        Ok(result) => result,
                        Err(_) => return EditEntryAction::Continue,
                    };

                updated.secret = "[encrypted]".to_string();
                updated.encrypted_secret = Some(encrypted_secret);
                updated.encrypted_secret_nonce = Some(encrypted_secret_nonce);
            } else {
                updated.secret = self.secret.clone();
            }
        }

        updated.updated_at = Utc::now();
        EditEntryAction::Save(updated)
    }

    pub fn render(&self, frame: &mut Frame) {
        let area = frame.area();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(1),
                Constraint::Length(22),
                Constraint::Min(1),
            ])
            .split(area);

        let form_area = centered_rect(80, chunks[1]);

        if self.show_type_select {
            self.render_type_select(frame, form_area);
            return;
        }

        if self.show_network_select {
            self.render_network_select(frame, form_area);
            return;
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Edit Entry ")
            .title_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .border_style(Style::default().fg(Color::Cyan));

        frame.render_widget(block.clone(), form_area);

        let inner = block.inner(form_area);
        let available_height = inner.height as usize;

        let mut scroll_offset = self.scroll_offset;
        if self.current_field >= scroll_offset + available_height / 2 {
            scroll_offset = self.current_field.saturating_sub(available_height / 2 - 1);
        } else if self.current_field < scroll_offset {
            scroll_offset = self.current_field;
        }

        let mut lines = vec![];
        let mut field_idx = 0;
        let secondary_masked = "\u{2022}".repeat(self.current_secondary_password.len());

        lines.push(self.render_field(field_idx, "Entry name", &self.entry.name));
        field_idx += 1;

        lines.push(Line::from(""));
        let secret_type_str = self.entry.secret_type.to_string();
        lines.push(self.render_field(field_idx, "Secret type", &secret_type_str));
        field_idx += 1;

        if self.has_custom_type_field() {
            lines.push(Line::from(""));
            lines.push(self.render_field(field_idx, "Custom type", &self.custom_secret_type));
            field_idx += 1;
        }

        lines.push(Line::from(""));
        let secret_masked = "\u{2022}".repeat(self.secret.len());
        lines.push(self.render_field(field_idx, "New secret (optional)", &secret_masked));
        field_idx += 1;

        lines.push(Line::from(""));
        let secret_confirm_masked = "\u{2022}".repeat(self.secret_confirm.len());
        lines.push(self.render_field(field_idx, "Confirm new secret", &secret_confirm_masked));
        field_idx += 1;

        if self.is_crypto_type() {
            lines.push(Line::from(""));
            let network_value = if self.use_custom_network {
                "Other"
            } else {
                &self.network
            };
            lines.push(self.render_field(field_idx, "Network", network_value));
            field_idx += 1;

            if self.has_custom_network_field() {
                lines.push(Line::from(""));
                lines.push(self.render_field(field_idx, "Custom network", &self.custom_network));
                field_idx += 1;
            }

            lines.push(Line::from(""));
            let addr_value = self.entry.public_address.as_deref().unwrap_or("");
            lines.push(self.render_field(field_idx, "Public address (optional)", addr_value));
            field_idx += 1;
        } else if self.is_password_type() {
            lines.push(Line::from(""));
            let username_value = self.entry.username.as_deref().unwrap_or("");
            lines.push(self.render_field(field_idx, "Username (optional)", username_value));
            field_idx += 1;

            lines.push(Line::from(""));
            let url_value = self.entry.url.as_deref().unwrap_or("");
            lines.push(self.render_field(field_idx, "URL (optional)", url_value));
            field_idx += 1;
        }

        lines.push(Line::from(""));
        lines.push(self.render_field(field_idx, "Notes (optional)", &self.entry.notes));
        field_idx += 1;

        if self.entry.has_secondary_password {
            lines.push(Line::from(""));
            lines.push(self.render_field(field_idx, "Current secondary pwd", &secondary_masked));
            lines.push(Line::from(vec![Span::styled(
                "Only needed when changing the secret.",
                Style::default().fg(Color::DarkGray),
            )]));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(""));

        let help_text = if self.current_field == 1 {
            "\u{2191}\u{2193}: Scroll \u{2502} Enter: Select \u{2502} Tab: Next \u{2502} Esc: Cancel"
        } else if self.is_crypto_type() && self.current_field == self.network_field() {
            "\u{2191}\u{2193}: Scroll \u{2502} Enter: Select \u{2502} Tab: Next \u{2502} Esc: Cancel"
        } else {
            "\u{2191}\u{2193}: Scroll \u{2502} Tab: Next \u{2502} Shift+Tab: Previous \u{2502} Ctrl+S: Save \u{2502} Esc: Cancel"
        };

        lines.push(Line::from(vec![Span::styled(
            help_text,
            Style::default().fg(Color::DarkGray),
        )]));

        let visible_lines: Vec<Line> = lines
            .into_iter()
            .skip(scroll_offset * 2)
            .take(available_height)
            .collect();

        let paragraph = Paragraph::new(visible_lines);
        frame.render_widget(paragraph, inner);
    }

    fn render_field<'a>(&self, idx: usize, label: &str, value: &'a str) -> Line<'a> {
        let is_active = self.current_field == idx;
        let label_style = if is_active {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        let value_style = if is_active {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Gray)
        };

        let cursor = if is_active { "█" } else { "" };

        Line::from(vec![
            Span::styled(format!("{}: ", label), label_style),
            Span::styled(value, value_style),
            Span::styled(cursor, Style::default().fg(Color::Cyan)),
        ])
    }

    fn render_type_select(&self, frame: &mut Frame, area: Rect) {
        let types = ["Private Key", "Seed Phrase", "Password", "Other"];
        let items: Vec<ListItem> = types
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let prefix = if i == self.type_selected {
                    "\u{25b8} "
                } else {
                    "  "
                };
                let style = if i == self.type_selected {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                ListItem::new(format!("{}{}", prefix, t)).style(style)
            })
            .collect();

        let list = List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Select Secret Type (\u{2191}/\u{2193} to navigate, Enter to select) ")
                .border_style(Style::default().fg(Color::Cyan)),
        );

        frame.render_widget(list, area);
    }

    fn render_network_select(&self, frame: &mut Frame, area: Rect) {
        let networks = ["Ethereum", "Bitcoin", "Solana", "Other"];
        let items: Vec<ListItem> = networks
            .iter()
            .enumerate()
            .map(|(i, n)| {
                let prefix = if i == self.network_selected {
                    "\u{25b8} "
                } else {
                    "  "
                };
                let style = if i == self.network_selected {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                ListItem::new(format!("{}{}", prefix, n)).style(style)
            })
            .collect();

        let list = List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Select Network (\u{2191}/\u{2193} to navigate, Enter to select) ")
                .border_style(Style::default().fg(Color::Cyan)),
        );

        frame.render_widget(list, area);
    }
}

fn centered_rect(percent: u16, r: Rect) -> Rect {
    let width = r.width * percent / 100;
    let x = r.x + (r.width - width) / 2;
    Rect {
        x,
        y: r.y,
        width,
        height: r.height,
    }
}

pub enum EditEntryAction {
    Continue,
    Save(Entry),
    Cancel,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::entry_key;

    fn make_entry(secret_type: SecretType) -> Entry {
        Entry {
            name: "Entry".to_string(),
            secret: "secret".to_string(),
            secret_type,
            network: "Ethereum".to_string(),
            public_address: Some("0x123".to_string()),
            username: Some("user".to_string()),
            url: Some("https://example.com".to_string()),
            notes: "notes".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            has_secondary_password: false,
            entry_key_wrapped: None,
            entry_key_nonce: None,
            entry_key_salt: None,
            encrypted_secret: None,
            encrypted_secret_nonce: None,
        }
    }

    #[test]
    fn plain_entry_secret_can_be_changed() {
        let entry = make_entry(SecretType::Password);
        let mut screen = EditEntryScreen::new(entry);
        screen.secret = "new-secret".to_string();
        screen.secret_confirm = "new-secret".to_string();

        let EditEntryAction::Save(updated) = screen.try_save() else {
            panic!("expected save");
        };

        assert_eq!(updated.secret, "new-secret");
    }

    #[test]
    fn other_type_clears_password_and_network_metadata() {
        let entry = make_entry(SecretType::Password);
        let mut screen = EditEntryScreen::new(entry);
        screen.entry.secret_type = SecretType::Other(String::new());
        screen.custom_secret_type = "API".to_string();

        let EditEntryAction::Save(updated) = screen.try_save() else {
            panic!("expected save");
        };

        assert_eq!(updated.secret_type, SecretType::Other("API".to_string()));
        assert!(updated.network.is_empty());
        assert!(updated.public_address.is_none());
        assert!(updated.username.is_none());
        assert!(updated.url.is_none());
    }

    #[test]
    fn protected_entry_secret_can_be_reencrypted() {
        let entry_key_bytes = entry_key::generate_entry_key();
        let (encrypted_secret, encrypted_secret_nonce) =
            entry_key::encrypt_secret(&entry_key_bytes, "secret").unwrap();
        let (wrapped_key, wrapped_nonce, wrapped_salt) =
            entry_key::wrap_entry_key(&entry_key_bytes, "view-pass").unwrap();

        let mut entry = make_entry(SecretType::PrivateKey);
        entry.secret = "[encrypted]".to_string();
        entry.has_secondary_password = true;
        entry.entry_key_wrapped = Some(wrapped_key);
        entry.entry_key_nonce = Some(wrapped_nonce);
        entry.entry_key_salt = Some(wrapped_salt);
        entry.encrypted_secret = Some(encrypted_secret);
        entry.encrypted_secret_nonce = Some(encrypted_secret_nonce);

        let mut screen = EditEntryScreen::new(entry);
        screen.secret = "updated-secret".to_string();
        screen.secret_confirm = "updated-secret".to_string();
        screen.current_secondary_password = "view-pass".to_string();

        let EditEntryAction::Save(updated) = screen.try_save() else {
            panic!("expected save");
        };

        let decrypted_key = entry_key::unwrap_entry_key(
            updated.entry_key_wrapped.as_ref().unwrap(),
            updated.entry_key_nonce.as_ref().unwrap(),
            updated.entry_key_salt.as_ref().unwrap(),
            "view-pass",
        )
        .unwrap();
        let decrypted_secret = entry_key::decrypt_secret(
            &decrypted_key,
            updated.encrypted_secret.as_ref().unwrap(),
            updated.encrypted_secret_nonce.as_ref().unwrap(),
        )
        .unwrap();

        assert_eq!(updated.secret, "[encrypted]");
        assert_eq!(&*decrypted_secret, "updated-secret");
    }
}
