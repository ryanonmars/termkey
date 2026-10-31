use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Stdout};
use std::ops::{Deref, DerefMut};
use std::sync::Once;

pub type Tui = Terminal<CrosstermBackend<Stdout>>;

struct RestoreGuard {
    active: bool,
    restore: fn() -> io::Result<()>,
}

impl RestoreGuard {
    fn finish(&mut self) -> io::Result<()> {
        if !self.active {
            return Ok(());
        }
        self.active = false;
        (self.restore)()
    }
}

impl Drop for RestoreGuard {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

/// Own terminal restoration across normal exit, partial initialization and unwind.
pub struct TerminalSession {
    terminal: Tui,
    restoration: RestoreGuard,
}

impl Deref for TerminalSession {
    type Target = Tui;
    fn deref(&self) -> &Tui {
        &self.terminal
    }
}
impl DerefMut for TerminalSession {
    fn deref_mut(&mut self) -> &mut Tui {
        &mut self.terminal
    }
}
impl TerminalSession {
    pub fn finish(mut self) -> io::Result<()> {
        self.restoration.finish()
    }
}

pub fn init() -> io::Result<TerminalSession> {
    static PANIC_HOOK: Once = Once::new();
    PANIC_HOOK.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = restore();
            previous(info);
        }));
    });
    let restoration = RestoreGuard {
        active: true,
        restore,
    };
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    terminal.clear()?;
    Ok(TerminalSession {
        terminal,
        restoration,
    })
}

fn restore_with(
    disable: impl FnOnce() -> io::Result<()>,
    leave: impl FnOnce() -> io::Result<()>,
) -> io::Result<()> {
    // Attempt both operations even when either one fails: restoring echo takes priority.
    let raw = disable();
    let screen = leave();
    raw.and(screen)
}

pub fn restore() -> io::Result<()> {
    restore_with(disable_raw_mode, || {
        execute!(io::stdout(), LeaveAlternateScreen)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn restore_attempts_both_steps_on_failure() {
        let left = std::cell::Cell::new(false);
        assert!(restore_with(
            || Err(io::Error::other("synthetic failure")),
            || {
                left.set(true);
                Ok(())
            }
        )
        .is_err());
        assert!(left.get());
    }

    #[test]
    fn restoration_runs_on_early_return_and_unwind_once() {
        static RESTORES: AtomicUsize = AtomicUsize::new(0);
        fn restore_test() -> io::Result<()> {
            RESTORES.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        RESTORES.store(0, Ordering::SeqCst);
        {
            let _guard = RestoreGuard {
                active: true,
                restore: restore_test,
            };
        }
        assert_eq!(RESTORES.load(Ordering::SeqCst), 1);
        let _ = std::panic::catch_unwind(|| {
            let _guard = RestoreGuard {
                active: true,
                restore: restore_test,
            };
            panic!("synthetic terminal unwind");
        });
        assert_eq!(RESTORES.load(Ordering::SeqCst), 2);
        {
            let mut guard = RestoreGuard {
                active: true,
                restore: restore_test,
            };
            guard.finish().unwrap();
        }
        assert_eq!(RESTORES.load(Ordering::SeqCst), 3);
    }
}
