pub mod app;
pub mod borders;
pub mod header;
pub mod prompt;
pub mod screens;
pub mod terminal;
pub mod terminal_graphics;
pub mod theme;
pub mod widgets;

use std::io::IsTerminal;
use std::thread;

/// Get the current terminal width, with a fallback of 80.
pub fn get_terminal_width() -> u16 {
    let term = console::Term::stdout();
    let (_, cols) = term.size();
    if cols == 0 {
        80
    } else {
        cols
    }
}

/// Get the current terminal height, with a fallback of 24.
pub fn get_terminal_height() -> u16 {
    let term = console::Term::stdout();
    let (rows, _) = term.size();
    if rows == 0 {
        24
    } else {
        rows
    }
}

/// Check if stdout is connected to an interactive terminal.
pub fn is_interactive() -> bool {
    std::io::stdout().is_terminal()
}

/// Set up the app theme: clear screen, set window title, print header.
pub fn setup_app_theme(clear: bool) {
    if !is_interactive() {
        return;
    }
    theme::set_title("TermKey");
    if clear {
        theme::clear_screen();
    }
    header::print_header();
}

pub fn show_startup_splash() {
    if !is_interactive() {
        return;
    }

    theme::set_title("TermKey");
    theme::clear_screen();

    let width = get_terminal_width();
    let height = get_terminal_height();
    if terminal_graphics::print_splash_icon_if_supported(width, height) {
        thread::sleep(terminal_graphics::splash_delay());
        theme::clear_screen();
    }
}
