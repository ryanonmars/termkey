use std::fmt;

use dialoguer::theme::{SimpleTheme, Theme};

use crate::links::sanitize_terminal_text;

/// Sanitize the display layer only: accepting an existing default must preserve
/// its original value, including legacy metadata containing control characters.
pub(crate) struct SafePromptTheme;

pub(crate) static SAFE_THEME: SafePromptTheme = SafePromptTheme;

impl Theme for SafePromptTheme {
    fn format_prompt(&self, f: &mut dyn fmt::Write, prompt: &str) -> fmt::Result {
        SimpleTheme.format_prompt(f, &sanitize_terminal_text(prompt))
    }

    fn format_error(&self, f: &mut dyn fmt::Write, error: &str) -> fmt::Result {
        SimpleTheme.format_error(f, &sanitize_terminal_text(error))
    }

    fn format_confirm_prompt(
        &self,
        f: &mut dyn fmt::Write,
        prompt: &str,
        default: Option<bool>,
    ) -> fmt::Result {
        SimpleTheme.format_confirm_prompt(f, &sanitize_terminal_text(prompt), default)
    }

    fn format_confirm_prompt_selection(
        &self,
        f: &mut dyn fmt::Write,
        prompt: &str,
        selection: Option<bool>,
    ) -> fmt::Result {
        SimpleTheme.format_confirm_prompt_selection(f, &sanitize_terminal_text(prompt), selection)
    }

    fn format_input_prompt(
        &self,
        f: &mut dyn fmt::Write,
        prompt: &str,
        default: Option<&str>,
    ) -> fmt::Result {
        let default = default.map(sanitize_terminal_text);
        SimpleTheme.format_input_prompt(f, &sanitize_terminal_text(prompt), default.as_deref())
    }

    fn format_input_prompt_selection(
        &self,
        f: &mut dyn fmt::Write,
        prompt: &str,
        selection: &str,
    ) -> fmt::Result {
        SimpleTheme.format_input_prompt_selection(
            f,
            &sanitize_terminal_text(prompt),
            &sanitize_terminal_text(selection),
        )
    }

    fn format_select_prompt_item(
        &self,
        f: &mut dyn fmt::Write,
        text: &str,
        active: bool,
    ) -> fmt::Result {
        SimpleTheme.format_select_prompt_item(f, &sanitize_terminal_text(text), active)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_untrusted_prompts_defaults_and_selections_without_controls() {
        let payload = "login\x1b]52;c;cGF5bG9hZA==\x07\u{009b}2J\rspoof";
        let mut rendered = String::new();
        SAFE_THEME.format_prompt(&mut rendered, payload).unwrap();
        SAFE_THEME.format_error(&mut rendered, payload).unwrap();
        SAFE_THEME
            .format_confirm_prompt(&mut rendered, payload, Some(false))
            .unwrap();
        SAFE_THEME
            .format_confirm_prompt_selection(&mut rendered, payload, Some(true))
            .unwrap();
        SAFE_THEME
            .format_input_prompt(&mut rendered, payload, Some(payload))
            .unwrap();
        SAFE_THEME
            .format_input_prompt_selection(&mut rendered, payload, payload)
            .unwrap();
        SAFE_THEME
            .format_select_prompt(&mut rendered, payload)
            .unwrap();
        SAFE_THEME
            .format_select_prompt_item(&mut rendered, payload, true)
            .unwrap();
        SAFE_THEME
            .format_select_prompt_selection(&mut rendered, payload, payload)
            .unwrap();

        assert!(!rendered.chars().any(char::is_control));
        assert!(rendered.contains("login"));
        assert!(payload.contains('\x1b'));
    }
}
