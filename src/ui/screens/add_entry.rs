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

use crate::crypto::derive::derive_address;
use crate::crypto::entry_key;
use crate::vault::model::{Entry, SecretType};

pub struct AddEntryScreen {
    current_field: usize,
    name: String,
    secret_type: SecretType,
    custom_secret_type: String,
    secret: String,
    secret_confirm: String,
    network: String,
    custom_network: String,
    use_custom_network: bool,
    username: String,
    url: String,
    notes: String,
    use_secondary_password: bool,
    secondary_password: String,
    secondary_password_confirm: String,
    show_type_select: bool,
    type_selected: usize,
    show_network_select: bool,
    network_selected: usize,
    scroll_offset: usize,
}

impl Drop for AddEntryScreen {
    fn drop(&mut self) {
        self.secret.zeroize();
        self.secret_confirm.zeroize();
        self.secondary_password.zeroize();
        self.secondary_password_confirm.zeroize();
    }
}

impl AddEntryScreen {
    pub fn new() -> Self {
        Self {
            current_field: 0,
            name: String::new(),
            secret_type: SecretType::PrivateKey,
            custom_secret_type: String::new(),
            secret: String::new(),
            secret_confirm: String::new(),
            network: "Ethereum".to_string(),
            custom_network: String::new(),
            use_custom_network: false,
            username: String::new(),
            url: String::new(),
            notes: String::new(),
            use_secondary_password: false,
            secondary_password: String::new(),
            secondary_password_confirm: String::new(),
            show_type_select: false,
            type_selected: 0,
            show_network_select: false,
            network_selected: 0,
            scroll_offset: 0,
        }
    }

    pub fn handle_key(&mut self, key: KeyCode, modifiers: KeyModifiers) -> AddEntryAction {
        if key == KeyCode::Esc {
            return AddEntryAction::Cancel;
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
                AddEntryAction::Continue
            }
            KeyCode::BackTab => {
                if self.current_field == 0 {
                    self.current_field = self.field_count() - 1;
                } else {
                    self.current_field -= 1;
                }
                AddEntryAction::Continue
            }
            KeyCode::Up => {
                if self.current_field > 0 {
                    self.current_field -= 1;
                }
                AddEntryAction::Continue
            }
            KeyCode::Down => {
                self.current_field = (self.current_field + 1) % self.field_count();
                AddEntryAction::Continue
            }
            KeyCode::Enter => {
                // Secret type selector
                if self.current_field == 1 {
                    self.show_type_select = true;
                }
                // Network selector (crypto only)
                else if self.is_crypto_type() && self.current_field == self.network_field() {
                    self.show_network_select = true;
                }
                // Secondary password toggle
                else if self.current_field == self.secondary_toggle_field() {
                    self.use_secondary_password = !self.use_secondary_password;
                    if !self.use_secondary_password {
                        self.secondary_password.zeroize();
                        self.secondary_password = String::new();
                        self.secondary_password_confirm.zeroize();
                        self.secondary_password_confirm = String::new();
                    }
                }
                // Last field -> save
                else if self.current_field == self.field_count() - 1 {
                    return self.try_save();
                } else {
                    self.current_field = (self.current_field + 1) % self.field_count();
                }
                AddEntryAction::Continue
            }
            KeyCode::Char(c) if !modifiers.contains(KeyModifiers::CONTROL) => {
                self.insert_char(c);
                AddEntryAction::Continue
            }
            KeyCode::Backspace => {
                self.delete_char();
                AddEntryAction::Continue
            }
            _ => AddEntryAction::Continue,
        }
    }

    fn handle_type_select(&mut self, key: KeyCode) -> AddEntryAction {
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
                self.secret_type = match self.type_selected {
                    0 => SecretType::PrivateKey,
                    1 => SecretType::SeedPhrase,
                    2 => SecretType::Password,
                    _ => SecretType::Other(self.custom_secret_type.trim().to_string()),
                };
                if self.is_crypto_type() && self.network.is_empty() && !self.use_custom_network {
                    self.network = "Ethereum".to_string();
                }
                if !self.secret_type.is_other_type() {
                    self.custom_secret_type.clear();
                }
                self.show_type_select = false;
                self.current_field += 1;
            }
            KeyCode::Esc => {
                self.show_type_select = false;
            }
            _ => {}
        }
        AddEntryAction::Continue
    }

    fn handle_network_select(&mut self, key: KeyCode) -> AddEntryAction {
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
                self.current_field += 1;
            }
            KeyCode::Esc => {
                self.show_network_select = false;
            }
            _ => {}
        }
        AddEntryAction::Continue
    }

    /// Field index of the secondary password toggle.
    fn secondary_toggle_field(&self) -> usize {
        self.notes_field() + 1
    }

    fn insert_char(&mut self, c: char) {
        match self.current_field {
            0 => self.name.push(c),
            f if self.has_custom_type_field() && f == self.custom_type_field() => {
                self.custom_secret_type.push(c);
            }
            f if f == self.secret_field() => self.secret.push(c),
            f if f == self.confirm_field() => self.secret_confirm.push(c),
            f if self.has_custom_network_field() && f == self.custom_network_field() => {
                self.custom_network.push(c);
            }
            f if self.is_password_type() && f == self.username_field() => self.username.push(c),
            f if self.is_password_type() && f == self.url_field() => self.url.push(c),
            f if f == self.notes_field() => self.notes.push(c),
            f if self.use_secondary_password && f == self.secondary_toggle_field() + 1 => {
                self.secondary_password.push(c);
            }
            f if self.use_secondary_password && f == self.secondary_toggle_field() + 2 => {
                self.secondary_password_confirm.push(c);
            }
            _ => {}
        }
    }

    fn delete_char(&mut self) {
        match self.current_field {
            0 => {
                self.name.pop();
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
            f if self.is_password_type() && f == self.username_field() => {
                self.username.pop();
            }
            f if self.is_password_type() && f == self.url_field() => {
                self.url.pop();
            }
            f if f == self.notes_field() => {
                self.notes.pop();
            }
            f if self.use_secondary_password && f == self.secondary_toggle_field() + 1 => {
                self.secondary_password.pop();
            }
            f if self.use_secondary_password && f == self.secondary_toggle_field() + 2 => {
                self.secondary_password_confirm.pop();
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
            base += 1; // network
        }
        if self.has_custom_network_field() {
            base += 1;
        }
        if self.is_password_type() {
            base += 2; // username, url
        }
        base += 1; // toggle
        if self.use_secondary_password {
            base + 2 // secondary password + confirm
        } else {
            base
        }
    }

    fn is_crypto_type(&self) -> bool {
        self.secret_type.is_crypto_type()
    }

    fn is_password_type(&self) -> bool {
        self.secret_type.is_password_type()
    }

    fn has_custom_type_field(&self) -> bool {
        self.secret_type.is_other_type()
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

    fn username_field(&self) -> usize {
        self.confirm_field() + 1
    }

    fn url_field(&self) -> usize {
        self.username_field() + 1
    }

    fn notes_field(&self) -> usize {
        let mut idx = self.confirm_field() + 1;
        if self.is_crypto_type() {
            idx += 1;
            if self.has_custom_network_field() {
                idx += 1;
            }
        } else if self.is_password_type() {
            idx += 2;
        }
        idx
    }

    fn effective_secret_type(&self) -> Option<SecretType> {
        match &self.secret_type {
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

    fn try_save(&self) -> AddEntryAction {
        if self.name.is_empty() {
            return AddEntryAction::Continue;
        }

        let Some(secret_type) = self.effective_secret_type() else {
            return AddEntryAction::Continue;
        };

        if self.secret.is_empty() || self.secret != self.secret_confirm {
            return AddEntryAction::Continue;
        }

        if self.use_secondary_password {
            if self.secondary_password.is_empty()
                || self.secondary_password != self.secondary_password_confirm
            {
                return AddEntryAction::Continue;
            }
        }

        let network = self.effective_network();
        if self.is_crypto_type() && network.is_empty() {
            return AddEntryAction::Continue;
        }

        // Auto-derive public address for crypto types
        let public_address = if self.is_crypto_type() {
            match derive_address(&self.secret, &self.secret_type, &network) {
                Ok(addr) => addr,
                Err(_) => None, // Bad key format — save with no address
            }
        } else {
            None
        };

        let now = Utc::now();

        // Handle secondary password encryption
        let (
            has_secondary,
            secret_to_store,
            encrypted_secret,
            encrypted_secret_nonce,
            entry_key_wrapped,
            entry_key_nonce,
            entry_key_salt,
        ) = if self.use_secondary_password {
            let ek = entry_key::generate_entry_key();
            let (ct, ct_nonce) = match entry_key::encrypt_secret(&ek, &self.secret) {
                Ok(v) => v,
                Err(_) => return AddEntryAction::Continue,
            };
            let (wrapped, wrap_nonce, salt) =
                match entry_key::wrap_entry_key(&ek, &self.secondary_password) {
                    Ok(v) => v,
                    Err(_) => return AddEntryAction::Continue,
                };
            (
                true,
                "[encrypted]".to_string(),
                Some(ct),
                Some(ct_nonce),
                Some(wrapped),
                Some(wrap_nonce),
                Some(salt),
            )
        } else {
            (false, self.secret.clone(), None, None, None, None, None)
        };

        let entry = Entry {
            name: self.name.clone(),
            secret: secret_to_store,
            secret_type,
            network,
            public_address,
            username: if self.username.is_empty() {
                None
            } else {
                Some(self.username.clone())
            },
            url: if self.url.is_empty() {
                None
            } else {
                Some(self.url.clone())
            },
            notes: self.notes.clone(),
            created_at: now,
            updated_at: now,
            has_secondary_password: has_secondary,
            entry_key_wrapped,
            entry_key_nonce,
            entry_key_salt,
            encrypted_secret,
            encrypted_secret_nonce,
        };

        AddEntryAction::Save(entry)
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
            .title(" Add New Entry ")
            .title_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .border_style(Style::default().fg(Color::Cyan));

        frame.render_widget(block.clone(), form_area);

        let inner = block.inner(form_area);

        // Calculate visible area and scroll offset
        let available_height = inner.height as usize;

        // Ensure current field is visible
        let mut scroll_offset = self.scroll_offset;
        if self.current_field >= scroll_offset + available_height / 2 {
            scroll_offset = self.current_field.saturating_sub(available_height / 2 - 1);
        } else if self.current_field < scroll_offset {
            scroll_offset = self.current_field;
        }

        let mut lines = vec![];
        let mut field_idx = 0;

        // Field 0: Name
        lines.push(self.render_field(field_idx, "Entry name", &self.name, false));
        field_idx += 1;

        // Field 1: Secret type
        lines.push(Line::from(""));
        let secret_type_str = self.secret_type.to_string();
        lines.push(self.render_field(field_idx, "Secret type", &secret_type_str, false));
        field_idx += 1;

        if self.has_custom_type_field() {
            lines.push(Line::from(""));
            lines.push(self.render_field(
                field_idx,
                "Custom type",
                &self.custom_secret_type,
                false,
            ));
            field_idx += 1;
        }

        // Field 2: Secret
        lines.push(Line::from(""));
        let secret_masked = "\u{2022}".repeat(self.secret.len());
        lines.push(self.render_field(field_idx, "Secret", &secret_masked, false));
        field_idx += 1;

        // Field 3: Confirm secret
        lines.push(Line::from(""));
        let secret_confirm_masked = "\u{2022}".repeat(self.secret_confirm.len());
        lines.push(self.render_field(field_idx, "Confirm secret", &secret_confirm_masked, false));
        field_idx += 1;

        if self.is_crypto_type() {
            // Field 4: Network
            lines.push(Line::from(""));
            let network_value = if self.use_custom_network {
                "Other"
            } else {
                &self.network
            };
            lines.push(self.render_field(field_idx, "Network", network_value, false));
            field_idx += 1;

            if self.use_custom_network {
                lines.push(Line::from(""));
                lines.push(self.render_field(
                    field_idx,
                    "Custom network",
                    &self.custom_network,
                    false,
                ));
                field_idx += 1;
            }
        } else if self.is_password_type() {
            // Field 4: Username
            lines.push(Line::from(""));
            lines.push(self.render_field(field_idx, "Username (optional)", &self.username, false));
            field_idx += 1;

            // Field 5: URL
            lines.push(Line::from(""));
            lines.push(self.render_field(field_idx, "URL (optional)", &self.url, false));
            field_idx += 1;
        }

        // Notes
        lines.push(Line::from(""));
        lines.push(self.render_field(field_idx, "Notes (optional)", &self.notes, false));
        field_idx += 1;

        // Secondary password toggle
        lines.push(Line::from(""));
        let toggle_value = if self.use_secondary_password {
            "Yes"
        } else {
            "No"
        };
        lines.push(self.render_field(field_idx, "Secondary password", toggle_value, false));
        field_idx += 1;

        // Secondary password fields (only when toggled on)
        let sp_masked = "\u{2022}".repeat(self.secondary_password.len());
        let sp_confirm_masked = "\u{2022}".repeat(self.secondary_password_confirm.len());
        if self.use_secondary_password {
            lines.push(Line::from(""));
            lines.push(self.render_field(field_idx, "Secondary pwd", &sp_masked, false));
            field_idx += 1;

            lines.push(Line::from(""));
            lines.push(self.render_field(
                field_idx,
                "Confirm secondary",
                &sp_confirm_masked,
                false,
            ));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(""));

        let help_text = if self.current_field == 1 {
            "\u{2191}\u{2193}: Scroll \u{2502} Enter: Select \u{2502} Tab: Next \u{2502} Esc: Cancel"
        } else if self.is_crypto_type() && self.current_field == self.network_field() {
            "\u{2191}\u{2193}: Scroll \u{2502} Enter: Select \u{2502} Tab: Next \u{2502} Esc: Cancel"
        } else if self.current_field == self.secondary_toggle_field() {
            "\u{2191}\u{2193}: Scroll \u{2502} Enter: Toggle \u{2502} Tab: Next \u{2502} Ctrl+S: Save \u{2502} Esc: Cancel"
        } else {
            "\u{2191}\u{2193}: Scroll \u{2502} Tab: Next \u{2502} Shift+Tab: Previous \u{2502} Ctrl+S: Save \u{2502} Esc: Cancel"
        };

        lines.push(Line::from(vec![Span::styled(
            help_text,
            Style::default().fg(Color::DarkGray),
        )]));

        // Skip lines based on scroll offset
        let visible_lines: Vec<Line> = lines
            .into_iter()
            .skip(scroll_offset * 2)
            .take(available_height)
            .collect();

        let paragraph = Paragraph::new(visible_lines);
        frame.render_widget(paragraph, inner);
    }

    fn render_field<'a>(
        &self,
        idx: usize,
        label: &str,
        value: &'a str,
        _multiline: bool,
    ) -> Line<'a> {
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

        let cursor = if is_active { "\u{2588}" } else { "" };

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

pub enum AddEntryAction {
    Continue,
    Save(Entry),
    Cancel,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_entries_do_not_keep_default_network() {
        let mut screen = AddEntryScreen::new();
        screen.name = "API Secret".to_string();
        screen.secret_type = SecretType::Password;
        screen.secret = "secret".to_string();
        screen.secret_confirm = "secret".to_string();

        let AddEntryAction::Save(entry) = screen.try_save() else {
            panic!("expected save action");
        };

        assert!(entry.network.is_empty());
        assert!(entry.public_address.is_none());
    }

    #[test]
    fn custom_secret_type_uses_typed_value() {
        let mut screen = AddEntryScreen::new();
        screen.name = "Service Secret".to_string();
        screen.secret_type = SecretType::Other(String::new());
        screen.custom_secret_type = "API".to_string();
        screen.secret = "secret".to_string();
        screen.secret_confirm = "secret".to_string();

        let AddEntryAction::Save(entry) = screen.try_save() else {
            panic!("expected save action");
        };

        assert_eq!(entry.secret_type, SecretType::Other("API".to_string()));
        assert!(entry.network.is_empty());
        assert!(entry.public_address.is_none());
    }

    #[test]
    fn custom_network_uses_typed_value() {
        let mut screen = AddEntryScreen::new();
        screen.name = "Custom Entry".to_string();
        screen.secret = "not-a-real-key".to_string();
        screen.secret_confirm = "not-a-real-key".to_string();
        screen.use_custom_network = true;
        screen.custom_network = "API".to_string();

        let AddEntryAction::Save(entry) = screen.try_save() else {
            panic!("expected save action");
        };

        assert_eq!(entry.network, "API");
    }
}
