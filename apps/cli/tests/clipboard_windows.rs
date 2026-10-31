//! These tests replace the clipboard only on an explicitly opted-in disposable VM.
//! The ordinary test suite ignores them; native Windows CI runs them separately.
#![cfg(windows)]

use std::time::{Duration, Instant};

use arboard::Clipboard;
use termkey::clipboard;
use zeroize::Zeroizing;

const SYNTHETIC: &str = "TermKey synthetic Windows clipboard fixture";
const REPLACEMENT: &str = "TermKey independent synthetic Windows replacement";

struct PrivateClipboard(Clipboard);

impl PrivateClipboard {
    fn new() -> Self {
        assert!(
            std::env::var("TERMKEY_TEST_WINDOWS_CLIPBOARD").as_deref() == Ok("1")
                && std::env::var("GITHUB_ACTIONS").as_deref() == Ok("true"),
            "explicit disposable Windows runner clipboard opt-in is required"
        );
        Self(Clipboard::new().expect("native Windows clipboard connection"))
    }

    fn wait_for(&mut self, value: &str) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let current = self.0.get_text().ok().map(Zeroizing::new);
            if current.as_deref().map(String::as_str) == Some(value) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "synthetic clipboard value was unavailable"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for PrivateClipboard {
    fn drop(&mut self) {
        if let Ok(current) = self.0.get_text().map(Zeroizing::new) {
            if matches!(current.as_str(), SYNTHETIC | REPLACEMENT) {
                let _ = self.0.set_text(String::new());
            }
        }
    }
}

#[test]
#[ignore = "replaces clipboard; requires an explicitly opted-in disposable Windows runner"]
fn native_clipboard_retains_and_clears_synthetic_secret() {
    let mut reader = PrivateClipboard::new();
    let cleanup = clipboard::copy_and_clear(
        Zeroizing::new(SYNTHETIC.to_string()),
        Duration::from_millis(750),
    )
    .unwrap();
    reader.wait_for(SYNTHETIC);
    cleanup.wait().unwrap();
    assert!(reader.0.get_text().ok().as_deref() != Some(SYNTHETIC));
}

#[test]
#[ignore = "replaces clipboard; requires an explicitly opted-in disposable Windows runner"]
fn native_clipboard_preserves_independent_replacement() {
    let mut reader = PrivateClipboard::new();
    let cleanup = clipboard::copy_and_clear(
        Zeroizing::new(SYNTHETIC.to_string()),
        Duration::from_secs(30),
    )
    .unwrap();
    reader.wait_for(SYNTHETIC);
    let mut independent_writer = Clipboard::new().unwrap();
    independent_writer
        .set_text(REPLACEMENT.to_string())
        .unwrap();
    cleanup.clear_now().unwrap();
    reader.wait_for(REPLACEMENT);
}

#[test]
#[ignore = "replaces clipboard; requires an explicitly opted-in disposable Windows runner"]
fn native_clipboard_older_cleanup_preserves_newer_identical_secret() {
    let mut reader = PrivateClipboard::new();
    let older = clipboard::copy_and_clear(
        Zeroizing::new(SYNTHETIC.to_string()),
        Duration::from_secs(30),
    )
    .unwrap();
    let newer = clipboard::copy_and_clear(
        Zeroizing::new(SYNTHETIC.to_string()),
        Duration::from_secs(30),
    )
    .unwrap();
    older.clear_now().unwrap();
    reader.wait_for(SYNTHETIC);
    newer.clear_now().unwrap();
    assert!(reader.0.get_text().ok().as_deref() != Some(SYNTHETIC));
}
