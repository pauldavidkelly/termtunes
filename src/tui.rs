use std::io::stdout;
use std::panic::{set_hook, take_hook};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crossterm::cursor;
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, LeaveAlternateScreen};

#[cfg(unix)]
use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
#[cfg(unix)]
use signal_hook::flag;

/// Restore the terminal to its original state.
///
/// Disables raw mode, leaves alternate screen, and shows cursor.
/// Uses `let _ =` to suppress errors because cleanup must never panic --
/// this function is called from panic hooks and signal handlers.
pub fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(stdout(), LeaveAlternateScreen, cursor::Show);
}

/// Install a panic hook that restores the terminal before delegating
/// to the original hook.
///
/// Must be called BEFORE `ratatui::init()` so the hook is in place
/// when the terminal enters alternate screen / raw mode.
pub fn install_panic_hook() {
    let original_hook = take_hook();
    set_hook(Box::new(move |panic_info| {
        restore_terminal();
        original_hook(panic_info);
    }));
}

/// Register signal handlers for clean shutdown.
///
/// Returns a shared `AtomicBool` that is set to `true` when a shutdown
/// signal is received. The main event loop checks this flag on every
/// iteration and exits gracefully when it becomes true.
///
/// On Unix: registers SIGINT, SIGTERM, SIGHUP via signal-hook.
/// On Windows: returns a stub flag (never set). Ctrl+C is handled by
/// crossterm's event stream instead, which works natively on Windows.
pub fn install_signal_handlers() -> Arc<AtomicBool> {
    let shutdown = Arc::new(AtomicBool::new(false));

    #[cfg(unix)]
    {
        flag::register(SIGINT, Arc::clone(&shutdown)).expect("register SIGINT handler");
        flag::register(SIGTERM, Arc::clone(&shutdown)).expect("register SIGTERM handler");
        flag::register(SIGHUP, Arc::clone(&shutdown)).expect("register SIGHUP handler");
    }

    shutdown
}
