use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table},
    Frame,
};

use crate::vault::model::EntryMeta;

pub struct EntryTable {
    entries: Vec<EntryMeta>,
    selected: usize,
    filter: String,
    number_buffer: String,
}

impl EntryTable {
    pub fn new(entries: Vec<EntryMeta>) -> Self {
        Self {
            entries,
            selected: 0,
            filter: String::new(),
            number_buffer: String::new(),
        }
    }

    pub fn selected_index(&self) -> Option<usize> {
        let filtered = self.filtered_entries();
        if filtered.is_empty() {
            None
        } else {
            Some(filtered[self.selected].0)
        }
    }

    pub fn filter_text(&self) -> &str {
        &self.filter
    }

    pub fn filtered_count(&self) -> usize {
        self.filtered_entries().len()
    }

    pub fn number_buffer(&self) -> &str {
        &self.number_buffer
    }

    pub fn set_filter(&mut self, filter: String) {
        self.filter = filter;
        self.selected = 0;
    }

    pub fn handle_key(&mut self, key: KeyCode, _modifiers: KeyModifiers) {
        match key {
            KeyCode::Up => {
                let filtered_len = self.filtered_entries().len();
                if filtered_len == 0 {
                    return;
                }
                self.number_buffer.clear();
                if self.selected > 0 {
                    self.selected -= 1;
                } else {
                    self.selected = filtered_len - 1;
                }
            }
            KeyCode::Down => {
                let filtered_len = self.filtered_entries().len();
                if filtered_len == 0 {
                    return;
                }
                self.number_buffer.clear();
                self.selected = (self.selected + 1) % filtered_len;
            }
            KeyCode::Char(c) if c.is_ascii_digit() => {
                if self.filtered_entries().is_empty() {
                    return;
                }
                self.number_buffer.push(c);
            }
            KeyCode::Enter if !self.number_buffer.is_empty() => {
                let filtered_len = self.filtered_entries().len();
                if filtered_len == 0 {
                    self.number_buffer.clear();
                    return;
                }
                if let Ok(num) = self.number_buffer.parse::<usize>() {
                    if num > 0 && num <= filtered_len {
                        self.selected = num - 1;
                    }
                }
                self.number_buffer.clear();
            }
            KeyCode::Char('/') => {
                self.number_buffer.clear();
            }
            KeyCode::Backspace => {
                if !self.number_buffer.is_empty() {
                    self.number_buffer.pop();
                } else if !self.filter.is_empty() {
                    self.filter.pop();
                    self.selected = 0;
                }
            }
            KeyCode::Esc => {
                if !self.number_buffer.is_empty() {
                    self.number_buffer.clear();
                } else if !self.filter.is_empty() {
                    self.filter.clear();
                    self.selected = 0;
                }
            }
            _ => {}
        }
    }

    fn filtered_entries(&self) -> Vec<(usize, &EntryMeta)> {
        if self.filter.is_empty() {
            self.entries.iter().enumerate().collect()
        } else {
            let filter_lower = self.filter.to_lowercase();
            let filter_normalized = normalize_search(&self.filter);
            self.entries
                .iter()
                .enumerate()
                .filter(|(_, e)| matches_entry_filter(e, &filter_lower, &filter_normalized))
                .collect()
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let filtered = self.filtered_entries();

        if filtered.is_empty() {
            let block = Block::default()
                .borders(Borders::ALL)
                .title(" Entries ")
                .border_style(Style::default().fg(Color::Cyan));

            let empty_msg = if self.filter.is_empty() {
                "No entries yet. Press 'a' to add one."
            } else {
                "No entries match filter."
            };

            let empty = ratatui::widgets::Paragraph::new(empty_msg)
                .block(block)
                .style(Style::default().fg(Color::DarkGray));

            frame.render_widget(empty, area);
            return;
        }

        let header_cells = ["#", "Name", "Type"].iter().map(|h| {
            Cell::from(*h).style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
        });
        let header = Row::new(header_cells).height(1);

        let rows = filtered
            .iter()
            .enumerate()
            .map(|(idx, (_original_idx, entry))| {
                let display_num = idx + 1;
                let lock_indicator = if entry.has_secondary_password {
                    " [locked]"
                } else {
                    ""
                };
                let name_display = format!("{}{}", entry.name, lock_indicator);

                let cells = vec![
                    Cell::from(display_num.to_string()),
                    Cell::from(name_display),
                    Cell::from(entry.secret_type.to_string()),
                ];

                let style = if idx == self.selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                Row::new(cells).style(style)
            });

        let widths = [
            ratatui::layout::Constraint::Length(4),
            ratatui::layout::Constraint::Percentage(66),
            ratatui::layout::Constraint::Percentage(34),
        ];

        let table = Table::new(rows, widths)
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Entries ")
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .column_spacing(1);

        frame.render_widget(table, area);
    }
}

fn normalize_search(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

fn field_matches(field: &str, filter_lower: &str, filter_normalized: &str) -> bool {
    let field_lower = field.to_lowercase();
    field_lower.contains(filter_lower)
        || (!filter_normalized.is_empty()
            && normalize_search(&field_lower).contains(filter_normalized))
}

fn matches_entry_filter(entry: &EntryMeta, filter_lower: &str, filter_normalized: &str) -> bool {
    field_matches(&entry.name, filter_lower, filter_normalized)
        || field_matches(
            &entry.secret_type.to_string(),
            filter_lower,
            filter_normalized,
        )
        || field_matches(&entry.network, filter_lower, filter_normalized)
        || field_matches(&entry.notes, filter_lower, filter_normalized)
        || entry
            .public_address
            .as_deref()
            .is_some_and(|value| field_matches(value, filter_lower, filter_normalized))
        || entry
            .username
            .as_deref()
            .is_some_and(|value| field_matches(value, filter_lower, filter_normalized))
        || entry
            .url
            .as_deref()
            .is_some_and(|value| field_matches(value, filter_lower, filter_normalized))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::model::SecretType;

    fn make_entry(name: &str, secret_type: SecretType) -> EntryMeta {
        EntryMeta {
            name: name.to_string(),
            network: "Ethereum".to_string(),
            secret_type,
            public_address: Some("0x1234567890abcdef".to_string()),
            username: Some("demo-user".to_string()),
            url: Some("https://example.com".to_string()),
            site_rules: Vec::new(),
            notes: "some notes".to_string(),
            has_secondary_password: false,
        }
    }

    #[test]
    fn filter_matches_secret_type_text() {
        let mut table = EntryTable::new(vec![make_entry("Wallet", SecretType::PrivateKey)]);
        table.set_filter("private".to_string());

        assert_eq!(table.filtered_count(), 1);
    }

    #[test]
    fn filter_matches_normalized_secret_type_text() {
        let mut table = EntryTable::new(vec![make_entry("Wallet", SecretType::PrivateKey)]);
        table.set_filter("privatekey".to_string());

        assert_eq!(table.filtered_count(), 1);
    }

    #[test]
    fn esc_clears_filter_even_when_no_results() {
        let mut table = EntryTable::new(vec![make_entry("Wallet", SecretType::PrivateKey)]);
        table.set_filter("does-not-exist".to_string());

        table.handle_key(KeyCode::Esc, KeyModifiers::NONE);

        assert_eq!(table.filter_text(), "");
        assert_eq!(table.filtered_count(), 1);
    }
}
