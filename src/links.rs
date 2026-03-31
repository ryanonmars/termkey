use std::io::{Error, ErrorKind};
use std::process::Command;

use crate::error::{Result, TermKeyError};

pub fn format_terminal_hyperlink(text: &str, url: &str) -> String {
    if !is_web_url(url) {
        return text.to_string();
    }

    format!("\x1b]8;;{}\x1b\\{}\x1b]8;;\x1b\\", url, text)
}

pub fn open_url(url: &str) -> Result<()> {
    if !is_web_url(url) {
        return Err(TermKeyError::Io(Error::new(
            ErrorKind::InvalidInput,
            "URL must start with http:// or https://",
        )));
    }

    let status = if cfg!(target_os = "macos") {
        Command::new("open").arg(url).status()
    } else if cfg!(target_os = "windows") {
        Command::new("cmd").args(["/C", "start", "", url]).status()
    } else {
        Command::new("xdg-open").arg(url).status()
    }
    .map_err(TermKeyError::Io)?;

    if status.success() {
        Ok(())
    } else {
        Err(TermKeyError::Io(Error::new(
            ErrorKind::Other,
            format!("failed to open URL: {}", url),
        )))
    }
}

pub fn is_web_url(url: &str) -> bool {
    let trimmed = url.trim();
    trimmed.starts_with("http://") || trimmed.starts_with("https://")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hyperlink_format_wraps_web_urls() {
        let rendered = format_terminal_hyperlink("https://example.com", "https://example.com");
        assert!(rendered.contains("]8;;https://example.com"));
        assert!(rendered.contains("https://example.com"));
    }

    #[test]
    fn hyperlink_format_leaves_non_web_urls_plain() {
        assert_eq!(
            format_terminal_hyperlink("example.com", "example.com"),
            "example.com"
        );
    }
}
