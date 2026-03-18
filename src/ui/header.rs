use colored::Colorize;
use unicode_width::UnicodeWidthStr;
use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use super::get_terminal_width;
use super::theme::dim_border;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Measure display width of a string, ignoring ANSI escape codes.
fn display_width(s: &str) -> usize {
    let stripped = console::strip_ansi_codes(s);
    UnicodeWidthStr::width(stripped.as_ref())
}

/// Print the application header, scaled to terminal width.
pub fn print_header() {
    let width = get_terminal_width() as usize;

    if width >= 50 {
        print_wide_header(width);
    } else {
        print_narrow_header(width);
    }
    println!();
}

fn print_wide_header(width: usize) {
    let inner = width.saturating_sub(4); // "│ " + " │"

    let title = "TermKey";
    let version_line = format!("v{}", VERSION);
    let tagline = "Encrypted vault for private keys & seed phrases";

    // Top border
    let title_embed = format!(" TermKey ");
    let title_dw = display_width(&title_embed);
    let remaining = (inner + 2).saturating_sub(title_dw + 1);
    println!(
        "{}{}{}{}{}",
        dim_border("┌"),
        dim_border("─"),
        title_embed.cyan().bold(),
        dim_border(&"─".repeat(remaining)),
        dim_border("┐")
    );

    // Empty line
    print_padded_line("", inner);

    // Title (centered, bold cyan)
    print_centered_line(&format!("{}", title.bold().cyan()), title, inner);

    // Empty line
    print_padded_line("", inner);

    // Version + tagline (centered, dimmed)
    let info = format!("{} — {}", version_line, tagline);
    print_centered_line(&format!("{}", info.dimmed()), &info, inner);

    // Empty line
    print_padded_line("", inner);

    // Bottom border
    println!(
        "{}{}{}",
        dim_border("└"),
        dim_border(&"─".repeat(inner + 2)),
        dim_border("┘")
    );
}

fn print_narrow_header(width: usize) {
    let text = format!("TermKey v{}", VERSION);
    let text_dw = display_width(&text);
    let side = width.saturating_sub(text_dw + 2) / 2;
    let right_side = width.saturating_sub(text_dw + 2 + side);
    println!(
        "{}{}{}{}{}",
        dim_border(&"─".repeat(side)),
        " ",
        text.cyan().bold(),
        " ",
        dim_border(&"─".repeat(right_side))
    );
}

fn print_padded_line(content: &str, inner: usize) {
    let content_width = display_width(content);
    let padding = inner.saturating_sub(content_width);
    println!(
        "{} {}{} {}",
        dim_border("│"),
        content,
        " ".repeat(padding),
        dim_border("│")
    );
}

fn print_centered_line(styled: &str, raw: &str, inner: usize) {
    let raw_width = display_width(raw);
    let left_pad = inner.saturating_sub(raw_width) / 2;
    let right_pad = inner.saturating_sub(raw_width + left_pad);
    println!(
        "{} {}{}{} {}",
        dim_border("│"),
        " ".repeat(left_pad),
        styled,
        " ".repeat(right_pad),
        dim_border("│")
    );
}

pub fn render_header(frame: &mut Frame, area: Rect) {
    let width = area.width as usize;

    let content = if width >= 50 {
        build_wide_header()
    } else {
        build_narrow_header()
    };

    frame.render_widget(content, area);
}

fn build_wide_header() -> Paragraph<'static> {
    let title = "TermKey";
    let version_line = format!("v{}", VERSION);
    let tagline = "Encrypted vault for private keys & seed phrases";
    let info = format!("{} — {}", version_line, tagline);

    let title_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let dim_style = Style::default().fg(Color::DarkGray);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(title, title_style)),
        Line::from(""),
        Line::from(Span::styled(info, dim_style)),
        Line::from(""),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" TermKey ")
        .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .border_style(Style::default().fg(Color::DarkGray));

    Paragraph::new(lines)
        .block(block)
        .alignment(Alignment::Center)
}

fn build_narrow_header() -> Paragraph<'static> {
    let text = format!("TermKey v{}", VERSION);

    let style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);

    let lines = vec![Line::from(Span::styled(text, style))];

    let block = Block::default()
        .borders(Borders::TOP | Borders::BOTTOM)
        .border_style(Style::default().fg(Color::DarkGray));

    Paragraph::new(lines)
        .block(block)
        .alignment(Alignment::Center)
}
