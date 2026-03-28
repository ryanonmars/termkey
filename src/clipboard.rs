use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use arboard::Clipboard;

use crate::error::{Result, TermKeyError};

/// Copy text to clipboard and spawn a background thread to clear it after `clear_after` seconds.
///
/// On Linux/X11, clipboard content is owned by the process. We keep the `Clipboard` object alive
/// in the background thread so it can respond to clipboard requests until the clear timeout.
pub fn copy_and_clear(text: &str, clear_after_secs: u64) -> Result<()> {
    let text_owned = text.to_string();
    let duration = Duration::from_secs(clear_after_secs);
    let (tx, rx) = mpsc::channel::<std::result::Result<(), String>>();

    thread::spawn(move || {
        let mut clipboard = match Clipboard::new() {
            Ok(cb) => cb,
            Err(e) => {
                let _ = tx.send(Err(e.to_string()));
                return;
            }
        };
        if let Err(e) = clipboard.set_text(&text_owned) {
            let _ = tx.send(Err(e.to_string()));
            return;
        }
        // Signal success before sleeping so the caller returns immediately.
        let _ = tx.send(Ok(()));
        // Keep `clipboard` alive to serve X11 selection requests until we clear.
        thread::sleep(duration);
        let _ = clipboard.set_text(String::new());
    });

    rx.recv()
        .map_err(|_| TermKeyError::Clipboard("clipboard thread crashed".to_string()))?
        .map_err(|e| TermKeyError::Clipboard(e))
}
