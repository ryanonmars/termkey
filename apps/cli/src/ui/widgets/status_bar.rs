use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::update::{UpdateStatus, CURRENT_VERSION};

pub struct StatusBar {
    vault_name: String,
    entry_count: usize,
    filter_text: String,
    number_buffer: String,
    update_status: UpdateStatus,
}

impl StatusBar {
    pub fn new(
        vault_name: &str,
        entry_count: usize,
        filter_text: &str,
        number_buffer: &str,
        update_status: UpdateStatus,
    ) -> Self {
        Self {
            vault_name: vault_name.to_string(),
            entry_count,
            filter_text: filter_text.to_string(),
            number_buffer: number_buffer.to_string(),
            update_status,
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let filter_display = if self.filter_text.is_empty() {
            String::new()
        } else {
            format!(" │ Filter: {}", self.filter_text)
        };

        let number_display = if self.number_buffer.is_empty() {
            String::new()
        } else {
            format!(" │ Go to: {}█", self.number_buffer)
        };

        let left_content = format!(
            " {} │ {} entries{}{} │ ? for help ",
            self.vault_name, self.entry_count, filter_display, number_display
        );

        let base_style = Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD);
        let update_style = Style::default()
            .fg(Color::Black)
            .bg(Color::Yellow)
            .add_modifier(Modifier::BOLD);

        let version_content = format!(" v{} ", CURRENT_VERSION);
        let update_badge = match &self.update_status {
            UpdateStatus::Available(info) => Some(format!(" Update: v{} ", info.latest_version)),
            _ => None,
        };

        let width = area.width as usize;
        let full_right_width =
            text_width(&version_content) + update_badge.as_deref().map_or(0, text_width);

        let (show_version, show_update, right_width) = if full_right_width <= width {
            (true, update_badge, full_right_width)
        } else {
            let version_width = text_width(&version_content);
            if version_width <= width {
                (true, None, version_width)
            } else {
                (false, None, 0)
            }
        };

        let left_max_width = width.saturating_sub(right_width);
        let left_content = truncate_to_width(&left_content, left_max_width);
        let spacer_width = width.saturating_sub(text_width(&left_content) + right_width);

        let mut spans = vec![Span::styled(left_content, base_style)];
        if spacer_width > 0 {
            spans.push(Span::styled(" ".repeat(spacer_width), base_style));
        }
        if show_version {
            spans.push(Span::styled(version_content, base_style));
        }
        if let Some(update_badge) = show_update {
            spans.push(Span::styled(update_badge, update_style));
        }

        let paragraph = Paragraph::new(Line::from(spans)).style(base_style);
        frame.render_widget(paragraph, area);
    }
}

fn text_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

fn truncate_to_width(text: &str, max_width: usize) -> String {
    if text_width(text) <= max_width {
        return text.to_string();
    }

    if max_width == 0 {
        return String::new();
    }

    let ellipsis = "...";
    let ellipsis_width = text_width(ellipsis);
    if max_width <= ellipsis_width {
        return ".".repeat(max_width);
    }

    let mut truncated = String::new();
    let mut used_width = 0usize;

    for ch in text.chars() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if used_width + ch_width + ellipsis_width > max_width {
            break;
        }
        truncated.push(ch);
        used_width += ch_width;
    }

    truncated.push_str(ellipsis);
    truncated
}
