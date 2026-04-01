use std::env;
use std::io::{self, Write};
use std::time::Duration;

const ICON_PNG_BASE64: &str = include_str!("../../assets/branding/termkey-icon-64.b64");
const ICON_PNG_NAME_BASE64: &str = "dGVybWtleS1pY29uLnBuZw==";
const MIN_WIDTH: u16 = 40;
const KITTY_CHUNK_SIZE: usize = 4096;
const SPLASH_ICON_CELL_WIDTH: u16 = 24;
const SPLASH_ICON_CELL_HEIGHT: u16 = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GraphicsProtocol {
    Kitty,
    Iterm2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GraphicsPreference {
    Auto,
    Disabled,
    Forced(GraphicsProtocol),
}

pub fn print_splash_icon_if_supported(term_width: u16, term_height: u16) -> bool {
    let top_padding = term_height.saturating_sub(SPLASH_ICON_CELL_HEIGHT) / 2;
    print_icon_with_layout(
        term_width,
        top_padding,
        SPLASH_ICON_CELL_WIDTH,
        SPLASH_ICON_CELL_HEIGHT,
    )
}

fn print_icon_with_layout(
    term_width: u16,
    top_padding: u16,
    cell_width: u16,
    cell_height: u16,
) -> bool {
    if term_width < MIN_WIDTH {
        return false;
    }

    let Some(protocol) = detect_protocol() else {
        return false;
    };

    let mut out = io::stdout();
    let row = top_padding.saturating_add(1);
    let col = term_width.saturating_sub(cell_width) / 2 + 1;
    if write!(out, "\x1b[{};{}H", row, col).is_err() {
        return false;
    }

    let payload = icon_payload();

    let result = match protocol {
        GraphicsProtocol::Kitty => print_kitty_image(&mut out, &payload, cell_width, cell_height),
        GraphicsProtocol::Iterm2 => print_iterm2_image(&mut out, &payload, cell_width, cell_height),
    };

    if result.is_err() {
        return false;
    }

    let next_row = row.saturating_add(cell_height);
    let _ = write!(out, "\x1b[{};1H", next_row);
    let _ = out.flush();
    true
}

pub fn splash_delay() -> Duration {
    let millis = env::var("TERMKEY_SPLASH_MS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(900);
    Duration::from_millis(millis)
}

fn detect_protocol() -> Option<GraphicsProtocol> {
    match graphics_preference() {
        GraphicsPreference::Disabled => None,
        GraphicsPreference::Forced(protocol) => Some(protocol),
        GraphicsPreference::Auto => auto_detect_protocol(),
    }
}

fn graphics_preference() -> GraphicsPreference {
    let Ok(value) = env::var("TERMKEY_GRAPHICS") else {
        return GraphicsPreference::Auto;
    };

    match value.trim().to_ascii_lowercase().as_str() {
        "0" | "false" | "off" | "disable" | "disabled" | "none" => GraphicsPreference::Disabled,
        "1" | "true" | "on" | "auto" => GraphicsPreference::Auto,
        "kitty" | "ghostty" => GraphicsPreference::Forced(GraphicsProtocol::Kitty),
        "iterm" | "iterm2" => GraphicsPreference::Forced(GraphicsProtocol::Iterm2),
        _ => GraphicsPreference::Auto,
    }
}

fn auto_detect_protocol() -> Option<GraphicsProtocol> {
    if env::var_os("TMUX").is_some() {
        return None;
    }

    if env::var_os("KITTY_WINDOW_ID").is_some() {
        return Some(GraphicsProtocol::Kitty);
    }

    if env::var_os("ITERM_SESSION_ID").is_some() {
        return Some(GraphicsProtocol::Iterm2);
    }

    let term_program = env::var("TERM_PROGRAM").ok();
    if matches!(term_program.as_deref(), Some("ghostty")) {
        return Some(GraphicsProtocol::Kitty);
    }
    if matches!(term_program.as_deref(), Some("iTerm.app")) {
        return Some(GraphicsProtocol::Iterm2);
    }

    let term = env::var("TERM").unwrap_or_default();
    if term.contains("kitty") || term.contains("ghostty") {
        return Some(GraphicsProtocol::Kitty);
    }

    None
}

fn icon_payload() -> String {
    ICON_PNG_BASE64
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect()
}

fn print_kitty_image(
    out: &mut impl Write,
    payload: &str,
    cell_width: u16,
    cell_height: u16,
) -> io::Result<()> {
    let mut chunks = payload.as_bytes().chunks(KITTY_CHUNK_SIZE).peekable();
    let mut first = true;

    while let Some(chunk) = chunks.next() {
        let has_more = chunks.peek().is_some();
        let control = if first {
            format!(
                "a=T,f=100,c={},r={},C=1,q=2,m={}",
                cell_width,
                cell_height,
                if has_more { 1 } else { 0 }
            )
        } else {
            format!("m={}", if has_more { 1 } else { 0 })
        };
        first = false;

        out.write_all(b"\x1b_G")?;
        out.write_all(control.as_bytes())?;
        out.write_all(b";")?;
        out.write_all(chunk)?;
        out.write_all(b"\x1b\\")?;
    }

    Ok(())
}

fn print_iterm2_image(
    out: &mut impl Write,
    payload: &str,
    cell_width: u16,
    cell_height: u16,
) -> io::Result<()> {
    write!(
        out,
        "\x1b]1337;File=name={};inline=1;width={};height={};preserveAspectRatio=1:{}\x07",
        ICON_PNG_NAME_BASE64, cell_width, cell_height, payload
    )
}
