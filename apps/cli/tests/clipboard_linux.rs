//! Native clipboard checks use synthetic values only and are opt-in. Never set
//! TERMKEY_TEST_GRAPHICAL_CLIPBOARD=1 against a user's desktop: the tests replace
//! clipboard content. scripts/test-linux-clipboard.sh starts isolated servers.
#![cfg(target_os = "linux")]

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::{mpsc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use arboard::Clipboard;
use termkey::clipboard;
use zeroize::Zeroizing;

static SERIAL: Mutex<()> = Mutex::new(());
const SYNTHETIC: &str = "TermKey synthetic clipboard fixture, not a credential";
const EXTERNAL: &str = "TermKey independent synthetic clipboard replacement";

fn graphical_enabled() -> bool {
    std::env::var("TERMKEY_TEST_GRAPHICAL_CLIPBOARD").as_deref() == Ok("1")
}

fn wait_for_value(reader: &mut Clipboard, expected: &str) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if reader.get_text().ok().as_deref() == Some(expected) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "synthetic clipboard value was not available"
        );
        thread::sleep(Duration::from_millis(20));
    }
}

struct ExternalWriter(Child);

impl ExternalWriter {
    fn start() -> Self {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "external_writer_helper", "--nocapture"])
            .env("TERMKEY_TEST_GRAPHICAL_CLIPBOARD", "1")
            .env("TERMKEY_TEST_EXTERNAL_WRITER", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let output = child.stdout.take().unwrap();
        let (ready, notification) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(output).lines().map_while(Result::ok) {
                if line.ends_with("TERMKEY_CLIPBOARD_WRITER_READY") {
                    let _ = ready.send(());
                    break;
                }
            }
        });
        let mut writer = Self(child);
        if notification.recv_timeout(Duration::from_secs(10)).is_err() {
            let _ = writer.0.kill();
            let mut diagnostic = String::new();
            if let Some(mut error_output) = writer.0.stderr.take() {
                let _ = error_output.read_to_string(&mut diagnostic);
            }
            let _ = writer.0.wait();
            panic!("isolated external clipboard writer did not start: {diagnostic}");
        }
        writer
    }
}

impl Drop for ExternalWriter {
    fn drop(&mut self) {
        if let Some(mut input) = self.0.stdin.take() {
            let _ = input.write_all(b"q");
        }
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn external_writer_helper() {
    if !graphical_enabled() || std::env::var("TERMKEY_TEST_EXTERNAL_WRITER").as_deref() != Ok("1") {
        return;
    }
    let mut backend = Clipboard::new().expect("isolated external clipboard connection");
    backend.set_text(EXTERNAL.to_string()).unwrap();
    println!("TERMKEY_CLIPBOARD_WRITER_READY");
    std::io::stdout().flush().unwrap();
    let mut gate = [0u8; 1];
    let _ = std::io::stdin().read_exact(&mut gate);
    drop(backend);
}

#[test]
fn text_copy_retains_selection_ownership() {
    if !graphical_enabled() {
        return;
    }
    let _guard = SERIAL.lock().unwrap();
    let retained = clipboard::copy_text(SYNTHETIC).unwrap();
    let mut reader = Clipboard::new().unwrap();
    wait_for_value(&mut reader, SYNTHETIC);
    thread::sleep(Duration::from_millis(150));
    wait_for_value(&mut reader, SYNTHETIC);
    drop(retained);
}

#[test]
fn synthetic_secret_is_cleared_after_timeout() {
    if !graphical_enabled() {
        return;
    }
    let _guard = SERIAL.lock().unwrap();
    let cleanup = clipboard::copy_and_clear(
        Zeroizing::new(SYNTHETIC.to_string()),
        Duration::from_millis(600),
    )
    .unwrap();
    let mut reader = Clipboard::new().unwrap();
    wait_for_value(&mut reader, SYNTHETIC);
    cleanup.wait().unwrap();
    assert_ne!(reader.get_text().ok().as_deref(), Some(SYNTHETIC));
}

#[test]
fn external_process_replacement_survives_secret_cleanup() {
    if !graphical_enabled() {
        return;
    }
    let _guard = SERIAL.lock().unwrap();
    let cleanup = clipboard::copy_and_clear(
        Zeroizing::new(SYNTHETIC.to_string()),
        Duration::from_secs(30),
    )
    .unwrap();
    let mut reader = Clipboard::new().unwrap();
    wait_for_value(&mut reader, SYNTHETIC);
    let _external = ExternalWriter::start();
    wait_for_value(&mut reader, EXTERNAL);
    cleanup.clear_now().unwrap();
    wait_for_value(&mut reader, EXTERNAL);
}

#[test]
fn older_timer_preserves_newer_identical_secret() {
    if !graphical_enabled() {
        return;
    }
    let _guard = SERIAL.lock().unwrap();
    let older = clipboard::copy_and_clear(
        Zeroizing::new(SYNTHETIC.to_string()),
        Duration::from_millis(300),
    )
    .unwrap();
    let newer = clipboard::copy_and_clear(
        Zeroizing::new(SYNTHETIC.to_string()),
        Duration::from_secs(30),
    )
    .unwrap();
    older.wait().unwrap();
    let mut reader = Clipboard::new().unwrap();
    wait_for_value(&mut reader, SYNTHETIC);
    newer.clear_now().unwrap();
    assert_ne!(reader.get_text().ok().as_deref(), Some(SYNTHETIC));
}

#[test]
fn headless_clipboard_error_does_not_expose_value() {
    if std::env::var("TERMKEY_TEST_HEADLESS_CLIPBOARD").as_deref() != Ok("1") {
        return;
    }
    let _guard = SERIAL.lock().unwrap();
    assert!(std::env::var_os("DISPLAY").is_none());
    assert!(std::env::var_os("WAYLAND_DISPLAY").is_none());
    let error = clipboard::copy_and_clear(
        Zeroizing::new(SYNTHETIC.to_string()),
        Duration::from_millis(10),
    )
    .expect_err("headless clipboard connection must fail safely");
    assert!(!error.to_string().contains(SYNTHETIC));
    assert!(error.to_string().contains("clipboard") || error.to_string().contains("Clipboard"));
}
